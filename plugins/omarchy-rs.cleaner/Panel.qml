import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Quickshell.Io
import qs.Commons
import qs.Ui

Panel {
  id: root
  moduleName: "omarchy-rs.cleaner"
  ipcTarget: "omarchy-rs.cleaner"
  manageIpc: false

  readonly property color foreground: bar ? bar.foreground : Color.foreground
  readonly property color dim: Qt.darker(foreground, 1.5)
  readonly property color rustAccent: "#ff6a1a"
  readonly property double cleanupAlertGiB: {
    var configured = Number(setting("cleanupAlertGiB", 400))
    return isFinite(configured) && configured >= 1 ? configured : 400
  }
  readonly property double cleanupAlertBytes: cleanupAlertGiB * 1024 * 1024 * 1024
  readonly property string fontFamily: bar ? bar.fontFamily : Style.font.family
  readonly property string configuredRoot: String(setting("root", "~/Work"))

  property var report: null
  property var plan: null
  property var applyReport: null
  property var selected: ({})
  property int selectionRevision: 0
  property string error: ""
  property string phase: "idle"

  visible: true
  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight

  function formatBytes(bytes) {
    var value = Number(bytes || 0)
    var units = ["B", "KiB", "MiB", "GiB", "TiB"]
    var index = 0
    while (value >= 1024 && index < units.length - 1) { value /= 1024; index++ }
    return (index === 0 ? value.toFixed(0) : value.toFixed(1)) + " " + units[index]
  }

  function parseJson(text, nextPhase) {
    try {
      var value = JSON.parse(String(text || "{}"))
      error = ""
      phase = nextPhase
      return value
    } catch (exception) {
      error = "Invalid cleaner response: " + exception
      phase = "error"
      return null
    }
  }

  function scanNow() {
    if (scanProcess.running || planProcess.running || applyProcess.running) return
    report = null
    plan = null
    applyReport = null
    selected = ({})
    selectionRevision++
    error = ""
    phase = "scanning"
    scanProcess.command = ["omarchy-rs", "cleaner", "scan", "--root", configuredRoot, "--json"]
    scanProcess.running = true
  }

  function toggleCandidate(id) {
    var next = ({})
    for (var key in selected) next[key] = selected[key]
    next[id] = !next[id]
    selected = next
    selectionRevision++
    plan = null
  }

  function selectedIds() {
    var revision = selectionRevision
    var ids = []
    for (var id in selected) if (selected[id]) ids.push(id)
    ids.sort()
    return ids
  }

  function selectedBytes() {
    var ids = selectedIds()
    var wanted = ({})
    for (var i = 0; i < ids.length; i++) wanted[ids[i]] = true
    var total = 0
    var candidates = report ? (report.candidates || []) : []
    for (var j = 0; j < candidates.length; j++)
      if (wanted[candidates[j].id]) total += Number(candidates[j].bytes || 0)
    return total
  }

  function createPlan() {
    var ids = selectedIds()
    if (ids.length === 0 || planProcess.running) return
    var command = ["omarchy-rs", "cleaner", "plan", "--root", configuredRoot, "--json"]
    for (var i = 0; i < ids.length; i++) command.push("--candidate", ids[i])
    phase = "planning"
    error = ""
    planProcess.command = command
    planProcess.running = true
  }

  function applyPlan() {
    if (!plan || applyProcess.running) return
    phase = "cleaning"
    error = ""
    applyProcess.command = ["omarchy-rs", "cleaner", "apply", "--plan", plan.id,
                            "--confirm", plan.confirmationToken, "--json"]
    applyProcess.running = true
  }

  onOpenedChanged: if (opened && !report && phase === "idle") scanNow()

  IpcHandler {
    target: root.ipcTarget
    function open(): void { root.open() }
    function close(): void { root.close() }
    function toggle(): void { root.toggle() }
    function scan(): string { root.scanNow(); return "ok" }
  }

  Process {
    id: scanProcess
    stdout: StdioCollector {
      id: scanStdout
      waitForEnd: true
      onStreamFinished: {
        var value = root.parseJson(text, "review")
        if (value) root.report = value
      }
    }
    stderr: StdioCollector { id: scanStderr; waitForEnd: true }
    onExited: function(code) {
      if (code !== 0) { root.error = String(scanStderr.text || "Scan failed").trim(); root.phase = "error" }
    }
  }

  Process {
    id: planProcess
    stdout: StdioCollector {
      id: planStdout
      waitForEnd: true
      onStreamFinished: {
        var value = root.parseJson(text, "confirm")
        if (value) root.plan = value
      }
    }
    stderr: StdioCollector { id: planStderr; waitForEnd: true }
    onExited: function(code) {
      if (code !== 0) { root.error = String(planStderr.text || "Plan failed").trim(); root.phase = "error" }
    }
  }

  Process {
    id: applyProcess
    stdout: StdioCollector {
      id: applyStdout
      waitForEnd: true
      onStreamFinished: {
        var value = root.parseJson(text, "done")
        if (value) root.applyReport = value
      }
    }
    stderr: StdioCollector { id: applyStderr; waitForEnd: true }
    onExited: function(code) {
      if (code !== 0) { root.error = String(applyStderr.text || "Cleanup failed").trim(); root.phase = "error" }
    }
  }

  BarIconButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    text: "󰃢"
    tooltipText: root.report
      ? "Workspace Cleaner · " + root.formatBytes(root.report.totalBytes)
      : "Workspace Cleaner"
    active: root.report !== null && Number(root.report.totalBytes || 0) >= root.cleanupAlertBytes
    onPressed: root.toggle()
  }

  KeyboardPanel {
    id: panel
    anchorItem: button
    owner: root
    bar: root.bar
    open: root.opened
    contentWidth: panel.fittedContentWidth(Style.space(420))
    contentHeight: panel.fittedContentHeight(content.implicitHeight, Style.space(620))

    Flickable {
      anchors.fill: parent
      contentWidth: width
      contentHeight: content.implicitHeight
      clip: true
      boundsBehavior: Flickable.StopAtBounds
      ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }

      Column {
        id: content
        width: parent.width
        spacing: Style.space(12)

        Item {
          width: parent.width
          implicitHeight: Math.max(cleanerMark.implicitHeight, heroLabels.implicitHeight)

          Text {
            id: cleanerMark
            anchors.left: parent.left
            anchors.verticalCenter: parent.verticalCenter
            text: "󰃢"
            color: root.foreground
            font.family: root.fontFamily
            font.pixelSize: Style.font.display
          }

          Column {
            id: heroLabels
            anchors.left: cleanerMark.right
            anchors.leftMargin: Style.space(14)
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            spacing: Style.space(2)

            Row {
              width: parent.width

              Text {
                id: heroTitle
                text: "Workspace Cleaner"
                color: root.foreground
                font.family: root.fontFamily
                font.pixelSize: Style.font.title
                font.bold: true
              }

              Item {
                width: Math.max(0, parent.width - heroTitle.implicitWidth - rustBadge.implicitWidth)
                height: 1
              }

              RustBadge {
                id: rustBadge
                anchors.verticalCenter: parent.verticalCenter
                highlighted: true
                foreground: root.foreground
                fontFamily: root.fontFamily
              }
            }

            Text {
              width: parent.width
              text: (root.phase === "scanning" ? "SCANNING ~/WORK" : root.configuredRoot).toUpperCase()
              color: root.dim
              font.family: root.fontFamily
              font.pixelSize: Style.font.caption
              font.bold: true
              font.letterSpacing: 1.2
              elide: Text.ElideRight
            }
          }
        }

        Text {
          visible: root.error !== ""
          width: parent.width
          text: root.error
          color: Color.urgent
          font.family: root.fontFamily
          font.pixelSize: Style.font.body
          wrapMode: Text.Wrap
        }

        Text {
          visible: root.report !== null
          width: parent.width
          text: root.report ? root.formatBytes(root.report.totalBytes) + " regenerable · "
                              + Number(root.report.totalFiles || 0) + " files" : ""
          color: root.foreground
          font.family: root.fontFamily
          font.pixelSize: Style.font.title
          font.bold: true
        }

        Repeater {
          model: root.report ? (root.report.candidates || []) : []
          delegate: Button {
            required property var modelData
            width: content.width
            enabled: modelData.eligible === true && root.phase !== "cleaning"
            leftAlign: true
            bordered: true
            selected: root.selected[modelData.id] === true
            iconText: root.selected[modelData.id] ? "󰄬" : (modelData.eligible ? "󰄱" : "󰅖")
            text: String(modelData.projectRoot).split("/").pop() + " · "
                  + modelData.kind + " · " + root.formatBytes(modelData.bytes)
            tooltipText: modelData.eligible ? String(modelData.path)
                                             : "Recent build — wait five minutes"
            onClicked: root.toggleCandidate(modelData.id)
          }
        }

        Button {
          width: parent.width
          enabled: root.phase !== "scanning" && root.phase !== "planning" && root.phase !== "cleaning"
          text: "Scan again"
          onClicked: root.scanNow()
        }

        Button {
          visible: root.selectedIds().length > 0 && root.phase !== "confirm"
          width: parent.width
          enabled: root.phase === "review" || root.phase === "done" || root.phase === "error"
          text: "Review cleanup · " + root.formatBytes(root.selectedBytes())
          onClicked: root.createPlan()
        }

        Button {
          visible: root.phase === "confirm" && root.plan !== null
          width: parent.width
          text: "Confirm removal · " + (root.plan ? root.formatBytes(root.plan.totalBytes) : "")
          onClicked: root.applyPlan()
        }

        Text {
          visible: root.applyReport !== null
          width: parent.width
          text: root.applyReport ? "Reclaimed " + root.formatBytes(root.applyReport.reclaimedBytes)
                                   + " · " + root.applyReport.removed + " removed · "
                                   + root.applyReport.skipped + " skipped" : ""
          color: Color.accent
          font.family: root.fontFamily
          font.pixelSize: Style.font.body
          wrapMode: Text.Wrap
        }
      }
    }
  }
}

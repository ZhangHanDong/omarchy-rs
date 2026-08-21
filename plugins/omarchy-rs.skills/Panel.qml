import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Quickshell.Io
import qs.Commons
import qs.Ui

Panel {
  id: root
  moduleName: "omarchy-rs.skills"
  ipcTarget: "omarchy-rs.skills"
  manageIpc: false

  readonly property color foreground: bar ? bar.foreground : Color.foreground
  readonly property color dim: Qt.darker(foreground, 1.5)
  readonly property color rustAccent: "#ff6a1a"
  readonly property string fontFamily: bar ? bar.fontFamily : Style.font.family
  property var report: null
  property var selectedSkill: null
  property var plan: null
  property var applyReport: null
  property string operation: "sync"
  property string selectedAgent: "claude"
  property string error: ""
  property string phase: "idle"
  readonly property var agents: [
    { id: "claude", label: "Claude" },
    { id: "codex", label: "Codex" },
    { id: "grok", label: "Grok" },
    { id: "octoscode", label: "Octos" }
  ]

  visible: true
  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight

  function parseJson(text, nextPhase) {
    try { error = ""; phase = nextPhase; return JSON.parse(String(text || "{}")) }
    catch (exception) { error = "Invalid Skill response: " + exception; phase = "error"; return null }
  }

  function scanNow() {
    if (scanProcess.running || planProcess.running || applyProcess.running) return
    report = null; selectedSkill = null; plan = null; applyReport = null
    error = ""; phase = "scanning"
    scanProcess.command = ["omarchy-rs", "skills", "scan", "--json"]
    scanProcess.running = true
  }

  function createPlan(nextOperation) {
    if (!selectedSkill || planProcess.running) return
    operation = nextOperation; error = ""; phase = "planning"
    planProcess.command = ["omarchy-rs", "skills", "plan", "--skill", selectedSkill.name,
                           "--operation", nextOperation, "--agent", selectedAgent, "--json"]
    planProcess.running = true
  }

  function applyPlan() {
    if (!plan || applyProcess.running) return
    error = ""; phase = "applying"
    applyProcess.command = ["omarchy-rs", "skills", "apply", "--plan", plan.id,
                            "--confirm", plan.confirmationToken, "--json"]
    applyProcess.running = true
  }

  function activationSummary(skill) {
    var active = []
    var values = skill.activations || []
    for (var i = 0; i < values.length; i++)
      if (values[i].state === "active" || values[i].state === "active-read-only" || values[i].state === "managed" || values[i].state === "backend-visible")
        active.push(values[i].agent)
    return active.length ? active.join(" · ") : "not active"
  }

  function activationFor(skill, agent) {
    var values = skill.activations || []
    for (var i = 0; i < values.length; i++) if (values[i].agent === agent) return values[i]
    return null
  }

  function activationLabel(skill) {
    var activation = activationFor(skill, selectedAgent)
    if (!activation) return "not installed"
    var state = String(activation.state || "unknown")
    if (state === "active" || state === "active-read-only" || state === "managed") return "active"
    if (state === "backend-visible") return "available via Codex backend"
    if (state === "inactive") return "available"
    return state.replace(/-/g, " ")
  }

  function selectAgent(agent) {
    selectedAgent = agent
    selectedSkill = null
    plan = null
    applyReport = null
    panelFlick.contentY = 0
  }

  function displaySkills() {
    var input = report ? (report.skills || []) : []
    var byName = ({})
    var order = []
    for (var i = 0; i < input.length; i++) {
      var source = input[i]
      var key = String(source.name || source.id)
      if (!byName[key]) {
        byName[key] = JSON.parse(JSON.stringify(source))
        byName[key].activations = []
        order.push(key)
      }
      var target = byName[key]
      if (String(source.sourceClass).indexOf("shared") === 0) {
        target.id = source.id
        target.path = source.path
        target.sourceClass = source.sourceClass
        target.healthy = source.healthy
        target.healthReason = source.healthReason
      }
      var activations = source.activations || []
      for (var j = 0; j < activations.length; j++) {
        var next = activations[j]
        var replaced = false
        for (var k = 0; k < target.activations.length; k++) {
          if (target.activations[k].agent === next.agent) {
            var currentState = String(target.activations[k].state)
            if (currentState === "inactive" || currentState === "unavailable") target.activations[k] = next
            replaced = true
            break
          }
        }
        if (!replaced) target.activations.push(next)
      }
    }
    var output = []
    for (var n = 0; n < order.length; n++) output.push(byName[order[n]])
    output.sort(function(left, right) {
      if (left.healthy !== right.healthy) return left.healthy ? -1 : 1
      return String(left.name).localeCompare(String(right.name))
    })
    return output
  }

  function agentSkills() {
    var values = displaySkills()
    var output = []
    for (var i = 0; i < values.length; i++)
      if (activationFor(values[i], selectedAgent)) output.push(values[i])
    return output
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
    stdout: StdioCollector { id: scanStdout; waitForEnd: true; onStreamFinished: { var value = root.parseJson(text, "review"); if (value) root.report = value } }
    stderr: StdioCollector { id: scanStderr; waitForEnd: true }
    onExited: function(code) { if (code !== 0) { root.error = String(scanStderr.text || "Scan failed").trim(); root.phase = "error" } }
  }
  Process {
    id: planProcess
    stdout: StdioCollector { id: planStdout; waitForEnd: true; onStreamFinished: { var value = root.parseJson(text, "confirm"); if (value) root.plan = value } }
    stderr: StdioCollector { id: planStderr; waitForEnd: true }
    onExited: function(code) { if (code !== 0) { root.error = String(planStderr.text || "Plan failed").trim(); root.phase = "error" } }
  }
  Process {
    id: applyProcess
    stdout: StdioCollector { id: applyStdout; waitForEnd: true; onStreamFinished: { var value = root.parseJson(text, "done"); if (value) root.applyReport = value } }
    stderr: StdioCollector { id: applyStderr; waitForEnd: true }
    onExited: function(code) { if (code !== 0) { root.error = String(applyStderr.text || "Apply failed").trim(); root.phase = "error" } }
  }

  BarIconButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    iconComponent: Component {
      Rectangle {
        width: Style.space(16)
        height: width
        radius: Style.space(4)
        color: root.opened ? root.rustAccent : "transparent"
        border.color: root.opened ? root.rustAccent : root.foreground
        border.width: 1
        Text {
          anchors.centerIn: parent
          text: "S"
          color: root.opened ? "#111111" : root.foreground
          font.family: root.fontFamily
          font.bold: true
          font.pixelSize: Style.font.caption
        }
      }
    }
    tooltipText: report ? "Local Skills · " + report.skills.length : "Local Skill Manager"
    active: root.opened
    onPressed: root.toggle()
  }

  KeyboardPanel {
    id: panel
    anchorItem: button
    owner: root
    bar: root.bar
    open: root.opened
    contentWidth: panel.fittedContentWidth(Style.space(440))
    contentHeight: panel.fittedContentHeight(content.implicitHeight, Style.space(640))

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
        spacing: Style.space(10)

        PanelHero {
          width: parent.width
          title: "Local Skills"
          meta: root.report ? root.agentSkills().length + " Skills · " + root.selectedAgent : "~/.agents/skills"
          foreground: root.foreground
          fontFamily: root.fontFamily
          trailingControl: Component {
            RustBadge {
              highlighted: true
              foreground: root.foreground
              fontFamily: root.fontFamily
            }
          }
          iconComponent: Component {
            Rectangle {
              width: Style.font.display
              height: width
              radius: Style.space(7)
              color: root.rustAccent
              Text {
                anchors.centerIn: parent
                text: "S"
                color: "#111111"
                font.family: root.fontFamily
                font.bold: true
                font.pixelSize: Style.font.title
              }
            }
          }
        }


        Row {
          id: agentSwitch
          width: parent.width
          spacing: Style.spacing.md

          Repeater {
            model: root.agents

            Button {
              required property var modelData
              width: (agentSwitch.width - agentSwitch.spacing * 3) / 4
              text: modelData.label
              selected: root.selectedAgent === modelData.id
              bordered: true
              foreground: root.foreground
              fontFamily: root.fontFamily
              fontSize: Style.font.bodySmall
              verticalPadding: Style.spacing.controlPaddingY
              onClicked: root.selectAgent(modelData.id)
            }
          }
        }

        Text { visible: root.phase === "scanning"; text: "Scanning Skill metadata…"; color: root.dim; font.family: root.fontFamily }
        Text { visible: root.error.length > 0; width: parent.width; wrapMode: Text.Wrap; text: root.error; color: "#ff6b6b"; font.family: root.fontFamily }

        BorderSurface {
          visible: root.selectedSkill !== null
          width: parent.width
          implicitHeight: actionColumn.implicitHeight + Style.space(16)
          radius: Style.cornerRadius
          color: Qt.rgba(1, 0.42, 0.1, 0.10)
          borderSpec: Border.flat(Qt.rgba(1, 0.42, 0.1, 0.35), 1)

          Column {
            id: actionColumn
            anchors { left: parent.left; right: parent.right; top: parent.top; margins: Style.space(8) }
            spacing: Style.space(7)
            Text { text: (root.selectedSkill ? root.selectedSkill.name : "") + " · " + root.selectedAgent; color: root.foreground; font.family: root.fontFamily; font.bold: true }
            Row {
              visible: root.plan === null
              spacing: Style.space(8)
              Button { text: "Review sync"; enabled: root.selectedSkill && String(root.selectedSkill.sourceClass).indexOf("shared") === 0; onClicked: root.createPlan("sync") }
              Button { text: "Review cancel"; onClicked: root.createPlan("cancel") }
              Button { text: "Refresh"; onClicked: root.scanNow() }
            }
            Text {
              visible: root.plan !== null
              width: parent.width
              wrapMode: Text.Wrap
              text: "Ready to " + root.operation + " across the selected Agent adapters. Foreign files remain untouched."
              color: root.dim
              font.family: root.fontFamily
            }
            Row {
              visible: root.plan !== null
              spacing: Style.space(8)
              Button { text: "Confirm"; onClicked: root.applyPlan() }
              Button { text: "Back"; onClicked: root.plan = null }
            }
          }
        }

        Text { visible: root.applyReport !== null; width: parent.width; wrapMode: Text.Wrap; text: "Operation finished. Refresh to inspect the four adapters."; color: root.rustAccent; font.family: root.fontFamily }

        Repeater {
          model: root.agentSkills()
          delegate: BorderSurface {
            required property var modelData
            width: content.width
            implicitHeight: skillColumn.implicitHeight + Style.space(14)
            radius: Style.cornerRadius
            color: root.selectedSkill && root.selectedSkill.id === modelData.id ? Qt.rgba(1, 0.42, 0.1, 0.16) : Qt.rgba(1, 1, 1, 0.04)
            borderSpec: Border.flat(modelData.healthy ? Qt.rgba(1, 1, 1, 0.08) : "#ff6b6b", 1)
            MouseArea { anchors.fill: parent; onClicked: { root.selectedSkill = modelData; root.plan = null; root.applyReport = null } }
            Column {
              id: skillColumn
              anchors { left: parent.left; right: parent.right; top: parent.top; margins: Style.space(8) }
              spacing: Style.space(3)
              Text { text: modelData.name; color: root.foreground; font.family: root.fontFamily; font.bold: true }
              Text {
                width: parent.width
                elide: Text.ElideRight
                text: modelData.healthy ? root.activationLabel(modelData) : "Needs attention · " + String(modelData.healthReason || "invalid Skill")
                color: modelData.healthy ? root.dim : "#ff6b6b"
                font.family: root.fontFamily
                font.pixelSize: Style.font.caption
              }
            }
          }
        }

      }
    }
  }
}

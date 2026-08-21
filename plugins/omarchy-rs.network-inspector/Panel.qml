import QtQuick
import QtQuick.Controls
import Quickshell.Io
import qs.Commons
import qs.Ui

Panel {
  id: root
  moduleName: "omarchy-rs.network-inspector"
  ipcTarget: "omarchy-rs.network-inspector"
  manageIpc: false

  readonly property color foreground: bar ? bar.foreground : Color.foreground
  readonly property color dim: Qt.darker(foreground, 1.5)
  readonly property color rustAccent: "#ff6a1a"
  readonly property string fontFamily: bar ? bar.fontFamily : Style.font.family
  property var report: null
  property bool sniffnetRunning: false
  property bool sniffnetLaunchPending: false
  property int sniffnetRefreshAttempts: 0
  property string selectedAgent: String(setting("agent", "codex"))
  property string error: ""
  property string phase: "idle"

  visible: !!root.report && !!root.report.sniffnet && root.report.sniffnet.installed
  implicitWidth: visible ? button.implicitWidth : 0
  implicitHeight: visible ? button.implicitHeight : 0

  Component.onCompleted: refresh()

  function parseJson(text, next) {
    try {
      var value = JSON.parse(String(text || "{}"))
      error = ""
      phase = next
      return value
    } catch (exception) {
      error = "Invalid Network Inspector response: " + exception
      phase = "error"
      return null
    }
  }

  function refresh() {
    if (statusProcess.running) return
    phase = "checking"
    statusProcess.command = ["omarchy-rs", "network", "status", "--json"]
    statusProcess.running = true
  }

  function openSniffnet() {
    if (openProcess.running) return
    error = ""
    root.sniffnetLaunchPending = !root.sniffnetRunning
    root.sniffnetRefreshAttempts = root.sniffnetLaunchPending ? 6 : 0
    root.sniffnetRunning = true
    openProcess.command = ["omarchy-rs", "network", "open", "--json"]
    openProcess.running = true
  }

  function applyOpenResult(value) {
    if (!value) return
    root.sniffnetRunning = true
    if (value.action === "focused") root.sniffnetLaunchPending = false
    root.phase = "ready"
    launchRefreshTimer.restart()
  }

  function launchAgentTerminal() {
    if (agentTerminalProcess.running) return
    error = ""
    phase = "launching-agent"
    agentTerminalProcess.command = ["omarchy-rs", "network", "agent-terminal", "--agent", selectedAgent, "--json"]
    agentTerminalProcess.running = true
  }

  onOpenedChanged: if (opened) refresh()

  IpcHandler {
    target: root.ipcTarget
    function open(): void { root.open() }
    function close(): void { root.close() }
    function toggle(): void { root.toggle() }
    function refresh(): string { root.refresh(); return "ok" }
  }

  Process {
    id: statusProcess
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        var value = root.parseJson(text, "ready")
        if (value) {
          root.report = value
          var observedRunning = !!(value.sniffnet && value.sniffnet.running)
          if (observedRunning) {
            root.sniffnetRunning = true
            root.sniffnetLaunchPending = false
            root.sniffnetRefreshAttempts = 0
          } else if (root.sniffnetLaunchPending && root.sniffnetRefreshAttempts > 0) {
            root.sniffnetRunning = true
            root.sniffnetRefreshAttempts -= 1
            launchRefreshTimer.restart()
          } else {
            root.sniffnetRunning = false
            root.sniffnetLaunchPending = false
          }
        }
      }
    }
    stderr: StdioCollector { id: statusError; waitForEnd: true }
    onExited: function(code) { if (code !== 0) { root.error = String(statusError.text).trim(); root.phase = "error" } }
  }

  Process {
    id: openProcess
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        var value = root.parseJson(text, "ready")
        if (value) root.applyOpenResult(value)
      }
    }
    stderr: StdioCollector { id: openError; waitForEnd: true }
    onExited: function(code) {
      if (code !== 0) {
        root.sniffnetLaunchPending = false
        root.sniffnetRunning = !!(root.report && root.report.sniffnet && root.report.sniffnet.running)
        root.error = String(openError.text).trim()
        root.phase = "error"
      }
      if (code !== 0) root.refresh()
    }
  }


  Timer {
    id: launchRefreshTimer
    interval: 500
    repeat: false
    onTriggered: root.refresh()
  }

  Process {
    id: agentTerminalProcess
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        var value = root.parseJson(text, "ready")
        if (value) root.phase = "agent-launched"
      }
    }
    stderr: StdioCollector { id: agentTerminalError; waitForEnd: true }
    onExited: function(code) {
      if (code !== 0) { root.error = String(agentTerminalError.text).trim(); root.phase = "error" }
    }
  }

  BarIconButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    text: "󰛳"
    tooltipText: "Network Inspector"
    active: root.report && (root.report.issues || []).length > 0
    onPressed: root.toggle()
  }

  KeyboardPanel {
    id: panel
    anchorItem: button
    owner: root
    bar: root.bar
    open: root.opened
    contentWidth: panel.fittedContentWidth(Style.space(430))
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
        spacing: Style.space(12)

        PanelHero {
          width: parent.width
          title: "Network Inspector"
          meta: "Local Network Health"
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
            Text {
              text: "󰛳"
              color: root.foreground
              font.family: root.fontFamily
              font.pixelSize: Style.font.display
            }
          }
        }

        Text {
          width: parent.width
          text: root.error !== "" ? root.error : root.summaryText()
          color: root.error !== "" ? Color.error : root.dim
          font.family: root.fontFamily
          font.pixelSize: Style.font.body
          wrapMode: Text.WordWrap
          textFormat: Text.PlainText
        }

        Row {
          width: parent.width
          spacing: Style.space(8)
          Button { text: "Refresh"; enabled: !statusProcess.running; onClicked: root.refresh() }
          Button {
            text: root.sniffnetRunning ? "Focus Sniffnet" : "Open Sniffnet"
            enabled: root.report && root.report.sniffnet && root.report.sniffnet.installed
            onClicked: root.openSniffnet()
          }
        }

        PanelSeparator { width: parent.width; foreground: root.foreground }
        PanelSectionHeader { width: parent.width; text: "ASK AGENT"; foreground: root.foreground; fontFamily: root.fontFamily }

        Row {
          spacing: Style.space(8)
          Repeater {
            model: ["codex", "claude", "grok"]
            Button {
              required property string modelData
              text: modelData.charAt(0).toUpperCase() + modelData.slice(1)
              selected: root.selectedAgent === modelData
              onClicked: root.selectedAgent = modelData
            }
          }
        }

        Text {
          width: parent.width
          text: "Opens " + root.selectedAgent + " in a separate terminal with this content-free network snapshot as the first question. Continue diagnosis and approve any actions there."
          color: root.dim
          font.family: root.fontFamily
          font.pixelSize: Style.font.bodySmall
          wrapMode: Text.WordWrap
          textFormat: Text.PlainText
        }

        Button {
          text: agentTerminalProcess.running ? "Opening terminal…" : "Ask in " + root.selectedAgent
          enabled: !!root.report && !agentTerminalProcess.running
          onClicked: root.launchAgentTerminal()
        }
      }
    }
  }

  function summaryText() {
    if (!report) return phase === "checking" ? "Inspecting local network health…" : "Open the panel to inspect network health."
    var iface = report.interface || null
    var lines = []
    lines.push("Route: " + (report.defaultRoute ? "ready" : "missing"))
    lines.push("Interface: " + (iface ? iface.name + " · " + iface.kind + " · " + iface.operState : "unavailable"))
    lines.push("DNS: " + (report.dnsConfigured ? "configured" : "missing"))
    lines.push("Sniffnet: " + (report.sniffnet.installed ? (root.sniffnetRunning ? "running" : report.sniffnet.captureStatus) : "not installed"))
    var issues = report.issues || []
    if (issues.length > 0) lines.push("Issues: " + issues.join(", "))
    return lines.join("\n")
  }
}

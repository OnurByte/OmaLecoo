import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Hyprland
import Quickshell.Wayland

// One service owns arbitration and pacing across all bar instances. Rust owns
// locking, persistent state, and every HID operation. No keyboard input is read.
Item {
  id: root
  property var shell: null
  property string binary: Quickshell.env("HOME") + "/.local/bin/lecoo-gk301"
  property bool automationEnabled: false
  property bool restoreOnReconnect: false
  property bool connected: false
  property bool writable: false
  property bool presenceKnown: false
  property bool rulesLoaded: false
  property bool suspended: false
  property string activeRule: "Disabled"
  property string ruleReason: "Turn on automation to follow fullscreen and idle state."
  property string lastOutcome: "Waiting for the controller"
  property string connectionMessage: "Checking keyboard presence…"
  property double manualUntil: 0
  property double lastWriteFinished: 0
  property var manualQueue: []
  property string appliedRule: ""
  property bool reconnectPending: false
  readonly property bool busy: writer.running || manualQueue.length > 0
  readonly property var activeToplevel: ToplevelManager.activeToplevel
  readonly property bool fullscreen: activeToplevel ? activeToplevel.fullscreen : false
  signal panelCommandFinished(string requestId, int code, string error, string output)
  signal commandFinished()

  function reloadState() {
    if (!rulesProc.running) { rulesProc.output = ""; rulesProc.running = true }
    checkPresence()
    schedule()
  }

  function checkPresence() {
    if (!presenceProc.running) { presenceProc.output = ""; presenceProc.running = true }
  }

  function setRules(automation, recovery) {
    if (rulesWrite.running || suspended) return
    rulesWrite.command = [binary, "rules", "set", "--automation", automation ? "on" : "off",
                          "--restore-on-reconnect", recovery ? "on" : "off"]
    rulesWrite.error = ""
    rulesWrite.running = true
  }

  function pauseAutomation() {
    manualUntil = Date.now() + 30 * 60 * 1000
    reconnectPending = false
    appliedRule = ""
    schedule()
  }

  function submitPanelCommand(args, requestId) {
    pauseAutomation()
    manualQueue = manualQueue.concat([{ args: args, requestId: requestId }])
    schedule()
  }

  function schedule() { coalesce.restart() }

  function updateRule() {
    if (suspended) { activeRule = "Paused"; ruleReason = "Setup or removal is in progress." }
    else if (!automationEnabled) { activeRule = "Disabled"; ruleReason = "Automation is off." }
    else if (Date.now() < manualUntil) {
      activeRule = "Manual override"
      ruleReason = "Manual control pauses automation until " + new Date(manualUntil).toLocaleTimeString()
    } else if (idleMonitor.isIdle) { activeRule = "Idle · off"; ruleReason = "No activity for five minutes." }
    else if (fullscreen) { activeRule = "Fullscreen · game"; ruleReason = "The active window is fullscreen." }
    else { activeRule = "Default · restore"; ruleReason = "Restore the last manually selected settings." }
  }

  function dispatch() {
    updateRule()
    if (writer.running || suspended || !presenceKnown) return
    // Do not retry HID writes while absent or permission denied. Manual calls
    // still complete so the panel does not get stuck waiting for a callback.
    if (!connected || !writable) {
      var rejected = manualQueue
      manualQueue = []
      for (var i = 0; i < rejected.length; ++i)
        panelCommandFinished(rejected[i].requestId, 1, connectionMessage, "")
      return
    }
    var delay = 2000 - (Date.now() - lastWriteFinished)
    if (delay > 0) { pacing.interval = Math.ceil(delay); pacing.restart(); return }
    var args = []
    writer.requestId = ""
    writer.rule = ""
    if (manualQueue.length > 0) {
      var request = manualQueue[0]
      manualQueue = manualQueue.slice(1)
      writer.requestId = request.requestId
      args = request.args.concat(["--origin", "panel"])
    } else if (reconnectPending) {
      reconnectPending = false
      if (!restoreOnReconnect) return
      args = ["restore", "--origin", "signal"]
      writer.rule = "reconnect"
    } else if (rulesLoaded && automationEnabled && Date.now() >= manualUntil && appliedRule !== activeRule) {
      writer.rule = activeRule
      args = activeRule === "Idle · off" ? ["preset", "off", "--origin", "signal"]
        : activeRule === "Fullscreen · game" ? ["preset", "game", "--origin", "signal"]
        : ["restore", "--origin", "signal"]
      // Consume even a failed transition; another compositor event should
      // not cause an endless retry. A later rule transition can try again.
      appliedRule = activeRule
    }
    if (args.length === 0) return
    writer.output = ""
    writer.error = ""
    writer.handled = false
    writer.command = [binary].concat(args)
    writer.running = true
  }

  onFullscreenChanged: schedule()
  onActiveToplevelChanged: schedule()
  onSuspendedChanged: schedule()
  onBinaryChanged: { rulesLoaded = false; presenceKnown = false; appliedRule = ""; reloadState() }
  Component.onCompleted: reloadState()

  Connections {
    target: Hyprland
    function onRawEvent(event) {
      if (["fullscreen", "activewindow", "activewindowv2", "closewindow", "workspace", "focusedmon"].indexOf(event.name) !== -1)
        root.schedule()
    }
  }
  IdleMonitor {
    id: idleMonitor
    timeout: 300
    enabled: root.automationEnabled
    respectInhibitors: true
    onIsIdleChanged: root.schedule()
  }
  Timer { id: coalesce; interval: 250; onTriggered: root.dispatch() }
  Timer { id: pacing; onTriggered: root.dispatch() }
  Timer { interval: 1000; repeat: true; running: root.manualUntil > 0; onTriggered: {
    if (Date.now() >= root.manualUntil) { root.manualUntil = 0; root.schedule() }
  } }
  Timer { interval: 10000; repeat: true; running: !root.suspended; onTriggered: root.checkPresence() }

  Process {
    id: rulesProc
    property string output: ""
    command: [root.binary, "rules", "--json"]
    stdout: StdioCollector { onStreamFinished: rulesProc.output = String(text || "") }
    onExited: function(code) {
      if (code === 0) {
        try {
          var data = JSON.parse(output)
          root.automationEnabled = data.automation_enabled === true
          root.restoreOnReconnect = data.restore_on_reconnect === true
          root.rulesLoaded = true
        } catch (e) { root.lastOutcome = "Could not read automation settings" }
      } else { root.lastOutcome = "Controller unavailable. Open Setup to install it." }
      root.schedule()
    }
  }
  Process {
    id: rulesWrite
    property string error: ""
    stderr: StdioCollector { onStreamFinished: rulesWrite.error = String(text || "").trim() }
    onExited: function(code) {
      if (code !== 0) root.lastOutcome = error || "Could not save automation settings"
      root.appliedRule = ""
      root.reloadState()
    }
  }
  Process {
    id: presenceProc
    property string output: ""
    command: [root.binary, "dev", "--json"]
    stdout: StdioCollector { onStreamFinished: presenceProc.output = String(text || "") }
    onExited: function(code) {
      var wasKnown = root.presenceKnown
      var wasConnected = root.connected
      var wasWritable = root.writable
      try {
        if (code !== 0) throw new Error("unavailable")
        var data = JSON.parse(output)
        root.connected = data.connected === true
        root.writable = data.writable === true
        root.connectionMessage = !root.connected ? "Ghost Mode · keyboard disconnected. Showing cached settings."
          : !root.writable ? "Keyboard connected · permission denied. Authorize device access in Setup."
          : "Keyboard connected · settings are cached commands, not lighting readback."
      } catch (e) {
        root.connected = false
        root.writable = false
        root.connectionMessage = "Controller unavailable · install or verify it in Setup. Showing cached settings."
      }
      root.presenceKnown = true
      if (!root.connected) root.reconnectPending = false
      if (wasKnown && !wasConnected && root.connected) {
        root.reconnectPending = root.restoreOnReconnect
        // Recovery is opt-in. Do not turn a presence transition into a
        // second automation write (or an implicit restore when disabled).
        root.updateRule()
        root.appliedRule = root.activeRule
      } else if (wasConnected && root.connected && !wasWritable && root.writable) root.appliedRule = ""
      root.schedule()
    }
  }
  Process {
    id: writer
    property string output: ""
    property string error: ""
    property string requestId: ""
    property string rule: ""
    property bool handled: true
    stdout: StdioCollector { onStreamFinished: writer.output = String(text || "").trim() }
    stderr: StdioCollector { onStreamFinished: writer.error = String(text || "").trim() }
    onExited: function(code) {
      Qt.callLater(function() { root.finishWrite(code) })
    }
    onRunningChanged: if (!running && !handled) writerFailureGuard.restart()
  }
  function finishWrite(code) {
    if (writer.handled) return
    writer.handled = true
    lastWriteFinished = Date.now()
    lastOutcome = code === 0 ? (writer.output || "Command completed") : (writer.error || "Could not start or complete the controller command")
    if (writer.requestId !== "") panelCommandFinished(writer.requestId, code, writer.error || (code !== 0 ? lastOutcome : ""), writer.output)
    commandFinished()
    schedule()
  }
  Timer {
    id: writerFailureGuard
    interval: 100
    onTriggered: if (!writer.running && !writer.handled) root.finishWrite(1)
  }
}

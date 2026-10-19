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
  property var attachedWidgets: []
  readonly property bool automaticWritesAllowed: attachedWidgets.length > 0
  // Keep reload requests across in-flight attempts. Generation identifies the
  // configured controller; each Process also identifies its own callbacks.
  property int controllerGeneration: 0
  property bool rulesReloadPending: false
  property bool presenceReloadPending: false
  readonly property bool busy: writer.running || manualQueue.length > 0
  readonly property var activeToplevel: ToplevelManager.activeToplevel
  readonly property bool fullscreen: activeToplevel ? activeToplevel.fullscreen : false
  signal panelCommandFinished(string requestId, int code, string error, string output)
  signal commandFinished()

  function attachWidget(widget) {
    if (attachedWidgets.indexOf(widget) === -1)
      attachedWidgets = attachedWidgets.concat([widget])
    schedule()
  }

  function detachWidget(widget) {
    attachedWidgets = attachedWidgets.filter(function(item) { return item !== widget })
    if (!automaticWritesAllowed) {
      reconnectPending = false
      appliedRule = ""
    }
    schedule()
  }

  function reloadState() {
    rulesReloadPending = true
    startRulesRead()
    checkPresence()
    schedule()
  }

  function startRulesRead() {
    if (!rulesReloadPending || rulesProc.running || !rulesProc.handled
        || rulesWrite.running || !rulesWrite.handled) return
    rulesReloadPending = false
    rulesProc.output = ""
    rulesProc.error = ""
    rulesProc.generation = controllerGeneration
    rulesProc.attempt += 1
    rulesProc.command = [binary, "rules", "--json"]
    rulesProc.handled = false
    rulesProc.running = true
  }

  function checkPresence() {
    presenceReloadPending = true
    startPresenceRead()
  }

  function startPresenceRead() {
    if (!presenceReloadPending || presenceProc.running || !presenceProc.handled) return
    presenceReloadPending = false
    presenceProc.output = ""
    presenceProc.generation = controllerGeneration
    presenceProc.attempt += 1
    presenceProc.command = [binary, "dev", "--json"]
    presenceProc.handled = false
    presenceProc.running = true
  }

  function setRules(automation, recovery) {
    if (rulesWrite.running || !rulesWrite.handled || suspended) return
    rulesWrite.command = [binary, "rules", "set", "--automation", automation ? "on" : "off",
                          "--restore-on-reconnect", recovery ? "on" : "off"]
    rulesWrite.error = ""
    rulesWrite.generation = controllerGeneration
    rulesWrite.attempt += 1
    rulesWrite.handled = false
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
    else if (!automaticWritesAllowed) { activeRule = "Unavailable"; ruleReason = "Automation requires an attached Omarchy built-in bar widget." }
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
    if (writer.running || !writer.handled || suspended || !presenceKnown) return
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
    } else if (!automaticWritesAllowed) return
    else if (reconnectPending) {
      reconnectPending = false
      if (!restoreOnReconnect || Date.now() < manualUntil) return
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
    writer.attempt += 1
    writer.command = [binary].concat(args)
    writer.running = true
  }

  onFullscreenChanged: schedule()
  onActiveToplevelChanged: schedule()
  onSuspendedChanged: schedule()
  onBinaryChanged: {
    controllerGeneration += 1
    rulesLoaded = false
    automationEnabled = false
    restoreOnReconnect = false
    presenceKnown = false
    reconnectPending = false
    appliedRule = ""
    reloadState()
  }
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
    enabled: root.automationEnabled && root.automaticWritesAllowed
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
    property string error: ""
    property bool handled: true
    property int generation: -1
    property int attempt: 0
    stdout: StdioCollector { onStreamFinished: rulesProc.output = String(text || "") }
    stderr: StdioCollector { onStreamFinished: rulesProc.error = String(text || "").trim() }
    onExited: function(code) {
      var completedAttempt = attempt
      Qt.callLater(function() { root.finishRulesRead(code, completedAttempt) })
    }
    onRunningChanged: if (!running && !handled) rulesReadFailureGuard.restart()
  }
  function finishRulesRead(code, attempt) {
    if (rulesProc.handled || attempt !== rulesProc.attempt) return
    rulesProc.handled = true
    rulesReadFailureGuard.stop()
    if (rulesProc.generation !== controllerGeneration) {
      startRulesRead()
      return
    }
    root.rulesLoaded = false
    try {
      if (code !== 0) throw new Error("unavailable")
      var data = JSON.parse(rulesProc.output)
      root.automationEnabled = data.automation_enabled === true
      root.restoreOnReconnect = data.restore_on_reconnect === true
      root.rulesLoaded = true
    } catch (e) {
      root.automationEnabled = false
      root.restoreOnReconnect = false
      root.lastOutcome = code === 0 ? "Could not read automation settings"
        : rulesProc.error || "Controller unavailable. Open Setup to install or verify it."
    }
    root.schedule()
    startRulesRead()
  }
  Timer {
    id: rulesReadFailureGuard
    interval: 100
    onTriggered: if (!rulesProc.running && !rulesProc.handled) root.finishRulesRead(1, rulesProc.attempt)
  }
  Process {
    id: rulesWrite
    property string error: ""
    property bool handled: true
    property int generation: -1
    property int attempt: 0
    stderr: StdioCollector { onStreamFinished: rulesWrite.error = String(text || "").trim() }
    onExited: function(code) {
      var completedAttempt = attempt
      Qt.callLater(function() { root.finishRulesWrite(code, completedAttempt) })
    }
    onRunningChanged: if (!running && !handled) rulesWriteFailureGuard.restart()
  }
  function finishRulesWrite(code, attempt) {
    if (rulesWrite.handled || attempt !== rulesWrite.attempt) return
    rulesWrite.handled = true
    rulesWriteFailureGuard.stop()
    if (rulesWrite.generation !== controllerGeneration) {
      startRulesRead()
      return
    }
    root.appliedRule = ""
    if (code !== 0) {
      root.rulesLoaded = false
      root.automationEnabled = false
      root.restoreOnReconnect = false
      root.lastOutcome = rulesWrite.error || "Could not start the controller to save automation settings. Open Setup to install or verify it."
      root.checkPresence()
      root.schedule()
    } else root.reloadState()
    startRulesRead()
  }
  Timer {
    id: rulesWriteFailureGuard
    interval: 100
    onTriggered: if (!rulesWrite.running && !rulesWrite.handled) root.finishRulesWrite(1, rulesWrite.attempt)
  }
  Process {
    id: presenceProc
    property string output: ""
    property bool handled: true
    property int generation: -1
    property int attempt: 0
    stdout: StdioCollector { onStreamFinished: presenceProc.output = String(text || "") }
    onExited: function(code) {
      var completedAttempt = attempt
      Qt.callLater(function() { root.finishPresence(code, completedAttempt) })
    }
    onRunningChanged: if (!running && !handled) presenceFailureGuard.restart()
  }
  function finishPresence(code, attempt) {
    if (presenceProc.handled || attempt !== presenceProc.attempt) return
    presenceProc.handled = true
    presenceFailureGuard.stop()
    if (presenceProc.generation !== controllerGeneration) {
      startPresenceRead()
      return
    }
    var wasKnown = root.presenceKnown
    var wasConnected = root.connected
    var wasWritable = root.writable
    try {
      if (code !== 0) throw new Error("unavailable")
      var data = JSON.parse(presenceProc.output)
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
      root.reconnectPending = root.automaticWritesAllowed && root.restoreOnReconnect
      // Recovery is opt-in. Do not turn a presence transition into a
      // second automation write (or an implicit restore when disabled).
      root.updateRule()
      root.appliedRule = root.activeRule
    } else if (wasConnected && root.connected && !wasWritable && root.writable) {
      // A queued reconnect already consumed this rule. Permission recovery
      // must not re-arm it and produce a second write after the restore.
      if (!root.reconnectPending) root.appliedRule = ""
    }
    root.schedule()
    startPresenceRead()
  }
  Timer {
    id: presenceFailureGuard
    interval: 100
    onTriggered: if (!presenceProc.running && !presenceProc.handled) root.finishPresence(1, presenceProc.attempt)
  }
  Process {
    id: writer
    property string output: ""
    property string error: ""
    property string requestId: ""
    property string rule: ""
    property bool handled: true
    property int attempt: 0
    stdout: StdioCollector { onStreamFinished: writer.output = String(text || "").trim() }
    stderr: StdioCollector { onStreamFinished: writer.error = String(text || "").trim() }
    onExited: function(code) {
      var completedAttempt = attempt
      Qt.callLater(function() { root.finishWrite(code, completedAttempt) })
    }
    onRunningChanged: if (!running && !handled) writerFailureGuard.restart()
  }
  function finishWrite(code, attempt) {
    if (writer.handled || attempt !== writer.attempt) return
    writer.handled = true
    writerFailureGuard.stop()
    lastWriteFinished = Date.now()
    lastOutcome = code === 0 ? (writer.output || "Command completed") : (writer.error || "Could not start or complete the controller command")
    if (writer.requestId !== "") panelCommandFinished(writer.requestId, code, writer.error || (code !== 0 ? lastOutcome : ""), writer.output)
    commandFinished()
    schedule()
  }
  Timer {
    id: writerFailureGuard
    interval: 100
    onTriggered: if (!writer.running && !writer.handled) root.finishWrite(1, writer.attempt)
  }
}

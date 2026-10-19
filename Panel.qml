import QtQuick
import QtQuick.Controls
import Quickshell
import Quickshell.Io
import qs.Commons
import qs.Ui
import qs.Ui as Ui

// Lecoo GK301 (SEMICO 1a2c:9df4) RGB control panel.
//
// The bar icon is a thin shell; every device write goes through the Rust
// binary (~/.local/bin/lecoo-gk301), which owns the HID protocol and caches
// the last applied state in ~/.local/state/lecoo-gk301/state.json.
//
// Controls reflect the lighting paths recovered from the Windows tool.
// Arbitrary RGB and on-device macros remain unverified.
Panel {
  id: root
  moduleName: "borat.lecoo-rgb"
  manageIpc: false

  // Injected by the host bar widget.
  property var anchorItem: null
  property var hostWidget: null
  property var signalService: null
  readonly property var barIdentity: hostWidget || root

  readonly property string homePath: Quickshell.env("HOME")
  readonly property string binary: setting("binary", "") || (homePath + "/.local/bin/lecoo-gk301")
  readonly property string localBinary: homePath + "/.local/bin/lecoo-gk301"
  readonly property bool deviceConnected: signalService ? signalService.connected : fallbackConnected
  readonly property bool deviceWritable: signalService ? signalService.writable : fallbackWritable
  property bool fallbackConnected: false
  property bool fallbackWritable: false
  property double lastWriteFinished: 0
  property int requestSerial: 0
  property string activeRequest: ""
  property var blackboxEntries: []
  property string blackboxError: ""
  property string page: "lighting"
  property bool cargoAvailable: false
  property bool setupBusy: false
  property string setupMessage: ""
  property string lifecycleAction: ""
  property var lifecycleQueue: []
  property bool removeController: false
  property bool removePlugin: true
  property bool removeUdev: false
  property bool removeState: false
  property bool removalConfirmed: false

  // ---- last state reported by the Rust cache ----
  property int effect: 0
  property int brightness: 5
  property int speed: 2
  property int select: 0
  property string panelName: ""
  property string lastError: ""
  property string connectionError: ""
  property string debugText: ""

  property var panelNames: []

  // ---- macros (timed lighting sequences) ----
  property var macroEntries: []
  property string macroError: ""
  property string macroMessage: ""
  property string macroName: ""
  property var macroSteps: []
  property string macroDelay: "200"
  property bool macroBusy: false

  // True while a text field in the panel has the keyboard focus.
  property bool textEditing: false

  // Scroll bookkeeping: the target position the arrows and the wheel share.
  property real scrollTarget: 0

  // ---- GK301 function keys ----
  property var keyEntries: []
  property string keyError: ""
  property string keyMessage: ""

  // Extras the Windows app does not have.
  property bool autoCycle: false
  property int cycleSeconds: 60

  // The panel pages, in the order they appear in the navigation row.
  readonly property var pages: ["lighting", "automation", "macros", "keys", "blackbox", "setup", "uninstall"]

  // The CLI owns one state file, so device writes must not overlap. Every
  // command goes through this queue.
  property var queue: []
  property bool busy: false
  property bool stateSyncPending: false
  property int stateGeneration: 0

  readonly property bool lit: brightness > 0
  readonly property int panelCount: panelNames.length > 0 ? panelNames.length : 20

  // ---- reading state ----

  function refresh() {
    if (!root.busy && root.queue.length === 0 && !statusProc.running) {
      statusProc.generation = root.stateGeneration
      statusProc.running = true
    }
  }
  function refreshCatalog() { if (!catalogProc.running) catalogProc.running = true }
  function refreshInfo() {
    if (signalService) signalService.checkPresence()
    else if (!presenceProc.running) { presenceProc.output = ""; presenceProc.running = true }
  }
  function refreshBlackbox() { if (!blackboxProc.running && !setupBusy) blackboxProc.running = true }
  function refreshCargo() {
    if (cargoProbe.running) return
    root.cargoAvailable = false
    cargoProbe.running = true
  }
  onPageChanged: if (page === "setup") refreshCargo()

  function parseStatus(raw, generation) {
    if (root.busy || root.queue.length > 0 || (generation !== undefined && generation !== root.stateGeneration)) return
    try {
      var s = JSON.parse(String(raw || "{}"))
      root.effect = Number(s.effect) || 0
      root.brightness = Number(s.brightness) || 0
      root.speed = Number(s.speed) || 0
      root.select = Number(s.select) || 0
      root.panelName = String(s.name || "")
      root.stateSyncPending = false
    } catch (e) {
      root.lastError = "Could not read lighting settings"
    }
  }

  function effectLabel(index) {
    var i = Math.max(0, Number(index) || 0)
    // One source of truth: the catalog the controller ships. The names are
    // the original Windows tool's own effect names.
    if (i < root.panelNames.length) return String(root.panelNames[i]).replace(/_/g, " ")
    return "Effect " + (i + 1)
  }

  // ---- macros ----
  function refreshMacros() { if (!macroProc.running) macroProc.running = true }
  function refreshKeys() { if (!keysProc.running) keysProc.running = true }

  // The scroll position the arrows and the wheel move towards. It is clamped
  // to the content, so the page can never be scrolled past its own height and
  // a short page can never be pushed out of view.
  function clampScroll(value) {
    var max = Math.max(0, body.implicitHeight - scroller.height)
    return Math.max(0, Math.min(max, value))
  }
  function scrollPage(direction) {
    root.scrollTarget = root.clampScroll(root.scrollTarget + direction * Style.space(48))
    scroller.contentItem.contentY = root.scrollTarget
  }

  function addMacroStep() {
    root.macroSteps = root.macroSteps.concat([{
      delay_ms: Math.max(0, Math.round(Number(root.macroDelay) || 0)),
      effect: root.effect,
      brightness: root.brightness,
      speed: root.speed,
      select: root.select
    }])
  }

  function removeMacroStep(index) {
    var next = root.macroSteps.slice()
    next.splice(index, 1)
    root.macroSteps = next
  }

  function saveMacro() {
    if (root.macroName.trim() === "") { root.macroError = "Give the macro a name"; return }
    if (root.macroSteps.length === 0) { root.macroError = "Add at least one step"; return }
    root.macroError = ""
    root.macroMessage = ""
    macroActionProc.command = [root.binary, "macro", "save", "--name", root.macroName.trim(),
                               "--steps", JSON.stringify(root.macroSteps)]
    macroActionProc.running = true
  }

  function playMacro(name) {
    root.macroError = ""
    root.macroMessage = ""
    macroActionProc.command = [root.binary, "macro", "play", name]
    macroActionProc.running = true
  }

  function removeMacro(name) {
    root.macroError = ""
    root.macroMessage = ""
    macroActionProc.command = [root.binary, "macro", "remove", name]
    macroActionProc.running = true
  }

  // ---- function keys ----
  function saveKey(name, command) {
    root.keyError = ""
    root.keyMessage = ""
    keyActionProc.command = [root.binary, "keys", "set", name, "--command", command]
    keyActionProc.running = true
  }

  function exportKeys() {
    root.keyError = ""
    root.keyMessage = ""
    keyActionProc.command = [root.binary, "keys", "export"]
    keyActionProc.running = true
  }



  function tooltipText() {
    var name = root.effectLabel(root.effect)
    // Cached settings are only worth saying when the keyboard is not talking
    // back. A connected, writable device says nothing about lighting state.
    var state = !root.deviceConnected ? " · Ghost Mode · cached"
      : !root.deviceWritable ? " · permission denied"
      : ""
    return "OmaLecoo · " + name + " · brightness " + root.brightness + "/13"
      + (root.autoCycle ? " · auto" : "")
      + state
      + (root.signalService ? " · " + root.signalService.activeRule : "")
  }

  // ---- writing state ----

  function enqueue(args) {
    if (root.stateSyncPending || root.setupBusy) return
    if (root.signalService) root.signalService.pauseAutomation()
    if (["preset", "reset", "restore"].indexOf(args[0]) !== -1)
      root.stateSyncPending = true
    root.stateGeneration += 1
    root.queue = root.queue.concat([args])
    if (!root.busy) root.runNext()
  }

  function runNext() {
    if (root.queue.length === 0) {
      root.busy = false
      return
    }
    if (!root.signalService && Date.now() - root.lastWriteFinished < 2000) {
      fallbackPacing.interval = Math.ceil(2000 - (Date.now() - root.lastWriteFinished))
      fallbackPacing.restart()
      return
    }
    var args = root.queue[0]
    root.queue = root.queue.slice(1)
    worker.err = ""
    root.busy = true
    if (root.signalService) {
      root.activeRequest = String(root) + ":" + String(++root.requestSerial)
      root.signalService.submitPanelCommand(args, root.activeRequest)
    } else {
      worker.handled = false
      worker.output = ""
      worker.command = [root.binary].concat(args).concat(["--origin", "panel"])
      worker.running = true
    }
  }

  function finishCommand(code, error, output) {
    root.lastWriteFinished = Date.now()
    if (output.indexOf("device:") === 0) root.debugText = output
    if (code !== 0) {
      root.queue = []
      root.autoCycle = false
      root.lastError = error || ("Command failed (" + code + ")")
      root.stateSyncPending = false
    } else root.lastError = ""
    root.busy = false
    root.runNext()
    if (!root.busy) { root.refresh(); root.refreshBlackbox() }
  }

  Connections {
    target: root.signalService
    function onPanelCommandFinished(requestId, code, error, output) {
      if (requestId === root.activeRequest) root.finishCommand(code, error, output)
    }
    function onCommandFinished() { root.refresh(); if (root.opened) root.refreshBlackbox() }
  }
  Timer { id: fallbackPacing; onTriggered: root.runNext() }
  onBinaryChanged: {
    root.removeController = false
    root.removalConfirmed = false
    if (signalService) signalService.binary = binary
  }
  onSignalServiceChanged: { if (signalService) signalService.binary = binary }

  // The panel command carries the whole panel configuration in one report, so
  // effect, brightness, speed and zone always land together.
  function pushPanel() {
    root.enqueue([
      "panel", String(root.effect),
      "--brightness", String(root.brightness),
      "--speed", String(root.speed),
      "--select", String(root.select)
    ])
  }



  function setEffect(index) {
    if (root.stateSyncPending) return
    var n = Math.max(1, root.panelCount)
    var i = ((Math.round(Number(index) || 0) % n) + n) % n
    if (i === root.effect) return
    root.effect = i
    root.panelName = root.effectLabel(i)
    root.pushPanel()
  }

  function nextEffect() { root.setEffect((root.effect + 1) % root.panelCount) }
  function prevEffect() { root.setEffect((root.effect + root.panelCount - 1) % root.panelCount) }

  function setBrightness(value) {
    if (root.stateSyncPending) return
    var b = Math.max(0, Math.min(13, Math.round(Number(value) || 0)))
    if (b === root.brightness) return
    root.brightness = b
    root.enqueue(["brightness", String(b)])
  }

  function stepBrightness(direction) { root.setBrightness(root.brightness + direction) }

  function setSpeed(value) {
    if (root.stateSyncPending) return
    var s = Math.max(0, Math.min(2, Math.round(Number(value) || 0)))
    if (s === root.speed) return
    root.speed = s
    root.pushPanel()
  }

  function setSelect(value) {
    if (root.stateSyncPending) return
    var s = Math.max(0, Math.min(2, Math.round(Number(value) || 0)))
    if (s === root.select) return
    root.select = s
    root.pushPanel()
  }





  function applyPreset(name) { root.enqueue(["preset", name]) }



  Component.onCompleted: {
    root.refreshCatalog()
    root.refresh()
    root.refreshInfo()
    root.refreshCargo()
  }

  function open() {
    if (root.page === "setup") root.refreshCargo()
    root.refresh()
    root.refreshInfo()
    root.refreshBlackbox()
    if (scroller.contentItem) scroller.contentItem.contentY = 0
    root.controller.show()
  }

  Timer {
    id: cycleTimer
    interval: Math.max(5, root.cycleSeconds) * 1000
    repeat: true
    running: root.autoCycle && !root.setupBusy
    onTriggered: root.nextEffect()
  }


  // The keyboard can also be driven from the CLI, so the panel (and the bar
  // tooltip) cannot assume it is the only writer. Slow poll while closed,
  // quick one while the panel is open.
  Timer {
    interval: root.opened ? 2500 : 15000
    running: !root.setupBusy
    repeat: true
    triggeredOnStart: true
    onTriggered: root.refresh()
  }

  Timer {
    interval: 15000
    running: !root.setupBusy
    repeat: true
    onTriggered: root.refreshInfo()
  }

  // ---- the CLI ----

  Process {
    id: worker
    property string err: ""
    property string output: ""
    property bool handled: true
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        worker.output = String(text || "").trim()
        if (worker.command.length > 1 && worker.command[1] === "doctor")
          root.debugText = String(text || "").trim()
      }
    }
    stderr: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        var value = String(text || "").trim()
        if (value !== "") worker.err = value
      }
    }
    onExited: function(code) {
      Qt.callLater(function() {
        if (worker.handled) return
        worker.handled = true
        root.finishCommand(code, worker.err, worker.output)
      })
    }
    onRunningChanged: if (!running && !handled) fallbackFailureGuard.restart()
  }
  Timer {
    id: fallbackFailureGuard
    interval: 100
    onTriggered: if (!worker.running && !worker.handled) {
      worker.handled = true
      root.finishCommand(1, worker.err || "Could not start the controller. Open Setup to install or verify it.", "")
    }
  }

  Process {
    id: statusProc
    property int generation: 0
    command: [root.binary, "status", "--json"]
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: root.parseStatus(text, statusProc.generation)
    }
    onExited: function(code) {
      if (code !== 0 && root.lastError === "")
        root.lastError = "Could not load settings (lecoo-gk301)"
      if (statusProc.generation !== root.stateGeneration) Qt.callLater(root.refresh)
    }
  }

  Process {
    id: catalogProc
    command: [root.binary, "catalog", "--json"]
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        try {
          var data = JSON.parse(String(text || "{}"))
          root.panelNames = data.panel || []
        } catch (e) {
          root.lastError = "Could not read the effect list"
        }
      }
    }
  }

  Process {
    id: presenceProc
    property string output: ""
    command: [root.binary, "dev", "--json"]
    stdout: StdioCollector { onStreamFinished: presenceProc.output = String(text || "") }
    onExited: function(code) {
      try {
        if (code !== 0) throw new Error("unavailable")
        var data = JSON.parse(output)
        root.fallbackConnected = data.connected === true
        root.fallbackWritable = data.writable === true
        root.connectionError = !root.fallbackConnected ? "Ghost Mode · disconnected. Showing cached settings."
          : !root.fallbackWritable ? "Keyboard connected · permission denied. Authorize device access in Setup." : ""
      } catch (e) {
        root.fallbackConnected = false
        root.fallbackWritable = false
        root.connectionError = "Controller unavailable. Open Setup to install or verify it."
      }
    }
  }

  Process {
    id: blackboxProc
    command: [root.binary, "blackbox", "--json"]
    stdout: StdioCollector {
      onStreamFinished: {
        try {
          root.blackboxEntries = JSON.parse(String(text || "[]")).slice(0, 40)
          root.blackboxError = ""
        } catch (e) { root.blackboxError = "Could not read Blackbox history" }
      }
    }
    onExited: function(code) { if (code !== 0) root.blackboxError = "Blackbox unavailable. Verify the controller in Setup." }
  }

  // ---- macros ----
  Process {
    id: macroProc
    command: [root.binary, "macro", "list", "--json"]
    stdout: StdioCollector {
      onStreamFinished: {
        try {
          root.macroEntries = JSON.parse(String(text || "{}")).macros || []
          root.macroError = ""
        } catch (e) { root.macroError = "Could not read the macros" }
      }
    }
    onExited: function(code) { if (code !== 0) root.macroError = "Macros unavailable. Verify the controller in Setup." }
  }

  Process {
    id: macroActionProc
    property string output: ""
    property string error: ""
    stdout: StdioCollector { onStreamFinished: macroActionProc.output = String(text || "").trim() }
    stderr: StdioCollector { onStreamFinished: macroActionProc.error = String(text || "").trim() }
    onExited: function(code) {
      root.macroBusy = false
      if (code !== 0) { root.macroError = macroActionProc.error || "The macro command failed"; return }
      root.macroError = ""
      root.macroMessage = macroActionProc.output
      if (macroActionProc.output.indexOf("Saved macro") === 0) {
        root.macroName = ""
        root.macroSteps = []
      }
      root.refreshMacros()
    }
    onRunningChanged: if (running) root.macroBusy = true
  }

  // ---- function keys ----
  Process {
    id: keysProc
    command: [root.binary, "keys", "list", "--json"]
    stdout: StdioCollector {
      onStreamFinished: {
        try {
          root.keyEntries = JSON.parse(String(text || "{}")).keys || []
          root.keyError = ""
        } catch (e) { root.keyError = "Could not read the key map" }
      }
    }
    onExited: function(code) { if (code !== 0) root.keyError = "Key map unavailable. Verify the controller in Setup." }
  }

  Process {
    id: keyActionProc
    property string output: ""
    property string error: ""
    stdout: StdioCollector { onStreamFinished: keyActionProc.output = String(text || "").trim() }
    stderr: StdioCollector { onStreamFinished: keyActionProc.error = String(text || "").trim() }
    onExited: function(code) {
      if (code !== 0) { root.keyError = keyActionProc.error || "The key command failed"; return }
      root.keyError = ""
      root.keyMessage = keyActionProc.output
      root.refreshKeys()
    }
  }

  function beginLifecycle(steps) {
    if (setupBusy || busy || queue.length > 0 || (signalService && signalService.busy)) return
    root.autoCycle = false
    root.setupBusy = true
    if (signalService) signalService.suspended = true
    root.lifecycleQueue = steps
    root.runLifecycleStep()
  }

  function runLifecycleStep() {
    if (lifecycleQueue.length === 0) {
      root.setupBusy = false
      root.lifecycleAction = ""
      root.removalConfirmed = false
      if (signalService) { signalService.suspended = false; signalService.reloadState() }
      root.refreshCatalog(); root.refresh(); root.refreshInfo()
      return
    }
    var step = lifecycleQueue[0]
    root.lifecycleQueue = lifecycleQueue.slice(1)
    root.lifecycleAction = step.label
    root.setupMessage = step.label
    if (step.detached) {
      // This final command must outlive the plugin that it unloads.
      Quickshell.execDetached(["omarchy", "plugin", "remove", "borat.lecoo-rgb", "--yes"])
      root.setupMessage = "Plugin removal started. OmaLecoo will close when Omarchy unloads it."
      root.lifecycleQueue = []
      root.setupBusy = false
      return
    }
    lifecycleProc.output = ""
    lifecycleProc.error = ""
    lifecycleProc.handled = false
    lifecycleProc.command = step.argv
    lifecycleProc.running = true
  }

  function finishLifecycle(code) {
    if (lifecycleProc.handled) return
    lifecycleProc.handled = true
    if (code !== 0) {
      root.setupMessage = lifecycleAction + " failed: " + (lifecycleProc.error || "Could not start or complete the command.")
      root.lifecycleQueue = []
      root.setupBusy = false
      if (signalService) { signalService.suspended = false; signalService.reloadState() }
      return
    }
    root.setupMessage = lifecycleAction + " completed. " + lifecycleProc.output.slice(-1500)
    root.runLifecycleStep()
  }

  function installController() {
    if (!cargoAvailable) { setupMessage = "Install Rust and Cargo first: omarchy pkg add rust"; return }
    beginLifecycle([
      { label: "Installing controller from source", argv: ["cargo", "install", "--git", "https://github.com/OnurByte/OmaLecoo", "--root", homePath + "/.local", "--locked"] },
      { label: "Verifying installed controller", argv: [localBinary, "capabilities", "--json"] }
    ])
  }

  function authorizeDeviceAccess() {
    beginLifecycle([{ label: "Authorizing the GK301 udev rule", argv: ["pkexec", binary, "udev", "install"] }])
  }

  function uninstallSelected() {
    if (!removalConfirmed) return
    var steps = []
    // Cleanup requiring the controller must finish before Cargo removes it.
    if (removeUdev) steps.push({ label: "Removing the GK301 udev rule", argv: ["pkexec", binary, "udev", "remove"] })
    if (removeState) steps.push({ label: "Removing OmaLecoo state and logs", argv: [binary, "state", "clear"] })
    if (removeController && binary === localBinary)
      steps.push({ label: "Uninstalling the local controller", argv: ["cargo", "uninstall", "--root", homePath + "/.local", "omalecoo"] })
    if (removePlugin) steps.push({ label: "Removing the Omarchy plugin", detached: true })
    if (steps.length === 0) { setupMessage = "Select an item to remove."; return }
    beginLifecycle(steps)
  }

  Process {
    id: cargoProbe
    command: ["cargo", "--version"]
    onExited: function(code) { root.cargoAvailable = code === 0 }
  }
  Process {
    id: lifecycleProc
    property string output: ""
    property string error: ""
    property bool handled: true
    stdout: StdioCollector { onStreamFinished: lifecycleProc.output = String(text || "").trim() }
    stderr: StdioCollector { onStreamFinished: lifecycleProc.error = String(text || "").trim() }
    onExited: function(code) { Qt.callLater(function() { root.finishLifecycle(code) }) }
    onRunningChanged: if (!running && !handled) lifecycleFailureGuard.restart()
  }
  Timer {
    id: lifecycleFailureGuard
    interval: 100
    onTriggered: if (!lifecycleProc.running && !lifecycleProc.handled) root.finishLifecycle(1)
  }

  // ---- the panel itself ----

  KeyboardPanel {
    id: panel
    anchorItem: root.anchorItem
    owner: root.barIdentity
    bar: root.bar
    open: root.opened
    focusTarget: keyCatcher
    contentWidth: panel.fittedContentWidth(Style.space(520))
    contentHeight: panel.fittedContentHeight(body.implicitHeight)

    PanelKeyCatcher {
      id: keyCatcher
      anchors.fill: parent
      // Any text field in the panel takes the keys; the shortcuts must not
      // fire while somebody is typing a name or a command.
      blocked: effectSearch.activeFocus || root.textEditing
      onTabRequested: function(direction) {
        var item = keyCatcher.nextItemInFocusChain(direction > 0)
        if (item) item.forceActiveFocus(Qt.TabFocusReason)
      }
      onCloseRequested: root.close()
      onMoveRequested: function(dx, dy) {
        // Arrows scroll the panel. The scroll is clamped to the content, so
        // a short page can never be pushed out of view, and it is animated so
        // the content glides instead of jumping.
        if (dy !== 0) root.scrollPage(dy)
      }
      onTextKey: function(t) {
        if (effectSearch.activeFocus) return
        if (t === "n") root.nextEffect()
        else if (t === "p") root.prevEffect()
        else if (t === "o") root.applyPreset("off")
        else if (t === "r") { root.refresh(); root.refreshInfo() }
      }

      ScrollView {
        id: scroller
        anchors.fill: parent
        clip: true
        ScrollBar.horizontal.policy: ScrollBar.AlwaysOff
        ScrollBar.vertical.policy: body.implicitHeight > height ? ScrollBar.AsNeeded : ScrollBar.AlwaysOff
        Binding {
          target: scroller.contentItem
          property: "interactive"
          value: body.implicitHeight > scroller.height
        }
        // Wheel scrolling and arrow scrolling share one clamped target.
        Connections {
          target: scroller.contentItem
          function onContentYChanged() {
            root.scrollTarget = root.clampScroll(scroller.contentItem.contentY)
          }
        }

        Column {
          id: body
          width: scroller.availableWidth
          spacing: Style.space(10)

          // ---------- header ----------
          Row {
            width: parent.width
            spacing: Style.space(8)

            Column {
              width: parent.width - headerActions.width - Style.space(8)
              spacing: Style.space(2)

              Text {
                width: parent.width
                text: "OmaLecoo · GK301"
                color: Color.foreground
                font.family: Style.font.family
                font.pixelSize: Style.font.title
                font.bold: true
                font.letterSpacing: 1
                elide: Text.ElideRight
              }

              Text {
                width: parent.width
                text: (root.deviceConnected ? "Cached settings" : "Ghost Mode · cached")
                      + "  ·  " + root.effectLabel(root.effect)
                      + "  ·  " + root.brightness + "/13"
                      + (root.autoCycle ? "  ·  auto " + root.cycleSeconds + " s" : "")
                color: Color.muted
                font.family: Style.font.family
                font.pixelSize: Style.font.caption
                elide: Text.ElideRight
              }
            }

            Row {
              id: headerActions
              spacing: Style.space(4)

              PanelActionButton {
                iconText: "󰑐"
                tooltipText: "Refresh status"
                onClicked: { root.refresh(); root.refreshInfo() }
              }

              PanelActionButton {
                iconText: "󰒓"
                tooltipText: "Run connection diagnostics"
                onClicked: root.enqueue(["doctor"])
              }
              PanelActionButton {
                iconText: "󰑐"
                tooltipText: "Reapply saved lighting settings"
                onClicked: root.enqueue(["restore"])
              }
              PanelActionButton {
                iconText: "󰓜"
                tooltipText: "Restore factory lighting defaults"
                onClicked: root.enqueue(["restore", "--factory"])
              }
              PanelActionButton {
                iconText: "󰐥"
                tooltipText: "Turn lighting off"
                onClicked: root.applyPreset("off")
              }
            }
          }

          Text {
            visible: root.lastError !== "" || root.connectionError !== ""
            width: parent.width
            text: [root.lastError, root.connectionError].filter(function(value) { return value !== "" }).join("\n")
            textFormat: Text.PlainText
            wrapMode: Text.WordWrap
            color: Color.urgent
            font.family: Style.font.family
            font.pixelSize: Style.font.caption
          }

          Note {
            width: parent.width
            text: root.signalService ? root.signalService.connectionMessage : root.connectionError
            color: root.deviceConnected && root.deviceWritable ? Color.muted : Color.urgent
          }

          Row {
            id: navigation
            width: parent.width
            spacing: Style.space(4)
            Repeater {
              model: root.pages
              delegate: Button {
                required property string modelData
                width: (navigation.width - navigation.spacing * (root.pages.length - 1)) / root.pages.length
                text: modelData.charAt(0).toUpperCase() + modelData.slice(1)
                fontSize: Style.font.caption
                horizontalPadding: Style.space(2)
                bordered: true
                onClicked: {
                  root.page = modelData
                  root.removalConfirmed = false
                  if (modelData === "blackbox") root.refreshBlackbox()
                  if (modelData === "macros") root.refreshMacros()
                  if (modelData === "keys") root.refreshKeys()
                }
              }
            }
          }

          PanelSeparator { foreground: Color.foreground }

          Column {
            visible: root.page === "lighting"
            width: parent.width
            spacing: Style.space(10)
            enabled: !root.setupBusy
            opacity: root.deviceConnected ? 1 : 0.6
          // ---------- panel effects ----------
          Row {
            width: parent.width

            PanelSectionHeader {
              width: parent.width - effectCounter.width - Style.space(8)
              text: "LIGHTING EFFECTS"
              foreground: Color.foreground
            }

            Text {
              id: effectCounter
              text: (root.effect + 1) + " / " + root.panelCount
              color: Color.muted
              font.family: Style.font.family
              font.pixelSize: Style.font.caption
            }
          }

          Ui.TextField {
            id: effectSearch
            width: parent.width
            placeholderText: "Search effects…"
            selectByMouse: true
            Accessible.name: "Search lighting effects"
            onActiveFocusChanged: root.textEditing = activeFocus
          }

          Grid {
            id: effectGrid
            width: parent.width
            columns: 2
            columnSpacing: Style.space(6)
            rowSpacing: Style.space(6)

            Repeater {
              model: root.panelNames

              delegate: EffectChip {
                required property string modelData
                required property int index
                width: Math.floor((effectGrid.width - effectGrid.columnSpacing * (effectGrid.columns - 1)) / effectGrid.columns)
                indexLabel: String(index + 1)
                label: root.effectLabel(index)
                visible: effectSearch.text.trim() === "" || label.toLocaleLowerCase().indexOf(effectSearch.text.trim().toLocaleLowerCase()) !== -1
                selected: root.effect === index
                onPicked: root.setEffect(index)
              }
            }
          }

          Text {
            visible: root.panelNames.length === 0
            width: parent.width
            text: "Loading effects…"
            color: Color.muted
            font.family: Style.font.family
            font.pixelSize: Style.font.caption
          }

          // ---------- panel brightness ----------
          Row {
            width: parent.width
            spacing: Style.space(8)

            Text {
              width: Style.space(88)
              height: brightnessSlider.implicitHeight
              text: "Brightness"
              color: Color.foreground
              font.family: Style.font.family
              font.pixelSize: Style.font.caption
              verticalAlignment: Text.AlignVCenter
            }

            PanelSlider {
              id: brightnessSlider
              width: parent.width - Style.space(88) - brightnessValue.width - Style.space(16)
              bar: root.bar
              minimum: 0
              maximum: 13
              step: 1
              integer: true
              tickCount: 14
              value: root.brightness
              onReleased: function(v) { root.setBrightness(v) }
            }

            Text {
              id: brightnessValue
              width: Style.space(34)
              height: brightnessSlider.implicitHeight
              text: Math.round(brightnessSlider.liveValue) + "/13"
              color: Color.muted
              horizontalAlignment: Text.AlignRight
              verticalAlignment: Text.AlignVCenter
              font.family: Style.font.family
              font.pixelSize: Style.font.caption
            }
          }

          // ---------- speed ----------
          // The row keeps its place for every effect so the panel does not
          // change height when the effect changes. Speed only applies to the
          // effects the original tool mapped it to.
          Row {
            width: parent.width
            spacing: Style.space(8)
            enabled: root.effect >= 2
            opacity: root.effect >= 2 ? 1 : 0.45

            Text {
              width: Style.space(88)
              height: Style.space(28)
              text: "Speed"
              color: Color.foreground
              font.family: Style.font.family
              font.pixelSize: Style.font.caption
              verticalAlignment: Text.AlignVCenter
            }

            ButtonGroup {
              options: [
                { value: "0", label: "Slow" },
                { value: "1", label: "Medium" },
                { value: "2", label: "Fast" }
              ]
              value: String(root.speed)
              foreground: Color.foreground
              background: Color.popups.background
              accent: Color.accent
              onChanged: function(v) { root.setSpeed(Number(v)) }
            }
          }

          // ---------- loader (M1 / M2 / M3) ----------
          // The loader is the original tool's own control for the keyboard's
          // M1/M2/M3 key group. It rides in the same report as the effect.
          Row {
            width: parent.width
            spacing: Style.space(8)
            enabled: root.effect === 1
            opacity: root.effect === 1 ? 1 : 0.45

            Text {
              width: Style.space(88)
              height: Style.space(28)
              text: "M1 / M2 / M3"
              color: Color.foreground
              font.family: Style.font.family
              font.pixelSize: Style.font.caption
              verticalAlignment: Text.AlignVCenter
            }

            ButtonGroup {
              options: [
                { value: "0", label: "M1" },
                { value: "1", label: "M2" },
                { value: "2", label: "M3" }
              ]
              value: String(root.select)
              foreground: Color.foreground
              background: Color.popups.background
              accent: Color.accent
              onChanged: function(v) { root.setSelect(Number(v)) }
            }
          }

          Note {
            width: parent.width
            text: "M1, M2 and M3 are the original tool's loader values. The keyboard's own FN+1 … FN+9 keys pick which key group glows (FN+1 lights ESC, WASD and the arrow keys); that choice lives in the keyboard, and the recovered protocol has no field for it. The Game preset re-applies the Gaming Special Key effect, so the keyboard lights whichever group FN+N last selected."
          }

          // ---------- presets ----------
          PanelSectionHeader {
            width: parent.width
            text: "PRESETS"
            foreground: Color.foreground
          }

          Row {
            id: presetRow
            width: parent.width
            spacing: Style.space(6)

            Repeater {
              model: [
                { key: "silent", label: "Calm" },
                { key: "game", label: "Game" },
                { key: "night", label: "Night" },
                { key: "party", label: "Party" },
                { key: "off", label: "Off" },
                { key: "random", label: "Random" }
              ]

              delegate: Button {
                required property var modelData
                width: Math.floor((presetRow.width - presetRow.spacing * 5) / 6)
                height: Style.space(28)
                text: modelData.label
                fontSize: Style.font.caption
                horizontalPadding: Style.space(2)
                verticalPadding: 0
                bordered: true
                background: Color.popups.background
                onClicked: root.applyPreset(modelData.key)
              }
            }
          }

          // ---------- auto rotation ----------
          Note {
            width: parent.width
            text: "Game applies the keyboard's Gaming Special Key effect: it lights the key group the keyboard's own FN+1 … FN+9 key last chose."
          }

          Row {
            width: parent.width
            spacing: Style.space(8)

            Text {
              width: Style.space(88)
              height: Style.space(28)
              text: "Auto cycle"
              color: Color.foreground
              font.family: Style.font.family
              font.pixelSize: Style.font.caption
              verticalAlignment: Text.AlignVCenter
            }

            ButtonGroup {
              width: parent.width - Style.space(88) - autoSwitch.width - Style.space(16)
              options: [
                { value: "30", label: "30 s" },
                { value: "60", label: "1 min" },
                { value: "300", label: "5 min" }
              ]
              value: String(root.cycleSeconds)
              foreground: Color.foreground
              background: Color.popups.background
              accent: Color.accent
              onChanged: function(v) { root.cycleSeconds = Number(v) }
            }

            ToggleSwitch {
              id: autoSwitch
              checked: root.autoCycle
              foreground: Color.foreground
              accent: Color.accent
              onToggled: root.autoCycle = !root.autoCycle
            }
          }

          Text {
            width: parent.width
            text: "Speed support varies by effect. Settings show the last command sent to the keyboard."
            wrapMode: Text.WordWrap
            color: Color.muted
            font.pixelSize: Style.font.caption
          }
          Text {
            visible: root.debugText !== ""
            width: parent.width
            text: root.debugText
            textFormat: Text.PlainText
            wrapMode: Text.WrapAnywhere
            color: Color.muted
            font.pixelSize: Style.font.caption
          }

          }

          Column {
            visible: root.page === "automation"
            width: parent.width
            spacing: Style.space(10)
            PanelSectionHeader { text: "SIGNAL ENGINE"; foreground: Color.foreground }
            Note {
              width: parent.width
              text: root.signalService ? root.signalService.activeRule + "\n" + root.signalService.ruleReason
                : "Automation requires Omarchy’s built-in bar. Manual lighting controls remain available."
            }
            OptionRow {
              width: parent.width
              label: "Enable automation"
              checked: root.signalService ? root.signalService.automationEnabled : false
              enabled: !!root.signalService && !root.setupBusy
              onToggled: root.signalService.setRules(!checked, root.signalService.restoreOnReconnect)
            }
            OptionRow {
              width: parent.width
              label: "Restore settings once after reconnect"
              checked: root.signalService ? root.signalService.restoreOnReconnect : false
              enabled: !!root.signalService && !root.setupBusy
              onToggled: root.signalService.setRules(root.signalService.automationEnabled, !checked)
            }
            Note {
              width: parent.width
              text: "Priority: idle for 5 minutes → off; active fullscreen window → Game; otherwise → last manual settings. Manual actions pause automation for 30 minutes. Idle inhibitors are respected."
            }
            Note { width: parent.width; text: root.signalService ? root.signalService.lastOutcome : "" }
          }

          Column {
            visible: root.page === "macros"
            width: parent.width
            spacing: Style.space(8)
            PanelSectionHeader { text: "MACROS"; foreground: Color.foreground }
            Note {
              width: parent.width
              text: "A macro is a timed sequence of lighting states, played on the keyboard. The original Windows tool recorded key sequences; on Linux the compositor owns keyboard input, so OmaLecoo macros move the lights."
            }

            Repeater {
              model: root.macroEntries
              delegate: Row {
                required property var modelData
                width: parent.width
                spacing: Style.space(6)

                Column {
                  width: parent.width - macroActions.width - Style.space(6)
                  spacing: Style.space(2)
                  Text {
                    width: parent.width
                    text: String(modelData.name || "")
                    color: Color.foreground
                    font.family: Style.font.family
                    font.pixelSize: Style.font.body
                    elide: Text.ElideRight
                  }
                  Note {
                    width: parent.width
                    text: String(modelData.group || "") + " · " + (modelData.steps ? modelData.steps.length : 0)
                      + " step(s) · repeat " + String(modelData.repeat || 1)
                  }
                }

                Row {
                  id: macroActions
                  spacing: Style.space(4)
                  Button {
                    text: "Play"
                    fontSize: Style.font.caption
                    horizontalPadding: Style.space(2)
                    bordered: true
                    onClicked: root.playMacro(String(modelData.name || ""))
                  }
                  Button {
                    text: "Remove"
                    fontSize: Style.font.caption
                    horizontalPadding: Style.space(2)
                    bordered: true
                    onClicked: root.removeMacro(String(modelData.name || ""))
                  }
                }
              }
            }
            Note {
              width: parent.width
              visible: root.macroEntries.length === 0
              text: "No macros saved yet."
            }

            PanelSeparator { foreground: Color.foreground }
            PanelSectionHeader { text: "NEW MACRO"; foreground: Color.foreground }

            Ui.TextField {
              id: macroNameField
              width: parent.width
              placeholderText: "Macro name…"
              selectByMouse: true
              Accessible.name: "Macro name"
              onTextChanged: root.macroName = text
              onActiveFocusChanged: root.textEditing = activeFocus
            }

            Row {
              width: parent.width
              spacing: Style.space(6)
              Ui.TextField {
                id: macroDelayField
                width: parent.width - captureStep.width - Style.space(6)
                placeholderText: "Delay before the step in ms"
                text: root.macroDelay
                selectByMouse: true
                Accessible.name: "Step delay in milliseconds"
                onTextChanged: root.macroDelay = text
                onActiveFocusChanged: root.textEditing = activeFocus
              }
              Button {
                id: captureStep
                text: "Add current lighting"
                fontSize: Style.font.caption
                horizontalPadding: Style.space(2)
                bordered: true
                onClicked: root.addMacroStep()
              }
            }

            Repeater {
              model: root.macroSteps
              delegate: Row {
                required property var modelData
                required property int index
                width: parent.width
                spacing: Style.space(6)
                Note {
                  width: parent.width - dropStep.width - Style.space(6)
                  text: (index + 1) + ". after " + String(modelData.delay_ms) + " ms · effect "
                    + (Number(modelData.effect) + 1) + " · brightness " + String(modelData.brightness)
                    + "/13 · speed " + String(modelData.speed) + "/2"
                }
                Button {
                  id: dropStep
                  text: "×"
                  fontSize: Style.font.caption
                  horizontalPadding: Style.space(2)
                  bordered: true
                  onClicked: root.removeMacroStep(index)
                }
              }
            }
            Note {
              width: parent.width
              visible: root.macroSteps.length === 0
              text: "Add the lighting states the macro should play, in order."
            }

            Button {
              text: "Save macro"
              fontSize: Style.font.caption
              horizontalPadding: Style.space(2)
              bordered: true
              onClicked: root.saveMacro()
            }
            Note { width: parent.width; text: root.macroError }
            Note { width: parent.width; text: root.macroMessage }
          }

          Column {
            visible: root.page === "keys"
            width: parent.width
            spacing: Style.space(8)
            PanelSectionHeader { text: "FUNCTION KEYS"; foreground: Color.foreground }
            Note {
              width: parent.width
              text: "The GK301's function and media keys, as the original Windows tool listed them in its config.xml. The commands run through Hyprland; the plugin writes a fragment you include from your own Hyprland config."
            }

            Repeater {
              model: root.keyEntries
              delegate: Row {
                required property var modelData
                width: parent.width
                spacing: Style.space(6)

                Column {
                  width: Style.space(140)
                  spacing: Style.space(2)
                  Text {
                    width: parent.width
                    text: String(modelData.name || "")
                    color: Color.foreground
                    font.family: Style.font.family
                    font.pixelSize: Style.font.caption
                    elide: Text.ElideRight
                  }
                  Note {
                    width: parent.width
                    text: String(modelData.linux_key || "")
                    font.family: "monospace"
                  }
                }

                Ui.TextField {
                  id: keyCommandField
                  width: parent.width - keySave.width - Style.space(6)
                  text: String(modelData.command || "")
                  placeholderText: "Command for this key"
                  selectByMouse: true
                  Accessible.name: "Command for " + String(modelData.name || "")
                  onEditingFinished: root.saveKey(String(modelData.name || ""), text)
                  onActiveFocusChanged: root.textEditing = activeFocus
                }
                Button {
                  id: keySave
                  text: "Save"
                  fontSize: Style.font.caption
                  horizontalPadding: Style.space(2)
                  bordered: true
                  onClicked: root.saveKey(String(modelData.name || ""), keyCommandField.text)
                }
              }
            }

            Button {
              text: "Write Hyprland fragment"
              fontSize: Style.font.caption
              horizontalPadding: Style.space(2)
              bordered: true
              onClicked: root.exportKeys()
            }
            Note { width: parent.width; text: root.keyError }
            Note { width: parent.width; text: root.keyMessage }
          }

          Column {
            visible: root.page === "blackbox"
            width: parent.width
            spacing: Style.space(8)
            PanelSectionHeader { text: "HID BLACKBOX"; foreground: Color.foreground }
            Note { width: parent.width; text: "Recent vendor HID commands only. Firmware responses are acknowledgements; they do not report lighting state. Typed keys are never recorded." }
            Button { text: "Refresh history"; onClicked: root.refreshBlackbox() }
            Note { width: parent.width; text: root.blackboxError || (root.blackboxEntries.length === 0 ? "No recorded HID commands yet." : "Newest first · up to 40 entries") }
            Repeater {
              model: root.blackboxEntries
              delegate: Column {
                required property var modelData
                width: parent.width
                spacing: Style.space(3)
                Note { width: parent.width; text: String(modelData.utc || "") + " · " + String(modelData.origin || "unknown") }
                Note { width: parent.width; text: "Report: " + String(modelData.report || "No report sent"); font.family: "monospace"; wrapMode: Text.WrapAnywhere }
                Note {
                  width: parent.width
                  text: String(modelData.result || "Unknown outcome")
                    + " · syscall " + (modelData.syscall ? String(modelData.syscall.return) : "—")
                    + (modelData.syscall && modelData.syscall.error ? " · " + modelData.syscall.error : "")
                }
                PanelSeparator { width: parent.width; foreground: Color.foreground }
              }
            }
          }

          Column {
            visible: root.page === "setup"
            width: parent.width
            spacing: Style.space(10)
            PanelSectionHeader { text: "SETUP"; foreground: Color.foreground }
            Note { width: parent.width; text: "1. Install the controller from source into ~/.local. Cargo and Rust are required. The source is https://github.com/OnurByte/OmaLecoo." }
            Note { width: parent.width; visible: !root.cargoAvailable; text: "Cargo was not found. Install the prerequisite: omarchy pkg add rust" }
            Button { text: "Refresh Cargo availability"; enabled: !root.setupBusy; onClicked: root.refreshCargo() }
            Button {
              text: "Install controller from source"
              enabled: root.cargoAvailable && !root.setupBusy && !root.busy && !(root.signalService && root.signalService.busy)
              onClicked: root.installController()
            }
            Note { width: parent.width; text: "2. Authorize device access. A polkit password prompt installs only /etc/udev/rules.d/70-lecoo-gk301.rules for 1a2c:9df4 interface 01 and reloads udev. Reconnect the keyboard if access is still denied." }
            Button {
              text: "Authorize device access…"
              enabled: !root.setupBusy && !root.busy && !(root.signalService && root.signalService.busy)
              onClicked: root.authorizeDeviceAccess()
            }
            Note { width: parent.width; text: "Controller path: " + root.binary; wrapMode: Text.WrapAnywhere }
            Note { width: parent.width; visible: root.binary !== root.localBinary; text: "A custom controller path is configured. Source installation places a separate controller in ~/.local; update the widget’s binary setting to use it." }
          }

          Column {
            visible: root.page === "uninstall"
            width: parent.width
            spacing: Style.space(10)
            PanelSectionHeader { text: "REMOVE OMALECOO"; foreground: Color.foreground }
            Note { width: parent.width; text: "Choose what to remove. State and device access are preserved unless selected. Cleanup finishes before the plugin is unloaded." }
            OptionRow { width: parent.width; label: "Remove Omarchy plugin checkout"; checked: root.removePlugin; enabled: !root.setupBusy; onToggled: { root.removePlugin = !checked; root.removalConfirmed = false } }
            OptionRow { width: parent.width; label: "Cargo uninstall controller under ~/.local"; checked: root.removeController; enabled: !root.setupBusy && root.binary === root.localBinary && root.cargoAvailable; onToggled: { root.removeController = !checked; root.removalConfirmed = false } }
            Note { width: parent.width; visible: root.binary !== root.localBinary; text: "The configured binary is outside the managed ~/.local path. It will be left in place; remove it using its original installation method." }
            OptionRow { width: parent.width; label: "Remove GK301 udev rule (polkit authorization)"; checked: root.removeUdev; enabled: !root.setupBusy; onToggled: { root.removeUdev = !checked; root.removalConfirmed = false } }
            OptionRow { width: parent.width; label: "Delete OmaLecoo settings and diagnostic logs"; checked: root.removeState; enabled: !root.setupBusy; onToggled: { root.removeState = !checked; root.removalConfirmed = false } }
            Button {
              visible: !root.removalConfirmed
              text: "Review selected removal…"
              enabled: !root.setupBusy && (root.removePlugin || root.removeController || root.removeUdev || root.removeState)
              onClicked: root.removalConfirmed = true
            }
            Note {
              width: parent.width
              visible: root.removalConfirmed
              color: Color.urgent
              text: "Confirm removal of: " + [root.removePlugin ? "plugin checkout" : "", root.removeController ? "local controller" : "", root.removeUdev ? "udev rule" : "", root.removeState ? "settings and logs" : ""].filter(function(v) { return v !== "" }).join(", ") + "."
            }
            Button {
              visible: root.removalConfirmed
              text: "Confirm removal"
              enabled: !root.setupBusy && !root.busy && !(root.signalService && root.signalService.busy)
              onClicked: root.uninstallSelected()
            }
            Button { visible: root.removalConfirmed; text: "Cancel"; enabled: !root.setupBusy; onClicked: root.removalConfirmed = false }
          }
          Note { width: parent.width; visible: root.setupMessage !== ""; text: root.setupMessage; wrapMode: Text.WrapAnywhere }

        }
      }
    }
  }

  component Note: Text {
    textFormat: Text.PlainText
    wrapMode: Text.WordWrap
    color: Color.muted
    font.family: Style.font.family
    font.pixelSize: Style.font.caption
  }

  component OptionRow: Row {
    id: option
    property string label: ""
    property bool checked: false
    signal toggled()
    spacing: Style.space(8)
    Note { width: option.width - optionSwitch.width - option.spacing; text: option.label; anchors.verticalCenter: parent.verticalCenter }
    ToggleSwitch { id: optionSwitch; checked: option.checked; foreground: Color.foreground; accent: Color.accent; onToggled: option.toggled() }
  }

  // Compact chip with an index number and an elided label; the stock Button
  // does not elide its label.
  component EffectChip: BorderSurface {
    id: chip

    property string label: ""
    property string indexLabel: ""
    property bool selected: false
    property color foreground: Color.foreground
    property color accent: Color.accent
    property color background: Color.popups.background

    signal picked()

    height: Style.space(28)
    radius: Style.cornerRadius
    color: chipMouse.pressed
      ? Style.pressedFillFor(chip.foreground, chip.accent)
      : chipMouse.containsMouse
        ? Style.hoverFillFor(chip.foreground, chip.accent)
        : chip.selected
          ? Style.selectedFillFor(chip.foreground, chip.accent)
          : chip.background
    borderSpec: chip.selected
      ? Border.controlSpec("selected", chip.foreground, chip.accent)
      : Border.controlSpec("normal", chip.foreground, chip.accent)

    Row {
      id: chipRow
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.leftMargin: Style.space(7)
      anchors.rightMargin: Style.space(7)
      anchors.verticalCenter: parent.verticalCenter
      spacing: Style.space(5)

      Text {
        id: chipIndex
        width: Style.space(15)
        text: chip.indexLabel
        textFormat: Text.PlainText
        color: chip.selected ? chip.accent : Color.muted
        font.family: Style.font.family
        font.pixelSize: Style.font.caption
        anchors.verticalCenter: parent.verticalCenter
      }

      Text {
        id: chipLabel
        width: Math.max(0, chipRow.width - chipIndex.width - chipRow.spacing)
        text: chip.label
        textFormat: Text.PlainText
        elide: Text.ElideRight
        color: chip.selected ? Style.selectedStateColor(chip.foreground, chip.accent) : chip.foreground
        font.family: Style.font.family
        font.pixelSize: Style.font.caption
        font.bold: chip.selected
        anchors.verticalCenter: parent.verticalCenter
      }
    }

    MouseArea {
      id: chipMouse
      anchors.fill: parent
      hoverEnabled: true
      cursorShape: Qt.PointingHandCursor
      onClicked: chip.picked()
    }
  }
}

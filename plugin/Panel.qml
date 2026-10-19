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
  readonly property var barIdentity: hostWidget || root

  readonly property string homePath: Quickshell.env("HOME")
  readonly property string binary: setting("binary", "") || (homePath + "/.local/bin/lecoo-gk301")

  // ---- last state reported by the Rust cache ----
  property int effect: 0
  property int brightness: 5
  property int speed: 2
  property int select: 0
  property string panelName: ""
  property string firmware: ""
  property string lastError: ""
  property string connectionError: ""
  property string debugText: ""

  property var panelNames: []
  property var profiles: []

  // Extras the Windows app does not have.
  property bool autoCycle: false
  property int cycleSeconds: 60

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
  function refreshProfiles() { if (!profilesProc.running) profilesProc.running = true }
  function refreshInfo() {
    if (!root.busy && root.queue.length === 0 && !infoProc.running) {
      infoProc.err = ""
      infoProc.running = true
    }
  }

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
    var labels = [
      "Steady", "Gaming keys", "Breathing", "Key fade", "Neon stream",
      "Streamer", "Color flow", "Dripping ripples", "Sparkle", "Flash",
      "Fading trail", "Shining ripples", "Radiance", "Marquee", "Rotating storm",
      "Snake chase", "Twinkling stars", "Retro snake", "Diagonal shift", "Sine wave"
    ]
    if (i < labels.length) return labels[i]
    if (i < root.panelNames.length) return String(root.panelNames[i]).replace(/_/g, " ")
    return "Effect " + (i + 1)
  }



  function tooltipText() {
    var name = root.effectLabel(root.effect)
    return "OmaLecoo · " + name + " · brightness " + root.brightness + "/13"
      + (root.firmware !== "" ? " · fw " + root.firmware : "")
      + (root.autoCycle ? " · auto" : "")
      + (root.connectionError !== "" ? " · disconnected" : "")
  }

  // ---- writing state ----

  function enqueue(args) {
    if (root.stateSyncPending) return
    // GK301's confirmed path is the keyboard panel, not the shared Mouse page.
    if (["profile-load", "preset", "restore"].indexOf(args[0]) !== -1)
      args = args.concat(["--panel-only"])
    if (["profile-load", "preset", "reset", "restore"].indexOf(args[0]) !== -1)
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
    var args = root.queue[0]
    root.queue = root.queue.slice(1)
    worker.err = ""
    console.log("Lecoo command:", JSON.stringify(args))
    worker.command = [root.binary].concat(args)
    root.busy = true
    worker.running = true
  }

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
    root.refreshProfiles()
    root.refresh()
    root.refreshInfo()
  }

  function open() {
    root.refreshProfiles()
    root.refresh()
    root.refreshInfo()
    if (scroller.contentItem) scroller.contentItem.contentY = 0
    root.controller.show()
  }

  Timer {
    id: cycleTimer
    interval: Math.max(5, root.cycleSeconds) * 1000
    repeat: true
    running: root.autoCycle
    onTriggered: root.nextEffect()
  }


  // The keyboard can also be driven from the CLI, so the panel (and the bar
  // tooltip) cannot assume it is the only writer. Slow poll while closed,
  // quick one while the panel is open.
  Timer {
    interval: root.opened ? 2500 : 15000
    running: true
    repeat: true
    triggeredOnStart: true
    onTriggered: root.refresh()
  }

  Timer {
    interval: 15000
    running: true
    repeat: true
    onTriggered: root.refreshInfo()
  }

  // ---- the CLI ----

  Process {
    id: worker
    property string err: ""
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
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
        if (code !== 0) {
          root.queue = []
          root.autoCycle = false
          root.lastError = worker.err !== "" ? worker.err : ("Command failed (" + code + ")")
        } else {
          root.lastError = ""
          if (root.queue.length === 0) root.refresh()
        }
        root.busy = false
        root.runNext()
        if (!root.busy) { root.refresh(); root.refreshProfiles() }
      })
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
    id: infoProc
    property string err: ""
    stderr: StdioCollector { waitForEnd: true; onStreamFinished: infoProc.err = String(text || "").trim() }
    onExited: function(code) {
      Qt.callLater(function() {
        if (code !== 0) {
          root.firmware = ""
          root.connectionError = infoProc.err || "Could not connect to the keyboard"
        } else {
          root.connectionError = root.firmware !== "" ? "" : "Could not read keyboard firmware"
        }
      })
    }
    command: [root.binary, "info", "--json"]
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        root.firmware = ""
        try {
          var data = JSON.parse(String(text || "{}"))
          root.firmware = String(data.firmware || "")
        } catch (e) {}
      }
    }
  }

  Process {
    id: profilesProc
    command: [root.binary, "profiles", "--json"]
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        try {
          var names = JSON.parse(String(text || "[]"))
          if (Array.isArray(names)) root.profiles = names
        } catch (e) { root.lastError = "Could not read the profile list" }
      }
    }
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
      blocked: studioCard.editing || effectSearch.activeFocus
      onTabRequested: function(direction) {
        var item = keyCatcher.nextItemInFocusChain(direction > 0)
        if (item) item.forceActiveFocus(Qt.TabFocusReason)
      }
      onCloseRequested: root.close()
      onMoveRequested: function(dx, dy) {
        // Arrows scroll the panel instead of picking an effect: the grid is
        // long, and a cursor that walks twenty effects would need two dozen
        // presses to reach the bottom. The flickable is the ScrollView's
        // content item, not the ScrollView itself.
        if (dy !== 0) scroller.contentItem.contentY += dy * Style.space(48)
      }
      onTextKey: function(t) {
        if (studioCard.editing || effectSearch.activeFocus) return
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
                text: "FW " + (root.firmware !== "" ? root.firmware : "—")
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

          PanelSeparator { foreground: Color.foreground }

          ProfileControls {
            id: studioCard
            width: parent.width
            controller: root
          }

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
          Row {
            visible: root.effect >= 2
            width: parent.width
            spacing: Style.space(8)

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

          // ---------- game group (M1 / M2) ----------
          Row {
            visible: root.effect === 1
            width: parent.width
            spacing: Style.space(8)

            Text {
              width: Style.space(88)
              height: Style.space(28)
              text: "Game keys"
              color: Color.foreground
              font.family: Style.font.family
              font.pixelSize: Style.font.caption
              verticalAlignment: Text.AlignVCenter
            }

            ButtonGroup {
              options: [
                { value: "0", label: "M1" },
                { value: "1", label: "M2" }
              ]
              value: String(root.select)
              foreground: Color.foreground
              background: Color.popups.background
              accent: Color.accent
              onChanged: function(v) { root.setSelect(Number(v)) }
            }
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
      }
    }
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

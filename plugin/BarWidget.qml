import QtQuick
import Quickshell
import Quickshell.Io
import qs.Commons
import qs.Ui

// Bar entry point for the Lecoo GK301 RGB panel.
//
// The glyph in the bar is just a door: left click opens the control panel,
// right click steps to the next panel effect and the wheel walks the panel
// brightness, so the two most common changes stay one gesture away without
// opening anything.
BarWidget {
  id: root
  moduleName: "borat.lecoo-rgb"

  readonly property var panel: panelLoader.item

  // ---- popup lifecycle, in the shape the bar's popout coordinator expects
  //      (open / close / opened on the widget root).
  readonly property bool opened: panel ? panel.opened === true : false
  readonly property bool popoutSwitchClosing: panel ? panel.popoutSwitchClosing === true : false

  function open() { if (panel) panel.open() }
  function close() { if (panel) panel.close() }
  function toggle() { if (panel) panel.toggle() }
  function closeForPopoutSwitch() { if (panel) panel.closeForPopoutSwitch() }

  function injectPanel() {
    var target = panelLoader.item
    if (!target) return
    if ("bar" in target) target.bar = root.bar
    if ("settings" in target) target.settings = root.settings
    if ("anchorItem" in target) target.anchorItem = button
    if ("hostWidget" in target) target.hostWidget = root
  }

  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight

  onBarChanged: injectPanel()
  onSettingsChanged: injectPanel()

  // The panel owns the device logic; it stays loaded (but invisible) so the
  // bar's tooltip always has fresh state to show.
  Loader {
    id: panelLoader
    active: true
    source: Qt.resolvedUrl("Panel.qml")
    visible: false
    onLoaded: {
      root.injectPanel()
      Qt.callLater(root.injectPanel)
    }
  }

  IpcHandler {
    target: "borat.lecoo-rgb"

    function open(): void { root.open() }
    function close(): void { root.close() }
    function show(): void { root.open() }
    function hide(): void { root.close() }
    function toggle(): void { root.toggle() }
  }

  BarIconButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    text: "\uf11c"
    active: root.panel ? (root.panel.firmware !== "" && root.panel.lit) : false
    activeColor: Color.accent
    tooltipText: root.panel ? root.panel.tooltipText() : "Lecoo GK301"
    onPressed: function(b) {
      if (b === Qt.RightButton) {
        if (root.panel) root.panel.nextEffect()
      } else {
        root.toggle()
      }
    }
    onWheelMoved: function(delta) {
      if (delta === 0) return
      if (root.panel) root.panel.stepBrightness(delta > 0 ? 1 : -1)
    }
  }
}

import QtQuick
import qs.Commons
import qs.Ui

BorderSurface {
  id: studio
  required property var controller
  readonly property bool editing: profileName.activeFocus || profileChoice.activeFocus || profileChoice.popupOpen
  readonly property color accent: Color.accent
  color: Color.popups.background
  borderSpec: Border.controlSpec("normal", Color.foreground, Color.accent)
  radius: Style.cornerRadius
  implicitHeight: content.implicitHeight + Style.space(28)

  Column {
    id: content
    anchors.left: parent.left
    anchors.right: parent.right
    anchors.top: parent.top
    anchors.margins: Style.space(14)
    spacing: Style.space(10)

    PanelSectionHeader { text: "LIGHTING PROFILES"; foreground: Color.foreground }
    Row {
      width: parent.width
      spacing: Style.space(6)
      TextField {
        id: profileName
        width: parent.width - saveButton.width - parent.spacing
        placeholderText: "Profile name (e.g. Evening)"
        maximumLength: 48
        validator: RegularExpressionValidator { regularExpression: /[A-Za-z0-9_-]{1,48}/ }
        selectByMouse: true
        Accessible.name: "Name for the saved profile"
        onAccepted: if (acceptableInput) studio.controller.enqueue(["profile-save", text.trim()])
      }
      Button {
        id: saveButton
        text: "Save"
        enabled: profileName.acceptableInput && !studio.controller.busy && !studio.controller.stateSyncPending
        bordered: true
        onClicked: studio.controller.enqueue(["profile-save", profileName.text.trim()])
      }
    }
    Row {
      width: parent.width
      spacing: Style.space(6)
      Dropdown {
        id: profileChoice
        width: Math.max(80, parent.width - loadButton.width - deleteButton.width - parent.spacing * 2)
        options: studio.controller.profiles.length > 0 ? studio.controller.profiles : [{ value: "", label: "No saved profiles" }]
        value: ""
        Accessible.name: "Saved lighting profiles"
        onChanged: function(v) { value = v }
        onOptionsChanged: {
          if (studio.controller.profiles.indexOf(value) < 0)
            value = studio.controller.profiles.length > 0 ? String(studio.controller.profiles[0]) : ""
        }
      }
      Button {
        id: loadButton
        text: "Apply"
        enabled: profileChoice.value !== "" && !studio.controller.busy && !studio.controller.stateSyncPending
        bordered: true
        onClicked: studio.controller.enqueue(["profile-load", profileChoice.value])
      }
      Button {
        id: deleteButton
        text: "Delete"
        enabled: profileChoice.value !== "" && !studio.controller.busy && !studio.controller.stateSyncPending
        bordered: true
        onClicked: studio.controller.enqueue(["profile-delete", profileChoice.value])
      }
    }
  }
}

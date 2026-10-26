import QtQuick
import QtQuick.Controls
import Ductus

AbstractButton {
    id: control

    property color swatch
    property bool selected: false

    implicitWidth: 28
    implicitHeight: 28
    focusPolicy: Qt.NoFocus
    opacity: enabled ? 1 : 0.35

    // Hidden once pressed, until the pointer leaves.
    property bool tipDismissed: false
    onPressed: tipDismissed = true
    onHoveredChanged: if (!hovered) tipDismissed = false
    ToolTip.visible: hovered && !tipDismissed
    ToolTip.delay: 600
    ToolTip.text: swatch.toString()

    contentItem: Item {}
    background: Item {
        Rectangle {
            anchors.centerIn: parent
            width: 26
            height: 26
            radius: 13
            color: "transparent"
            border.width: 2
            border.color: control.selected ? Theme.accent : (control.hovered ? Theme.separator : "transparent")
        }
        Rectangle {
            anchors.centerIn: parent
            width: 18
            height: 18
            radius: 9
            color: control.swatch
            // A light rim keeps dark colors visible on the dark theme.
            border.width: 1
            border.color: Theme.dark ? Qt.rgba(1, 1, 1, 0.45) : Qt.darker(control.swatch, 1.3)
        }
    }
}

import QtQuick
import QtQuick.Controls
import Ductus

ToolButton {
    id: control

    property string iconName
    property string tip
    property bool onTitleBar: false

    implicitWidth: Math.max(Theme.buttonSize, implicitContentWidth + leftPadding + rightPadding)
    implicitHeight: Theme.buttonSize
    padding: 6
    focusPolicy: Qt.NoFocus
    display: text.length > 0 ? AbstractButton.TextBesideIcon : AbstractButton.IconOnly

    icon.source: iconName.length > 0 ? Qt.resolvedUrl("icons/" + iconName + ".svg") : ""
    icon.width: 20
    icon.height: 20
    icon.color: !enabled ? (onTitleBar ? Theme.titleTextMuted : Theme.textDisabled)
                         : (onTitleBar ? Theme.titleText : Theme.text)

    font.pixelSize: 13
    palette.buttonText: icon.color

    // Once pressed it stays hidden until the pointer leaves, so it doesn't cover a menu it opened.
    property bool tipDismissed: false
    onPressed: tipDismissed = true
    onHoveredChanged: if (!hovered) tipDismissed = false
    // Screen readers name icon-only buttons by their tip.
    Accessible.name: text.length > 0 ? text : tip
    ToolTip.visible: hovered && tip.length > 0 && !tipDismissed
    ToolTip.delay: 600
    ToolTip.text: tip

    background: Rectangle {
        radius: Theme.radius
        color: {
            if (control.onTitleBar)
                return control.down || control.hovered ? Theme.titleHover : "transparent"
            if (control.checked)
                return control.enabled ? Theme.checked : Theme.hover
            return control.down || control.hovered ? Theme.hover : "transparent"
        }
        border.width: control.checked && !control.onTitleBar ? 1 : 0
        border.color: control.enabled ? Theme.accent : Theme.separator
    }
}

import QtQuick
import QtQuick.Controls
import Ductus

Menu {
    id: menu

    // The press that closes a menu shouldn't also draw or place something on the page.
    modal: true
    dim: false
    topPadding: 6
    bottomPadding: 6

    delegate: AppMenuItem {}

    background: Rectangle {
        implicitWidth: 240
        color: Theme.popup
        radius: 8
        border.width: 1
        border.color: Theme.ribbonBorder
    }
}

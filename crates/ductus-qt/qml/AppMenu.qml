import QtQuick
import QtQuick.Controls
import Ductus

Menu {
    id: menu

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

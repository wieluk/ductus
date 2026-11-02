import QtQuick
import QtQuick.Controls
import Ductus

MenuSeparator {
    height: visible ? implicitHeight : 0
    topPadding: 4
    bottomPadding: 4
    contentItem: Rectangle {
        implicitHeight: 1
        color: Theme.separator
    }
}

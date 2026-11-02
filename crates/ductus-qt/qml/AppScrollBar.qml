import QtQuick
import QtQuick.Controls
import Ductus

// Wide enough to hit with a pen, and always faintly there while the content scrolls, since a pen
// can't find a bar that only shows on hover.
ScrollBar {
    id: bar

    readonly property real thickness: bar.hovered || bar.pressed ? 8 : 5
    // The strip keeps its width; only the handle in it grows.
    implicitWidth: vertical ? 14 : 0
    implicitHeight: vertical ? 0 : 14
    padding: 3
    minimumSize: 0.1
    visible: policy === ScrollBar.AlwaysOn || (policy === ScrollBar.AsNeeded && size < 1)

    contentItem: Item {
        Rectangle {
            anchors.centerIn: parent
            width: bar.vertical ? bar.thickness : parent.width
            height: bar.vertical ? parent.height : bar.thickness
            radius: bar.thickness / 2
            color: Theme.text
            opacity: bar.pressed ? 0.55 : bar.active ? 0.4 : 0.2
            Behavior on opacity { NumberAnimation { duration: 150 } }
        }
    }
    background: Item {}
}

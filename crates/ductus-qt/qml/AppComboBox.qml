import QtQuick
import QtQuick.Controls
import Ductus

// The Basic style draws the chosen row white on white with our palette: rows look like menu items.
ComboBox {
    id: box

    delegate: ItemDelegate {
        required property var modelData
        required property int index
        width: ListView.view ? ListView.view.width : implicitWidth
        implicitHeight: 34
        leftPadding: 14
        rightPadding: 14
        highlighted: box.highlightedIndex === index
        contentItem: Label {
            text: box.textRole ? modelData[box.textRole] : modelData
            font.pixelSize: 13
            font.bold: box.currentIndex === index
            color: Theme.text
            verticalAlignment: Text.AlignVCenter
            elide: Text.ElideRight
        }
        background: Rectangle {
            anchors.fill: parent
            anchors.leftMargin: 6
            anchors.rightMargin: 6
            radius: 4
            color: highlighted ? Theme.hover : "transparent"
        }
    }
}

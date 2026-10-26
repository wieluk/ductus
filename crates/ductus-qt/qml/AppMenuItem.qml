import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Ductus

MenuItem {
    id: item

    implicitHeight: 34
    leftPadding: 14
    rightPadding: 14
    font.pixelSize: 13

    // Checkable items reserve a column for their check mark.
    indicator: null
    contentItem: RowLayout {
        spacing: 8
        Label {
            visible: item.checkable
            Layout.preferredWidth: 14
            text: item.checked ? "✓" : ""
            font: item.font
            color: Theme.accent
        }
        Label {
            Layout.fillWidth: true
            text: item.text
            font: item.font
            color: item.enabled ? Theme.text : Theme.textDisabled
            verticalAlignment: Text.AlignVCenter
            elide: Text.ElideRight
        }
    }
    background: Rectangle {
        anchors.fill: parent
        anchors.leftMargin: 6
        anchors.rightMargin: 6
        radius: 4
        color: item.highlighted && item.enabled ? Theme.hover : "transparent"
    }
}

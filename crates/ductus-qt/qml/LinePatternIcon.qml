import QtQuick
import Ductus

Item {
    id: icon

    property string pattern: "solid"
    property color color: Theme.text

    implicitWidth: 24
    implicitHeight: 24

    Row {
        anchors.centerIn: parent
        spacing: icon.pattern === "dotted" ? 3 : 3
        visible: icon.pattern !== "solid"
        Repeater {
            model: icon.pattern === "dotted" ? 5 : 3
            delegate: Rectangle {
                width: icon.pattern === "dotted" ? 3 : 6
                height: 3
                radius: 1.5
                color: icon.color
            }
        }
    }
    Rectangle {
        visible: icon.pattern === "solid"
        anchors.centerIn: parent
        width: 22
        height: 3
        radius: 1.5
        color: icon.color
    }
}

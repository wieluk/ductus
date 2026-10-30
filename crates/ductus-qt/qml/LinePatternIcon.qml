import QtQuick
import Ductus

Item {
    id: icon

    property string pattern: "solid"
    property color color: Theme.text

    // A light rim keeps dark pen colors visible on the dark theme.
    readonly property color outline: Theme.dark ? Qt.rgba(1, 1, 1, 0.5) : "transparent"

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
                border.width: Theme.dark ? 0.75 : 0
                border.color: icon.outline
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
        border.width: Theme.dark ? 0.75 : 0
        border.color: icon.outline
    }
}

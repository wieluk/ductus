import QtQuick
import QtQuick.Controls
import Ductus

Rectangle {
    id: toast

    function show(message, isError) {
        label.text = message
        error = isError
        opacity = 1
        hideTimer.restart()
    }

    property bool error: false

    objectName: "toast"
    width: Math.min(label.implicitWidth + 32, parent.width - 48)
    height: label.implicitHeight + 18
    radius: 8
    color: error ? Theme.danger : Theme.toast
    opacity: 0
    visible: opacity > 0
    Behavior on opacity { NumberAnimation { duration: 180 } }

    Label {
        id: label
        objectName: "toastLabel"
        anchors.centerIn: parent
        width: parent.width - 32
        wrapMode: Text.WordWrap
        horizontalAlignment: Text.AlignHCenter
        color: toast.error ? "#ffffff" : Theme.toastText
        font.pixelSize: 13
    }
    Timer {
        id: hideTimer
        interval: toast.error ? 6000 : 3500
        onTriggered: toast.opacity = 0
    }
}

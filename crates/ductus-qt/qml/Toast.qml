import QtQuick
import QtQuick.Controls
import Ductus

Rectangle {
    id: toast

    /// `actionText` and `action` add a button, such as Undo, that stays a little longer.
    function show(message, isError, actionText, action) {
        label.text = message
        error = isError
        button.text = actionText || ""
        toast.action = action || null
        opacity = 1
        hideTimer.restart()
    }

    property bool error: false
    property var action: null

    objectName: "toast"
    width: Math.min(row.implicitWidth + 32, parent.width - 48)
    height: row.implicitHeight + 18
    radius: 8
    color: error ? Theme.danger : Theme.toast
    opacity: 0
    visible: opacity > 0
    Behavior on opacity { NumberAnimation { duration: 180 } }

    Row {
        id: row
        anchors.centerIn: parent
        spacing: 14
        Label {
            id: label
            objectName: "toastLabel"
            anchors.verticalCenter: parent.verticalCenter
            width: Math.min(implicitWidth, toast.parent.width - 80 - (button.visible ? button.width + row.spacing : 0))
            wrapMode: Text.WordWrap
            horizontalAlignment: Text.AlignHCenter
            color: toast.error ? "#ffffff" : Theme.toastText
            font.pixelSize: 13
        }
        AbstractButton {
            id: button
            objectName: "toastAction"
            anchors.verticalCenter: parent.verticalCenter
            visible: text.length > 0
            implicitWidth: actionLabel.implicitWidth + 16
            implicitHeight: actionLabel.implicitHeight + 8
            focusPolicy: Qt.NoFocus
            onClicked: {
                const action = toast.action
                toast.action = null
                toast.opacity = 0
                if (action)
                    action()
            }
            contentItem: Label {
                id: actionLabel
                text: button.text
                horizontalAlignment: Text.AlignHCenter
                color: toast.error ? "#ffffff" : Theme.toastAction
                font.pixelSize: 13
                font.weight: Font.DemiBold
            }
            background: Rectangle {
                radius: 4
                color: button.hovered ? Qt.rgba(1, 1, 1, 0.12) : "transparent"
            }
        }
    }
    Timer {
        id: hideTimer
        interval: toast.action ? 8000 : toast.error ? 6000 : 3500
        onTriggered: toast.opacity = 0
    }
}

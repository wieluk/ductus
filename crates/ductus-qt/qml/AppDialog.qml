import QtQuick
import QtQuick.Controls
import Ductus

Dialog {
    id: dialog

    property string message
    property string acceptText: qsTr("OK")
    property string rejectText: qsTr("Cancel")
    property bool destructive: false
    property alias acceptButton: acceptButton

    anchors.centerIn: parent
    modal: true
    padding: 20
    topPadding: 8
    // Fixed width, or the window's on a phone: binding to the parent (the overlay while open)
    // loops through the implicit height.
    property real preferredWidth: 440
    width: Math.min(preferredWidth, (Overlay.overlay ? Overlay.overlay.width : preferredWidth + 24) - 24)

    Overlay.modal: Rectangle { color: Theme.dark ? "#99000000" : "#55000000" }

    background: Rectangle {
        color: Theme.popup
        radius: 12
        border.width: 1
        border.color: Theme.ribbonBorder
    }

    // The title wraps too, so it gets the same explicit width treatment.
    header: Label {
        width: dialog.width
        text: dialog.title
        visible: text.length > 0
        padding: 20
        bottomPadding: 0
        wrapMode: Text.WordWrap
        color: Theme.text
        font.pixelSize: 16
        font.weight: Font.DemiBold
    }

    // Width from the dialog, not the content item, to avoid a binding loop.
    contentItem: Item {
        implicitHeight: messageLabel.implicitHeight
        Label {
            id: messageLabel
            width: dialog.width - dialog.leftPadding - dialog.rightPadding
            text: dialog.message
            wrapMode: Text.WordWrap
            color: Theme.textMuted
            font.pixelSize: 13
        }
    }

    component DialogButton: Button {
        id: button
        property color fill: "transparent"
        property color ink: Theme.text
        implicitHeight: 36
        leftPadding: 18
        rightPadding: 18
        focusPolicy: Qt.NoFocus
        font.pixelSize: 13
        font.weight: Font.DemiBold
        contentItem: Label {
            text: button.text
            font: button.font
            color: button.ink
            horizontalAlignment: Text.AlignHCenter
            verticalAlignment: Text.AlignVCenter
        }
        background: Rectangle {
            radius: 8
            color: button.fill
            opacity: button.down ? 0.8 : 1
            border.width: button.fill === "transparent" ? 1 : 0
            border.color: Theme.separator
        }
    }

    footer: Item {
        implicitHeight: 72
        Row {
            anchors.right: parent.right
            anchors.rightMargin: 20
            anchors.verticalCenter: parent.verticalCenter
            spacing: 8
            DialogButton {
                objectName: "dialogReject"
                visible: dialog.rejectText.length > 0
                text: dialog.rejectText
                onClicked: dialog.reject()
            }
            DialogButton {
                id: acceptButton
                text: dialog.acceptText
                fill: dialog.destructive ? Theme.danger : Theme.accent
                ink: "#ffffff"
                onClicked: dialog.accept()
            }
        }
    }
}

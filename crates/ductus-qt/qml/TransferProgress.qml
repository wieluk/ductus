import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Ductus

// A running download or update: what it's doing, a bar with the percentage, and Cancel.
RowLayout {
    id: transfer

    property string label
    /// 0 to 1, or below 0 while there's nothing to count yet.
    property real value: -1
    readonly property bool counting: value >= 0

    signal cancelled()

    spacing: 12

    ColumnLayout {
        Layout.fillWidth: true
        spacing: 4
        RowLayout {
            Layout.fillWidth: true
            spacing: 8
            Label {
                Layout.fillWidth: true
                text: transfer.label
                color: Theme.textMuted
                font.pixelSize: 12
                elide: Text.ElideRight
            }
            Label {
                objectName: transfer.objectName + "Percent"
                visible: transfer.counting
                // Rounded down, so 100 % means done.
                text: qsTr("%1%").arg(Math.floor(Math.min(1, transfer.value) * 100))
                color: Theme.text
                font.pixelSize: 12
                font.weight: Font.DemiBold
                font.features: { "tnum": 1 }
            }
        }
        Rectangle {
            id: track
            Layout.fillWidth: true
            implicitHeight: 6
            radius: 3
            color: Theme.hover
            clip: true
            Rectangle {
                height: parent.height
                radius: parent.radius
                color: Theme.accent
                width: transfer.counting ? track.width * Math.min(1, transfer.value) : track.width / 3
                Behavior on width {
                    enabled: transfer.counting
                    NumberAnimation { duration: 200 }
                }
                // Nothing to count: a stretch of bar sweeps across instead.
                NumberAnimation on x {
                    running: !transfer.counting && transfer.visible
                    from: -track.width / 3
                    to: track.width
                    duration: 1200
                    loops: Animation.Infinite
                }
                Binding on x {
                    when: transfer.counting
                    value: 0
                }
            }
        }
    }
    RibbonButton {
        objectName: transfer.objectName + "Cancel"
        text: qsTr("Cancel")
        onClicked: transfer.cancelled()
    }
}

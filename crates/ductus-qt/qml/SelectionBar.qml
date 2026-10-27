import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Ductus

Rectangle {
    id: bar

    required property InkCanvas canvas
    readonly property rect frame: canvas.selectionRect
    /// Room kept free at the sides, such as for the pen bar.
    property real leftInset: 8
    property real rightInset: 8

    visible: canvas.selectionCount > 0
    x: Math.max(leftInset, Math.min(frame.x + frame.width / 2 - width / 2, parent.width - width - rightInset))
    // Below the frame it leaves room for the rotation grip.
    y: frame.y - height - 14 >= 8 ? frame.y - height - 14
                                 : Math.min(frame.y + frame.height + 44, parent.height - height - 8)
    // On a narrow screen the groups wrap onto more rows instead of running off the edge.
    readonly property real room: parent.width - leftInset - rightInset - 12
    readonly property real natural: edits.implicitWidth
        + (inks.visible ? flow.spacing + inks.implicitWidth : 0)
        + (converts.visible ? flow.spacing + converts.implicitWidth : 0)
    width: flow.width + 12
    height: flow.implicitHeight + 8
    radius: 10
    color: Theme.popup
    border.width: 1
    border.color: Theme.ribbonBorder

    Flow {
        id: flow
        anchors.centerIn: parent
        width: Math.min(bar.natural, bar.room)
        spacing: 2
        RowLayout {
            id: edits
            spacing: 2
            RibbonButton {
                objectName: "selectionCut"
                iconName: "cut"
                tip: qsTr("Cut (Ctrl+X)")
                onClicked: bar.canvas.cutSelection()
            }
            RibbonButton {
                objectName: "selectionCopy"
                iconName: "copy"
                tip: qsTr("Copy (Ctrl+C)")
                onClicked: bar.canvas.copySelection()
            }
            RibbonButton {
                objectName: "selectionDuplicate"
                iconName: "paste"
                tip: qsTr("Duplicate")
                onClicked: {
                    bar.canvas.copySelection()
                    bar.canvas.paste()
                }
            }
            RibbonButton {
                objectName: "selectionDelete"
                iconName: "delete"
                tip: qsTr("Delete (Del)")
                onClicked: bar.canvas.deleteSelection()
            }
        }
        // Colors only apply to pen ink, not to pictures or highlights.
        RowLayout {
            id: inks
            visible: bar.canvas.selectionHasInk
            spacing: 2
            Rectangle {
                // Only between groups on the same row; opacity, so the wrapping doesn't change.
                opacity: inks.x > 0 ? 1 : 0
                Layout.preferredWidth: 1
                Layout.preferredHeight: 22
                Layout.leftMargin: 4
                Layout.rightMargin: 4
                color: Theme.separator
            }
            Repeater {
                model: ["#1a1a1f", "#1f4fd1", "#d12f2f", "#1d8a3b"]
                delegate: ColorSwatch {
                    required property string modelData
                    required property int index
                    objectName: "selectionColor" + index
                    swatch: modelData
                    onClicked: bar.canvas.recolorSelection(modelData)
                }
            }
        }
        RowLayout {
            id: converts
            visible: bar.canvas.selectionHasInk
            spacing: 2
            Rectangle {
                opacity: converts.x > 0 ? 1 : 0
                Layout.preferredWidth: 1
                Layout.preferredHeight: 22
                Layout.leftMargin: 4
                Layout.rightMargin: 4
                color: Theme.separator
            }
            RibbonButton {
                objectName: "selectionToText"
                enabled: !AppController.converting
                iconName: "text"
                text: qsTr("Text")
                tip: qsTr("Replace the handwriting with typed text")
                onClicked: AppController.convertSelection("text")
            }
            RibbonButton {
                objectName: "selectionToMath"
                enabled: !AppController.converting
                iconName: "math"
                text: qsTr("Formula")
                tip: qsTr("Replace the handwriting with a formula")
                onClicked: AppController.convertSelection("math")
            }
        }
    }
}

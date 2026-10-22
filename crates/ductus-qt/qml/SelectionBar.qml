import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Ductus

Rectangle {
    id: bar

    required property InkCanvas canvas
    readonly property rect frame: canvas.selectionRect

    visible: canvas.selectionCount > 0
    x: Math.max(8, Math.min(frame.x + frame.width / 2 - width / 2, parent.width - width - 8))
    y: frame.y - height - 14 >= 8 ? frame.y - height - 14
                                 : Math.min(frame.y + frame.height + 14, parent.height - height - 8)
    width: row.implicitWidth + 12
    height: row.implicitHeight + 8
    radius: 10
    color: Theme.popup
    border.width: 1
    border.color: Theme.ribbonBorder

    RowLayout {
        id: row
        anchors.centerIn: parent
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
        // Colors only apply to pen ink, not to pictures or highlights.
        Rectangle {
            visible: bar.canvas.selectionHasInk
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
                visible: bar.canvas.selectionHasInk
                objectName: "selectionColor" + index
                swatch: modelData
                onClicked: bar.canvas.recolorSelection(modelData)
            }
        }
        Rectangle {
            visible: bar.canvas.selectionHasInk
            Layout.preferredWidth: 1
            Layout.preferredHeight: 22
            Layout.leftMargin: 4
            Layout.rightMargin: 4
            color: Theme.separator
        }
        RibbonButton {
            objectName: "selectionToText"
            visible: bar.canvas.selectionHasInk
            enabled: !AppController.converting
            iconName: "text"
            text: qsTr("Text")
            tip: qsTr("Replace the handwriting with typed text")
            onClicked: AppController.convertSelection("text")
        }
        RibbonButton {
            objectName: "selectionToMath"
            visible: bar.canvas.selectionHasInk
            enabled: !AppController.converting
            iconName: "math"
            text: qsTr("Formula")
            tip: qsTr("Replace the handwriting with a formula")
            onClicked: AppController.convertSelection("math")
        }
    }
}

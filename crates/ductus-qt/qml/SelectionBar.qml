import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
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
        + (transforms.visible ? flow.spacing + transforms.implicitWidth : 0)
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
            RibbonButton {
                objectName: "selectionMoreColors"
                iconName: "color"
                tip: qsTr("More colors…")
                onClicked: moreColors.open()
            }
        }
        RowLayout {
            id: transforms
            spacing: 2
            Rectangle {
                opacity: transforms.x > 0 ? 1 : 0
                Layout.preferredWidth: 1
                Layout.preferredHeight: 22
                Layout.leftMargin: 4
                Layout.rightMargin: 4
                color: Theme.separator
            }
            RibbonButton {
                objectName: "selectionRotateLeft"
                iconName: "rotate-left"
                tip: qsTr("Rotate left")
                onClicked: bar.canvas.rotateSelection(-1)
            }
            RibbonButton {
                objectName: "selectionRotateRight"
                iconName: "rotate-right"
                tip: qsTr("Rotate right")
                onClicked: bar.canvas.rotateSelection(1)
            }
            RibbonButton {
                objectName: "selectionFlipHorizontal"
                iconName: "flip-horizontal"
                tip: qsTr("Flip horizontally")
                onClicked: bar.canvas.flipSelection(true)
            }
            RibbonButton {
                objectName: "selectionFlipVertical"
                iconName: "flip-vertical"
                tip: qsTr("Flip vertically")
                onClicked: bar.canvas.flipSelection(false)
            }
            RibbonButton {
                objectName: "selectionAlign"
                iconName: "align"
                tip: qsTr("Align…")
                onClicked: alignMenu.popup(this, 0, height)
            }
            RibbonButton {
                objectName: "selectionLayer"
                iconName: "layers"
                tip: qsTr("Layering…")
                onClicked: layerMenu.popup(this, 0, height)
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

    ColorDialog {
        id: moreColors
        title: qsTr("Choose a color")
        onAccepted: bar.canvas.recolorSelection(selectedColor)
    }

    AppMenu {
        id: alignMenu
        AppMenuItem {
            objectName: "selectionAlignLeft"
            text: qsTr("Align left")
            onTriggered: bar.canvas.alignSelection(0)
        }
        AppMenuItem {
            objectName: "selectionAlignCenterH"
            text: qsTr("Align center")
            onTriggered: bar.canvas.alignSelection(1)
        }
        AppMenuItem {
            objectName: "selectionAlignRight"
            text: qsTr("Align right")
            onTriggered: bar.canvas.alignSelection(2)
        }
        AppMenuSeparator {}
        AppMenuItem {
            objectName: "selectionAlignTop"
            text: qsTr("Align top")
            onTriggered: bar.canvas.alignSelection(3)
        }
        AppMenuItem {
            objectName: "selectionAlignMiddle"
            text: qsTr("Align middle")
            onTriggered: bar.canvas.alignSelection(4)
        }
        AppMenuItem {
            objectName: "selectionAlignBottom"
            text: qsTr("Align bottom")
            onTriggered: bar.canvas.alignSelection(5)
        }
    }

    AppMenu {
        id: layerMenu
        AppMenuItem {
            objectName: "selectionBringToFront"
            text: qsTr("Bring to front")
            onTriggered: bar.canvas.reorderSelection(0)
        }
        AppMenuItem {
            objectName: "selectionBringForward"
            text: qsTr("Bring forward")
            onTriggered: bar.canvas.reorderSelection(1)
        }
        AppMenuItem {
            objectName: "selectionSendBackward"
            text: qsTr("Send backward")
            onTriggered: bar.canvas.reorderSelection(2)
        }
        AppMenuItem {
            objectName: "selectionSendToBack"
            text: qsTr("Send to back")
            onTriggered: bar.canvas.reorderSelection(3)
        }
    }
}

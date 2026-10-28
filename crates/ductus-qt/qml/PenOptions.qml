import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import QtQuick.Layouts
import Ductus

// Color, width and line style of a pen or highlighter; `picked` reports one change.
ColumnLayout {
    id: options

    property bool highlighter: false
    property color color
    property real strokeWidth
    property string pattern: "solid"
    /// Starts the object names of the choices, for tests.
    property string namePrefix

    signal picked(var change)

    readonly property var penColors: ["#1a1a1f", "#5f6368", "#1f4fd1", "#0a8a8a", "#1d8a3b",
                                      "#7b2fd1", "#c2185b", "#d12f2f", "#e07b00", "#8d5524"]
    readonly property var highlighterColors: ["#ffd91a", "#7ee07e", "#5cc8ff", "#ff8fd0",
                                              "#ffa94d", "#c9a6ff"]
    readonly property var penWidths: [1, 1.5, 2, 3, 4.5, 6]
    readonly property var highlighterWidths: [8, 12, 18, 24, 32]

    spacing: 10

    ColorDialog {
        id: moreColors
        title: qsTr("Choose a color")
        onAccepted: options.picked({ color: selectedColor.toString() })
    }

    Label {
        text: qsTr("Color")
        color: Theme.textMuted
        font.pixelSize: 12
    }
    GridLayout {
        // The highlighter's few colors and More colors fit one row.
        columns: options.highlighter ? 7 : 6
        rowSpacing: 2
        columnSpacing: 2
        Repeater {
            model: options.highlighter ? options.highlighterColors : options.penColors
            delegate: ColorSwatch {
                required property string modelData
                required property int index
                objectName: options.namePrefix + "Color" + index
                swatch: modelData
                selected: Qt.colorEqual(options.color, modelData)
                onClicked: options.picked({ color: modelData })
            }
        }
        RibbonButton {
            objectName: options.namePrefix + "MoreColors"
            iconName: "color"
            tip: qsTr("More colors…")
            implicitWidth: 28
            implicitHeight: 28
            padding: 3
            onClicked: {
                moreColors.selectedColor = options.color
                moreColors.open()
            }
        }
    }

    Label {
        text: qsTr("Width")
        color: Theme.textMuted
        font.pixelSize: 12
    }
    RowLayout {
        spacing: 2
        Repeater {
            model: options.highlighter ? options.highlighterWidths : options.penWidths
            delegate: RibbonButton {
                id: widthChoice
                required property real modelData
                required property int index
                objectName: options.namePrefix + "Width" + index
                implicitWidth: 36
                checkable: true
                checked: Math.abs(options.strokeWidth - modelData) < 0.01
                tip: String(modelData)
                onClicked: options.picked({ width: modelData })
                contentItem: Item {
                    Rectangle {
                        anchors.centerIn: parent
                        width: options.highlighter ? 22 : 2 + widthChoice.modelData * 1.8
                        height: options.highlighter ? widthChoice.modelData / 2.5
                                                    : 2 + widthChoice.modelData * 1.8
                        radius: options.highlighter ? 2 : width / 2
                        color: options.color
                        opacity: options.highlighter ? 0.6 : 1
                    }
                }
            }
        }
    }

    Label {
        visible: !options.highlighter
        text: qsTr("Line")
        color: Theme.textMuted
        font.pixelSize: 12
    }
    RowLayout {
        visible: !options.highlighter
        spacing: 2
        Repeater {
            model: ["solid", "dashed", "dotted"]
            delegate: RibbonButton {
                id: patternChoice
                required property string modelData
                objectName: options.namePrefix + "Line_" + modelData
                implicitWidth: 44
                checkable: true
                checked: options.pattern === modelData
                onClicked: options.picked({ pattern: modelData })
                contentItem: LinePatternIcon {
                    pattern: patternChoice.modelData
                    color: options.color
                }
            }
        }
    }
}

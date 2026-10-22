import QtQuick
import QtQuick.Controls
import QtQuick.Controls.impl
import QtQuick.Dialogs
import QtQuick.Layouts
import Ductus

Rectangle {
    id: bar

    required property InkCanvas canvas
    /// JSON: { type: pen|highlighter, color, width } or { type: tool, tool }.
    property string presetsJson: ""
    property bool onRight: false

    signal screenshotRequested()
    signal pictureRequested()

    readonly property var tools: [
        { tool: "lasso", icon: "lasso", label: qsTr("Lasso") },
        { tool: "eraser", icon: "eraser", label: qsTr("Eraser") },
        { tool: "text", icon: "text", label: qsTr("Text") },
        { tool: "math", icon: "math", label: qsTr("Math") },
        { tool: "screenshot", icon: "screenshot", label: qsTr("Screenshot") },
        { tool: "picture", icon: "image", label: qsTr("Picture…") },
        { tool: "paste", icon: "paste", label: qsTr("Paste") },
        { tool: "undo", icon: "undo", label: qsTr("Undo") },
        { tool: "redo", icon: "redo", label: qsTr("Redo") },
        { tool: "shapes", icon: "shapes", label: qsTr("Shapes on/off") }
    ]
    readonly property var defaults: [
        { type: "tool", tool: "lasso" },
        { type: "tool", tool: "eraser" },
        { type: "tool", tool: "text" },
        { type: "pen", color: "#1a1a1f", width: 1.5 },
        { type: "pen", color: "#1a1a1f", width: 3 },
        { type: "pen", color: "#1f4fd1", width: 2 },
        { type: "pen", color: "#d12f2f", width: 2 },
        { type: "pen", color: "#1d8a3b", width: 2 },
        { type: "highlighter", color: "#ffd91a", width: 18 },
        { type: "highlighter", color: "#7ee07e", width: 18 },
        { type: "highlighter", color: "#ff8fd0", width: 18 },
        { type: "tool", tool: "screenshot" }
    ]
    readonly property var penColors: ["#1a1a1f", "#5f6368", "#1f4fd1", "#0a8a8a", "#1d8a3b",
                                      "#7b2fd1", "#c2185b", "#d12f2f", "#e07b00", "#8d5524"]
    readonly property var highlighterColors: ["#ffd91a", "#7ee07e", "#5cc8ff", "#ff8fd0",
                                              "#ffa94d", "#c9a6ff"]
    readonly property var penWidths: [1, 1.5, 2, 3, 4.5, 6]
    readonly property var highlighterWidths: [8, 12, 18, 24, 32]

    readonly property var items: {
        try {
            const list = JSON.parse(presetsJson)
            if (Array.isArray(list))
                // Older settings stored only pens, with the kind in "tool".
                return list.map(i => i.type ? i : { type: i.tool, color: i.color, width: i.width })
        } catch (e) {}
        return defaults
    }

    function store(list) { presetsJson = JSON.stringify(list) }

    function toolInfo(name) {
        return tools.find(t => t.tool === name) || { tool: name, icon: "more", label: name }
    }

    function current() {
        const hl = canvas.tool === InkCanvas.Highlighter
        const item = { type: hl ? "highlighter" : "pen",
                       color: (hl ? canvas.highlighterColor : canvas.penColor).toString(),
                       width: hl ? canvas.highlighterWidth : canvas.penWidth }
        if (!hl && canvas.penPattern !== "solid")
            item.pattern = canvas.penPattern
        return item
    }

    function apply(p) {
        if (p.type === "highlighter") {
            canvas.highlighterColor = p.color
            canvas.highlighterWidth = p.width
            canvas.tool = InkCanvas.Highlighter
        } else {
            canvas.penColor = p.color
            canvas.penWidth = p.width
            canvas.penPattern = p.pattern || "solid"
            canvas.tool = InkCanvas.Pen
        }
    }

    function isActive(p) {
        const hl = p.type === "highlighter"
        if (canvas.tool !== (hl ? InkCanvas.Highlighter : InkCanvas.Pen))
            return false
        return Qt.colorEqual(hl ? canvas.highlighterColor : canvas.penColor, p.color)
            && Math.abs((hl ? canvas.highlighterWidth : canvas.penWidth) - p.width) < 0.01
            && (hl || (p.pattern || "solid") === canvas.penPattern)
    }

    function toolChecked(name) {
        switch (name) {
        case "lasso": return canvas.tool === InkCanvas.Lasso
        case "eraser": return canvas.tool === InkCanvas.Eraser
        case "text": return canvas.tool === InkCanvas.Text
        case "math": return canvas.tool === InkCanvas.Math
        case "shapes": return canvas.shapeAssist
        default: return false
        }
    }

    function toolEnabled(name) {
        switch (name) {
        case "undo": return canvas.canUndo
        case "redo": return canvas.canRedo
        case "screenshot": case "picture": case "text": case "math": return canvas.picturesAllowed
        default: return true
        }
    }

    function runTool(name) {
        switch (name) {
        case "lasso": canvas.tool = InkCanvas.Lasso; break
        case "eraser": canvas.tool = InkCanvas.Eraser; break
        case "text": canvas.tool = InkCanvas.Text; break
        case "math": canvas.tool = InkCanvas.Math; break
        case "screenshot": screenshotRequested(); break
        case "picture": pictureRequested(); break
        case "paste": canvas.paste(); break
        case "undo": canvas.undo(); break
        case "redo": canvas.redo(); break
        case "shapes": canvas.shapeAssist = !canvas.shapeAssist; break
        }
    }

    function update(index, changes) {
        const list = items.slice()
        list[index] = Object.assign({}, list[index], changes)
        store(list)
        if (list[index].type !== "tool")
            apply(list[index])
    }

    function move(index, delta) {
        const target = index + delta
        if (target < 0 || target >= items.length)
            return
        const list = items.slice()
        const [item] = list.splice(index, 1)
        list.splice(target, 0, item)
        store(list)
        if (editor.index === index)
            editor.index = target
    }

    function remove(index) {
        const list = items.slice()
        list.splice(index, 1)
        store(list)
    }

    function add(item) {
        store(items.concat([item]))
    }

    function openEditor(index, anchor) {
        editor.index = index
        const p = anchor.mapToItem(bar, 0, 0)
        editor.x = bar.onRight ? -editor.width - 8 : bar.width + 8
        editor.y = Math.max(0, p.y - 40)
        editor.open()
    }

    Connections {
        target: bar.canvas
        function onEdited() { editor.close() }
    }

    width: 48
    implicitHeight: column.implicitHeight + 12
    radius: 10
    color: Theme.popup
    border.width: 1
    border.color: Theme.ribbonBorder

    ColumnLayout {
        id: column
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.top: parent.top
        anchors.topMargin: 6
        spacing: 2

        Repeater {
            model: bar.items
            delegate: AbstractButton {
                id: item
                required property var modelData
                required property int index
                readonly property bool isTool: modelData.type === "tool"
                readonly property bool highlighter: modelData.type === "highlighter"
                readonly property var info: isTool ? bar.toolInfo(modelData.tool) : null
                readonly property bool active: isTool ? bar.toolChecked(modelData.tool)
                                                      : bar.isActive(modelData)
                objectName: "preset" + index
                Layout.preferredWidth: Theme.buttonSize
                Layout.preferredHeight: Theme.buttonSize
                focusPolicy: Qt.NoFocus
                enabled: !isTool || bar.toolEnabled(modelData.tool)
                onClicked: {
                    if (isTool && modelData.tool === "eraser" && active)
                        toolMenu.show(index, item)   // Tapping the active eraser sets it up.
                    else if (isTool)
                        bar.runTool(modelData.tool)
                    else if (active)
                        bar.openEditor(index, item)   // Tapping the active pen edits it.
                    else
                        bar.apply(modelData)
                }
                ToolTip.visible: hovered
                ToolTip.delay: 600
                ToolTip.text: isTool ? info.label
                                     : (highlighter ? qsTr("Highlighter") : qsTr("Pen")) + " · "
                                       + modelData.width + " — " + qsTr("tap again to change")

                TapHandler {
                    acceptedButtons: Qt.RightButton
                    onTapped: item.isTool ? toolMenu.show(item.index, item)
                                          : bar.openEditor(item.index, item)
                }
                TapHandler {
                    acceptedDevices: PointerDevice.TouchScreen | PointerDevice.Stylus
                    onLongPressed: item.isTool ? toolMenu.show(item.index, item)
                                               : bar.openEditor(item.index, item)
                }

                background: Rectangle {
                    radius: Theme.radius
                    color: item.active ? Theme.checked : (item.hovered ? Theme.hover : "transparent")
                    border.width: item.active ? 1 : 0
                    border.color: Theme.accent
                }
                contentItem: Item {
                    LinePatternIcon {
                        visible: !item.isTool && !item.highlighter
                                 && (item.modelData.pattern || "solid") !== "solid"
                        anchors.centerIn: parent
                        rotation: -35
                        pattern: item.modelData.pattern || "solid"
                        color: item.isTool ? "transparent" : item.modelData.color
                    }
                    Rectangle {
                        visible: !item.isTool && (item.highlighter
                                 || (item.modelData.pattern || "solid") === "solid")
                        anchors.centerIn: parent
                        width: 22
                        height: item.highlighter ? Math.min(12, item.modelData.width / 2)
                                                 : Math.max(2, Math.min(8, item.modelData.width * 1.3))
                        radius: item.highlighter ? 2 : height / 2
                        rotation: -35
                        color: item.isTool ? "transparent" : item.modelData.color
                        opacity: item.highlighter ? 0.6 : 1
                        border.width: Theme.dark && !item.highlighter ? 1 : 0
                        border.color: Qt.rgba(1, 1, 1, 0.4)
                    }
                    IconImage {
                        visible: item.isTool
                        anchors.centerIn: parent
                        width: 20
                        height: 20
                        sourceSize: Qt.size(20, 20)
                        source: item.isTool ? Qt.resolvedUrl("icons/" + item.info.icon + ".svg") : ""
                        color: item.enabled ? Theme.text : Theme.textDisabled
                    }
                }
            }
        }

        RibbonButton {
            objectName: "presetAdd"
            iconName: "plus"
            tip: qsTr("Add a pen or a tool")
            onClicked: addMenu.popup(this, bar.onRight ? -addMenu.width : width, 0)
        }
    }

    AppMenu {
        id: addMenu
        objectName: "penBarAddMenu"
        AppMenuItem {
            objectName: "addCurrentPen"
            text: qsTr("Add the current pen")
            enabled: bar.canvas.tool === InkCanvas.Pen || bar.canvas.tool === InkCanvas.Highlighter
            onTriggered: bar.add(bar.current())
        }
        AppMenuItem {
            objectName: "addPen"
            text: qsTr("New pen")
            onTriggered: {
                bar.add({ type: "pen", color: "#1f4fd1", width: 2 })
                bar.apply(bar.items[bar.items.length - 1])
            }
        }
        AppMenuItem {
            objectName: "addHighlighter"
            text: qsTr("New highlighter")
            onTriggered: {
                bar.add({ type: "highlighter", color: "#ffd91a", width: 18 })
                bar.apply(bar.items[bar.items.length - 1])
            }
        }
        AppMenuSeparator {}
        Repeater {
            model: bar.tools
            delegate: AppMenuItem {
                required property var modelData
                objectName: "addTool_" + modelData.tool
                text: qsTr("Add %1").arg(modelData.label)
                // Each tool once.
                visible: !bar.items.some(i => i.type === "tool" && i.tool === modelData.tool)
                height: visible ? implicitHeight : 0
                onTriggered: bar.add({ type: "tool", tool: modelData.tool })
            }
        }
        AppMenuSeparator {}
        AppMenuItem {
            objectName: "presetReset"
            text: qsTr("Restore the default bar")
            onTriggered: bar.presetsJson = ""
        }
    }

    AppMenu {
        id: toolMenu
        property int index: -1
        readonly property bool eraser: index >= 0 && index < bar.items.length
                                       && bar.items[index].tool === "eraser"
        function show(index, item) {
            toolMenu.index = index
            popup(item, bar.onRight ? -width : item.width, 0)
        }
        // The eraser's own settings come first.
        AppMenuItem {
            objectName: "barEraserPartial"
            visible: toolMenu.eraser
            height: visible ? implicitHeight : 0
            text: qsTr("Erase only what you touch")
            checkable: true
            checked: !bar.canvas.eraserWholeStrokes
            onTriggered: {
                bar.canvas.eraserWholeStrokes = false
                bar.canvas.tool = InkCanvas.Eraser
            }
        }
        AppMenuItem {
            objectName: "barEraserWhole"
            visible: toolMenu.eraser
            height: visible ? implicitHeight : 0
            text: qsTr("Erase whole strokes")
            checkable: true
            checked: bar.canvas.eraserWholeStrokes
            onTriggered: {
                bar.canvas.eraserWholeStrokes = true
                bar.canvas.tool = InkCanvas.Eraser
            }
        }
        Repeater {
            model: [{ size: 4, label: qsTr("Small eraser") }, { size: 8, label: qsTr("Medium eraser") },
                    { size: 16, label: qsTr("Large eraser") }, { size: 30, label: qsTr("Extra large eraser") }]
            delegate: AppMenuItem {
                required property var modelData
                objectName: "barEraserSize" + modelData.size
                visible: toolMenu.eraser
                height: visible ? implicitHeight : 0
                text: modelData.label
                checkable: true
                checked: bar.canvas.eraserSize === modelData.size
                onTriggered: {
                    bar.canvas.eraserSize = modelData.size
                    bar.canvas.tool = InkCanvas.Eraser
                }
            }
        }
        AppMenuSeparator {
            visible: toolMenu.eraser
            height: visible ? implicitHeight : 0
        }
        AppMenuItem {
            objectName: "toolMoveUp"
            text: qsTr("Move up")
            onTriggered: bar.move(toolMenu.index, -1)
        }
        AppMenuItem {
            objectName: "toolMoveDown"
            text: qsTr("Move down")
            onTriggered: bar.move(toolMenu.index, 1)
        }
        AppMenuItem {
            objectName: "toolRemove"
            text: qsTr("Remove from the bar")
            onTriggered: bar.remove(toolMenu.index)
        }
    }

    ColorDialog {
        id: moreColors
        title: qsTr("Choose a color")
        onAccepted: bar.update(editor.index, { color: selectedColor.toString() })
    }

    Popup {
        id: editor
        objectName: "penEditor"
        property int index: -1
        readonly property var pen: index >= 0 && index < bar.items.length ? bar.items[index] : null
        readonly property bool highlighter: pen !== null && pen.type === "highlighter"
        width: 264
        padding: 12
        modal: false
        closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
        background: Rectangle {
            radius: 10
            color: Theme.popup
            border.width: 1
            border.color: Theme.ribbonBorder
        }

        contentItem: ColumnLayout {
            spacing: 10

            RowLayout {
                spacing: 2
                Repeater {
                    model: [{ type: "pen", label: qsTr("Pen") },
                            { type: "highlighter", label: qsTr("Highlighter") }]
                    delegate: RibbonButton {
                        required property var modelData
                        objectName: "editorType_" + modelData.type
                        text: modelData.label
                        checkable: true
                        checked: editor.pen !== null && editor.pen.type === modelData.type
                        onClicked: {
                            if (editor.pen.type === modelData.type)
                                return
                            const hl = modelData.type === "highlighter"
                            bar.update(editor.index, { type: modelData.type,
                                                       color: hl ? bar.highlighterColors[0] : bar.penColors[0],
                                                       width: hl ? 18 : 2 })
                        }
                    }
                }
            }

            Label {
                text: qsTr("Color")
                color: Theme.textMuted
                font.pixelSize: 12
            }
            GridLayout {
                columns: 6
                rowSpacing: 2
                columnSpacing: 2
                Repeater {
                    model: editor.highlighter ? bar.highlighterColors : bar.penColors
                    delegate: ColorSwatch {
                        required property string modelData
                        required property int index
                        objectName: "editorColor" + index
                        swatch: modelData
                        selected: editor.pen !== null && Qt.colorEqual(editor.pen.color, modelData)
                        onClicked: bar.update(editor.index, { color: modelData })
                    }
                }
                RibbonButton {
                    objectName: "editorMoreColors"
                    iconName: "color"
                    tip: qsTr("More colors…")
                    implicitWidth: 28
                    implicitHeight: 28
                    padding: 3
                    onClicked: {
                        moreColors.selectedColor = editor.pen.color
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
                    model: editor.highlighter ? bar.highlighterWidths : bar.penWidths
                    delegate: RibbonButton {
                        id: widthChoice
                        required property real modelData
                        required property int index
                        objectName: "editorWidth" + index
                        implicitWidth: 36
                        checkable: true
                        checked: editor.pen !== null && Math.abs(editor.pen.width - modelData) < 0.01
                        tip: String(modelData)
                        onClicked: bar.update(editor.index, { width: modelData })
                        contentItem: Item {
                            Rectangle {
                                anchors.centerIn: parent
                                width: editor.highlighter ? 22 : Math.max(3, widthChoice.modelData * 1.6)
                                height: editor.highlighter ? widthChoice.modelData / 2.5
                                                           : Math.max(3, widthChoice.modelData * 1.6)
                                radius: editor.highlighter ? 2 : width / 2
                                color: editor.pen !== null ? editor.pen.color : "black"
                                opacity: editor.highlighter ? 0.6 : 1
                            }
                        }
                    }
                }
            }

            Label {
                visible: !editor.highlighter
                text: qsTr("Line")
                color: Theme.textMuted
                font.pixelSize: 12
            }
            RowLayout {
                visible: !editor.highlighter
                spacing: 2
                Repeater {
                    model: ["solid", "dashed", "dotted"]
                    delegate: RibbonButton {
                        id: patternChoice
                        required property string modelData
                        objectName: "editorLine_" + modelData
                        implicitWidth: 44
                        checkable: true
                        checked: editor.pen !== null && (editor.pen.pattern || "solid") === modelData
                        onClicked: bar.update(editor.index, { pattern: modelData })
                        contentItem: LinePatternIcon {
                            pattern: patternChoice.modelData
                            color: editor.pen !== null ? editor.pen.color : "black"
                        }
                    }
                }
            }

            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: 1
                color: Theme.separator
            }
            RowLayout {
                spacing: 2
                RibbonButton {
                    objectName: "editorMoveUp"
                    text: qsTr("Up")
                    enabled: editor.index > 0
                    onClicked: bar.move(editor.index, -1)
                }
                RibbonButton {
                    objectName: "editorMoveDown"
                    text: qsTr("Down")
                    enabled: editor.index < bar.items.length - 1
                    onClicked: bar.move(editor.index, 1)
                }
                Item { Layout.fillWidth: true }
                RibbonButton {
                    objectName: "editorRemove"
                    iconName: "delete"
                    tip: qsTr("Remove from the bar")
                    onClicked: {
                        editor.close()
                        bar.remove(editor.index)
                    }
                }
            }
        }
    }
}

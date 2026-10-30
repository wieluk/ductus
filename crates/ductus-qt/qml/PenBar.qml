import QtQuick
import QtQuick.Controls
import QtQuick.Controls.impl
import QtQuick.Layouts
import Ductus

Rectangle {
    id: bar

    required property InkCanvas canvas
    /// JSON: { type: pen|highlighter, color, width, pattern? } or { type: tool, tool }.
    property string presetsJson: ""
    property bool onRight: false
    property bool fullScreen: false

    signal screenshotRequested()
    signal pictureRequested()
    signal fullScreenToggled()

    readonly property var tools: [
        { tool: "lasso", icon: "lasso", label: qsTr("Lasso") },
        { tool: "hand", icon: "hand", label: qsTr("Hand: move text, formulas and pictures, or scroll") },
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
        { type: "tool", tool: "undo" },
        { type: "tool", tool: "redo" },
        { type: "tool", tool: "lasso" },
        { type: "tool", tool: "hand" },
        { type: "tool", tool: "eraser" },
        { type: "tool", tool: "text" },
        { type: "pen", color: "#1a1a1f", width: 1.5 },
        { type: "pen", color: "#1f4fd1", width: 2 },
        { type: "pen", color: "#d12f2f", width: 2 },
        { type: "pen", color: "#1d8a3b", width: 2 },
        { type: "pen", color: "#5f6368", width: 2, pattern: "dotted" },
        { type: "highlighter", color: "#ffd91a", width: 18 },
        { type: "highlighter", color: "#5cc8ff", width: 18 },
        { type: "highlighter", color: "#ff8fd0", width: 18 },
        { type: "tool", tool: "math" }
    ]
    readonly property var items: {
        try {
            const list = JSON.parse(presetsJson)
            if (Array.isArray(list))
                return list
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
        case "hand": return canvas.tool === InkCanvas.Hand
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
        default: return true
        }
    }

    function runTool(name) {
        switch (name) {
        case "lasso": canvas.tool = InkCanvas.Lasso; break
        case "hand": canvas.tool = InkCanvas.Hand; break
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

    // A short window (a phone held sideways) scrolls the tools instead of cutting them off.
    Flickable {
        id: barScroller
        anchors.fill: parent
        anchors.topMargin: 6
        anchors.bottomMargin: 6
        clip: true
        contentWidth: width
        contentHeight: column.implicitHeight
        boundsBehavior: Flickable.StopAtBounds
        interactive: contentHeight > height
        ScrollIndicator.vertical: ScrollIndicator {}

        ColumnLayout {
            id: column
            x: (barScroller.width - width) / 2
            spacing: 2

            Repeater {
                model: bar.items
                delegate: AbstractButton {
                    id: item
                    required property var modelData
                    required property int index
                    readonly property bool isTool: modelData.type === "tool"
                    readonly property bool highlighter: modelData.type === "highlighter"
                    /// The preview line's height, so the halo behind it can match.
                    readonly property real swatchHeight: highlighter
                        ? Math.min(12, modelData.width / 2)
                        : Math.max(2, Math.min(8, modelData.width * 1.3))
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
                    property bool tipDismissed: false
                    onPressed: tipDismissed = true
                    onHoveredChanged: if (!hovered) tipDismissed = false
                    ToolTip.visible: hovered && !tipDismissed
                    ToolTip.delay: 600
                    ToolTip.text: isTool ? info.label
                                         : (highlighter ? qsTr("Highlighter") : qsTr("Pen")) + " · " + modelData.width
                    Accessible.name: ToolTip.text

                    TapHandler {
                        acceptedButtons: Qt.RightButton
                        onTapped: item.isTool ? toolMenu.show(item.index, item)
                                              : bar.openEditor(item.index, item)
                    }
                    TapHandler {
                        acceptedDevices: PointerDevice.TouchScreen | PointerDevice.Mouse | PointerDevice.TouchPad
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
                        // A light halo keeps dark pens visible on the dark bar, without a white chip.
                        Rectangle {
                            visible: Theme.dark && !item.isTool
                                     && (item.highlighter || (item.modelData.pattern || "solid") === "solid")
                            anchors.centerIn: parent
                            width: 26
                            height: item.swatchHeight + 4
                            radius: item.highlighter ? 4 : height / 2
                            rotation: -35
                            color: Qt.rgba(1, 1, 1, 0.32)
                        }
                        Rectangle {
                            visible: !item.isTool && (item.highlighter
                                     || (item.modelData.pattern || "solid") === "solid")
                            anchors.centerIn: parent
                            width: 22
                            height: item.swatchHeight
                            radius: item.highlighter ? 2 : height / 2
                            rotation: -35
                            color: item.isTool ? "transparent" : item.modelData.color
                            opacity: item.highlighter ? 0.6 : 1
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
            RibbonButton {
                objectName: "penBarFullScreen"
                visible: bar.fullScreen
                iconName: "fullscreen"
                tip: qsTr("Full screen (F11)")
                checkable: true
                checked: bar.fullScreen
                onClicked: bar.fullScreenToggled()
            }
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
                                                       color: hl ? editorOptions.highlighterColors[0] : editorOptions.penColors[0],
                                                       width: hl ? 18 : 2 })
                        }
                    }
                }
            }

            PenOptions {
                id: editorOptions
                namePrefix: "editor"
                highlighter: editor.highlighter
                color: editor.pen !== null ? editor.pen.color : "black"
                strokeWidth: editor.pen !== null ? editor.pen.width : 0
                pattern: editor.pen !== null ? editor.pen.pattern || "solid" : "solid"
                onPicked: (change) => bar.update(editor.index, change)
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

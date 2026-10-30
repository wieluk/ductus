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
    property bool undoRedoShown: true
    /// Items can be dragged into another order or removed.
    property bool arranging: false
    // The item being dragged, and how far.
    property int dragFrom: -1
    property real dragDelta: 0
    readonly property int slot: Theme.buttonSize + column.spacing
    readonly property int dragTo: dragFrom < 0 ? -1
        : Math.max(0, Math.min(items.length - 1, dragFrom + Math.round(dragDelta / slot)))

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
        { tool: "shapes", icon: "shapes", label: qsTr("Shapes on/off") }
    ]
    // Groups: what the pen does, pens, highlighters, then what it places.
    readonly property var defaults: [
        { type: "tool", tool: "lasso" },
        { type: "tool", tool: "hand" },
        { type: "tool", tool: "eraser" },
        { type: "pen", color: "#1a1a1f", width: 1.5 },
        { type: "pen", color: "#1f4fd1", width: 2 },
        { type: "pen", color: "#d12f2f", width: 2 },
        { type: "pen", color: "#1d8a3b", width: 2 },
        { type: "pen", color: "#5f6368", width: 2, pattern: "dotted" },
        { type: "highlighter", color: "#ffd91a", width: 18 },
        { type: "highlighter", color: "#5cc8ff", width: 18 },
        { type: "highlighter", color: "#ff8fd0", width: 18 },
        { type: "tool", tool: "text" },
        { type: "tool", tool: "math" }
    ]
    readonly property var items: {
        try {
            const list = JSON.parse(presetsJson)
            if (Array.isArray(list))
                return list.filter(i => i.type !== "tool" || tools.some(t => t.tool === i.tool))
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

    function moveTo(from, to) {
        if (from === to || from < 0 || to < 0)
            return
        const list = items.slice()
        const [item] = list.splice(from, 1)
        list.splice(to, 0, item)
        store(list)
    }

    function finishDrag(from, to) {
        dragFrom = -1
        moveTo(from, to)
    }

    /// Tests find items by what they are, not where they sit.
    function itemName(p) {
        return p.type === "tool" ? "barTool_" + p.tool
                                 : (p.type === "highlighter" ? "barHighlighter_" : "barPen_")
                                   + p.color.replace("#", "")
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

    onArrangingChanged: {
        editor.close()
        dragFrom = -1
    }

    Connections {
        target: bar.canvas
        function onEdited() { editor.close() }
        // Writing on the page ends arranging, as does Done.
        function onPressed() { bar.arranging = false }
    }

    Shortcut {
        sequence: "Escape"
        enabled: bar.arranging
        onActivated: bar.arranging = false
        onActivatedAmbiguously: bar.arranging = false
    }

    width: 48
    implicitHeight: column.implicitHeight + footer.implicitHeight + 12
    radius: 10
    color: Theme.popup
    border.width: 1
    border.color: Theme.ribbonBorder

    // A short window (a phone held sideways) scrolls the tools instead of cutting them off;
    // undo, redo and + stay put below them.
    Flickable {
        id: barScroller
        anchors.top: parent.top
        anchors.bottom: footer.top
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.topMargin: 6
        clip: true
        contentWidth: width
        contentHeight: column.implicitHeight
        boundsBehavior: Flickable.StopAtBounds
        // Arranging drags the items themselves.
        interactive: contentHeight > height && !bar.arranging
        ScrollIndicator.vertical: ScrollIndicator {}

        Column {
            id: column
            x: (barScroller.width - width) / 2
            spacing: 2

            Repeater {
                model: bar.items
                delegate: Item {
                    id: cell
                    required property var modelData
                    required property int index
                    // A line between groups; arranging leaves them out, so every step is one slot.
                    readonly property bool divided: index > 0 && !bar.arranging
                                                    && bar.items[index - 1].type !== modelData.type
                    width: Theme.buttonSize
                    height: Theme.buttonSize + (divided ? 9 : 0)
                    z: item.dragged ? 1 : 0

                    Rectangle {
                        visible: cell.divided
                        x: 6
                        y: 3
                        width: parent.width - 12
                        height: 1
                        color: Theme.separator
                    }

                    AbstractButton {
                        id: item
                        readonly property var modelData: cell.modelData
                        readonly property int index: cell.index
                        readonly property bool isTool: modelData.type === "tool"
                        readonly property bool highlighter: modelData.type === "highlighter"
                        /// The preview line's height, so the halo behind it can match.
                        readonly property real swatchHeight: highlighter
                            ? Math.min(12, modelData.width / 2)
                            : Math.max(2, Math.min(8, modelData.width * 1.3))
                        readonly property var info: isTool ? bar.toolInfo(modelData.tool) : null
                        readonly property bool active: isTool ? bar.toolChecked(modelData.tool)
                                                              : bar.isActive(modelData)
                        readonly property bool dragged: bar.dragFrom === index
                        objectName: bar.itemName(modelData)
                        y: cell.divided ? 9 : 0
                        width: Theme.buttonSize
                        height: Theme.buttonSize
                        focusPolicy: Qt.NoFocus
                        onClicked: {
                            if (bar.arranging)
                                return
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
                        ToolTip.visible: hovered && !tipDismissed && !bar.arranging
                        ToolTip.delay: 600
                        ToolTip.text: isTool ? info.label
                                             : (highlighter ? qsTr("Highlighter") : qsTr("Pen")) + " · " + modelData.width
                        Accessible.name: ToolTip.text

                        // The others make room while one is dragged past them.
                        transform: Translate {
                            y: {
                                if (item.dragged)
                                    return bar.dragDelta
                                if (bar.dragFrom < 0)
                                    return 0
                                if (bar.dragFrom < item.index && item.index <= bar.dragTo)
                                    return -bar.slot
                                if (bar.dragTo <= item.index && item.index < bar.dragFrom)
                                    return bar.slot
                                return 0
                            }
                            Behavior on y {
                                enabled: !item.dragged
                                NumberAnimation { duration: 120; easing.type: Easing.OutCubic }
                            }
                        }

                        DragHandler {
                            enabled: bar.arranging
                            target: null
                            xAxis.enabled: false
                            onActiveChanged: {
                                if (active) {
                                    bar.dragDelta = 0
                                    bar.dragFrom = item.index
                                } else if (bar.dragFrom === item.index) {
                                    // Not from inside the item that the move replaces.
                                    Qt.callLater(bar.finishDrag, bar.dragFrom, bar.dragTo)
                                }
                            }
                            onActiveTranslationChanged: if (active) bar.dragDelta = activeTranslation.y
                        }
                        TapHandler {
                            enabled: !bar.arranging
                            acceptedButtons: Qt.RightButton
                            onTapped: item.isTool ? toolMenu.show(item.index, item)
                                                  : bar.openEditor(item.index, item)
                        }
                        // Holding an item starts arranging, as on a phone's home screen.
                        TapHandler {
                            enabled: !bar.arranging
                            acceptedDevices: PointerDevice.TouchScreen | PointerDevice.Mouse | PointerDevice.TouchPad
                            onLongPressed: bar.arranging = true
                        }

                        background: Rectangle {
                            radius: Theme.radius
                            color: item.active ? Theme.checked
                                               : (item.hovered || bar.arranging ? Theme.hover : "transparent")
                            border.width: item.active || item.dragged ? 1 : 0
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
                                color: Theme.text
                            }
                        }

                        AbstractButton {
                            objectName: item.objectName + "_remove"
                            visible: bar.arranging && !item.dragged
                            anchors.top: parent.top
                            anchors.right: parent.right
                            width: 16
                            height: 16
                            focusPolicy: Qt.NoFocus
                            Accessible.name: qsTr("Remove from the bar")
                            onClicked: Qt.callLater(bar.remove, item.index)
                            background: Rectangle {
                                radius: width / 2
                                color: Theme.popup
                                border.width: 1
                                border.color: Theme.separator
                            }
                            contentItem: IconImage {
                                source: Qt.resolvedUrl("icons/close.svg")
                                sourceSize: Qt.size(10, 10)
                                color: Theme.text
                            }
                        }
                    }
                }
            }
        }
    }

    Column {
        id: footer
        anchors.bottom: parent.bottom
        anchors.bottomMargin: 6
        anchors.horizontalCenter: parent.horizontalCenter
        spacing: 2

        Rectangle {
            x: 6
            width: Theme.buttonSize - 12
            height: 9
            color: "transparent"
            Rectangle {
                y: 4
                width: parent.width
                height: 1
                color: Theme.separator
            }
        }
        RibbonButton {
            objectName: "barUndo"
            visible: bar.undoRedoShown && !bar.arranging
            iconName: "undo"
            tip: qsTr("Undo")
            enabled: bar.canvas.canUndo
            onClicked: bar.canvas.undo()
        }
        RibbonButton {
            objectName: "barRedo"
            visible: bar.undoRedoShown && !bar.arranging
            iconName: "redo"
            tip: qsTr("Redo")
            enabled: bar.canvas.canRedo
            onClicked: bar.canvas.redo()
        }
        RibbonButton {
            objectName: bar.arranging ? "barArrangeDone" : "presetAdd"
            iconName: bar.arranging ? "check" : "plus"
            tip: bar.arranging ? qsTr("Done") : qsTr("Add a pen or a tool")
            checked: bar.arranging
            onClicked: {
                if (bar.arranging)
                    bar.arranging = false
                else
                    addMenu.popup(this, bar.onRight ? -addMenu.width : width, 0)
            }
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
            objectName: "barArrange"
            text: qsTr("Rearrange…")
            onTriggered: bar.arranging = true
        }
        AppMenuItem {
            objectName: "barUndoRedoShown"
            text: qsTr("Show undo and redo")
            checkable: true
            checked: bar.undoRedoShown
            onTriggered: bar.undoRedoShown = !bar.undoRedoShown
        }
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

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Ductus

// Boxes not being edited are disabled, so input passes through to the canvas.
Item {
    // Not "layer": that is a built-in property of every Item.
    id: overlay

    required property InkCanvas canvas
    /// Room taken by the pen bar at the sides, which the format bar keeps clear of.
    property real leftInset: 8
    property real rightInset: 8
    /// The box being edited: its id, -1 for a new box, 0 for none.
    property real editingId: 0
    property TextEdit activeEdit: null
    property point newAt
    property real newWidth: 400

    signal noticeRequested(string message)

    /// Text on paper is dark in both themes, like ink.
    readonly property color ink: "#1a1a1f"
    readonly property int fontSize: 16

    function commit() {
        const edit = activeEdit
        if (!edit)
            return
        // Markdown of our own: Qt's would drop underline, colors and sizes.
        const markdown = formatter.markdown(edit.textDocument)
        if (editingId === -1) {
            if (markdown.trim().length > 0)
                canvas.addText(newAt, newWidth, edit.contentHeight, markdown)
            // Formats would carry over into the next new box otherwise.
            formatter.clear(edit.textDocument)
        } else if (editingId > 0) {
            const box = edit.parent
            canvas.setText(editingId, markdown, edit.contentHeight * box.docScale)
            edit.text = Qt.binding(() => box.markdown)
        }
        editingId = 0
        activeEdit = null
        // A closed box must not keep the keyboard, or Ctrl+Z undoes its text instead of the page.
        if (edit.activeFocus)
            canvas.forceActiveFocus()
    }

    function beginNew(position) {
        newAt = canvas.toDoc(position)
        newWidth = Math.max(120, Math.min(400, (overlay.width - position.x) / canvas.zoom - 16))
        editingId = -1
        activeEdit = newEdit
        newEdit.forceActiveFocus()
    }

    Connections {
        target: overlay.canvas
        function onTextRequested(position, id) {
            overlay.commit()
            if (!overlay.canvas.picturesAllowed) {
                overlay.noticeRequested(qsTr("Text boxes can't be added to a PDF you write on directly yet. Import the PDF as note pages to type on it."))
                return
            }
            if (id > 0) {
                overlay.editingId = id
                for (let i = 0; i < boxes.count; ++i) {
                    const box = boxes.itemAt(i)
                    if (box && box.textId === id) {
                        overlay.activeEdit = box.edit
                        box.edit.forceActiveFocus()
                        const p = box.edit.mapFromItem(overlay.canvas, position.x, position.y)
                        box.edit.cursorPosition = box.edit.positionAt(p.x, p.y)
                    }
                }
            } else {
                overlay.beginNew(position)
            }
        }
        function onToolChanged() {
            if (overlay.canvas.tool !== InkCanvas.Text)
                overlay.commit()
        }
        function onTextCommitRequested() { overlay.commit() }
    }

    // Inline components can't see this file's ids.
    component BoxEdit: TextEdit {
        id: boxEdit
        signal finished()
        signal boldRequested()
        signal italicRequested()
        signal underlineRequested()
        textFormat: TextEdit.MarkdownText
        wrapMode: TextEdit.Wrap
        font.pixelSize: 16
        color: "#1a1a1f"
        selectionColor: "#b9adf0"
        selectedTextColor: "#1a1a1f"
        selectByMouse: true
        persistentSelection: true
        transformOrigin: Item.TopLeft
        Keys.onEscapePressed: boxEdit.finished()
        signal spaceTyped(var event)
        Keys.onPressed: (event) => {
            if (event.key === Qt.Key_Space && event.modifiers === Qt.NoModifier) {
                boxEdit.spaceTyped(event)
                return
            }
            if (!(event.modifiers & Qt.ControlModifier))
                return
            if (event.key === Qt.Key_B) { boxEdit.boldRequested(); event.accepted = true }
            else if (event.key === Qt.Key_I) { boxEdit.italicRequested(); event.accepted = true }
            else if (event.key === Qt.Key_U) { boxEdit.underlineRequested(); event.accepted = true }
        }
    }

    Repeater {
        id: boxes
        model: overlay.canvas.texts
        delegate: Item {
            id: box
            required property real textId
            required property string markdown
            required property real boxX
            required property real boxY
            required property real boxWidth
            required property real boxHeight
            required property real zoomScale
            required property real docScale
            readonly property bool editing: overlay.editingId === box.textId
            property alias edit: edit

            objectName: "text_" + textId
            x: boxX
            y: boxY
            width: boxWidth
            height: editing ? edit.contentHeight * zoomScale : boxHeight

            Rectangle {
                visible: box.editing
                anchors.fill: parent
                anchors.margins: -4
                color: "transparent"
                radius: 3
                border.width: 1
                border.color: Theme.accent
            }

            BoxEdit {
                id: edit
                objectName: "textEdit_" + box.textId
                width: box.boxWidth / box.zoomScale
                scale: box.zoomScale
                text: box.markdown
                enabled: box.editing
                readOnly: !box.editing
                onFinished: overlay.commit()
                onBoldRequested: formatter.toggleBold()
                onSpaceTyped: (event) => event.accepted = formatter.autoFormat()
                onItalicRequested: formatter.toggleItalic()
                onUnderlineRequested: formatter.toggleUnderline()
                // Another machine's fonts may lay the same text out differently.
                onContentHeightChanged: if (overlay.canvas && !box.editing && contentHeight > 0)
                    overlay.canvas.setTextHeight(box.textId, contentHeight * box.docScale)
            }
        }
    }

    Item {
        id: newBox
        visible: overlay.editingId === -1
        // Reading the zoom re-evaluates the placement on every pan and zoom.
        readonly property point origin: {
            overlay.canvas.zoom
            return overlay.canvas.toScreen(overlay.newAt)
        }
        x: origin.x
        y: origin.y
        width: overlay.newWidth * overlay.canvas.zoom
        height: Math.max(newEdit.contentHeight, overlay.fontSize * 1.3) * overlay.canvas.zoom

        Rectangle {
            anchors.fill: parent
            anchors.margins: -4
            color: "transparent"
            radius: 3
            border.width: 1
            border.color: Theme.accent
        }
        BoxEdit {
            id: newEdit
            objectName: "newTextEdit"
            width: overlay.newWidth
            scale: overlay.canvas.zoom
            enabled: newBox.visible
            onFinished: overlay.commit()
            // An empty new box closes once the keyboard goes elsewhere, e.g. to search.
            onActiveFocusChanged: if (!activeFocus && length === 0)
                Qt.callLater(() => {
                    if (!newEdit.activeFocus && newEdit.length === 0 && !sizeMenu.visible && !colorMenu.visible)
                        overlay.commit()
                })
            onBoldRequested: formatter.toggleBold()
            onItalicRequested: formatter.toggleItalic()
            onUnderlineRequested: formatter.toggleUnderline()
            onSpaceTyped: (event) => event.accepted = formatter.autoFormat()
        }
    }

    TextFormatter {
        id: formatter
        document: overlay.activeEdit ? overlay.activeEdit.textDocument : null
        selectionStart: overlay.activeEdit ? overlay.activeEdit.selectionStart : 0
        selectionEnd: overlay.activeEdit ? overlay.activeEdit.selectionEnd : 0
        cursorPosition: overlay.activeEdit ? overlay.activeEdit.cursorPosition : 0
    }

    Rectangle {
        id: formatBar
        objectName: "textFormatBar"
        readonly property Item target: overlay.editingId === -1 ? newBox
                                       : (overlay.activeEdit ? overlay.activeEdit.parent : null)
        visible: overlay.activeEdit !== null && target !== null
        x: target ? Math.max(overlay.leftInset, Math.min(target.x, overlay.width - width - overlay.rightInset)) : 0
        y: target ? (target.y - height - 12 >= 8 ? target.y - height - 12
                                                  : target.y + target.height + 12) : 0
        width: Math.min(formatRow.implicitWidth + 12, overlay.width - overlay.leftInset - overlay.rightInset)
        height: formatRow.implicitHeight + 8
        radius: 10
        color: Theme.popup
        border.width: 1
        border.color: Theme.ribbonBorder

        component FormatButton: RibbonButton {
            implicitWidth: 34
            implicitHeight: 32
            font.pixelSize: 14
            checkable: true
        }

        // A phone's screen is narrower than all the buttons.
        Flickable {
            id: formatScroller
            anchors.fill: parent
            anchors.margins: 4
            anchors.leftMargin: 6
            anchors.rightMargin: 6
            clip: true
            contentWidth: formatRow.implicitWidth
            contentHeight: height
            flickableDirection: Flickable.HorizontalFlick
            boundsBehavior: Flickable.StopAtBounds
            interactive: contentWidth > width

            RowLayout {
                id: formatRow
                height: formatScroller.height
                spacing: 2
                FormatButton {
                    objectName: "formatBold"
                    text: "B"
                    font.bold: true
                    tip: qsTr("Bold (Ctrl+B)")
                    checked: formatter.bold
                    onClicked: formatter.toggleBold()
                }
                FormatButton {
                    objectName: "formatItalic"
                    text: "I"
                    font.italic: true
                    tip: qsTr("Italic (Ctrl+I)")
                    checked: formatter.italic
                    onClicked: formatter.toggleItalic()
                }
                FormatButton {
                    objectName: "formatStrike"
                    text: "S"
                    font.strikeout: true
                    tip: qsTr("Strikethrough")
                    checked: formatter.strikeOut
                    onClicked: formatter.toggleStrikeOut()
                }
                FormatButton {
                    objectName: "formatUnderline"
                    text: "U"
                    font.underline: true
                    tip: qsTr("Underline (Ctrl+U)")
                    checked: formatter.underline
                    onClicked: formatter.toggleUnderline()
                }
                FormatButton {
                    id: sizeButton
                    objectName: "formatSize"
                    text: "A"
                    checkable: false
                    font.pixelSize: [11, 14, 17, 20][formatter.size]
                    tip: qsTr("Text size")
                    onClicked: sizeMenu.popup(this, 0, height)
                    AppMenu {
                        id: sizeMenu
                        objectName: "sizeMenu"
                        Repeater {
                            model: [qsTr("Small"), qsTr("Normal"), qsTr("Large"), qsTr("Huge")]
                            delegate: AppMenuItem {
                                required property string modelData
                                required property int index
                                objectName: "formatSize" + index
                                text: modelData
                                checkable: true
                                checked: formatter.size === index
                                onTriggered: formatter.setSize(index)
                            }
                        }
                    }
                }
                FormatButton {
                    objectName: "formatColor"
                    checkable: false
                    tip: qsTr("Text color")
                    onClicked: colorMenu.popup(this, 0, height)
                    contentItem: Item {
                        Rectangle {
                            anchors.centerIn: parent
                            width: 16
                            height: 16
                            radius: 8
                            color: formatter.color.length > 0 ? formatter.color : overlay.ink
                            border.width: 1
                            border.color: Theme.separator
                        }
                    }
                    AppMenu {
                        id: colorMenu
                        objectName: "colorMenu"
                        MenuItem {
                            height: 40
                            background: null
                            contentItem: Row {
                                leftPadding: 6
                                Repeater {
                                    // The first one is the text's own color.
                                    model: ["", "#d12f2f", "#1f4fd1", "#1d8a3b", "#e07b00", "#7b2fd1", "#6a6779"]
                                    delegate: ColorSwatch {
                                        required property string modelData
                                        required property int index
                                        objectName: "textColor" + index
                                        swatch: modelData.length > 0 ? modelData : overlay.ink
                                        selected: formatter.color === modelData
                                        onClicked: {
                                            formatter.setColor(modelData)
                                            colorMenu.close()
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Rectangle {
                    Layout.preferredWidth: 1
                    Layout.preferredHeight: 22
                    Layout.leftMargin: 4
                    Layout.rightMargin: 4
                    color: Theme.separator
                }
                Repeater {
                    model: [{ level: 1, label: "H1", tip: qsTr("Title") },
                            { level: 2, label: "H2", tip: qsTr("Heading") },
                            { level: 3, label: "H3", tip: qsTr("Subheading") }]
                    delegate: FormatButton {
                        required property var modelData
                        objectName: "formatH" + modelData.level
                        text: modelData.label
                        tip: modelData.tip
                        checked: formatter.heading === modelData.level
                        onClicked: formatter.setHeading(checked ? modelData.level : 0)
                    }
                }
                Rectangle {
                    Layout.preferredWidth: 1
                    Layout.preferredHeight: 22
                    Layout.leftMargin: 4
                    Layout.rightMargin: 4
                    color: Theme.separator
                }
                FormatButton {
                    objectName: "formatBullets"
                    text: "•"
                    tip: qsTr("Bulleted list")
                    checked: formatter.list === 1
                    onClicked: formatter.toggleList(1)
                }
                FormatButton {
                    objectName: "formatNumbers"
                    text: "1."
                    tip: qsTr("Numbered list")
                    checked: formatter.list === 2
                    onClicked: formatter.toggleList(2)
                }
                Rectangle {
                    Layout.preferredWidth: 1
                    Layout.preferredHeight: 22
                    Layout.leftMargin: 4
                    Layout.rightMargin: 4
                    color: Theme.separator
                }
                RibbonButton {
                    objectName: "formatDone"
                    text: qsTr("Done")
                    tip: qsTr("Finish typing (Esc)")
                    onClicked: overlay.commit()
                }
            }
        }
        ScrollArrows {
            target: formatScroller
            color: Theme.popup
        }
    }
}

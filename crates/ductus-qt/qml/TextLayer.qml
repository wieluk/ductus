import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Ductus

// Boxes not being edited are disabled, so input passes through to the canvas.
Item {
    // Not "layer": that is a built-in property of every Item.
    id: overlay

    required property InkCanvas canvas
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
        if (editingId === -1) {
            if (edit.text.trim().length > 0)
                canvas.addText(newAt, newWidth, edit.contentHeight, edit.text)
            edit.text = ""
        } else if (editingId > 0) {
            const box = edit.parent
            canvas.setText(editingId, edit.text, edit.contentHeight * box.docScale)
            edit.text = Qt.binding(() => box.markdown)
        }
        editingId = 0
        activeEdit = null
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

    // Inline components can't see this file's ids: they get what they need
    // through properties and signals.
    component BoxEdit: TextEdit {
        id: boxEdit
        signal finished()
        signal boldRequested()
        signal italicRequested()
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
            onBoldRequested: formatter.toggleBold()
            onItalicRequested: formatter.toggleItalic()
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
        x: target ? Math.max(8, Math.min(target.x, overlay.width - width - 8)) : 0
        y: target ? (target.y - height - 12 >= 8 ? target.y - height - 12
                                                  : target.y + target.height + 12) : 0
        width: formatRow.implicitWidth + 12
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

        RowLayout {
            id: formatRow
            anchors.centerIn: parent
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
}

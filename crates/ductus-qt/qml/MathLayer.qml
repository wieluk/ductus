import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Ductus

Item {
    id: maths

    required property InkCanvas canvas
    /// "latex" or "typst"; the last choice is kept for the next formula.
    property string syntax: "latex"
    property bool editing: false
    property string problem: ""

    signal noticeRequested(string message)

    property string category: "favorites"
    /// As JSON in the settings: [{ label, latex, typst }, …].
    property string favoritesJson: ""

    readonly property var favorites: {
        try {
            const list = JSON.parse(favoritesJson)
            if (Array.isArray(list))
                return list
        } catch (e) {}
        // Until the user picks their own: a starter set from the catalog.
        const all = MathSnippets.categories.reduce((acc, c) => acc.concat(c.items), [])
        return MathSnippets.defaultFavorites.map(l => all.find(i => i.label === l)).filter(i => i)
    }
    readonly property var shown: category === "favorites"
        ? favorites
        : (MathSnippets.categories.find(c => c.name === category) || { items: [] }).items

    function isFavorite(item) {
        return favorites.some(f => f.label === item.label && f.latex === item.latex
                                   && f.typst === item.typst)
    }

    function toggleFavorite(item) {
        let list = favorites.slice()
        if (isFavorite(item))
            list = list.filter(f => !(f.label === item.label && f.latex === item.latex
                                      && f.typst === item.typst))
        else
            list.push({ label: item.label, latex: item.latex || "", typst: item.typst || "" })
        favoritesJson = JSON.stringify(list)
    }

    function saveFormula() {
        const source = sourceField.text.trim()
        if (source.length === 0)
            return
        const item = { label: source.length > 14 ? source.slice(0, 13) + "…" : source,
                       latex: syntax === "latex" ? source : "",
                       typst: syntax === "typst" ? source : "" }
        if (!isFavorite(item))
            toggleFavorite(item)
        category = "favorites"
    }

    function open(source, syntaxName) {
        maths.syntax = syntaxName
        sourceField.text = source
        maths.problem = ""
        maths.editing = true
        sourceField.forceActiveFocus()
        sourceField.cursorPosition = sourceField.length
    }

    function commit() {
        if (!editing)
            return
        finish()
        canvas.commitMath()
    }

    function cancel() {
        if (!editing)
            return
        finish()
        canvas.cancelMath()
    }

    // The hidden source field must not keep the keyboard (Ctrl+Z would undo
    // its text instead of the page).
    function finish() {
        editing = false
        if (sourceField.activeFocus)
            canvas.forceActiveFocus()
    }

    function update() {
        if (editing)
            problem = canvas.setMathDraft(sourceField.text, syntax)
    }

    function insert(snippet) {
        const text = syntax === "latex" ? snippet.latex : snippet.typst
        if (!text)
            return
        const at = sourceField.cursorPosition
        sourceField.insert(at, text)
        // Into the first empty slot, else after the inserted text.
        const slot = text.search(/\{\}|\(\)|, \)/)
        sourceField.cursorPosition = at + (slot >= 0 ? slot + (text[slot] === "," ? 2 : 1) : text.length)
        sourceField.forceActiveFocus()
    }

    Connections {
        target: maths.canvas
        function onMathRequested(position, id) {
            maths.commit()
            if (!maths.canvas.picturesAllowed) {
                maths.noticeRequested(qsTr("Formulas can't be added to a PDF you write on directly yet. Import the PDF as note pages to write formulas on it."))
                return
            }
            if (id > 0) {
                const found = maths.canvas.editMath(id)
                if (found.length === 2)
                    maths.open(found[0], found[1])
            } else {
                maths.canvas.beginMath(position, maths.syntax)
                maths.open("", maths.syntax)
            }
        }
        function onToolChanged() {
            if (maths.canvas.tool !== InkCanvas.Math)
                maths.commit()
        }
        function onTextCommitRequested() { maths.commit() }
    }

    AppDialog {
        id: resetDialog
        objectName: "mathResetDialog"
        parent: Overlay.overlay
        title: qsTr("Reset the math favorites?")
        message: qsTr("Your own favorites and saved formulas are removed, and the starter set comes back.")
        acceptText: qsTr("Reset")
        destructive: true
        acceptButton.objectName: "mathResetConfirm"
        onAccepted: maths.favoritesJson = ""
        // Typing continues in the formula afterwards.
        onClosed: if (maths.editing) sourceField.forceActiveFocus()
    }

    Rectangle {
        id: panel
        objectName: "mathEditor"
        readonly property rect target: maths.canvas.mathDraftRect
        visible: maths.editing
        width: Math.min(460, maths.width - 16)
        height: column.implicitHeight + 20
        x: Math.max(8, Math.min(target.x, maths.width - width - 8))
        y: target.y + target.height + 12 + height <= maths.height - 8
           ? target.y + target.height + 12
           : Math.max(8, target.y - height - 12)
        radius: 10
        color: Theme.popup
        border.width: 1
        border.color: Theme.ribbonBorder

        ColumnLayout {
            id: column
            anchors.fill: parent
            anchors.margins: 10
            spacing: 8

            RowLayout {
                spacing: 2
                Repeater {
                    model: [{ key: "latex", label: "LaTeX" }, { key: "typst", label: "Typst" }]
                    delegate: RibbonButton {
                        required property var modelData
                        objectName: "mathSyntax_" + modelData.key
                        text: modelData.label
                        checkable: true
                        checked: maths.syntax === modelData.key
                        onClicked: {
                            maths.syntax = modelData.key
                            maths.update()
                        }
                    }
                }
                Item { Layout.fillWidth: true }
                Label {
                    text: qsTr("Enter: done · Shift+Enter: new line")
                    color: Theme.textMuted
                    font.pixelSize: 11
                }
            }

            Flow {
                Layout.fillWidth: true
                spacing: 2
                Repeater {
                    model: [{ key: "favorites", label: "★ " + qsTr("Favorites") }].concat(
                        MathSnippets.categories.map(c => ({ key: c.name, label: c.name })))
                    delegate: AbstractButton {
                        id: chip
                        required property var modelData
                        required property int index
                        readonly property bool active: maths.category === modelData.key
                        objectName: "mathCategory" + index
                        implicitHeight: 24
                        implicitWidth: chipLabel.implicitWidth + 16
                        focusPolicy: Qt.NoFocus
                        onClicked: maths.category = modelData.key
                        contentItem: Label {
                            id: chipLabel
                            text: chip.modelData.label
                            horizontalAlignment: Text.AlignHCenter
                            verticalAlignment: Text.AlignVCenter
                            font.pixelSize: 11
                            font.weight: Font.DemiBold
                            color: chip.active ? Theme.accent : Theme.textMuted
                        }
                        background: Rectangle {
                            radius: 12
                            color: chip.active ? Theme.checked : (chip.hovered ? Theme.hover : "transparent")
                        }
                    }
                }
            }

            Flow {
                Layout.fillWidth: true
                Layout.maximumHeight: 140
                clip: true
                spacing: 2
                Repeater {
                    model: maths.shown
                    delegate: RibbonButton {
                        id: snippet
                        required property var modelData
                        required property int index
                        readonly property string code: maths.syntax === "latex" ? modelData.latex
                                                                                 : modelData.typst
                        objectName: "mathSnippet" + index
                        text: modelData.label
                        implicitWidth: Math.max(38, implicitContentWidth + 12)
                        implicitHeight: 30
                        enabled: code.length > 0
                        tip: code + (maths.isFavorite(modelData) ? "  ★" : "")
                             + "\n" + qsTr("Right-click or hold: favorite")
                        onClicked: maths.insert(modelData)
                        TapHandler {
                            acceptedButtons: Qt.RightButton
                            onTapped: maths.toggleFavorite(snippet.modelData)
                        }
                        TapHandler {
                            acceptedDevices: PointerDevice.TouchScreen | PointerDevice.Stylus
                            onLongPressed: maths.toggleFavorite(snippet.modelData)
                        }
                    }
                }
                Label {
                    visible: maths.shown.length === 0
                    text: qsTr("No favorites yet: right-click a preset, or save the formula you typed.")
                    color: Theme.textMuted
                    font.pixelSize: 12
                }
            }

            RowLayout {
                visible: maths.category === "favorites" && maths.favoritesJson.length > 0
                Layout.fillWidth: true
                Item { Layout.fillWidth: true }
                AbstractButton {
                    id: resetLink
                    objectName: "mathResetFavorites"
                    focusPolicy: Qt.NoFocus
                    onClicked: resetDialog.open()
                    contentItem: Label {
                        text: qsTr("Reset favorites…")
                        color: resetLink.hovered ? Theme.accent : Theme.textMuted
                        font.pixelSize: 11
                        font.underline: resetLink.hovered
                    }
                    background: null
                }
            }

            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: Math.max(36, sourceField.implicitHeight + 12)
                radius: 6
                color: Theme.dark ? "#24222c" : "#f6f5f9"
                border.width: 1
                border.color: sourceField.activeFocus ? Theme.accent : Theme.separator

                TextEdit {
                    id: sourceField
                    objectName: "mathSource"
                    anchors.fill: parent
                    anchors.margins: 6
                    wrapMode: TextEdit.Wrap
                    font.family: "monospace"
                    font.pixelSize: 14
                    color: Theme.text
                    selectByMouse: true
                    onTextChanged: maths.update()
                    Keys.onReturnPressed: (event) => {
                        if (event.modifiers & Qt.ShiftModifier)
                            event.accepted = false
                        else
                            maths.commit()
                    }
                    Keys.onEnterPressed: maths.commit()
                    // Tab jumps into the next empty {} or () left by a quick insert.
                    Keys.onTabPressed: {
                        const rest = sourceField.text.slice(sourceField.cursorPosition)
                        const next = rest.search(/\{\}|\(\)/)
                        sourceField.cursorPosition = next >= 0
                            ? sourceField.cursorPosition + next + 1
                            : sourceField.length
                    }
                    Keys.onEscapePressed: maths.cancel()

                    Label {
                        visible: sourceField.length === 0
                        text: maths.syntax === "latex" ? "\\frac{a}{b} + \\sqrt{x^2 + 1}"
                                                       : "a/b + sqrt(x^2 + 1)"
                        color: Theme.textDisabled
                        font: sourceField.font
                    }
                }
            }

            Label {
                objectName: "mathProblem"
                Layout.fillWidth: true
                visible: maths.problem.length > 0
                text: maths.problem
                color: Theme.danger
                font.pixelSize: 12
                wrapMode: Text.WordWrap
            }

            RowLayout {
                Layout.fillWidth: true
                RibbonButton {
                    objectName: "mathSaveFavorite"
                    text: "☆ " + qsTr("Save as favorite")
                    tip: qsTr("Keep this formula under ★ Favorites")
                    enabled: sourceField.length > 0
                    onClicked: maths.saveFormula()
                }
                Item { Layout.fillWidth: true }
                RibbonButton {
                    objectName: "mathCancel"
                    text: qsTr("Cancel")
                    onClicked: maths.cancel()
                }
                RibbonButton {
                    objectName: "mathDone"
                    text: qsTr("Done")
                    checked: true
                    onClicked: maths.commit()
                }
            }
        }
    }
}

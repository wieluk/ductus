import QtCore
import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import QtQuick.Layouts
import Ductus

Dialog {
    id: dialog

    required property InkCanvas canvas
    required property ApplicationWindow window
    property int section: 0

    /// Handwriting, scrolled down to the recognizers to download.
    function showModels() {
        section = 4
        open()
        Qt.callLater(() => {
            const flick = handwritingSection.contentItem
            const y = modelsAnchor.mapToItem(flick.contentItem, 0, 0).y - 8
            flick.contentY = Math.max(0, Math.min(y, flick.contentHeight - flick.height))
        })
    }

    signal openFolderRequested()
    signal updateRequested()
    signal trashRequested()

    objectName: "settingsDialog"
    title: qsTr("Settings")
    anchors.centerIn: parent
    modal: true
    padding: 0
    /// Phones: sections in a strip on top, labels above their controls.
    readonly property bool narrow: window.width < 600
    width: narrow ? window.width - 16 : Math.min(780, window.width - 40)
    height: narrow ? window.height - 24 : Math.min(580, window.height - 60)

    Overlay.modal: Rectangle { color: Theme.dark ? "#99000000" : "#55000000" }

    background: Rectangle {
        color: Theme.popup
        radius: 12
        border.width: 1
        border.color: Theme.ribbonBorder
    }

    // Downloading it again takes a while: removing asks first.
    AppDialog {
        id: removeModelDialog
        objectName: "removeModelDialog"
        property string modelId
        property string modelTitle
        function ask(model) {
            modelId = model.id
            modelTitle = model.title
            open()
        }
        title: qsTr("Remove \u201c%1\u201d?").arg(modelTitle)
        message: qsTr("Handwriting can't be read with it until it is downloaded again.")
        acceptText: qsTr("Remove")
        destructive: true
        onAccepted: Models.remove(modelId)
    }

    // AppController reads these keys too (pages/, hwr/).
    Settings {
        id: hwr
        category: "hwr"
        property bool search: false
        property string backend: "builtin"
        property string url: ""
        property string model: ""
        property string key: ""
        /// Percent of the processor's cores the built-in recognizer may use.
        property int processor: 50
        /// The handwriting languages, as codes: "de,en"; "" follows the system.
        property string lang: ""
    }

    Settings {
        id: pages
        category: "pages"
        property bool useDefault: false
        property string kind: "endless"
        property bool landscape: false
        property string pattern: "dotted"
    }

    header: RowLayout {
        spacing: 0
        Label {
            Layout.fillWidth: true
            Layout.margins: 20
            Layout.bottomMargin: 8
            text: dialog.title
            color: Theme.text
            font.pixelSize: 18
            font.weight: Font.DemiBold
        }
        RibbonButton {
            objectName: "settingsClose"
            Layout.rightMargin: 12
            iconName: "close"
            tip: qsTr("Close")
            onClicked: dialog.close()
        }
    }

    component Heading: Label {
        Layout.fillWidth: true
        Layout.topMargin: 8
        color: Theme.accent
        font.pixelSize: 12
        font.weight: Font.DemiBold
        font.capitalization: Font.AllUppercase
    }

    component Described: ColumnLayout {
        property string label
        property string hint
        Layout.fillWidth: true
        spacing: 1
        Label {
            Layout.fillWidth: true
            text: parent.label
            color: Theme.text
            font.pixelSize: 13
            font.weight: Font.DemiBold
            wrapMode: Text.WordWrap
        }
        Label {
            Layout.fillWidth: true
            visible: text.length > 0
            text: parent.hint
            color: Theme.textMuted
            font.pixelSize: 12
            wrapMode: Text.WordWrap
        }
    }

    component Choice: GridLayout {
        id: choice
        property string label
        property string hint
        property var options: []
        property int value
        signal picked(int index)
        Layout.fillWidth: true
        columns: dialog.narrow ? 1 : 2
        rowSpacing: 6
        columnSpacing: 12
        Described {
            label: choice.label
            hint: choice.hint
        }
        RowLayout {
            spacing: 2
            Repeater {
                model: choice.options
                delegate: RibbonButton {
                    required property string modelData
                    required property int index
                    objectName: choice.objectName + index
                    text: modelData
                    checkable: true
                    checked: choice.value === index
                    onClicked: choice.picked(index)
                }
            }
        }
    }

    /// What a pen button stands for while held: one of the tools, or the chosen one.
    component PenButton: GridLayout {
        id: penButton
        property string label
        property string hint
        property int value
        signal picked(int tool)
        Layout.fillWidth: true
        columns: dialog.narrow ? 1 : 2
        rowSpacing: 6
        columnSpacing: 12
        Described {
            label: penButton.label
            hint: penButton.hint
        }
        AppComboBox {
            objectName: penButton.objectName + "Tool"
            Layout.preferredWidth: 240
            Layout.fillWidth: dialog.narrow
            model: [{ tool: -1, name: qsTr("Current tool") }, { tool: InkCanvas.Pen, name: qsTr("Pen") },
                    { tool: InkCanvas.Highlighter, name: qsTr("Highlighter") },
                    { tool: InkCanvas.Eraser, name: qsTr("Eraser") }, { tool: InkCanvas.Lasso, name: qsTr("Lasso") },
                    { tool: InkCanvas.Hand, name: qsTr("Hand") }]
            textRole: "name"
            valueRole: "tool"
            currentIndex: Math.max(0, model.findIndex(t => t.tool === penButton.value))
            onActivated: penButton.picked(currentValue)
        }
    }

    component Toggle: RowLayout {
        id: toggle
        property string label
        property string hint
        property bool checked
        signal toggled(bool on)
        Layout.fillWidth: true
        spacing: 12
        Described {
            label: toggle.label
            hint: toggle.hint
        }
        AbstractButton {
            id: knob
            objectName: toggle.objectName + "Switch"
            implicitWidth: 40
            implicitHeight: 22
            checkable: true
            checked: toggle.checked
            focusPolicy: Qt.NoFocus
            onClicked: toggle.toggled(checked)
            background: Rectangle {
                radius: 11
                color: knob.checked ? Theme.accent : Theme.separator
                Rectangle {
                    width: 18
                    height: 18
                    radius: 9
                    y: 2
                    x: knob.checked ? parent.width - width - 2 : 2
                    color: "white"
                    Behavior on x { NumberAnimation { duration: 120 } }
                }
            }
        }
    }

    component Field: GridLayout {
        id: fieldRow
        property string label
        property string hint
        property alias text: input.text
        property alias placeholder: input.placeholderText
        property bool secret: false
        signal edited(string text)
        Layout.fillWidth: true
        columns: dialog.narrow ? 1 : 2
        rowSpacing: 6
        columnSpacing: 12
        Described {
            label: fieldRow.label
            hint: fieldRow.hint
        }
        TextField {
            id: input
            objectName: fieldRow.objectName + "Input"
            Layout.preferredWidth: 240
            Layout.fillWidth: dialog.narrow
            echoMode: fieldRow.secret ? TextInput.Password : TextInput.Normal
            color: Theme.text
            placeholderTextColor: Theme.textDisabled
            font.pixelSize: 13
            selectByMouse: true
            onTextEdited: fieldRow.edited(text)
            background: Rectangle {
                radius: 6
                color: Theme.dark ? "#24222c" : "#f6f5f9"
                border.width: 1
                border.color: input.activeFocus ? Theme.accent : Theme.separator
            }
        }
    }

    component Section: ScrollView {
        id: section
        default property alias content: column.data
        // The scroll bar gets its own strip instead of covering the options.
        rightPadding: ScrollBar.vertical.visible ? ScrollBar.vertical.width + 8 : 0
        contentWidth: availableWidth
        clip: true
        ColumnLayout {
            id: column
            width: section.availableWidth
            spacing: 18
        }
    }

    contentItem: GridLayout {
        columns: dialog.narrow ? 1 : 3
        rowSpacing: 0
        columnSpacing: 0

        ListView {
            id: sections
            objectName: "settingsSections"
            Layout.preferredWidth: dialog.narrow ? -1 : 190
            Layout.preferredHeight: dialog.narrow ? 40 : -1
            Layout.fillWidth: dialog.narrow
            Layout.fillHeight: !dialog.narrow
            Layout.leftMargin: 10
            Layout.rightMargin: dialog.narrow ? 10 : 0
            Layout.bottomMargin: dialog.narrow ? 4 : 12
            orientation: dialog.narrow ? ListView.Horizontal : ListView.Vertical
            interactive: dialog.narrow
            clip: dialog.narrow
            currentIndex: dialog.section
            // The strip shows the open section, also when the app opens one far along.
            onCurrentIndexChanged: positionViewAtIndex(currentIndex, ListView.Contain)
            onWidthChanged: positionViewAtIndex(currentIndex, ListView.Contain)
            spacing: 2
            model: [qsTr("Pen & input"), qsTr("New pages"), qsTr("Appearance"),
                    qsTr("Library"), qsTr("Handwriting"), qsTr("About")]
            delegate: AbstractButton {
                id: entry
                required property string modelData
                required property int index
                objectName: "settingsSection" + index
                width: dialog.narrow ? implicitContentWidth : ListView.view.width
                height: 36
                onClicked: dialog.section = index
                contentItem: Label {
                    leftPadding: 12
                    rightPadding: dialog.narrow ? 12 : 0
                    text: entry.modelData
                    verticalAlignment: Text.AlignVCenter
                    color: dialog.section === entry.index ? Theme.accent : Theme.text
                    font.pixelSize: 13
                    font.weight: dialog.section === entry.index ? Font.DemiBold : Font.Normal
                }
                background: Rectangle {
                    radius: Theme.radius
                    color: dialog.section === entry.index ? Theme.checked
                                                          : (entry.hovered ? Theme.hover : "transparent")
                }
            }
        }

        Rectangle {
            Layout.preferredWidth: dialog.narrow ? -1 : 1
            Layout.preferredHeight: dialog.narrow ? 1 : -1
            Layout.fillWidth: dialog.narrow
            Layout.fillHeight: !dialog.narrow
            Layout.bottomMargin: 12
            color: Theme.separator
        }

        StackLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.leftMargin: dialog.narrow ? 16 : 20
            Layout.rightMargin: dialog.narrow ? 16 : 20
            Layout.bottomMargin: 16
            currentIndex: dialog.section

            Section {
                Heading { text: qsTr("Writing") }
                Choice {
                    objectName: "pressure"
                    label: qsTr("Pressure")
                    hint: qsTr("How hard you press for a bold line.")
                    options: [qsTr("Light", "pen pressure"), qsTr("Normal"), qsTr("Firm")]
                    value: dialog.canvas.pressureResponse
                    onPicked: (index) => dialog.canvas.pressureResponse = index
                }
                Choice {
                    objectName: "smoothing"
                    label: qsTr("Smoothing")
                    hint: qsTr("Evens out shaky lines. Off draws exactly what the pen reports.")
                    options: [qsTr("Off"), qsTr("Low"), qsTr("Normal"), qsTr("High")]
                    value: dialog.canvas.smoothing
                    onPicked: (index) => dialog.canvas.smoothing = index
                }
                Toggle {
                    objectName: "penTip"
                    label: qsTr("Show the pen tip")
                    hint: qsTr("A small dot marks where the pen hovers. The mouse pointer stays hidden either way.")
                    checked: dialog.canvas.penTipVisible
                    onToggled: (on) => dialog.canvas.penTipVisible = on
                }
                Choice {
                    objectName: "penTipSize"
                    enabled: dialog.canvas.penTipVisible
                    label: qsTr("Pen tip size")
                    readonly property var sizes: [1.5, 2.5, 4]
                    options: [qsTr("Small"), qsTr("Medium"), qsTr("Large")]
                    value: sizes.indexOf(dialog.canvas.penTipSize)
                    onPicked: (index) => dialog.canvas.penTipSize = sizes[index]
                }

                Heading { text: qsTr("Shapes") }
                Toggle {
                    objectName: "shapes"
                    label: qsTr("Hold to make a shape")
                    hint: qsTr("Rest the pen at the end of a stroke and it becomes a straight line, arrow, circle or polygon.")
                    checked: dialog.canvas.shapeAssist
                    onToggled: (on) => dialog.canvas.shapeAssist = on
                }
                Choice {
                    objectName: "holdDelay"
                    enabled: dialog.canvas.shapeAssist
                    label: qsTr("Hold for")
                    hint: qsTr("How long the pen has to rest before the shape appears.")
                    readonly property var delays: [350, 500, 800]
                    options: [qsTr("Short"), qsTr("Normal"), qsTr("Long")]
                    value: delays.indexOf(dialog.canvas.holdDelay)
                    onPicked: (index) => dialog.canvas.holdDelay = delays[index]
                }

                Heading { text: qsTr("Eraser") }
                Choice {
                    objectName: "eraserMode"
                    label: qsTr("The eraser removes")
                    hint: qsTr("Applies to the eraser tool and the pen's eraser end.")
                    options: [qsTr("What it touches"), qsTr("Whole strokes")]
                    value: dialog.canvas.eraserWholeStrokes ? 1 : 0
                    onPicked: (index) => dialog.canvas.eraserWholeStrokes = index === 1
                }
                Choice {
                    objectName: "eraserSize"
                    label: qsTr("Eraser size")
                    hint: qsTr("Also in the eraser's menus. The pen shows a ring this large while erasing.")
                    readonly property var sizes: [4, 8, 16, 30]
                    options: ["S", "M", "L", "XL"]
                    value: sizes.indexOf(dialog.canvas.eraserSize)
                    onPicked: (index) => dialog.canvas.eraserSize = sizes[index]
                }

                Heading { text: qsTr("Pen buttons") }
                PenButton {
                    objectName: "penButton"
                    label: qsTr("Side button")
                    hint: qsTr("Hold it while the tip touches the page to use this instead.")
                    value: dialog.canvas.penButtonTool
                    onPicked: (tool) => dialog.canvas.penButtonTool = tool
                }
                PenButton {
                    objectName: "penButton2"
                    label: qsTr("Second side button")
                    hint: qsTr("On pens with two, the one farther from the tip.")
                    value: dialog.canvas.penButton2Tool
                    onPicked: (tool) => dialog.canvas.penButton2Tool = tool
                }
                PenButton {
                    objectName: "eraserEnd"
                    label: qsTr("Eraser end")
                    value: dialog.canvas.eraserEndTool
                    onPicked: (tool) => dialog.canvas.eraserEndTool = tool
                }

                Heading { text: qsTr("Touch") }
                Toggle {
                    objectName: "fingerDrawing"
                    label: qsTr("Draw with a finger")
                    hint: qsTr("When off, fingers only scroll and zoom, and only the pen writes.")
                    checked: dialog.canvas.fingerDrawing
                    onToggled: (on) => dialog.canvas.fingerDrawing = on
                }
                Choice {
                    objectName: "palmRejection"
                    label: qsTr("Ignore touch after writing")
                    hint: qsTr("A hand resting on the screen while you write doesn't move the page. Longer is safer, shorter lets you scroll sooner.")
                    readonly property var delays: [300, 600, 1200]
                    options: [qsTr("0.3 s"), qsTr("0.6 s"), qsTr("1.2 s")]
                    value: delays.indexOf(dialog.canvas.palmRejection)
                    onPicked: (index) => dialog.canvas.palmRejection = delays[index]
                }
            }

            Section {
                Heading { text: qsTr("New pages") }
                Choice {
                    objectName: "newPageStyle"
                    label: qsTr("Style of new pages")
                    hint: qsTr("Copy the paper and background of the page you're on, or always use the style below.")
                    options: [qsTr("Like the open page"), qsTr("Always the same")]
                    value: pages.useDefault ? 1 : 0
                    onPicked: (index) => pages.useDefault = index === 1
                }
                Choice {
                    objectName: "newPageKind"
                    enabled: pages.useDefault
                    label: qsTr("Paper")
                    readonly property var kinds: ["endless", "a4", "a5", "letter"]
                    options: [qsTr("Endless"), "A4", "A5", qsTr("Letter")]
                    value: kinds.indexOf(pages.kind)
                    onPicked: (index) => pages.kind = kinds[index]
                }
                Toggle {
                    objectName: "newPageLandscape"
                    enabled: pages.useDefault && pages.kind !== "endless"
                    label: qsTr("Landscape")
                    checked: pages.landscape
                    onToggled: (on) => pages.landscape = on
                }
                Choice {
                    objectName: "newPagePattern"
                    enabled: pages.useDefault
                    label: qsTr("Background")
                    readonly property var patterns: ["blank", "lined", "grid", "dotted"]
                    options: [qsTr("Blank"), qsTr("Lined"), qsTr("Grid"), qsTr("Dotted")]
                    value: patterns.indexOf(pages.pattern)
                    onPicked: (index) => pages.pattern = patterns[index]
                }
            }

            Section {
                Heading { text: qsTr("Language") }
                GridLayout {
                    Layout.fillWidth: true
                    columns: dialog.narrow ? 1 : 2
                    rowSpacing: 6
                    columnSpacing: 12
                    Described {
                        label: qsTr("App language")
                        hint: qsTr("The language of menus and messages. Handwriting languages are under Handwriting.")
                    }
                    AppComboBox {
                        objectName: "appLanguage"
                        Layout.preferredWidth: 240
                        Layout.fillWidth: dialog.narrow
                        model: Translations.languages
                        textRole: "name"
                        valueRole: "code"
                        currentIndex: Math.max(0, Translations.languages.findIndex(l => l.code === Translations.language))
                        onActivated: Translations.language = currentValue
                    }
                }
                Heading { text: qsTr("Theme") }
                Choice {
                    objectName: "theme"
                    label: qsTr("Colors")
                    hint: qsTr("Follow the system switches with your desktop's dark mode.")
                    readonly property var modes: ["system", "light", "dark"]
                    options: [qsTr("System"), qsTr("Light"), qsTr("Dark")]
                    value: modes.indexOf(dialog.window.themeMode)
                    onPicked: (index) => dialog.window.themeMode = modes[index]
                }
                GridLayout {
                    Layout.fillWidth: true
                    columns: dialog.narrow ? 1 : 2
                    rowSpacing: 6
                    columnSpacing: 12
                    Described {
                        label: qsTr("App color")
                        hint: qsTr("The title bar, buttons and highlights; the grays take on its tint.")
                    }
                    Flow {
                        // The colors and More colors in one row.
                        Layout.preferredWidth: 280
                        Layout.fillWidth: dialog.narrow
                        spacing: 2
                        Repeater {
                            model: Theme.colors
                            delegate: ColorSwatch {
                                required property string modelData
                                required property int index
                                objectName: "appColor" + index
                                swatch: modelData
                                selected: Qt.colorEqual(dialog.window.appColor, modelData)
                                onClicked: dialog.window.appColor = modelData
                            }
                        }
                        RibbonButton {
                            objectName: "appColorMore"
                            iconName: "color"
                            tip: qsTr("More colors…")
                            implicitWidth: 28
                            implicitHeight: 28
                            padding: 3
                            onClicked: {
                                appColorDialog.selectedColor = dialog.window.appColor
                                appColorDialog.open()
                            }
                        }
                        ColorDialog {
                            id: appColorDialog
                            title: qsTr("Choose a color")
                            onAccepted: dialog.window.appColor = selectedColor
                        }
                    }
                }
                Heading { text: qsTr("Around the page") }
                Toggle {
                    objectName: "sidebar"
                    label: qsTr("Notebook list")
                    hint: qsTr("The sidebar with folders and notebooks (%1).").arg("Ctrl+\\")
                    checked: dialog.window.sidebarVisible
                    onToggled: (on) => dialog.window.sidebarVisible = on
                }
                Choice {
                    objectName: "sidebarSide"
                    enabled: dialog.window.sidebarVisible
                    label: qsTr("Notebook list side")
                    hint: qsTr("Next to the page on the left or the right.")
                    options: [qsTr("Left"), qsTr("Right")]
                    value: dialog.window.sidebarRight ? 1 : 0
                    onPicked: (index) => dialog.window.sidebarRight = index === 1
                }
                Toggle {
                    objectName: "penBar"
                    label: qsTr("Pen bar")
                    hint: qsTr("Your favorite pens in a column at the side of the page.")
                    checked: dialog.window.penBarVisible
                    onToggled: (on) => dialog.window.penBarVisible = on
                }
                Choice {
                    objectName: "penBarSide"
                    enabled: dialog.window.penBarVisible
                    label: qsTr("Pen bar side")
                    hint: qsTr("Put it on the side away from your writing hand, so your hand doesn't cover it.")
                    options: [qsTr("Left"), qsTr("Right")]
                    value: dialog.window.penBarRight ? 1 : 0
                    onPicked: (index) => dialog.window.penBarRight = index === 1
                }
                Heading {
                    visible: Qt.platform.os !== "android"
                    text: qsTr("Drawing")
                }
                Toggle {
                    objectName: "softwareRendering"
                    visible: Qt.platform.os !== "android"
                    label: qsTr("Draw without the graphics card")
                    hint: qsTr("Slower, but right where a graphics driver shows Ductus garbled. Takes effect when Ductus starts again.")
                    checked: AppController.softwareRendering
                    onToggled: (on) => AppController.softwareRendering = on
                }
            }

            Section {
                Heading { text: qsTr("Library folder") }
                Described {
                    label: AppController.libraryDisplayPath
                    hint: qsTr("Your notebooks are ordinary folders and files here. Sync this folder with Nextcloud, Syncthing or similar to use it on several devices.")
                }
                Flow {
                    Layout.fillWidth: true
                    spacing: 6
                    RibbonButton {
                        objectName: "settingsChangeLibrary"
                        iconName: "folder-open"
                        text: qsTr("Choose another folder…")
                        onClicked: dialog.openFolderRequested()
                    }
                    RibbonButton {
                        objectName: "settingsShowLibrary"
                        visible: Qt.platform.os !== "android"
                        iconName: "export"
                        text: qsTr("Show in file manager")
                        onClicked: AppController.showInFolder("")
                    }
                }
                // Android: inside the app the notes go with it when it is uninstalled.
                Described {
                    visible: AppController.notesInsideApp
                    label: qsTr("These notes are inside Ductus")
                    hint: qsTr("Uninstalling the app removes them, and backups and sync apps can't reach them. Documents/Ductus keeps them safe; Ductus needs access to your files for it.")
                }
                RibbonButton {
                    objectName: "settingsSharedStorage"
                    visible: AppController.notesInsideApp
                    iconName: "folder"
                    text: qsTr("Move them to Documents")
                    onClicked: AppController.useSharedStorage()
                }
                Heading { text: qsTr("Saving") }
                Choice {
                    id: autosave
                    readonly property var delays: [1500, 10000, 60000, 0]
                    objectName: "settingsAutosave"
                    label: qsTr("Save automatically")
                    hint: AppController.autosaveDelay > 0
                          ? qsTr("After you stop writing, and when you switch pages or quit.")
                          : qsTr("Only with Ctrl+S, and when you switch pages or quit. The title shows * while changes aren't saved.")
                    options: [qsTr("1.5 s"), qsTr("10 s"), qsTr("1 min"), qsTr("Off")]
                    value: Math.max(0, delays.indexOf(AppController.autosaveDelay))
                    onPicked: (index) => AppController.autosaveDelay = delays[index]
                }
                Described {
                    label: qsTr("Versions and trash")
                    hint: qsTr("Earlier versions of your pages and deleted items stay on this device, in Ductus' own data folder, so the library folder holds only your notes. Restore them with File › Versions and File › Trash.")
                }
                Flow {
                    Layout.fillWidth: true
                    spacing: 6
                    RibbonButton {
                        objectName: "settingsTrash"
                        iconName: "delete"
                        text: qsTr("Trash…")
                        onClicked: dialog.trashRequested()
                    }
                    RibbonButton {
                        objectName: "settingsShowData"
                        visible: Qt.platform.os !== "android"
                        iconName: "folder-open"
                        text: qsTr("Show data folder")
                        onClicked: AppController.showLibraryData()
                    }
                }
            }

            Section {
                id: handwritingSection
                Heading { text: qsTr("Recognizer") }
                Described {
                    label: qsTr("Turns handwriting into text or formulas.")
                    hint: qsTr("Select handwriting with the lasso and choose Text or Formula, or use Draw › Page to text.")
                }
                Choice {
                    objectName: "hwrBackend"
                    label: qsTr("Recognizer")
                    readonly property var kinds: ["builtin", "server"]
                    options: [qsTr("Built-in"), qsTr("Vision model server")]
                    value: Math.max(0, kinds.indexOf(hwr.backend))
                    onPicked: (index) => hwr.backend = kinds[index]
                }
                Choice {
                    objectName: "hwrProcessor"
                    visible: hwr.backend !== "server"
                    label: qsTr("Processor use")
                    hint: qsTr("How much of the processor reading handwriting may take. Less keeps the device cool and quiet; more reads faster.")
                    readonly property var shares: [25, 50, 80, 100]
                    options: shares.map(n => qsTr("%1%").arg(n))
                    value: Math.max(0, shares.indexOf(hwr.processor))
                    onPicked: (index) => hwr.processor = shares[index]
                }
                ColumnLayout {
                    id: langs
                    Layout.fillWidth: true
                    spacing: 6
                    readonly property var chosen: Models.chosenLanguages(hwr.lang).split(",")
                    readonly property string query: langSearch.text.trim().toLowerCase()

                    function toggle(code) {
                        const next = chosen.includes(code) ? chosen.filter(c => c !== code) : chosen.concat([code])
                        if (next.length > 0)
                            hwr.lang = Models.languages.filter(l => next.includes(l.code)).map(l => l.code).join(",")
                    }

                    Described {
                        label: qsTr("Languages")
                        hint: qsTr("Check the languages you write in. Each line is read in all of them and the clearest reading is kept; fewer languages read faster.")
                    }
                    Label {
                        Layout.fillWidth: true
                        text: Models.languages.filter(l => langs.chosen.includes(l.code)).map(l => l.name).join(", ")
                        color: Theme.accent
                        font.pixelSize: 12
                        wrapMode: Text.WordWrap
                    }
                    TextField {
                        id: langSearch
                        objectName: "hwrLangSearch"
                        Layout.fillWidth: true
                        placeholderText: qsTr("Search languages")
                        color: Theme.text
                        placeholderTextColor: Theme.textDisabled
                        font.pixelSize: 13
                        selectByMouse: true
                        background: Rectangle {
                            radius: 6
                            color: Theme.dark ? "#24222c" : "#f6f5f9"
                            border.width: 1
                            border.color: langSearch.activeFocus ? Theme.accent : Theme.separator
                        }
                    }
                    ListView {
                        objectName: "hwrLangList"
                        Layout.fillWidth: true
                        Layout.preferredHeight: Math.min(contentHeight, 200)
                        clip: true
                        boundsBehavior: Flickable.StopAtBounds
                        ScrollBar.vertical: ScrollBar {}
                        // Chosen languages first; the search matches English and own names.
                        model: Models.languages
                            .filter(l => langs.query.length === 0 || l.name.toLowerCase().includes(langs.query)
                                         || l.code === langs.query)
                            .sort((a, b) => langs.chosen.includes(b.code) - langs.chosen.includes(a.code)
                                            || a.name.localeCompare(b.name))
                        delegate: AbstractButton {
                            id: langRow
                            required property var modelData
                            readonly property bool picked: langs.chosen.includes(modelData.code)
                            objectName: "hwrLang_" + modelData.code
                            width: ListView.view.width
                            height: 32
                            // One language at least stays.
                            enabled: !(picked && langs.chosen.length === 1)
                            onClicked: langs.toggle(modelData.code)
                            contentItem: RowLayout {
                                spacing: 10
                                Rectangle {
                                    Layout.leftMargin: 6
                                    implicitWidth: 16
                                    implicitHeight: 16
                                    radius: 4
                                    color: langRow.picked ? Theme.accent : "transparent"
                                    border.width: 1
                                    border.color: langRow.picked ? Theme.accent : Theme.separator
                                    opacity: langRow.enabled ? 1 : 0.6
                                    Label {
                                        anchors.centerIn: parent
                                        visible: langRow.picked
                                        text: "✓"
                                        color: "white"
                                        font.pixelSize: 11
                                    }
                                }
                                Label {
                                    Layout.fillWidth: true
                                    text: langRow.modelData.name
                                    color: Theme.text
                                    font.pixelSize: 13
                                    elide: Text.ElideRight
                                }
                            }
                            background: Rectangle {
                                radius: Theme.radius
                                color: langRow.hovered ? Theme.hover : "transparent"
                            }
                        }
                    }
                }
                Item {
                    id: modelsAnchor
                    Layout.fillWidth: true
                }
                Repeater {
                    model: Models.entries
                    delegate: RowLayout {
                        id: modelRow
                        required property var modelData
                        readonly property bool loading: Models.busy === modelData.id
                        // The readers for the chosen languages, formulas, and whatever is already here.
                        visible: hwr.backend !== "server"
                                 && (modelData.id === "math" || modelData.installed
                                     || Models.textModels(langs.chosen.join(",")).includes(modelData.id))
                        Layout.fillWidth: true
                        spacing: 12
                        Described {
                            label: modelRow.modelData.title
                            hint: Math.round(modelRow.modelData.size / 1e6) + " MB"
                                  + (modelRow.modelData.installed ? " · " + qsTr("installed") : "")
                        }
                        // The bar below has Cancel while it downloads.
                        RibbonButton {
                            objectName: "model_" + modelRow.modelData.id
                            visible: !modelRow.loading
                            text: modelRow.modelData.installed ? qsTr("Remove") : qsTr("Download")
                            enabled: Models.busy === ""
                            onClicked: modelRow.modelData.installed ? removeModelDialog.ask(modelRow.modelData)
                                : Models.download(modelRow.modelData.id)
                        }
                    }
                }
                TransferProgress {
                    objectName: "modelProgress"
                    visible: hwr.backend !== "server" && Models.busy !== ""
                    Layout.fillWidth: true
                    label: {
                        const model = Models.entries.find(e => e.id === Models.busy)
                        return qsTr("Downloading “%1”…").arg(model ? model.title : Models.busy)
                    }
                    value: Models.progress
                    onCancelled: Models.cancel()
                }
                Label {
                    visible: hwr.backend !== "server" && Models.error.length > 0
                    Layout.fillWidth: true
                    text: Models.error
                    color: Theme.danger
                    font.pixelSize: 12
                    wrapMode: Text.WordWrap
                }
                Field {
                    objectName: "hwrUrl"
                    visible: hwr.backend === "server"
                    label: qsTr("Server")
                    hint: qsTr("An OpenAI-compatible API: Ollama, LM Studio, llama.cpp or a cloud service.")
                    placeholder: "http://localhost:11434/v1"
                    text: hwr.url
                    onEdited: (text) => hwr.url = text
                }
                Field {
                    objectName: "hwrModel"
                    visible: hwr.backend === "server"
                    label: qsTr("Model")
                    hint: qsTr("A model that reads images, e.g. qwen2.5vl.")
                    placeholder: "qwen2.5vl"
                    text: hwr.model
                    onEdited: (text) => hwr.model = text
                }
                Field {
                    objectName: "hwrKey"
                    visible: hwr.backend === "server"
                    label: qsTr("API key")
                    hint: qsTr("Only for services that ask for one.")
                    secret: true
                    text: hwr.key
                    onEdited: (text) => hwr.key = text
                }
                Described {
                    visible: hwr.backend === "server"
                    readonly property bool local: hwr.url.length === 0
                        || /^https?:\/\/(localhost|127\.|192\.168\.|10\.|\[::1\])/.test(hwr.url)
                    label: local ? qsTr("Your handwriting stays on your computer or network.")
                                 : qsTr("Your handwriting is sent to this server.")
                    hint: local ? ""
                                : qsTr("Line images leave your computer. Use a cloud service only if you are fine with that.")
                }

                Heading { text: qsTr("Search") }
                Toggle {
                    objectName: "hwrSearch"
                    label: qsTr("Read handwriting for search")
                    hint: qsTr("After a page is saved, the recognizer above reads its new lines of writing, so search finds them. The text is kept with the page and only read again when the ink changes.")
                    checked: hwr.search
                    onToggled: (on) => hwr.search = on
                }
                RowLayout {
                    visible: hwr.search
                    spacing: 12
                    RibbonButton {
                        objectName: "hwrNow"
                        iconName: "pen"
                        text: qsTr("Read this page now")
                        onClicked: AppController.recognizeNow()
                    }
                    Label {
                        Layout.fillWidth: true
                        text: AppController.recognitionStatus
                        color: Theme.textMuted
                        font.pixelSize: 12
                        wrapMode: Text.WordWrap
                    }
                }
            }

            Section {
                Heading { text: qsTr("Ductus") }
                Described {
                    objectName: "aboutVersion"
                    label: qsTr("Version %1").arg(Qt.application.version)
                    hint: qsTr("Handwritten notebooks · GPL-3.0-or-later")
                }
                Heading { text: qsTr("Updates") }
                Described {
                    objectName: "updateInfo"
                    label: Updater.channel === "none" ? qsTr("Development build: no updates")
                         : Updater.status.length > 0 ? Updater.status
                         : qsTr("Not checked yet")
                    hint: ({ flatpak: qsTr("Installed with Flatpak: updates come from the Ductus repository."),
                             installer: qsTr("Updates download and install themselves after you agree."),
                             android: qsTr("Updates download after you agree; Android asks before installing them."),
                             manual: qsTr("New versions are on the download page."),
                             none: "" })[Updater.channel]
                }
                TransferProgress {
                    objectName: "settingsUpdateProgress"
                    visible: Updater.downloading
                    Layout.fillWidth: true
                    value: Updater.progress
                    onCancelled: Updater.cancel()
                }
                Flow {
                    visible: Updater.channel !== "none"
                    Layout.fillWidth: true
                    spacing: 6
                    RibbonButton {
                        objectName: "settingsCheckUpdates"
                        iconName: "update"
                        text: qsTr("Check now")
                        enabled: !Updater.busy
                        onClicked: Updater.check()
                    }
                    RibbonButton {
                        objectName: "settingsInstallUpdate"
                        visible: Updater.available
                        text: qsTr("Update to %1…").arg(Updater.latestVersion)
                        onClicked: dialog.updateRequested()
                    }
                }
                Toggle {
                    objectName: "settingsAutoUpdate"
                    visible: Updater.channel !== "none"
                    label: qsTr("Look for updates automatically")
                    hint: qsTr("When Ductus starts and once a day, offering each new version once. Asks GitHub for the newest release; nothing else is sent.")
                    checked: Updater.autoCheck
                    onToggled: (on) => Updater.autoCheck = on
                }
            }
        }
    }
}

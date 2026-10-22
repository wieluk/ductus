import QtCore
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Ductus

Dialog {
    id: dialog

    required property InkCanvas canvas
    required property ApplicationWindow window
    property int section: 0

    signal openFolderRequested()
    signal updateRequested()

    objectName: "settingsDialog"
    title: qsTr("Settings")
    anchors.centerIn: parent
    modal: true
    padding: 0
    width: Math.min(780, window.width - 40)
    height: Math.min(580, window.height - 60)

    Overlay.modal: Rectangle { color: Theme.dark ? "#99000000" : "#55000000" }

    background: Rectangle {
        color: Theme.popup
        radius: 12
        border.width: 1
        border.color: Theme.ribbonBorder
    }

    // AppController reads these keys too (pages/, hwr/).
    Settings {
        id: hwr
        category: "hwr"
        property string backend: "off"
        property string url: ""
        property string model: ""
        property string key: ""
        property string lang: "auto"
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

    component Choice: RowLayout {
        id: choice
        property string label
        property string hint
        property var options: []
        property int value
        signal picked(int index)
        Layout.fillWidth: true
        spacing: 2
        Described {
            label: choice.label
            hint: choice.hint
            Layout.rightMargin: 12
        }
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

    component Field: RowLayout {
        id: fieldRow
        property string label
        property string hint
        property alias text: input.text
        property alias placeholder: input.placeholderText
        property bool secret: false
        signal edited(string text)
        Layout.fillWidth: true
        spacing: 12
        Described {
            label: fieldRow.label
            hint: fieldRow.hint
        }
        TextField {
            id: input
            objectName: fieldRow.objectName + "Input"
            Layout.preferredWidth: 240
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

    contentItem: RowLayout {
        spacing: 0

        ListView {
            id: sections
            objectName: "settingsSections"
            Layout.preferredWidth: 190
            Layout.fillHeight: true
            Layout.leftMargin: 10
            Layout.bottomMargin: 12
            interactive: false
            spacing: 2
            model: [qsTr("Pen & input"), qsTr("New pages"), qsTr("Appearance"),
                    qsTr("Library"), qsTr("Handwriting"), qsTr("About")]
            delegate: AbstractButton {
                id: entry
                required property string modelData
                required property int index
                objectName: "settingsSection" + index
                width: ListView.view.width
                height: 36
                onClicked: dialog.section = index
                contentItem: Label {
                    leftPadding: 12
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
            Layout.preferredWidth: 1
            Layout.fillHeight: true
            Layout.bottomMargin: 12
            color: Theme.separator
        }

        StackLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.leftMargin: 20
            Layout.rightMargin: 20
            Layout.bottomMargin: 16
            currentIndex: dialog.section

            Section {
                Heading { text: qsTr("Writing") }
                Choice {
                    objectName: "pressure"
                    label: qsTr("Pressure")
                    hint: qsTr("How hard you press for a bold line.")
                    options: [qsTr("Light"), qsTr("Normal"), qsTr("Firm")]
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
                Heading { text: qsTr("Around the page") }
                Toggle {
                    objectName: "sidebar"
                    label: qsTr("Notebook list")
                    hint: qsTr("The sidebar with subjects and pages (Ctrl+\\).")
                    checked: dialog.window.sidebarVisible
                    onToggled: (on) => dialog.window.sidebarVisible = on
                }
                Toggle {
                    objectName: "penBar"
                    label: qsTr("Pen bar")
                    hint: qsTr("Your favourite pens in a column at the side of the page.")
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
            }

            Section {
                Heading { text: qsTr("Library folder") }
                Described {
                    label: AppController.libraryPath
                    hint: qsTr("Your notebooks are ordinary folders and files here. Sync this folder with Nextcloud, Syncthing or similar to use it on several devices.")
                }
                RowLayout {
                    spacing: 6
                    RibbonButton {
                        objectName: "settingsChangeLibrary"
                        iconName: "folder-open"
                        text: qsTr("Choose another folder…")
                        onClicked: dialog.openFolderRequested()
                    }
                    RibbonButton {
                        objectName: "settingsShowLibrary"
                        iconName: "export"
                        text: qsTr("Show in file manager")
                        onClicked: AppController.showInFolder("")
                    }
                }
                Heading { text: qsTr("Saving") }
                Described {
                    label: qsTr("Pages save automatically")
                    hint: qsTr("1.5 seconds after you stop writing, when you switch pages and when you quit. Earlier versions are kept in .ductus/history, and deleted items in .ductus/trash inside the library.")
                }
            }

            Section {
                Heading { text: qsTr("Reading handwriting") }
                Described {
                    label: qsTr("Search finds handwriting once it has been read.")
                    hint: qsTr("After a page is saved, its new lines of writing are sent, one image per line, to the recognizer chosen here. The text is kept with the page and only read again when the ink changes.")
                }
                Choice {
                    objectName: "hwrBackend"
                    label: qsTr("Recognizer")
                    readonly property var kinds: ["off", "server"]
                    options: [qsTr("Off"), qsTr("Vision model server")]
                    value: kinds.indexOf(hwr.backend)
                    onPicked: (index) => hwr.backend = kinds[index]
                }
                Field {
                    objectName: "hwrUrl"
                    visible: hwr.backend !== "off"
                    label: qsTr("Server")
                    hint: qsTr("An OpenAI-compatible API: Ollama, LM Studio, llama.cpp or a cloud service.")
                    placeholder: "http://localhost:11434/v1"
                    text: hwr.url
                    onEdited: (text) => hwr.url = text
                }
                Field {
                    objectName: "hwrModel"
                    visible: hwr.backend !== "off"
                    label: qsTr("Model")
                    hint: qsTr("A model that reads images, e.g. qwen2.5vl.")
                    placeholder: "qwen2.5vl"
                    text: hwr.model
                    onEdited: (text) => hwr.model = text
                }
                Field {
                    objectName: "hwrKey"
                    visible: hwr.backend !== "off"
                    label: qsTr("API key")
                    hint: qsTr("Only for services that ask for one.")
                    secret: true
                    text: hwr.key
                    onEdited: (text) => hwr.key = text
                }
                Choice {
                    objectName: "hwrLang"
                    visible: hwr.backend !== "off"
                    label: qsTr("Language")
                    readonly property var langs: ["auto", "de", "en"]
                    options: [qsTr("Either"), qsTr("German"), qsTr("English")]
                    value: langs.indexOf(hwr.lang)
                    onPicked: (index) => hwr.lang = langs[index]
                }
                Described {
                    visible: hwr.backend !== "off"
                    readonly property bool local: hwr.url.length === 0
                        || /^https?:\/\/(localhost|127\.|192\.168\.|10\.|\[::1\])/.test(hwr.url)
                    label: local ? qsTr("Your handwriting stays on your computer or network.")
                                 : qsTr("Your handwriting is sent to this server.")
                    hint: local ? ""
                                : qsTr("Line images leave your computer. Use a cloud service only if you are fine with that.")
                }
                RowLayout {
                    visible: hwr.backend !== "off"
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
                             android: qsTr("New versions download in your browser; open them to install."),
                             manual: qsTr("New versions are on the download page."),
                             none: "" })[Updater.channel]
                }
                RowLayout {
                    visible: Updater.channel !== "none"
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
                    hint: qsTr("When Ductus starts and once a day. Asks GitHub for the newest release; nothing else is sent.")
                    checked: Updater.autoCheck
                    onToggled: (on) => Updater.autoCheck = on
                }
                Heading { text: qsTr("Support") }
                Described {
                    objectName: "pdfStatus"
                    label: qsTr("PDF support")
                    hint: AppController.pdfProblem.length > 0 ? AppController.pdfProblem
                                                             : qsTr("Ready (pdfium)")
                }
            }
        }
    }
}

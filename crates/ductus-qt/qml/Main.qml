import QtCore
import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import QtQuick.Layouts
import Ductus

ApplicationWindow {
    id: window

    /// "system", "light" or "dark".
    property string themeMode: "system"
    property bool darkMode: false
    property bool sidebarVisible: true
    property int sidebarWidth: Theme.sidebarWidth
    property bool penBarVisible: true
    property bool penBarRight: false
    property bool sidebarRight: false
    property alias currentTab: titleBar.currentTab

    // Small tablet screens (or large scaling) get a window that fits above the taskbar.
    width: Math.min(1280, Screen.desktopAvailableWidth)
    height: Math.min(820, Screen.desktopAvailableHeight)
    minimumWidth: 480
    minimumHeight: 320
    visible: true
    // Only without autosave: otherwise it would flash after every stroke.
    readonly property string pageTitle: (AppController.unsaved && AppController.autosaveDelay === 0 ? "* " : "")
                                        + AppController.currentTitle
    title: pageTitle.length > 0 ? pageTitle + " - Ductus" : "Ductus"
    // Our own OneNote-style title bar holds the ribbon tabs.
    flags: Qt.Window | Qt.FramelessWindowHint
    color: Theme.ribbon
    // Standard controls (fields, lists, check boxes, tooltips) follow the theme, light or dark.
    palette.window: Theme.popup
    palette.windowText: Theme.text
    palette.base: Theme.field
    palette.alternateBase: Theme.hover
    palette.text: Theme.text
    palette.placeholderText: Theme.textMuted
    palette.button: Theme.hover
    palette.buttonText: Theme.text
    palette.highlight: Theme.accent
    palette.highlightedText: Theme.dark ? Theme.titleBar : "#ffffff"
    palette.light: Theme.popup
    palette.midlight: Theme.hover
    palette.mid: Theme.separator
    palette.dark: Theme.textMuted
    palette.toolTipBase: Theme.popup
    palette.toolTipText: Theme.text

    function updateTheme() {
        darkMode = themeMode === "dark"
            || (themeMode === "system" && Qt.styleHints.colorScheme === Qt.ColorScheme.Dark)
    }

    onThemeModeChanged: updateTheme()
    onDarkModeChanged: Theme.dark = darkMode
    Component.onCompleted: {
        updateTheme()
        Theme.dark = darkMode
        AppController.canvas = canvas
    }

    Connections {
        target: Qt.styleHints
        function onColorSchemeChanged() { window.updateTheme() }
    }

    property int visibilityBeforeScreenshot: Window.Windowed

    function takeScreenshot() {
        if (!canvas.picturesAllowed) {
            toast.show(qsTr("Pictures can't be added to a PDF you write on directly yet."), true)
            return
        }
        visibilityBeforeScreenshot = visibility
        showMinimized()
        // Give the window manager time to hide us before the capture starts.
        screenshotDelay.start()
    }

    function restoreAfterScreenshot() {
        if (visibility === Window.Minimized) {
            if (visibilityBeforeScreenshot === Window.FullScreen) showFullScreen()
            else if (visibilityBeforeScreenshot === Window.Maximized) showMaximized()
            else showNormal()
        }
        raise()
        requestActivate()
    }

    Timer {
        id: screenshotDelay
        interval: 400
        onTriggered: AppController.takeScreenshot()
    }

    function toggleFullScreen() {
        if (visibility === Window.FullScreen)
            showNormal()
        else
            showFullScreen()
    }

    Settings {
        category: "ui"
        property alias themeMode: window.themeMode
        property alias sidebarVisible: window.sidebarVisible
        property alias sidebarWidth: window.sidebarWidth
        property alias penBarRight: window.penBarRight
        property alias sidebarRight: window.sidebarRight
        property alias holdDelay: canvas.holdDelay
        property alias palmRejection: canvas.palmRejection
        property alias penTipVisible: canvas.penTipVisible
        property alias fingerDrawing: canvas.fingerDrawing
        property alias penColor: canvas.penColor
        property alias penWidth: canvas.penWidth
        property alias penPattern: canvas.penPattern
        property alias highlighterColor: canvas.highlighterColor
        property alias highlighterWidth: canvas.highlighterWidth
        property alias eraserWholeStrokes: canvas.eraserWholeStrokes
        property alias eraserSize: canvas.eraserSize
        property alias shapeAssist: canvas.shapeAssist
        property alias penBarVisible: window.penBarVisible
        property alias penPresets: penBar.presetsJson
        property alias currentTab: titleBar.currentTab
        property alias mathSyntax: mathLayer.syntax
        property alias mathFavorites: mathLayer.favoritesJson
        property alias mathCategory: mathLayer.category
        property alias pressureResponse: canvas.pressureResponse
        property alias smoothing: canvas.smoothing
    }

    Connections {
        target: AppController
        function onErrorOccurred(message) { toast.show(message, true) }
        function onNotice(message) { toast.show(message, false) }
        function onTrashed(name, trashId) {
            toast.show(qsTr("\u201c%1\u201d is in the trash.").arg(name), false, qsTr("Undo"),
                       () => AppController.restoreFromTrash(trashId))
        }
    }
    Connections {
        target: canvas
        function onNotice(message, error) { toast.show(message, error) }
    }
    Connections {
        target: AppController
        function onModelsNeeded(id) {
            settingsDialog.section = 4
            settingsDialog.open()
            const model = Models.entries.find(e => e.id === id)
            toast.show(model ? qsTr("Download \u201c%1\u201d first, then try again.").arg(model.title)
                             : qsTr("Download the recognizer for this first."), false)
        }
    }
    Connections {
        target: Updater
        function onUpdateFound() {
            toast.show(qsTr("Ductus %1 is available: Update is in the title bar.").arg(Updater.latestVersion), false)
        }
    }

    UpdateDialog {
        id: updateDialog
        parent: Overlay.overlay
    }
    Connections {
        target: AppController
        function onScreenshotTaken(file) {
            window.restoreAfterScreenshot()
            canvas.insertImageFile(file)
        }
        function onScreenshotCancelled() { window.restoreAfterScreenshot() }
        function onScreenshotFailed(message) {
            window.restoreAfterScreenshot()
            toast.show(message, true)
        }
    }

    Shortcut { sequences: [StandardKey.Undo]; onActivated: canvas.undo() }
    // Some platforms' standard keys already include the extra one; Qt then
    // reports the key as ambiguous instead of activating.
    Shortcut {
        sequences: [StandardKey.Redo, "Ctrl+Y"]
        onActivated: canvas.redo()
        onActivatedAmbiguously: canvas.redo()
    }
    Shortcut { sequence: "Ctrl+0"; onActivated: canvas.resetView() }
    Shortcut {
        sequences: [StandardKey.ZoomIn, "Ctrl+="]
        onActivated: canvas.zoomBy(1.25)
        onActivatedAmbiguously: canvas.zoomBy(1.25)
    }
    Shortcut { sequences: [StandardKey.ZoomOut]; onActivated: canvas.zoomBy(0.8) }
    Shortcut { sequence: "F11"; onActivated: window.toggleFullScreen() }
    Shortcut { sequence: "Ctrl+N"; onActivated: newPageDialog.show("") }
    Shortcut { sequence: "Ctrl+\\"; onActivated: window.sidebarVisible = !window.sidebarVisible }
    Shortcut { sequences: [StandardKey.Open]; onActivated: openFileDialog.open() }
    Shortcut {
        sequences: [StandardKey.Find, "Ctrl+K"]
        onActivated: titleBar.focusSearch()
        onActivatedAmbiguously: titleBar.focusSearch()
    }
    Shortcut { sequences: [StandardKey.Save]; onActivated: AppController.saveNow() }
    Shortcut { sequence: "Ctrl+W"; onActivated: AppController.closePage() }
    // Not StandardKey.Quit: on Linux it is Ctrl+Q too, and a key listed twice never fires.
    Shortcut { sequence: "Ctrl+Q"; onActivated: Qt.quit() }
    Shortcut { sequences: [StandardKey.Print]; onActivated: AppController.print() }
    // Selection editing; text fields keep these keys while they have focus.
    Shortcut { sequences: [StandardKey.Copy]; onActivated: canvas.copySelection() }
    Shortcut { sequences: [StandardKey.Cut]; onActivated: canvas.cutSelection() }
    Shortcut { sequences: [StandardKey.Paste]; onActivated: canvas.paste() }
    Shortcut { sequences: [StandardKey.SelectAll]; onActivated: canvas.selectAll() }
    Shortcut {
        sequences: [StandardKey.Delete, "Backspace"]
        enabled: canvas.selectionCount > 0
        onActivated: canvas.deleteSelection()
        onActivatedAmbiguously: canvas.deleteSelection()
    }
    Shortcut {
        sequence: "Escape"
        enabled: canvas.selectionCount > 0
        onActivated: canvas.clearSelection()
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        TitleBar {
            id: titleBar
            Layout.fillWidth: true
            window: window
            canvas: canvas
            pageTitle: window.pageTitle
            visible: window.visibility !== Window.FullScreen
            onAboutRequested: aboutDialog.open()
            onNewPageRequested: newPageDialog.show("")
            onNewNotebookRequested: newNotebookDialog.show(AppController.treeFocus)
            onHistoryRequested: historyDialog.show(AppController.currentPage)
            onTrashRequested: trashDialog.show()
            onSettingsRequested: settingsDialog.open()
            onExportRequested: exportDialog.show()
            onOpenFolderRequested: folderDialog.open()
            onOpenFileRequested: openFileDialog.open()
            onPrintRequested: AppController.print()
            onSidebarToggled: window.sidebarVisible = !window.sidebarVisible
            onUpdateRequested: updateDialog.open()
        }

        Ribbon {
            Layout.fillWidth: true
            canvas: canvas
            window: window
            tab: titleBar.currentTab
            onPdfImportRequested: pdfFileDialog.open()
            onImageInsertRequested: imageFileDialog.open()
            onOpenFileRequested: openFileDialog.open()
            onOpenFolderRequested: folderDialog.open()
            onExportRequested: exportDialog.show()
            onPrintRequested: AppController.print()
            onSettingsRequested: settingsDialog.open()
            onAboutRequested: aboutDialog.open()
            onNewPageRequested: newPageDialog.show("")
            onNewNotebookRequested: newNotebookDialog.show(AppController.treeFocus)
            onHistoryRequested: historyDialog.show(AppController.currentPage)
            onTrashRequested: trashDialog.show()
        }

        RowLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            spacing: 0
            layoutDirection: window.sidebarRight ? Qt.RightToLeft : Qt.LeftToRight

            NotebookTree {
                id: tree
                objectName: "notebookTree"
                Layout.fillHeight: true
                Layout.preferredWidth: Math.min(window.sidebarWidth, window.width * 0.6)
                visible: window.sidebarVisible && window.visibility !== Window.FullScreen
                onRight: window.sidebarRight

                MouseArea {
                    objectName: "sidebarResize"
                    property real startX
                    property int startWidth
                    // On the edge next to the page.
                    x: window.sidebarRight ? 0 : parent.width - width
                    height: parent.height
                    width: 6
                    cursorShape: Qt.SizeHorCursor
                    onPressed: (mouse) => {
                        startX = mapToItem(null, mouse.x, 0).x
                        startWidth = tree.width
                    }
                    onPositionChanged: (mouse) => {
                        const moved = mapToItem(null, mouse.x, 0).x - startX
                        const grown = window.sidebarRight ? -moved : moved
                        window.sidebarWidth = Math.round(Math.max(200, Math.min(640, startWidth + grown)))
                    }
                }
                onDeleteRequested: (path, name, isFolder) => {
                    deleteDialog.path = path
                    deleteDialog.name = name
                    deleteDialog.isFolder = isFolder
                    deleteDialog.open()
                }
                onOpenFolderRequested: folderDialog.open()
                onNewPageRequested: (folder) => newPageDialog.show(folder)
                onNewNotebookRequested: (location) => newNotebookDialog.show(location)
                onMakeNotebookRequested: (folder) => newNotebookDialog.showForFolder(folder)
                onMoveRequested: (path, name, isFolder) => moveDialog.show(path, name, isFolder)
                onHistoryRequested: (path) => historyDialog.show(path)
            }

            ColumnLayout {
                Layout.fillWidth: true
                Layout.fillHeight: true
                spacing: 0

                LocationBar {
                    objectName: "locationBar"
                    Layout.fillWidth: true
                    visible: AppController.currentPage.length > 0
                }

                Item {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    clip: true

                    InkCanvas {
                        id: canvas
                        objectName: "canvas"
                        anchors.fill: parent
                        deskColor: Theme.desk
                        enabled: AppController.currentPage.length > 0
                    }
                    // Plain desk instead of an empty page that can't be written on.
                    Rectangle {
                        anchors.fill: parent
                        color: Theme.desk
                        visible: !canvas.enabled
                    }

                    ColumnLayout {
                        objectName: "noPage"
                        anchors.centerIn: parent
                        width: Math.min(360, parent.width - 40)
                        visible: !canvas.enabled
                        spacing: 12
                        Label {
                            Layout.fillWidth: true
                            horizontalAlignment: Text.AlignHCenter
                            wrapMode: Text.WordWrap
                            text: qsTr("No page open")
                            color: Theme.text
                            font.pixelSize: 18
                            font.weight: Font.DemiBold
                        }
                        Label {
                            Layout.fillWidth: true
                            horizontalAlignment: Text.AlignHCenter
                            wrapMode: Text.WordWrap
                            text: qsTr("Pick a page in the notebook list, or start a new one.")
                            color: Theme.textMuted
                            font.pixelSize: 13
                        }
                        RibbonButton {
                            objectName: "noPageNewPage"
                            Layout.alignment: Qt.AlignHCenter
                            iconName: "page-add"
                            text: qsTr("New page…")
                            onClicked: newPageDialog.show("")
                        }
                    }

                    TextLayer {
                        id: textLayer
                        objectName: "textLayer"
                        anchors.fill: canvas
                        canvas: canvas
                        onNoticeRequested: (message) => toast.show(message, true)
                    }

                    MathLayer {
                        id: mathLayer
                        objectName: "mathLayer"
                        anchors.fill: canvas
                        canvas: canvas
                        onNoticeRequested: (message) => toast.show(message, true)
                    }

                    PenBar {
                        id: penBar
                        objectName: "penBar"
                        canvas: canvas
                        onRight: window.penBarRight
                        onScreenshotRequested: window.takeScreenshot()
                        onPictureRequested: imageFileDialog.open()
                        visible: window.penBarVisible && canvas.enabled
                        x: window.penBarRight ? parent.width - width - 10 : 10
                        anchors.verticalCenter: parent.verticalCenter
                        height: Math.min(implicitHeight, parent.height - 20)
                        clip: true
                    }

                    SelectionBar {
                        objectName: "selectionBar"
                        canvas: canvas
                        // Beside the pen bar, never over it.
                        leftInset: penBar.visible && !window.penBarRight ? penBar.x + penBar.width + 8 : 8
                        rightInset: penBar.visible && window.penBarRight ? parent.width - penBar.x + 8 : 8
                    }

                    Toast {
                        id: toast
                        anchors.horizontalCenter: parent.horizontalCenter
                        anchors.bottom: parent.bottom
                        anchors.bottomMargin: 24
                    }
                }
            }
        }
    }

    ResizeHandles {
        anchors.fill: parent
        window: window
        visible: window.visibility === Window.Windowed
    }

    FolderDialog {
        id: folderDialog
        title: qsTr("Choose a library folder")
        currentFolder: "file://" + AppController.libraryPath
        onAccepted: AppController.openLibrary(selectedFolder.toString())
    }

    FileDialog {
        id: imageFileDialog
        title: qsTr("Add a picture")
        nameFilters: [qsTr("Pictures (*.png *.jpg *.jpeg *.webp *.gif *.bmp *.PNG *.JPG *.JPEG)"),
                      qsTr("All files (*)")]
        onAccepted: canvas.insertImageFile(selectedFile.toString())
    }

    FileDialog {
        id: openFileDialog
        title: qsTr("Open")
        nameFilters: [qsTr("Notes pages and PDFs (*.svg *.pdf *.PDF)"), qsTr("All files (*)")]
        onAccepted: {
            if (AppController.openFile(selectedFile.toString()) === "pdf") {
                pdfModeDialog.file = selectedFile.toString()
                pdfModeDialog.open()
            }
        }
    }

    FileDialog {
        id: pdfFileDialog
        title: qsTr("Choose a PDF")
        nameFilters: [qsTr("PDF documents (*.pdf)")]
        onAccepted: {
            pdfModeDialog.file = selectedFile.toString()
            pdfModeDialog.open()
        }
    }

    // Asked on every import: write on the PDF itself, or on a page over it.
    AppDialog {
        id: pdfModeDialog
        objectName: "pdfModeDialog"

        property string file
        property bool annotate: true

        title: qsTr("How do you want to write on this PDF?")
        acceptText: qsTr("Import")
        onAccepted: AppController.importPdf(file, annotate)

        contentItem: Column {
            spacing: 8
            Repeater {
                model: [
                    { annotate: true, label: qsTr("Write on the PDF"),
                      hint: qsTr("Ink is saved into the PDF as annotations. Other PDF readers show it too.") },
                    { annotate: false, label: qsTr("Import as note pages"),
                      hint: qsTr("The PDF becomes the background of a notes page. The original stays untouched.") }
                ]
                delegate: AbstractButton {
                    id: option
                    required property var modelData
                    readonly property bool chosen: pdfModeDialog.annotate === modelData.annotate
                    objectName: modelData.annotate ? "pdfModeAnnotate" : "pdfModePages"
                    width: 400
                    height: optionText.implicitHeight + 20
                    onClicked: pdfModeDialog.annotate = modelData.annotate
                    background: Rectangle {
                        radius: 8
                        color: option.chosen ? Theme.checked : (option.hovered ? Theme.hover : "transparent")
                        border.width: 1
                        border.color: option.chosen ? Theme.accent : Theme.separator
                    }
                    contentItem: Column {
                        id: optionText
                        padding: 10
                        spacing: 2
                        Label {
                            text: option.modelData.label
                            color: Theme.text
                            font.pixelSize: 13
                            font.weight: Font.DemiBold
                        }
                        Label {
                            width: 380
                            text: option.modelData.hint
                            color: Theme.textMuted
                            font.pixelSize: 12
                            wrapMode: Text.WordWrap
                        }
                    }
                }
            }
        }
    }

    FileDialog {
        id: exportDialog

        property string exportFolder

        // Named after the page, in Documents unless an export went elsewhere before.
        function show() {
            const name = AppController.currentTitle.replace(/[\\/:*?"<>|]/g, "_").trim() || qsTr("Page")
            const folder = exportFolder.length > 0 ? exportFolder
                         : StandardPaths.writableLocation(StandardPaths.DocumentsLocation).toString()
            currentFolder = folder
            selectedFile = folder + "/" + encodeURIComponent(name) + ".pdf"
            open()
        }

        title: qsTr("Export as PDF")
        fileMode: FileDialog.SaveFile
        nameFilters: [qsTr("PDF documents (*.pdf)")]
        defaultSuffix: "pdf"
        onAccepted: {
            exportFolder = currentFolder.toString()
            AppController.exportPdf(selectedFile.toString())
        }
    }

    TrashDialog {
        id: trashDialog
    }

    HistoryDialog {
        id: historyDialog
    }

    MoveDialog {
        id: moveDialog
    }

    AppDialog {
        id: deleteDialog
        objectName: "deleteDialog"

        property string path
        property string name
        property bool isFolder

        title: isFolder ? qsTr("Delete \u201c%1\u201d and everything in it?").arg(name)
                        : qsTr("Delete \u201c%1\u201d?").arg(name)
        message: qsTr("It moves to the trash on this device. Undo it right away, or restore it later from File › Trash.")
        acceptText: qsTr("Delete")
        destructive: true
        acceptButton.objectName: "confirmDelete"
        onAccepted: AppController.remove(path)
    }

    SettingsDialog {
        id: settingsDialog
        canvas: canvas
        window: window
        onOpenFolderRequested: folderDialog.open()
        onUpdateRequested: {
            close()
            updateDialog.open()
        }
        onTrashRequested: {
            close()
            trashDialog.show()
        }
    }

    NewPageDialog { id: newPageDialog }
    NewNotebookDialog { id: newNotebookDialog }

    AppDialog {
        id: aboutDialog
        objectName: "aboutDialog"
        title: qsTr("About Ductus")
        message: qsTr("Handwritten notebooks for Linux, Windows and Android.\nDevelopment preview · GPL-3.0-or-later\n\nLibrary: %1").arg(AppController.libraryPath)
        acceptText: qsTr("Close")
        rejectText: ""
    }
}

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
    property color appColor: Theme.colors[0]
    property bool sidebarVisible: true
    property int sidebarWidth: Theme.sidebarWidth
    property bool penBarVisible: true
    /// Only the page and the pen bar.
    readonly property bool fullScreen: visibility === Window.FullScreen
    property bool penBarRight: false
    property bool sidebarRight: false
    property alias currentTab: titleBar.currentTab
    /// A phone or a narrow window: the notebooks open as a drawer over the page.
    readonly property bool compact: width < 720
    property bool drawerOpen: false
    /// The canvas of the half the ribbon, pen bar and keys act on.
    readonly property InkCanvas canvas: AppController.canvas === canvasB ? canvasB : canvasA
    readonly property InkCanvas idleCanvas: canvas === canvasA ? canvasB : canvasA
    readonly property bool split: paneA.visible && paneB.visible

    function toggleSidebar() {
        if (compact)
            drawerOpen = !drawerOpen
        else
            sidebarVisible = !sidebarVisible
    }

    function showInSidebar(path, view) {
        if (compact)
            drawerOpen = true
        else
            sidebarVisible = true
        if (view === "notebooks")
            AppController.showInNotebooks(path)
        else if (view === "folders")
            AppController.reveal(path)
        else
            AppController.locate(path)
    }

    // Smaller than the screen even on small ones, so restoring from maximized visibly does something.
    // Android shows apps on the whole screen, however large.
    readonly property bool mobile: Qt.platform.os === "android"
    width: mobile ? Screen.desktopAvailableWidth : Math.min(1280, Math.round(Screen.desktopAvailableWidth * 0.9))
    height: mobile ? Screen.desktopAvailableHeight : Math.min(820, Math.round(Screen.desktopAvailableHeight * 0.9))
    /// Saved: a window maximized when Ductus closed opens maximized again.
    property bool maximized: false
    /// The left half's share of a split.
    property real splitRatio: 0.5
    // Showing the window reports "windowed" before the saved value is applied.
    property bool shown: false
    onVisibilityChanged: (state) => {
        if (shown && (state === Window.Maximized || state === Window.Windowed))
            maximized = state === Window.Maximized
    }
    minimumWidth: 480
    minimumHeight: 320
    visible: true
    // Only without autosave: otherwise it would flash after every stroke.
    readonly property string pageTitle: (AppController.unsaved && AppController.autosaveDelay === 0 ? "* " : "")
                                        + AppController.currentTitle
    title: pageTitle.length > 0 ? pageTitle + " - Ductus" : "Ductus"
    // Our own title bar holds the ribbon tabs.
    flags: Qt.Window | Qt.FramelessWindowHint
    color: Theme.ribbon
    // Standard controls follow the theme.
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
    onAppColorChanged: Theme.base = appColor
    Component.onCompleted: {
        updateTheme()
        Theme.dark = darkMode
        Theme.base = appColor
        AppController.otherCanvas = canvasB
        AppController.canvas = canvasA
        // After the settings are read.
        Qt.callLater(() => {
            if (maximized)
                showMaximized()
            shown = true
        })
        if (AppController.storageQuestionPending())
            storageDialog.open()
    }

    Connections {
        target: Qt.styleHints
        function onColorSchemeChanged() { window.updateTheme() }
    }

    property int visibilityBeforeScreenshot: Window.Windowed

    /// A PDF asks how to write on it first.
    function openFile(fileOrUrl) {
        if (AppController.openFile(fileOrUrl) === "pdf") {
            pdfModeDialog.file = fileOrUrl
            pdfModeDialog.open()
        }
    }

    function takeScreenshot() {
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
        // Back to a maximized window if it was one: showNormal() would also restore it.
        if (visibility !== Window.FullScreen)
            showFullScreen()
        else if (maximized)
            showMaximized()
        else
            showNormal()
    }

    Settings {
        category: "ui"
        property alias themeMode: window.themeMode
        property alias appColor: window.appColor
        property alias maximized: window.maximized
        property alias splitRatio: window.splitRatio
        property alias sidebarVisible: window.sidebarVisible
        property alias sidebarWidth: window.sidebarWidth
        property alias penBarRight: window.penBarRight
        property alias sidebarRight: window.sidebarRight
        property alias holdDelay: canvasA.holdDelay
        property alias palmRejection: canvasA.palmRejection
        property alias penButtonTool: canvasA.penButtonTool
        property alias penButton2Tool: canvasA.penButton2Tool
        property alias eraserEndTool: canvasA.eraserEndTool
        property alias penTipVisible: canvasA.penTipVisible
        property alias penTipSize: canvasA.penTipSize
        property alias fingerDrawing: canvasA.fingerDrawing
        property alias penColor: canvasA.penColor
        property alias penWidth: canvasA.penWidth
        property alias penPattern: canvasA.penPattern
        property alias highlighterColor: canvasA.highlighterColor
        property alias highlighterWidth: canvasA.highlighterWidth
        property alias eraserWholeStrokes: canvasA.eraserWholeStrokes
        property alias eraserSize: canvasA.eraserSize
        property alias shapeAssist: canvasA.shapeAssist
        property alias penBarVisible: window.penBarVisible
        property alias penPresets: penBar.presetsJson
        property alias penBarUndoRedo: penBar.undoRedoShown
        property alias currentTab: titleBar.currentTab
        property alias mathSyntax: paneA.mathSyntax
        property alias mathFavorites: paneA.mathFavorites
        property alias mathCategory: paneA.mathCategory
        property alias pressureResponse: canvasA.pressureResponse
        property alias smoothing: canvasA.smoothing
    }

    Connections {
        target: AppController
        function onErrorOccurred(message) { toast.show(message, true) }
        function onOpenRequested(file) { window.openFile(file) }
        function onNotice(message) { toast.show(message, false) }
        function onImportReady(kind, name, pages, sections, location) {
            // "Importing…" is over: the dialog says what was read.
            toast.hide()
            importDialog.show(kind, name, pages, sections, location)
        }
        function onTrashed(name, trashId) {
            toast.show(qsTr("\u201c%1\u201d is in the trash.").arg(name), false, qsTr("Undo"),
                       () => AppController.restoreFromTrash(trashId))
        }
    }
    Connections {
        target: canvasA
        function onNotice(message, error) { toast.show(message, error) }
    }
    Connections {
        target: canvasB
        function onNotice(message, error) { toast.show(message, error) }
    }
    // The idle half follows the active one's pens, tools and input settings.
    Instantiator {
        model: ["tool", "penColor", "penWidth", "penPattern", "highlighterColor", "highlighterWidth",
                "eraserWholeStrokes", "eraserSize", "shapeAssist", "fingerDrawing", "holdDelay",
                "palmRejection", "penTipVisible", "penTipSize", "pressureResponse", "smoothing",
                "penButtonTool", "penButton2Tool", "eraserEndTool"]
        delegate: Binding {
            required property string modelData
            target: window.idleCanvas
            property: modelData
            value: window.canvas[modelData]
            restoreMode: Binding.RestoreNone
        }
    }
    Instantiator {
        model: ["mathSyntax", "mathFavorites", "mathCategory"]
        delegate: Binding {
            required property string modelData
            target: paneA.active ? paneB : paneA
            property: modelData
            value: (paneA.active ? paneA : paneB)[modelData]
            restoreMode: Binding.RestoreNone
        }
    }
    // The drawer makes way for the page that was picked in it.
    Connections {
        target: AppController
        function onCurrentPageChanged() {
            if (window.compact && AppController.currentPage.length > 0)
                window.drawerOpen = false
        }
    }
    Connections {
        target: AppController
        // Settings opens scrolled to the download, which sits below a long list of languages.
        function onModelsNeeded(id) {
            settingsDialog.showModels()
            const model = Models.entries.find(e => e.id === id)
            toast.show(model ? qsTr("Download \u201c%1\u201d first, then try again.").arg(model.title)
                             : qsTr("Download the recognizer for this first."), false)
        }
    }
    Connections {
        target: Updater
        // One question at a time: the update waits for the storage question.
        function onUpdateFound() {
            if (storageDialog.visible)
                storageDialog.updateWaiting = true
            else
                updateDialog.open()
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
    // A key listed twice is ambiguous and never fires.
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
    Shortcut { sequence: "Ctrl+\\"; onActivated: window.toggleSidebar() }
    // Renames the open page where it is listed, like a file manager.
    Shortcut {
        sequence: "F2"
        enabled: AppController.currentPage.length > 0
        onActivated: {
            window.showInSidebar(AppController.currentPage, "")
            tree.renaming = AppController.currentPage
        }
    }
    Shortcut { sequences: [StandardKey.Open]; onActivated: openFileDialog.open() }
    Shortcut {
        sequences: [StandardKey.Find]
        onActivated: titleBar.focusSearch(true)
        onActivatedAmbiguously: titleBar.focusSearch(true)
    }
    Shortcut {
        sequence: "Ctrl+K"
        onActivated: titleBar.focusSearch(false)
        onActivatedAmbiguously: titleBar.focusSearch(false)
    }
    Shortcut { sequences: [StandardKey.Save]; onActivated: AppController.saveNow() }
    Shortcut { sequence: "Ctrl+W"; onActivated: AppController.closePage() }
    Shortcut { sequences: [StandardKey.NextChild, "Ctrl+PgDown"]; onActivated: AppController.switchTab(1) }
    Shortcut {
        sequences: [StandardKey.PreviousChild, "Ctrl+Shift+Tab", "Ctrl+PgUp"]
        onActivated: AppController.switchTab(-1)
        onActivatedAmbiguously: AppController.switchTab(-1)
    }
    // Not StandardKey.Quit: on Linux that is Ctrl+Q as well.
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
    Shortcut {
        sequence: "Escape"
        enabled: window.fullScreen && canvas.selectionCount === 0
        onActivated: window.toggleFullScreen()
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0
        // Android's Back goes up: from the page to the notebook list, from there out of the app.
        // Open dialogs and menus take it first. Focus here, so Back arrives when nothing else has it.
        focus: true
        Keys.onBackPressed: (event) => {
            if (Qt.platform.os !== "android")
                event.accepted = false
            else if (window.compact && !window.drawerOpen && AppController.currentPage.length > 0)
                window.drawerOpen = true
            else
                AppController.moveToBack()
        }

        TitleBar {
            id: titleBar
            Layout.fillWidth: true
            window: window
            canvas: window.canvas
            compact: window.compact
            visible: !window.fullScreen
            onNewPageRequested: newPageDialog.show("")
            onNewNotebookRequested: newNotebookDialog.show(AppController.defaultNotebookLocation())
            onHistoryRequested: historyDialog.show(AppController.currentPage)
            onTrashRequested: trashDialog.show()
            onSettingsRequested: settingsDialog.open()
            onExportRequested: exportDialog.show()
            onOpenFolderRequested: folderDialog.open()
            onOpenFileRequested: openFileDialog.open()
            onImportRequested: importFileDialog.open()
            onImportFolderRequested: importFolderDialog.open()
            onPrintRequested: AppController.print()
            onSidebarToggled: window.toggleSidebar()
            onUpdateRequested: updateDialog.open()
        }

        Ribbon {
            Layout.fillWidth: true
            visible: !window.fullScreen
            canvas: window.canvas
            window: window
            tab: titleBar.currentTab
            onPdfImportRequested: pdfFileDialog.open()
            onImageInsertRequested: imageFileDialog.open()
            onOpenFileRequested: openFileDialog.open()
            onImportRequested: importFileDialog.open()
            onImportFolderRequested: importFolderDialog.open()
            onOpenFolderRequested: folderDialog.open()
            onExportRequested: exportDialog.show()
            onPrintRequested: AppController.print()
            onSettingsRequested: settingsDialog.open()
            onNewPageRequested: newPageDialog.show("")
            onNewNotebookRequested: newNotebookDialog.show(AppController.defaultNotebookLocation())
            onHistoryRequested: historyDialog.show(AppController.currentPage)
            onTrashRequested: trashDialog.show()
        }

        // Beside the page, or a drawer over it on narrow screens.
        Item {
            id: body
            Layout.fillWidth: true
            Layout.fillHeight: true
            readonly property bool besidePage: !window.compact && tree.visible

            Rectangle {
                objectName: "drawerScrim"
                anchors.fill: parent
                z: 1
                visible: window.compact && window.drawerOpen
                color: "#66000000"
                MouseArea {
                    anchors.fill: parent
                    onClicked: window.drawerOpen = false
                }
            }

            NotebookTree {
                id: tree
                objectName: "notebookTree"
                z: 2
                height: parent.height
                width: window.compact ? Math.min(340, parent.width - 48)
                                      : Math.min(window.sidebarWidth, window.width * 0.6)
                x: window.sidebarRight ? parent.width - width : 0
                visible: window.compact ? window.drawerOpen
                                        : window.sidebarVisible && !window.fullScreen
                onRight: window.sidebarRight

                MouseArea {
                    objectName: "sidebarResize"
                    property real startX
                    property int startWidth
                    x: window.sidebarRight ? 0 : parent.width - width
                    height: parent.height
                    width: 6
                    cursorShape: Qt.SizeHorCursor
                    // Rows beneath can be dragged too.
                    preventStealing: true
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
                onCollapseRequested: window.toggleSidebar()
                onNewPageRequested: (folder) => newPageDialog.show(folder)
                onNewNotebookRequested: (location) => newNotebookDialog.show(location)
                onMakeNotebookRequested: (folder) => newNotebookDialog.showForFolder(folder)
                onMoveRequested: (path, name, isFolder) => moveDialog.show(path, name, isFolder)
                onHistoryRequested: (path) => historyDialog.show(path)
                onExportRequested: (folder, name) => exportDialog.show(folder, name)
            }

            Item {
                id: pageArea
                x: body.besidePage && !window.sidebarRight ? tree.width : 0
                width: parent.width - (body.besidePage ? tree.width : 0)
                height: parent.height

                // The half at the window's left edge, where the pen bar sits.
                readonly property PagePane leftPane: paneA.visible ? paneA : paneB
                readonly property PagePane rightPane: paneB.visible ? paneB : paneA

                function paneAt(scenePosition) {
                    for (const pane of [paneA, paneB]) {
                        const p = pane.area.mapFromItem(null, scenePosition)
                        if (pane.visible && p.x >= 0 && p.x < pane.area.width && p.y >= 0 && p.y < pane.area.height)
                            return pane
                    }
                    return null
                }

                // A tab dragged onto the other half moves there; onto the empty side of a lone half, it splits.
                function tabDragged(from, path, scenePosition, dropped) {
                    const target = paneAt(scenePosition)
                    let hint = ""
                    if (target && target !== from) {
                        hint = "pane"
                    } else if (target && !window.split && !window.compact && from.pages.length > 1) {
                        const p = target.area.mapFromItem(null, scenePosition)
                        const besideRight = target === paneA
                        if (besideRight ? p.x > target.area.width / 2 : p.x < target.area.width / 2)
                            hint = besideRight ? "right" : "left"
                    }
                    paneA.dropHint = dropped || target !== paneA ? "" : hint
                    paneB.dropHint = dropped || target !== paneB ? "" : hint
                    if (!dropped || hint.length === 0)
                        return
                    if (hint === "pane")
                        AppController.moveTab(path, target.canvas)
                    else
                        AppController.openBeside(path)
                }

                RowLayout {
                    anchors.fill: parent
                    spacing: 0

                    PagePane {
                        id: paneA
                        canvas: canvasA
                        // In a split, the divider sets this half's share; the other half takes the rest.
                        Layout.fillWidth: !window.split
                        Layout.fillHeight: true
                        Layout.preferredWidth: window.split ? Math.round((pageArea.width - divider.width) * window.splitRatio) : -1
                        // An emptied half gives way; a phone shows only the active one.
                        visible: (pages.length > 0 || paneB.pages.length === 0)
                                 && (!window.compact || active || paneB.pages.length === 0)
                        split: window.split
                        pageTitle: window.pageTitle
                        fullScreen: window.fullScreen
                        lowWindow: window.height < 560
                        leftInset: pageArea.leftPane === paneA && penBar.visible && !window.penBarRight
                                   ? penBar.x + penBar.width + 8 : 8
                        rightInset: pageArea.rightPane === paneA && penBar.visible && window.penBarRight
                                    ? pageArea.width - penBar.x + 8 : 8
                        onShowInSidebar: (path, view) => window.showInSidebar(path, view)
                        onNewPageRequested: newPageDialog.show("")
                        onTabDragged: (path, scenePosition, dropped) => pageArea.tabDragged(paneA, path, scenePosition, dropped)

                        InkCanvas {
                            id: canvasA
                            objectName: paneA.active ? "canvas" : "otherCanvas"
                            anchors.fill: parent
                            z: -1
                            deskColor: Theme.desk
                            enabled: paneA.page.length > 0
                        }
                    }

                    Item {
                        id: divider
                        objectName: "splitDivider"
                        Layout.fillHeight: true
                        Layout.preferredWidth: 7
                        visible: window.split
                        // Above the halves: its grip reaches into them, wide enough for a finger or pen.
                        z: 1
                        Rectangle {
                            anchors.horizontalCenter: parent.horizontalCenter
                            width: dividerDrag.containsMouse || dividerDrag.pressed ? 3 : 1
                            height: parent.height
                            color: dividerDrag.containsMouse || dividerDrag.pressed ? Theme.accent : Theme.ribbonBorder
                        }
                        MouseArea {
                            id: dividerDrag
                            anchors.fill: parent
                            anchors.leftMargin: -6
                            anchors.rightMargin: -6
                            hoverEnabled: true
                            cursorShape: Qt.SplitHCursor
                            onPositionChanged: (mouse) => {
                                if (!pressed)
                                    return
                                const x = mapToItem(pageArea, mouse.x, 0).x
                                window.splitRatio = Math.max(0.2, Math.min(0.8, x / pageArea.width))
                            }
                            onDoubleClicked: window.splitRatio = 0.5
                            ToolTip.visible: containsMouse && !pressed
                            ToolTip.delay: 800
                            ToolTip.text: qsTr("Drag to resize the halves; double-click for equal halves")
                        }
                    }

                    PagePane {
                        id: paneB
                        canvas: canvasB
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        visible: pages.length > 0 && (!window.compact || active)
                        split: window.split
                        pageTitle: window.pageTitle
                        fullScreen: window.fullScreen
                        lowWindow: window.height < 560
                        leftInset: pageArea.leftPane === paneB && penBar.visible && !window.penBarRight
                                   ? penBar.x + penBar.width + 8 : 8
                        rightInset: pageArea.rightPane === paneB && penBar.visible && window.penBarRight
                                    ? width - (penBar.x - x) + 8 : 8
                        onShowInSidebar: (path, view) => window.showInSidebar(path, view)
                        onNewPageRequested: newPageDialog.show("")
                        onTabDragged: (path, scenePosition, dropped) => pageArea.tabDragged(paneB, path, scenePosition, dropped)

                        InkCanvas {
                            id: canvasB
                            objectName: paneB.active ? "canvas" : "otherCanvas"
                            anchors.fill: parent
                            z: -1
                            deskColor: Theme.desk
                            enabled: paneB.page.length > 0
                        }
                    }
                }

                PenBar {
                    id: penBar
                    objectName: "penBar"
                    canvas: window.canvas
                    onRight: window.penBarRight
                    onScreenshotRequested: window.takeScreenshot()
                    onPictureRequested: imageFileDialog.open()
                    fullScreen: window.fullScreen
                    onFullScreenToggled: window.toggleFullScreen()
                    // Full screen has no ribbon, so the bar is the way to the tools and back.
                    visible: window.penBarVisible && window.canvas.enabled || window.fullScreen
                    x: window.penBarRight ? parent.width - width - 10 : 10
                    // Centred on the page, below the tabs.
                    readonly property Item area: pageArea.leftPane.area
                    y: area.y + (area.height - height) / 2
                    // Clear of the corner button on its side.
                    height: Math.min(implicitHeight, area.height - 20
                                     - (sidebarOpener.visible && window.penBarRight === window.sidebarRight
                                        ? 2 * (sidebarOpener.height + 8) : 0))
                    clip: true
                }

                // The hidden sidebar comes back from where it was collapsed.
                Rectangle {
                    id: sidebarOpener
                    visible: !window.compact && !window.sidebarVisible && !window.fullScreen
                    x: window.sidebarRight ? parent.width - width - 10 : 10
                    y: parent.height - height - 10
                    width: opener.implicitWidth + 8
                    height: opener.implicitHeight + 8
                    radius: 10
                    color: Theme.popup
                    border.width: 1
                    border.color: Theme.ribbonBorder
                    RibbonButton {
                        id: opener
                        objectName: "sidebarExpand"
                        anchors.centerIn: parent
                        iconName: "sidebar"
                        tip: qsTr("Show or hide notebooks (%1)").arg("Ctrl+\\")
                        onClicked: window.toggleSidebar()
                    }
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
        nameFilters: [qsTr("Notes pages and PDFs (*.svg *.pdf *.PDF)"),
                      importFileDialog.notesFilter, qsTr("All files (*)")]
        onAccepted: window.openFile(selectedFile.toString())
    }

    FileDialog {
        id: importFileDialog
        readonly property string notesFilter:
            qsTr("Notes from other apps (*.xopp *.xoj *.rnote *.one *.onetoc2 *.onepkg)")
        title: qsTr("Import notes")
        nameFilters: [notesFilter, qsTr("Xournal++ (*.xopp *.xoj)"), qsTr("Rnote (*.rnote)"),
                      qsTr("OneNote (*.one *.onetoc2 *.onepkg)"), qsTr("All files (*)")]
        onAccepted: AppController.importNotes(selectedFile.toString())
    }

    // A OneNote notebook folder, or any folder of notes, becomes one notebook.
    FolderDialog {
        id: importFolderDialog
        title: qsTr("Import a notebook folder")
        onAccepted: AppController.importNotes(selectedFolder.toString())
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
        /// "annotate", "pages" or "printout".
        property string mode: "annotate"
        readonly property bool printout: mode === "printout"

        title: qsTr("How do you want to write on this PDF?")
        acceptText: printout ? qsTr("Insert") : qsTr("Import")
        onAboutToShow: {
            pdfPlace.location = AppController.defaultPageFolder()
            pdfPlace.browsing = false
            if (printout && !window.canvas.enabled)
                mode = "annotate"
        }
        onAccepted: {
            if (printout)
                window.canvas.insertPdfFile(file)
            else
                AppController.importPdf(file, mode === "annotate", pdfPlace.location)
        }

        contentItem: Column {
            spacing: 8
            Repeater {
                model: [
                    { mode: "annotate", name: "pdfModeAnnotate", label: qsTr("Write on the PDF"),
                      hint: qsTr("A copy goes into the notebook; your ink is saved in that copy as annotations, so other PDF readers show it too.") },
                    { mode: "pages", name: "pdfModePages", label: qsTr("Import as note pages"),
                      hint: qsTr("The PDF becomes the background of a notes page.") },
                    { mode: "printout", name: "pdfModePrintout", label: qsTr("Insert into this page"),
                      hint: qsTr("Its pages become pictures on the open page, to move and resize like any picture.") }
                ]
                delegate: AbstractButton {
                    id: option
                    required property var modelData
                    readonly property bool chosen: pdfModeDialog.mode === modelData.mode
                    objectName: modelData.name
                    visible: modelData.mode !== "printout" || window.canvas.enabled
                    width: pdfModeDialog.availableWidth
                    height: visible ? optionText.implicitHeight + 20 : 0
                    onClicked: pdfModeDialog.mode = modelData.mode
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
                            width: option.width - 20
                            text: option.modelData.hint
                            color: Theme.textMuted
                            font.pixelSize: 12
                            wrapMode: Text.WordWrap
                        }
                    }
                }
            }
            PlacePicker {
                id: pdfPlace
                visible: !pdfModeDialog.printout
                width: pdfModeDialog.availableWidth
            }
            Label {
                width: pdfModeDialog.availableWidth
                text: qsTr("The original file is only read, never changed.")
                color: Theme.textMuted
                font.pixelSize: 11
                wrapMode: Text.WordWrap
            }
        }
    }

    // A new dialog per export: a reused portal dialog keeps its first name.
    QtObject {
        id: exportDialog

        property string exportFolder

        // Named after the page or notebook, in Documents or the last export folder.
        function show(folder, title) {
            const notebook = folder || ""
            const name = (notebook.length > 0 ? title : AppController.currentTitle)
                .replace(/[\\/:*?"<>|]/g, "_").trim() || qsTr("Page")
            const target = exportFolder.length > 0 ? exportFolder
                         : StandardPaths.writableLocation(StandardPaths.DocumentsLocation).toString()
            const dialog = exportDialogComponent.createObject(window, { notebook: notebook })
            // Set after creation: as creation properties the name is lost.
            dialog.currentFolder = target
            dialog.selectedFile = target + "/" + encodeURIComponent(name) + ".pdf"
            dialog.open()
        }
    }

    Component {
        id: exportDialogComponent
        FileDialog {
            id: dialog
            /// A notebook or folder to export whole; "" for the open page.
            property string notebook

            title: qsTr("Export as PDF")
            fileMode: FileDialog.SaveFile
            nameFilters: [qsTr("PDF documents (*.pdf)")]
            defaultSuffix: "pdf"
            onAccepted: {
                exportDialog.exportFolder = currentFolder.toString()
                if (notebook.length > 0)
                    AppController.exportFolder(notebook, selectedFile.toString())
                else
                    AppController.exportPdf(selectedFile.toString())
                dialog.destroy()
            }
            onRejected: dialog.destroy()
        }
    }

    // Android, once: notes inside the app go with it and are out of reach of sync apps.
    AppDialog {
        id: storageDialog
        objectName: "storageDialog"
        property bool updateWaiting: false
        title: qsTr("Keep your notes in Documents?")
        message: qsTr("In Documents/Ductus your notebooks stay when the app is removed, and backups and sync apps such as Nextcloud or Syncthing find them. Ductus needs access to your files for that (\u201cAll files access\u201d).")
        acceptText: qsTr("Allow access")
        rejectText: qsTr("Keep them in the app")
        onAccepted: {
            AppController.storageQuestionAnswered()
            AppController.useSharedStorage()
        }
        onRejected: AppController.storageQuestionAnswered()
        onClosed: {
            if (updateWaiting) {
                updateWaiting = false
                updateDialog.open()
            }
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
        canvas: window.canvas
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
    ImportDialog { id: importDialog }
    NewNotebookDialog { id: newNotebookDialog }
}

import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import QtQuick.Layouts
import Ductus

Rectangle {
    id: ribbon

    required property InkCanvas canvas
    required property ApplicationWindow window
    property string tab: "draw"

    signal pdfImportRequested()
    signal imageInsertRequested()
    signal openFileRequested()
    signal openFolderRequested()
    signal exportRequested()
    signal printRequested()
    signal settingsRequested()
    signal aboutRequested()
    signal newPageRequested()
    signal newNotebookRequested()

    implicitHeight: Theme.ribbonHeight
    color: Theme.ribbon

    Rectangle {
        anchors { left: parent.left; right: parent.right; bottom: parent.bottom }
        height: 1
        color: Theme.ribbonBorder
    }

    readonly property bool highlighting: canvas.tool === InkCanvas.Highlighter
    readonly property var penColors: ["#1a1a1f", "#1f4fd1", "#d12f2f", "#1d8a3b", "#7b2fd1", "#e07b00"]
    readonly property var highlighterColors: ["#ffd91a", "#7ee07e", "#5cc8ff", "#ff8fd0", "#ffa94d"]
    readonly property var penWidths: [{ w: 1.5, label: qsTr("Fine") },
                                      { w: 3, label: qsTr("Medium") },
                                      { w: 6, label: qsTr("Bold") }]
    readonly property var highlighterWidths: [{ w: 12, label: qsTr("Narrow") },
                                              { w: 18, label: qsTr("Medium") },
                                              { w: 28, label: qsTr("Wide") }]
    readonly property color activeColor: highlighting ? canvas.highlighterColor : canvas.penColor
    readonly property real activeWidth: highlighting ? canvas.highlighterWidth : canvas.penWidth

    function pickColor(color) {
        if (canvas.selectionCount > 0) {
            canvas.recolorSelection(color)
        } else if (highlighting) {
            canvas.highlighterColor = color
        } else {
            canvas.penColor = color
            if (canvas.tool !== InkCanvas.Pen)
                canvas.tool = InkCanvas.Pen
        }
    }

    function pickWidth(width) {
        if (highlighting)
            canvas.highlighterWidth = width
        else
            canvas.penWidth = width
    }

    component Separator: Rectangle {
        Layout.preferredWidth: 1
        Layout.preferredHeight: 26
        Layout.leftMargin: 6
        Layout.rightMargin: 6
        color: Theme.separator
    }

    component ToolButtons: RowLayout {
        spacing: 2
        RibbonButton {
            objectName: "toolLasso"
            iconName: "lasso"
            tip: qsTr("Lasso: circle ink to select it, then drag, copy or delete it")
            checkable: true
            checked: ribbon.canvas.tool === InkCanvas.Lasso
            onClicked: ribbon.canvas.tool = InkCanvas.Lasso
        }
        RibbonButton {
            objectName: "toolHand"
            iconName: "hand"
            tip: qsTr("Hand: drag text, formulas and pictures to move them, their corners to resize; elsewhere it scrolls")
            checkable: true
            checked: ribbon.canvas.tool === InkCanvas.Hand
            onClicked: ribbon.canvas.tool = InkCanvas.Hand
        }
        RibbonButton {
            objectName: "toolPen"
            iconName: "pen"
            tip: qsTr("Pen")
            checkable: true
            checked: ribbon.canvas.tool === InkCanvas.Pen
            onClicked: ribbon.canvas.tool = InkCanvas.Pen
        }
        RibbonButton {
            objectName: "toolHighlighter"
            iconName: "highlighter"
            tip: qsTr("Highlighter")
            checkable: true
            checked: ribbon.canvas.tool === InkCanvas.Highlighter
            onClicked: ribbon.canvas.tool = InkCanvas.Highlighter
        }
        RibbonButton {
            objectName: "toolEraser"
            iconName: "eraser"
            tip: ribbon.canvas.eraserWholeStrokes
                 ? qsTr("Eraser: removes whole strokes (also: pen eraser end or barrel button)")
                 : qsTr("Eraser: removes the ink it touches (also: pen eraser end or barrel button)")
            checkable: true
            checked: ribbon.canvas.tool === InkCanvas.Eraser
            onClicked: ribbon.canvas.tool = InkCanvas.Eraser
        }
        RibbonButton {
            objectName: "eraserOptions"
            iconName: "chevron"
            tip: qsTr("Eraser options")
            implicitWidth: 18
            padding: 0
            icon.width: 12
            icon.height: 12
            rotation: 90
            onClicked: eraserMenu.popup(this, 0, height)
        }
        RibbonButton {
            objectName: "toolText"
            iconName: "text"
            tip: qsTr("Text: click on the page to type; click a text box to edit it")
            checkable: true
            checked: ribbon.canvas.tool === InkCanvas.Text
            onClicked: ribbon.canvas.tool = InkCanvas.Text
        }
    }

    AppMenu {
        id: eraserMenu
        AppMenuItem {
            objectName: "eraserPartial"
            text: qsTr("Erase only what you touch")
            checkable: true
            checked: !ribbon.canvas.eraserWholeStrokes
            onTriggered: {
                ribbon.canvas.eraserWholeStrokes = false
                ribbon.canvas.tool = InkCanvas.Eraser
            }
        }
        AppMenuItem {
            objectName: "eraserWhole"
            text: qsTr("Erase whole strokes")
            checkable: true
            checked: ribbon.canvas.eraserWholeStrokes
            onTriggered: {
                ribbon.canvas.eraserWholeStrokes = true
                ribbon.canvas.tool = InkCanvas.Eraser
            }
        }
        AppMenuSeparator {}
        Repeater {
            model: [{ size: 4, label: qsTr("Small eraser") }, { size: 8, label: qsTr("Medium eraser") },
                    { size: 16, label: qsTr("Large eraser") }, { size: 30, label: qsTr("Extra large eraser") }]
            delegate: AppMenuItem {
                required property var modelData
                objectName: "eraserSize" + modelData.size
                text: modelData.label
                checkable: true
                checked: ribbon.canvas.eraserSize === modelData.size
                onTriggered: {
                    ribbon.canvas.eraserSize = modelData.size
                    ribbon.canvas.tool = InkCanvas.Eraser
                }
            }
        }
    }

    ColorDialog {
        id: customColor
        title: qsTr("Choose a color")
        selectedColor: ribbon.activeColor
        onAccepted: ribbon.pickColor(selectedColor)
    }

    StackLayout {
        anchors.fill: parent
        anchors.leftMargin: 10
        anchors.rightMargin: 10
        anchors.bottomMargin: 1
        currentIndex: ["file", "draw", "insert", "page", "view"].indexOf(ribbon.tab)

        RowLayout {
            spacing: 2
            RibbonButton {
                objectName: "fileNewPage"
                iconName: "page-add"
                text: qsTr("New page")
                tip: qsTr("New page: choose its name and notebook (Ctrl+N)")
                onClicked: ribbon.newPageRequested()
            }
            RibbonButton {
                objectName: "fileNewNotebook"
                iconName: "plus"
                text: qsTr("New notebook")
                tip: qsTr("New notebook: choose its name and where it is saved")
                onClicked: ribbon.newNotebookRequested()
            }
            RibbonButton {
                objectName: "fileClosePage"
                iconName: "close"
                text: qsTr("Close page")
                tip: qsTr("Close page (Ctrl+W)")
                enabled: AppController.currentPage.length > 0
                onClicked: AppController.closePage()
            }
            Separator {}
            RibbonButton {
                objectName: "fileOpen"
                iconName: "file-open"
                text: qsTr("Open…")
                tip: qsTr("Open a notes page or PDF (Ctrl+O)")
                onClicked: ribbon.openFileRequested()
            }
            RibbonButton {
                objectName: "fileOpenLibrary"
                iconName: "folder-open"
                text: qsTr("Library…")
                tip: qsTr("Choose the folder your notebooks live in")
                onClicked: ribbon.openFolderRequested()
            }
            RibbonButton {
                objectName: "fileSave"
                iconName: "save"
                text: qsTr("Save")
                tip: qsTr("Pages save automatically; this saves right now (Ctrl+S)")
                onClicked: {
                    AppController.saveNow()
                    AppController.notice(qsTr("Saved."))
                }
            }
            Separator {}
            RibbonButton {
                objectName: "fileExport"
                iconName: "export"
                text: qsTr("Export PDF…")
                onClicked: ribbon.exportRequested()
            }
            RibbonButton {
                objectName: "filePrint"
                iconName: "print"
                text: qsTr("Print…")
                tip: qsTr("Print this page (Ctrl+P)")
                enabled: AppController.pdfProblem.length === 0
                onClicked: ribbon.printRequested()
            }
            Separator {}
            RibbonButton {
                objectName: "fileSettings"
                iconName: "settings"
                text: qsTr("Settings")
                onClicked: ribbon.settingsRequested()
            }
            RibbonButton {
                objectName: "fileAbout"
                iconName: "info"
                tip: qsTr("About Ductus")
                onClicked: ribbon.aboutRequested()
            }
            Item { Layout.fillWidth: true }
        }

        RowLayout {
            // Without an open page there is nothing to draw on.
            enabled: ribbon.canvas.enabled
            spacing: 2
            ToolButtons {}
            Separator {}
            Repeater {
                model: ribbon.highlighting ? ribbon.highlighterColors : ribbon.penColors
                delegate: ColorSwatch {
                    required property string modelData
                    required property int index
                    objectName: "color" + index
                    swatch: modelData
                    selected: Qt.colorEqual(ribbon.activeColor, modelData)
                    onClicked: ribbon.pickColor(modelData)
                }
            }
            RibbonButton {
                objectName: "customColor"
                iconName: "color"
                tip: qsTr("More colors…")
                onClicked: customColor.open()
            }
            Separator {}
            Repeater {
                model: ribbon.highlighting ? ribbon.highlighterWidths : ribbon.penWidths
                delegate: RibbonButton {
                    id: widthButton
                    required property var modelData
                    required property int index
                    objectName: "width" + index
                    tip: modelData.label
                    checkable: true
                    checked: Math.abs(ribbon.activeWidth - modelData.w) < 0.01
                    onClicked: ribbon.pickWidth(modelData.w)
                    contentItem: Item {
                        Rectangle {
                            anchors.centerIn: parent
                            width: 20
                            height: ribbon.highlighting ? widthButton.modelData.w / 2.5
                                                        : Math.max(1.5, widthButton.modelData.w * 1.2)
                            radius: ribbon.highlighting ? 1 : height / 2
                            // Shows thickness, not color; the theme color stays visible in dark mode.
                            color: Theme.text
                            opacity: ribbon.highlighting ? 0.5 : 1
                        }
                    }
                }
            }
            Separator { visible: !ribbon.highlighting }
            Repeater {
                model: [{ key: "solid", label: qsTr("Solid line") },
                        { key: "dashed", label: qsTr("Dashed line") },
                        { key: "dotted", label: qsTr("Dotted line") }]
                delegate: RibbonButton {
                    id: patternButton
                    required property var modelData
                    objectName: "pattern" + modelData.key
                    visible: !ribbon.highlighting
                    tip: modelData.label
                    checkable: true
                    checked: ribbon.canvas.penPattern === modelData.key
                    onClicked: {
                        ribbon.canvas.penPattern = modelData.key
                        if (ribbon.canvas.tool !== InkCanvas.Pen)
                            ribbon.canvas.tool = InkCanvas.Pen
                    }
                    contentItem: LinePatternIcon {
                        pattern: patternButton.modelData.key
                        color: Theme.text
                    }
                }
            }
            Separator {}
            RibbonButton {
                objectName: "selectAll"
                iconName: "select-all"
                tip: qsTr("Select everything on the page (Ctrl+A)")
                onClicked: ribbon.canvas.selectAll()
            }
            RibbonButton {
                objectName: "pageToText"
                iconName: "text"
                text: qsTr("Page to text")
                tip: qsTr("Replace all handwriting on the page with typed text")
                enabled: !AppController.converting
                onClicked: AppController.convertSelection("page")
            }
            RibbonButton {
                objectName: "paste"
                iconName: "paste"
                tip: qsTr("Paste ink or a screenshot (Ctrl+V)")
                onClicked: ribbon.canvas.paste()
            }
            Separator {}
            RibbonButton {
                objectName: "shapeAssist"
                iconName: "shapes"
                text: qsTr("Shapes")
                tip: qsTr("Hold the pen still at the end of a stroke to turn it into a line, arrow, circle or polygon")
                checkable: true
                checked: ribbon.canvas.shapeAssist
                onClicked: ribbon.canvas.shapeAssist = checked
            }
            RibbonButton {
                objectName: "fingerDrawing"
                iconName: "finger"
                tip: qsTr("Draw with finger (when off, fingers only scroll and zoom)")
                checkable: true
                checked: ribbon.canvas.fingerDrawing
                onClicked: ribbon.canvas.fingerDrawing = checked
            }
            Item { Layout.fillWidth: true }
        }

        RowLayout {
            spacing: 2
            RibbonButton {
                objectName: "insertScreenshot"
                iconName: "screenshot"
                text: qsTr("Screenshot")
                tip: qsTr("Take a screenshot of an area of the screen and put it on the page. Pictures already on the clipboard paste with Ctrl+V.")
                enabled: ribbon.canvas.picturesAllowed && ribbon.canvas.enabled
                onClicked: ribbon.window.takeScreenshot()
            }
            RibbonButton {
                objectName: "insertImage"
                iconName: "image"
                text: qsTr("Picture…")
                tip: ribbon.canvas.picturesAllowed
                     ? qsTr("Add a picture file; you can also drop pictures onto the page")
                     : qsTr("PDFs written on directly can't hold pictures yet")
                enabled: ribbon.canvas.picturesAllowed && ribbon.canvas.enabled
                onClicked: ribbon.imageInsertRequested()
            }
            RibbonButton {
                objectName: "insertPdf"
                iconName: "pdf"
                text: qsTr("PDF…")
                tip: AppController.pdfProblem.length > 0 ? AppController.pdfProblem
                                                         : qsTr("Write on a PDF, e.g. lecture slides")
                enabled: AppController.pdfProblem.length === 0
                onClicked: ribbon.pdfImportRequested()
            }
            Separator {}
            RibbonButton {
                objectName: "insertText"
                iconName: "text"
                text: qsTr("Text")
                tip: qsTr("Type on the page: click where the text should go")
                checkable: true
                checked: ribbon.canvas.tool === InkCanvas.Text
                enabled: ribbon.canvas.picturesAllowed && ribbon.canvas.enabled
                onClicked: ribbon.canvas.tool = InkCanvas.Text
            }
            RibbonButton {
                objectName: "insertMath"
                iconName: "math"
                text: qsTr("Math")
                tip: qsTr("Write a formula in LaTeX or Typst: click where it should go, or on a formula to change it")
                checkable: true
                checked: ribbon.canvas.tool === InkCanvas.Math
                enabled: ribbon.canvas.picturesAllowed && ribbon.canvas.enabled
                onClicked: ribbon.canvas.tool = InkCanvas.Math
            }
            Item { Layout.fillWidth: true }
        }

        RowLayout {
            // Without an open page there is nothing to draw on.
            enabled: ribbon.canvas.enabled
            spacing: 2
            Repeater {
                model: [{ key: "endless", label: qsTr("Endless") },
                        { key: "a4", label: "A4" },
                        { key: "a5", label: "A5" },
                        { key: "letter", label: qsTr("Letter") }]
                delegate: RibbonButton {
                    required property var modelData
                    objectName: "page_" + modelData.key
                    text: modelData.label
                    tip: modelData.key === "endless" ? qsTr("A page without edges") : qsTr("Paper sheets")
                    checkable: true
                    enabled: ribbon.canvas.pageKind !== "pdf"
                    checked: ribbon.canvas.pageKind === modelData.key
                    onClicked: ribbon.canvas.setPageStyle(modelData.key, ribbon.canvas.pageLandscape, "")
                }
            }
            RibbonButton {
                objectName: "pageLandscape"
                iconName: "landscape"
                text: qsTr("Landscape")
                checkable: true
                enabled: ribbon.canvas.pageKind !== "endless" && ribbon.canvas.pageKind !== "pdf"
                checked: ribbon.canvas.pageLandscape
                onClicked: ribbon.canvas.setPageStyle("", checked, "")
            }
            Separator {}
            Repeater {
                model: [{ key: "blank", label: qsTr("Blank") },
                        { key: "lined", label: qsTr("Lined") },
                        { key: "grid", label: qsTr("Grid") },
                        { key: "dotted", label: qsTr("Dotted") }]
                delegate: RibbonButton {
                    required property var modelData
                    objectName: "pattern_" + modelData.key
                    text: modelData.label
                    checkable: true
                    enabled: ribbon.canvas.pageKind !== "pdf"
                    checked: enabled && ribbon.canvas.pagePattern === modelData.key
                    onClicked: ribbon.canvas.setPageStyle("", ribbon.canvas.pageLandscape, modelData.key)
                }
            }
            Item { Layout.fillWidth: true }
            Label {
                visible: ribbon.canvas.pageKind === "pdf"
                text: qsTr("The PDF sets the paper of this page.")
                color: Theme.textMuted
                font.pixelSize: 12
            }
        }

        RowLayout {
            spacing: 2
            RibbonButton {
                objectName: "zoomOut"
                iconName: "zoom-out"
                tip: qsTr("Zoom out (Ctrl+−)")
                onClicked: ribbon.canvas.zoomBy(0.8)
            }
            Label {
                objectName: "zoomLabel"
                Layout.preferredWidth: 52
                horizontalAlignment: Text.AlignHCenter
                text: Math.round(ribbon.canvas.zoom * 100) + "%"
                color: Theme.text
                font.pixelSize: 13
            }
            RibbonButton {
                objectName: "zoomIn"
                iconName: "zoom-in"
                tip: qsTr("Zoom in (Ctrl++)")
                onClicked: ribbon.canvas.zoomBy(1.25)
            }
            RibbonButton {
                objectName: "zoomReset"
                iconName: "zoom-reset"
                tip: qsTr("Actual size (Ctrl+0)")
                onClicked: ribbon.canvas.resetView()
            }
            Separator {}
            RibbonButton {
                objectName: "viewSidebar"
                iconName: "sidebar"
                text: qsTr("Notebooks")
                tip: qsTr("Show the notebook list (Ctrl+\\)")
                checkable: true
                checked: ribbon.window.sidebarVisible
                onClicked: ribbon.window.sidebarVisible = checked
            }
            RibbonButton {
                objectName: "penBarToggle"
                iconName: "pen-bar"
                text: qsTr("Pen bar")
                tip: qsTr("Your favorite pens at the side of the page")
                checkable: true
                checked: ribbon.window.penBarVisible
                onClicked: ribbon.window.penBarVisible = checked
            }
            Separator {}
            RibbonButton {
                objectName: "fullscreen"
                iconName: "fullscreen"
                tip: qsTr("Full screen (F11)")
                checkable: true
                checked: ribbon.window.visibility === Window.FullScreen
                onClicked: ribbon.window.toggleFullScreen()
            }
            RibbonButton {
                objectName: "darkMode"
                iconName: "dark-mode"
                tip: qsTr("Dark mode")
                checkable: true
                checked: ribbon.window.darkMode
                onClicked: ribbon.window.themeMode = checked ? "dark" : "light"
            }
            Item { Layout.fillWidth: true }
        }
    }
}

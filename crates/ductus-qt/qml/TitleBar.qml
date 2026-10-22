import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Ductus

Rectangle {
    id: bar

    required property ApplicationWindow window
    required property InkCanvas canvas
    property string currentTab: "draw"
    // A tab name saved by an older version falls back to Draw.
    onCurrentTabChanged: if (["file", "draw", "insert", "page", "view"].indexOf(currentTab) < 0)
                             currentTab = "draw"
    property string pageTitle: qsTr("Untitled page")

    signal aboutRequested()
    signal settingsRequested()
    signal exportRequested()
    signal openFolderRequested()
    signal openFileRequested()
    signal printRequested()
    signal sidebarToggled()
    signal updateRequested()

    implicitHeight: Theme.titleHeight
    color: Theme.titleBar

    function focusSearch() { searchBox.focusSearch() }

    function toggleMaximized() {
        if (window.visibility === Window.Maximized)
            window.showNormal()
        else
            window.showMaximized()
    }

    Item {
        anchors.fill: parent
        DragHandler {
            target: null
            grabPermissions: PointerHandler.CanTakeOverFromAnything
            onActiveChanged: if (active) bar.window.startSystemMove()
        }
        TapHandler {
            onDoubleTapped: bar.toggleMaximized()
        }
    }

    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: 4
        anchors.rightMargin: 0
        spacing: 2

        RibbonButton {
            objectName: "appMenuButton"
            iconName: "menu"
            tip: qsTr("Menu")
            onTitleBar: true
            implicitHeight: Theme.titleHeight - 4
            onClicked: appMenu.popup(this, 0, height)

            AppMenu {
                id: appMenu
                objectName: "appMenu"
                AppMenuItem {
                    objectName: "menuNewPage"
                    text: qsTr("New Page")
                    onTriggered: AppController.newPage(AppController.currentFolder())
                }
                AppMenuItem {
                    objectName: "menuNewSubject"
                    text: qsTr("New Subject")
                    onTriggered: AppController.newSubject()
                }
                AppMenuItem {
                    objectName: "menuOpenFile"
                    text: qsTr("Open…")
                    onTriggered: bar.openFileRequested()
                }
                AppMenuItem {
                    objectName: "menuOpenLibrary"
                    text: qsTr("Open Library Folder…")
                    onTriggered: bar.openFolderRequested()
                }
                AppMenuItem {
                    objectName: "menuSave"
                    text: qsTr("Save Now")
                    onTriggered: AppController.saveNow()
                }
                AppMenuSeparator {}
                AppMenuItem {
                    objectName: "menuExportPdf"
                    text: qsTr("Export as PDF…")
                    enabled: AppController.currentPage.length > 0 && AppController.pdfProblem.length === 0
                    onTriggered: bar.exportRequested()
                }
                AppMenuItem {
                    objectName: "menuPrint"
                    text: qsTr("Print…")
                    enabled: AppController.currentPage.length > 0 && AppController.pdfProblem.length === 0
                    onTriggered: bar.printRequested()
                }
                AppMenuSeparator {}
                AppMenuItem {
                    objectName: "menuClearPage"
                    text: qsTr("Clear Page")
                    onTriggered: bar.canvas.clear()
                }
                AppMenuSeparator {}
                AppMenuItem {
                    objectName: "menuSettings"
                    text: qsTr("Settings…")
                    onTriggered: bar.settingsRequested()
                }
                AppMenuItem {
                    objectName: "menuAbout"
                    text: qsTr("About Ductus")
                    onTriggered: bar.aboutRequested()
                }
                AppMenuItem { text: qsTr("Quit"); onTriggered: Qt.quit() }
            }
        }

        RibbonButton {
            objectName: "sidebarToggle"
            iconName: "sidebar"
            tip: qsTr("Show or hide notebooks (Ctrl+\\)")
            onTitleBar: true
            implicitHeight: Theme.titleHeight - 4
            onClicked: bar.sidebarToggled()
        }

        Label {
            text: "Ductus"
            color: Theme.titleText
            font.pixelSize: 13
            font.bold: true
            leftPadding: 4
            rightPadding: 10
        }

        Repeater {
            model: [
                { key: "file", label: qsTr("File") },
                { key: "draw", label: qsTr("Draw") },
                { key: "insert", label: qsTr("Insert") },
                { key: "page", label: qsTr("Page") },
                { key: "view", label: qsTr("View") }
            ]
            delegate: AbstractButton {
                id: tab
                required property var modelData
                readonly property bool active: bar.currentTab === modelData.key
                objectName: "tab_" + modelData.key
                Layout.alignment: Qt.AlignBottom
                implicitHeight: Theme.titleHeight - 6
                implicitWidth: tabLabel.implicitWidth + 28
                focusPolicy: Qt.NoFocus
                onClicked: bar.currentTab = modelData.key

                contentItem: Label {
                    id: tabLabel
                    text: tab.modelData.label
                    horizontalAlignment: Text.AlignHCenter
                    verticalAlignment: Text.AlignVCenter
                    font.pixelSize: 13
                    font.weight: tab.active ? Font.DemiBold : Font.Normal
                    color: tab.active ? Theme.accent : Theme.titleText
                }
                background: Rectangle {
                    color: tab.active ? Theme.titleTabActive : (tab.hovered ? Theme.titleHover : "transparent")
                    radius: Theme.radius
                    Rectangle {
                        anchors { left: parent.left; right: parent.right; bottom: parent.bottom }
                        height: Theme.radius
                        color: parent.color
                        visible: tab.active
                    }
                }
            }
        }

        Label {
            objectName: "pageTitle"
            Layout.fillWidth: true
            text: bar.pageTitle
            color: Theme.titleTextMuted
            font.pixelSize: 13
            horizontalAlignment: Text.AlignHCenter
            elide: Text.ElideRight
        }

        RibbonButton {
            objectName: "updateButton"
            visible: Updater.available
            iconName: "update"
            text: Updater.restartNeeded ? qsTr("Restart to update") : qsTr("Update")
            tip: qsTr("Ductus %1 is available").arg(Updater.latestVersion)
            onTitleBar: true
            checked: true
            implicitHeight: Theme.titleHeight - 8
            Layout.rightMargin: 6
            onClicked: bar.updateRequested()
        }

        SearchBox {
            id: searchBox
            objectName: "searchBox"
            Layout.preferredWidth: Math.min(260, bar.width / 4)
            Layout.rightMargin: 8
        }

        RibbonButton {
            objectName: "undoButton"
            iconName: "undo"
            tip: qsTr("Undo (Ctrl+Z)")
            onTitleBar: true
            implicitHeight: Theme.titleHeight - 4
            enabled: bar.canvas.canUndo
            onClicked: bar.canvas.undo()
        }
        RibbonButton {
            objectName: "redoButton"
            iconName: "redo"
            tip: qsTr("Redo (Ctrl+Shift+Z)")
            onTitleBar: true
            implicitHeight: Theme.titleHeight - 4
            enabled: bar.canvas.canRedo
            onClicked: bar.canvas.redo()
        }

        Item { implicitWidth: 12 }

        Repeater {
            model: [
                { icon: "minimize", tip: qsTr("Minimize"), action: "min" },
                { icon: bar.window.visibility === Window.Maximized ? "restore" : "maximize",
                  tip: qsTr("Maximize"), action: "max" },
                { icon: "close", tip: qsTr("Close"), action: "close" }
            ]
            delegate: ToolButton {
                id: winButton
                required property var modelData
                objectName: "window_" + modelData.action
                implicitWidth: 46
                implicitHeight: Theme.titleHeight
                focusPolicy: Qt.NoFocus
                display: AbstractButton.IconOnly
                icon.source: Qt.resolvedUrl("icons/" + modelData.icon + ".svg")
                icon.width: 16
                icon.height: 16
                icon.color: Theme.titleText
                onClicked: {
                    if (modelData.action === "min") bar.window.showMinimized()
                    else if (modelData.action === "max") bar.toggleMaximized()
                    else bar.window.close()
                }
                background: Rectangle {
                    color: winButton.hovered
                           ? (winButton.modelData.action === "close" ? "#d9363e" : Theme.titleHover)
                           : "transparent"
                }
            }
        }
    }
}

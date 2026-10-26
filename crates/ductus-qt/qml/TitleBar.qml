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
    /// A narrow window, such as a phone: less text, more scrolling.
    property bool compact: false
    readonly property bool mobile: Qt.platform.os === "android" || Qt.platform.os === "ios"

    signal aboutRequested()
    signal settingsRequested()
    signal exportRequested()
    signal openFolderRequested()
    signal openFileRequested()
    signal printRequested()
    signal sidebarToggled()
    signal updateRequested()
    signal newPageRequested()
    signal newNotebookRequested()
    signal historyRequested()
    signal trashRequested()

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

    // Too narrow (a phone): the row scrolls sideways instead of cutting off buttons.
    Flickable {
        id: scroller
        anchors.fill: parent
        anchors.leftMargin: 4
        clip: true
        contentWidth: Math.max(width, row.implicitWidth)
        contentHeight: height
        flickableDirection: Flickable.HorizontalFlick
        boundsBehavior: Flickable.StopAtBounds
        interactive: contentWidth > width

        RowLayout {
            id: row
            width: scroller.contentWidth
            height: scroller.height
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
                        text: qsTr("New Page…")
                        onTriggered: bar.newPageRequested()
                    }
                    AppMenuItem {
                        objectName: "menuNewNotebook"
                        text: qsTr("New Notebook…")
                        onTriggered: bar.newNotebookRequested()
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
                    AppMenuItem {
                        objectName: "menuClosePage"
                        text: qsTr("Close Page")
                        enabled: AppController.currentPage.length > 0
                        onTriggered: AppController.closePage()
                    }
                    AppMenuItem {
                        objectName: "menuHistory"
                        text: qsTr("Earlier Versions…")
                        enabled: AppController.currentPage.length > 0
                        onTriggered: bar.historyRequested()
                    }
                    AppMenuItem {
                        objectName: "menuTrash"
                        text: qsTr("Trash…")
                        onTriggered: bar.trashRequested()
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
                visible: !bar.compact
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
                    implicitWidth: tabLabel.implicitWidth + (bar.compact ? 14 : 28)
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

            Item {
                id: titleArea
                Layout.fillWidth: true
                Layout.fillHeight: true
                // The location bar names the page on a narrow screen.
                visible: !bar.compact

                Row {
                    anchors.centerIn: parent
                    spacing: 4
                    Label {
                        objectName: "pageTitle"
                        anchors.verticalCenter: parent.verticalCenter
                        // Shortened only when the space between tabs and search runs out.
                        width: Math.min(implicitWidth, titleArea.width - closePage.width - 4)
                        text: bar.pageTitle
                        color: Theme.titleTextMuted
                        font.pixelSize: 13
                        elide: Text.ElideRight
                    }
                    ToolButton {
                        id: closePage
                        objectName: "closePage"
                        anchors.verticalCenter: parent.verticalCenter
                        visible: AppController.currentPage.length > 0
                        implicitWidth: 24
                        implicitHeight: 24
                        focusPolicy: Qt.NoFocus
                        display: AbstractButton.IconOnly
                        icon.source: Qt.resolvedUrl("icons/close.svg")
                        icon.width: 12
                        icon.height: 12
                        icon.color: Theme.titleTextMuted
                        // Hidden once pressed, until the pointer leaves.
                        property bool tipDismissed: false
                        onPressed: tipDismissed = true
                        onHoveredChanged: if (!hovered) tipDismissed = false
                        ToolTip.visible: hovered && !tipDismissed
                        ToolTip.delay: 600
                        ToolTip.text: qsTr("Close page (Ctrl+W)")
                        onClicked: AppController.closePage()
                        background: Rectangle {
                            radius: Theme.radius
                            color: closePage.hovered ? Theme.titleHover : "transparent"
                        }
                    }
                }
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
                Layout.preferredWidth: !bar.compact ? Math.min(260, bar.width / 4) : active ? 200 : 32
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
                    // A phone's apps fill the screen and close through the system.
                    visible: !bar.mobile
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
}

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Ductus

// One half of the page area: its tabs, where its page is saved, and the page itself.
ColumnLayout {
    id: pane

    /// Declared by the window, which keeps its settings; it lies under this half's layers.
    required property InkCanvas canvas
    default property alias pageContent: area.data
    property alias mathSyntax: mathLayer.syntax
    property alias mathFavorites: mathLayer.favoritesJson
    property alias mathCategory: mathLayer.category
    readonly property alias area: area
    /// The ribbon, pen bar and keys act on this half.
    readonly property bool active: AppController.canvas === pane.canvas
    readonly property var pages: active ? AppController.openPages : AppController.otherPages
    readonly property string page: active ? AppController.currentPage : AppController.otherPage
    property string pageTitle
    property bool fullScreen: false
    /// A short window (a phone held sideways) leaves out the location bar: the page needs the room.
    property bool lowWindow: false
    /// Beside another half: the active one is outlined.
    property bool split: false
    /// Room the selection bar leaves for the pen bar.
    property real leftInset: 8
    property real rightInset: 8
    /// Where a dragged tab would go: "" nowhere, "pane" this half, "left" or "right" a new half there.
    property string dropHint

    signal showInSidebar(string path, string view)
    signal newPageRequested()
    signal tabDragged(string path, point scenePosition, bool dropped)

    spacing: 0

    PageTabs {
        Layout.fillWidth: true
        visible: pane.pages.length > 0 && !pane.fullScreen
        pages: pane.pages
        current: pane.page
        preview: pane.active ? AppController.previewPage : AppController.otherPreview
        focused: pane.active
        pageTitle: pane.active ? pane.pageTitle : AppController.displayName(pane.page)
        onTabDragged: (path, scenePosition, dropped) => pane.tabDragged(path, scenePosition, dropped)
    }

    LocationBar {
        objectName: pane.active ? "locationBar" : ""
        Layout.fillWidth: true
        page: pane.page
        visible: pane.page.length > 0 && !pane.fullScreen && !pane.lowWindow
        onShowRequested: (path, view) => pane.showInSidebar(path, view)
    }

    Item {
        id: area
        Layout.fillWidth: true
        Layout.fillHeight: true
        clip: true

        // Plain desk instead of an empty page that can't be written on.
        Rectangle {
            anchors.fill: parent
            color: Theme.desk
            visible: !pane.canvas.enabled
        }

        ColumnLayout {
            objectName: pane.active ? "noPage" : ""
            anchors.centerIn: parent
            width: Math.min(360, parent.width - 40)
            visible: !pane.canvas.enabled
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
                onClicked: pane.newPageRequested()
            }
        }

        TextLayer {
            objectName: pane.active ? "textLayer" : ""
            anchors.fill: parent
            canvas: pane.canvas
            leftInset: pane.leftInset
            rightInset: pane.rightInset
        }

        MathLayer {
            id: mathLayer
            objectName: pane.active ? "mathLayer" : ""
            anchors.fill: parent
            canvas: pane.canvas
            leftInset: pane.leftInset
            rightInset: pane.rightInset
        }

        SelectionBar {
            objectName: pane.active ? "selectionBar" : ""
            canvas: pane.canvas
            leftInset: pane.leftInset
            rightInset: pane.rightInset
        }

        // In a split, the half the ribbon and keys act on.
        Rectangle {
            anchors.fill: parent
            visible: pane.split && pane.active
            color: "transparent"
            border.width: 2
            border.color: Qt.alpha(Theme.accent, 0.6)
        }

        // Where a dragged tab lands.
        Rectangle {
            x: pane.dropHint === "right" ? parent.width / 2 : 0
            width: pane.dropHint === "pane" ? parent.width : parent.width / 2
            height: parent.height
            visible: pane.dropHint.length > 0
            color: Qt.alpha(Theme.accent, 0.12)
            border.width: 2
            border.color: Theme.accent
        }
    }
}

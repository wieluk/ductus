import QtQuick
import QtQuick.Controls
import QtQuick.Controls.impl
import Ductus

// The open pages, one tab each; the current one joins the location bar below it.
Rectangle {
    id: bar

    /// The current page's name, with the unsaved mark.
    property string pageTitle
    property var pages: []
    /// The page shown below, and the preview tab a single click reuses.
    property string current
    property string preview
    /// The half the ribbon and keys act on; only its tabs carry the names tests look for.
    property bool focused: true

    /// While a tab is dragged, and once more where it is dropped.
    signal tabDragged(string path, point scenePosition, bool dropped)

    implicitHeight: 32
    color: Theme.sidebar

    Rectangle {
        anchors { left: parent.left; right: parent.right; bottom: parent.bottom }
        height: 1
        color: Theme.ribbonBorder
    }

    ListView {
        id: tabs
        objectName: "pageTabs"
        anchors { fill: parent; leftMargin: 6; rightMargin: 6; topMargin: 4 }
        orientation: ListView.Horizontal
        spacing: 2
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        model: bar.pages

        function showCurrent() {
            const at = bar.pages.indexOf(bar.current)
            if (at >= 0)
                positionViewAtIndex(at, ListView.Contain)
        }
        Connections {
            target: bar
            function onCurrentChanged() { Qt.callLater(tabs.showCurrent) }
        }

        delegate: AbstractButton {
            id: tab
            required property string modelData
            readonly property bool active: modelData === bar.current
            readonly property bool preview: modelData === bar.preview
            objectName: "pageTab_" + modelData
            height: ListView.view.height
            width: Math.min(220, pageIcon.width + title.implicitWidth + close.width + 26)
            z: active ? 1 : 0
            focusPolicy: Qt.NoFocus
            onClicked: AppController.openPage(modelData)
            onDoubleClicked: AppController.keepTab(modelData)
            ToolTip.visible: hovered && !close.hovered
            ToolTip.delay: 600
            ToolTip.text: AppController.displayName(AppController.parentOf(modelData))
                          + (preview ? "\n" + qsTr("Preview: the next page you open replaces it. Double-click to keep it.") : "")

            TapHandler {
                acceptedButtons: Qt.MiddleButton
                onTapped: AppController.closeTab(tab.modelData)
            }
            // Dragged onto the other half, or the right side of the page, it opens there.
            DragHandler {
                target: null
                acceptedButtons: Qt.LeftButton
                onCentroidChanged: if (active) bar.tabDragged(tab.modelData, centroid.scenePosition, false)
                onActiveChanged: bar.tabDragged(tab.modelData, centroid.scenePosition, !active)
            }

            background: Rectangle {
                // Square at the bottom, so it runs on into the location bar.
                color: tab.active ? Theme.ribbon : tab.hovered ? Theme.hover : "transparent"
                radius: Theme.radius
                border.color: tab.active ? Theme.ribbonBorder : "transparent"
                Rectangle {
                    anchors { left: parent.left; right: parent.right; bottom: parent.bottom }
                    anchors.leftMargin: tab.active ? 1 : 0
                    anchors.rightMargin: tab.active ? 1 : 0
                    height: Theme.radius + 1
                    color: parent.color
                }
            }
            contentItem: Item {
                IconImage {
                    id: pageIcon
                    anchors.left: parent.left
                    anchors.leftMargin: 8
                    anchors.verticalCenter: parent.verticalCenter
                    width: 14
                    height: 14
                    sourceSize: Qt.size(14, 14)
                    source: Qt.resolvedUrl(tab.modelData.toLowerCase().endsWith(".pdf") ? "icons/pdf.svg" : "icons/page.svg")
                    color: tab.active ? Theme.accent : Theme.textMuted
                }
                Label {
                    id: title
                    objectName: tab.active && bar.focused ? "pageTitle" : ""
                    anchors.left: pageIcon.right
                    anchors.leftMargin: 6
                    anchors.verticalCenter: parent.verticalCenter
                    width: Math.min(implicitWidth, close.x - x - 2)
                    text: tab.active && bar.focused ? bar.pageTitle : tab.modelData.split("/").pop()
                    color: tab.active ? Theme.text : Theme.textMuted
                    font.pixelSize: 12
                    font.weight: tab.active ? Font.DemiBold : Font.Normal
                    font.italic: tab.preview
                    elide: Text.ElideRight
                }
                ToolButton {
                    id: close
                    objectName: tab.active && bar.focused ? "closePage" : "closeTab_" + tab.modelData
                    anchors.right: parent.right
                    anchors.rightMargin: 2
                    anchors.verticalCenter: parent.verticalCenter
                    implicitWidth: 20
                    implicitHeight: 20
                    focusPolicy: Qt.NoFocus
                    display: AbstractButton.IconOnly
                    icon.source: Qt.resolvedUrl("icons/close.svg")
                    icon.width: 10
                    icon.height: 10
                    icon.color: Theme.textMuted
                    // Hidden once pressed, until the pointer leaves.
                    property bool tipDismissed: false
                    onPressed: tipDismissed = true
                    onHoveredChanged: if (!hovered) tipDismissed = false
                    ToolTip.visible: hovered && !tipDismissed
                    ToolTip.delay: 600
                    ToolTip.text: tab.active && bar.focused ? qsTr("Close page (Ctrl+W)") : qsTr("Close")
                    onClicked: AppController.closeTab(tab.modelData)
                    background: Rectangle {
                        radius: Theme.radius
                        color: close.hovered ? Theme.hover : "transparent"
                    }
                }
            }
        }
    }
}

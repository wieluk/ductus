import QtQuick
import QtQuick.Controls
import QtQuick.Controls.impl
import QtQuick.Layouts
import Ductus

// Where the open page is saved: the library, its folders (notebooks under their name) and the page.
Rectangle {
    id: bar

    property var parts: []

    function update() {
        const page = AppController.currentPage
        const out = [{ name: AppController.displayName(""), path: "", notebook: false }]
        if (page.length > 0) {
            const names = page.split("/")
            for (let i = 0; i < names.length - 1; ++i) {
                const path = names.slice(0, i + 1).join("/")
                const notebook = AppController.notebookName(path)
                out.push({ name: notebook.length > 0 ? notebook : names[i], path: path,
                           folder: names[i], notebook: notebook.length > 0 })
            }
            out.push({ name: AppController.currentTitle, path: page, notebook: false, page: true })
        }
        parts = out
    }

    implicitHeight: 30
    color: Theme.ribbon

    Component.onCompleted: update()
    Connections {
        target: AppController
        function onCurrentPageChanged() { bar.update() }
        function onLibraryChanged() { bar.update() }
    }
    // Notebook names change with the sidebar.
    Connections {
        target: AppController.tree
        function onRowsChanged() { bar.update() }
    }

    Rectangle {
        anchors { left: parent.left; right: parent.right; bottom: parent.bottom }
        height: 1
        color: Theme.ribbonBorder
    }

    ListView {
        id: crumbs
        anchors { fill: parent; leftMargin: 10; rightMargin: 10 }
        orientation: ListView.Horizontal
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        model: bar.parts
        // The page and its notebook matter most.
        onCountChanged: positionViewAtEnd()
        onWidthChanged: positionViewAtEnd()
        delegate: AbstractButton {
            id: crumb
            required property var modelData
            required property int index
            readonly property bool last: modelData.page === true
            objectName: "location_" + modelData.path
            text: modelData.name
            height: crumbs.height
            implicitWidth: row.implicitWidth + 8
            focusPolicy: Qt.NoFocus
            enabled: !last
            onClicked: modelData.notebook ? AppController.showInNotebooks(modelData.path)
                                          : AppController.reveal(modelData.path)
            ToolTip.visible: hovered && modelData.notebook && modelData.folder !== modelData.name
            ToolTip.delay: 600
            ToolTip.text: qsTr("Notebook in the folder “%1”").arg(modelData.folder)
            contentItem: Row {
                id: row
                spacing: 4
                Label {
                    visible: crumb.index > 0
                    anchors.verticalCenter: parent.verticalCenter
                    text: "›"
                    color: Theme.textMuted
                    font.pixelSize: 12
                }
                IconImage {
                    visible: crumb.modelData.notebook
                    anchors.verticalCenter: parent.verticalCenter
                    width: 14
                    height: 14
                    source: Qt.resolvedUrl("icons/notebooks.svg")
                    sourceSize: Qt.size(14, 14)
                    color: Theme.accent
                }
                Label {
                    anchors.verticalCenter: parent.verticalCenter
                    text: crumb.modelData.name
                    color: crumb.last ? Theme.text : crumb.hovered ? Theme.accent : Theme.textMuted
                    font.pixelSize: 12
                    font.weight: crumb.last || crumb.modelData.notebook ? Font.DemiBold : Font.Normal
                }
            }
            background: Rectangle {
                radius: Theme.radius
                color: crumb.hovered && !crumb.last ? Theme.hover : "transparent"
            }
        }
    }
}

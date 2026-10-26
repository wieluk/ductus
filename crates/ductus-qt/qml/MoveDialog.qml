import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Ductus

// A page, PDF or folder into another notebook, section or folder.
AppDialog {
    id: dialog

    property string path
    property string name
    property bool isFolder: false
    property string location
    property var folders: []

    function show(item, label, folder) {
        path = item
        name = label
        isFolder = folder
        location = AppController.parentOf(item)
        folders = AppController.subfolders(location)
        open()
    }

    function crumbs(where) {
        AppController.libraryPath // displayName() follows it.
        const parts = [AppController.displayName("")]
        const names = where.length > 0 ? where.split("/") : []
        for (let i = 0; i < names.length; ++i) {
            const notebook = AppController.notebookName(names.slice(0, i + 1).join("/"))
            parts.push(notebook.length > 0 ? notebook : names[i])
        }
        return parts.join(" › ")
    }

    // A folder can't go into itself.
    readonly property bool inside: isFolder && (location === path || location.startsWith(path + "/"))

    onLocationChanged: folders = AppController.subfolders(location)

    objectName: "moveDialog"
    title: qsTr("Move “%1”").arg(name)
    acceptText: qsTr("Move here")
    acceptButton.objectName: "moveConfirm"
    acceptButton.enabled: !inside && location !== AppController.parentOf(path)
    onAccepted: AppController.moveTo(path, location)

    contentItem: ColumnLayout {
        spacing: 6
        Label {
            text: qsTr("Move to")
            color: Theme.textMuted
            font.pixelSize: 12
        }
        Label {
            objectName: "moveWhere"
            Layout.fillWidth: true
            text: dialog.crumbs(dialog.location)
            color: Theme.text
            font.pixelSize: 13
            elide: Text.ElideMiddle
        }
        RibbonButton {
            objectName: "moveUp"
            enabled: dialog.location.length > 0
            iconName: "arrow-up"
            text: qsTr("Up one folder")
            onClicked: dialog.location = AppController.parentOf(dialog.location)
        }
        ListView {
            id: folderList
            objectName: "moveFolders"
            Layout.fillWidth: true
            Layout.preferredHeight: 200
            clip: true
            model: dialog.folders
            boundsBehavior: Flickable.StopAtBounds
            ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }
            delegate: ItemDelegate {
                id: folderItem
                required property var modelData
                objectName: "moveFolder_" + modelData.name
                width: ListView.view.width
                height: 34
                // The folder that is being moved isn't a place to move it to.
                enabled: !(dialog.isFolder && modelData.path === dialog.path)
                text: modelData.notebook.length === 0 ? modelData.name
                      : modelData.notebook === modelData.name ? qsTr("%1 (notebook)").arg(modelData.name)
                      : qsTr("%1 (notebook “%2”)").arg(modelData.name).arg(modelData.notebook)
                font.pixelSize: 13
                rightPadding: 28
                onClicked: dialog.location = modelData.path
                Label {
                    anchors.right: parent.right
                    anchors.rightMargin: 10
                    anchors.verticalCenter: parent.verticalCenter
                    text: "›"
                    color: Theme.textMuted
                    font.pixelSize: 16
                }
            }
            Label {
                anchors.centerIn: parent
                visible: folderList.count === 0
                text: qsTr("No folders in here")
                color: Theme.textMuted
                font.pixelSize: 12
            }
        }
        Label {
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            visible: dialog.inside || dialog.location === AppController.parentOf(dialog.path)
            text: dialog.inside ? qsTr("A folder can't go inside itself.")
                                : qsTr("It is in this folder already. Open the folder to move it into.")
            color: Theme.textMuted
            font.pixelSize: 11
        }
    }
}

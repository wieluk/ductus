import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Ductus

// A notebook's name and where it is saved, or an existing folder turned into a notebook.
AppDialog {
    id: dialog

    property string location
    /// The folder itself becomes the notebook; nothing new is created.
    property bool markOnly: false
    property bool browsing: false
    property var folders: []

    function show(where) {
        markOnly = false
        location = where
        folders = AppController.subfolders(location)
        browsing = false
        ownFolder.checked = true
        nameField.text = qsTr("New notebook")
        open()
    }

    function showForFolder(folder) {
        markOnly = true
        location = folder
        browsing = false
        nameField.text = folder.split("/").pop()
        open()
    }

    function crumbs(path) {
        AppController.libraryPath // displayName() follows it.
        return [AppController.displayName("")].concat(path.length > 0 ? path.split("/") : []).join(" › ")
    }

    onLocationChanged: folders = AppController.subfolders(location)

    objectName: "newNotebookDialog"
    title: markOnly ? qsTr("Make this folder a notebook") : qsTr("New notebook")
    acceptText: markOnly ? qsTr("Make notebook") : qsTr("Create")
    // The library folder itself is no notebook.
    acceptButton.enabled: nameField.text.trim().length > 0
                          && (ownFolder.checked && !markOnly || location.length > 0)
    onOpened: {
        nameField.forceActiveFocus()
        nameField.selectAll()
    }
    onAccepted: AppController.addNotebook(location, nameField.text, ownFolder.checked && !markOnly)

    contentItem: ColumnLayout {
        spacing: 6
        Label {
            text: qsTr("Name")
            color: Theme.textMuted
            font.pixelSize: 12
        }
        TextField {
            id: nameField
            objectName: "newNotebookName"
            Layout.fillWidth: true
            selectByMouse: true
            onAccepted: if (dialog.acceptButton.enabled) dialog.accept()
        }
        Label {
            Layout.topMargin: 8
            text: dialog.markOnly ? qsTr("Folder") : qsTr("Saved in")
            color: Theme.textMuted
            font.pixelSize: 12
        }
        RowLayout {
            Layout.fillWidth: true
            spacing: 6
            Label {
                objectName: "newNotebookWhere"
                Layout.fillWidth: true
                text: dialog.crumbs(dialog.location)
                color: Theme.text
                font.pixelSize: 13
                elide: Text.ElideMiddle
            }
            RibbonButton {
                objectName: "newNotebookChange"
                visible: !dialog.markOnly
                text: dialog.browsing ? qsTr("Done") : qsTr("Change…")
                onClicked: dialog.browsing = !dialog.browsing
            }
        }
        // Folders to step into; the one shown above is where it goes.
        ColumnLayout {
            visible: dialog.browsing
            Layout.fillWidth: true
            spacing: 2
            RibbonButton {
                objectName: "newNotebookUp"
                enabled: dialog.location.length > 0
                iconName: "arrow-up"
                text: qsTr("Up one folder")
                onClicked: dialog.location = AppController.parentOf(dialog.location)
            }
            ListView {
                id: folderList
                objectName: "newNotebookFolders"
                Layout.fillWidth: true
                Layout.preferredHeight: 150
                clip: true
                model: dialog.folders
                boundsBehavior: Flickable.StopAtBounds
                ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }
                delegate: ItemDelegate {
                    id: folderItem
                    required property var modelData
                    objectName: "pickFolder_" + modelData.name
                    width: ListView.view.width
                    height: 32
                    text: modelData.notebook.length === 0 ? modelData.name
                          : modelData.notebook === modelData.name ? qsTr("%1 (notebook)").arg(modelData.name)
                          : qsTr("%1 (notebook “%2”)").arg(modelData.name).arg(modelData.notebook)
                    font.pixelSize: 13
                    rightPadding: 28
                    onClicked: dialog.location = modelData.path
                    // Tapping opens the folder; the place shown above is where it goes.
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
        }
        CheckBox {
            id: ownFolder
            objectName: "newNotebookOwnFolder"
            visible: !dialog.markOnly
            checked: true
            text: qsTr("In a new folder with its name")
            font.pixelSize: 13
        }
        Label {
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            text: dialog.markOnly || !ownFolder.checked
                  ? qsTr("The folder keeps its name on disk; the notebook shows as “%1”.").arg(nameField.text.trim())
                  : qsTr("Creates the folder “%1” with a first page.").arg(dialog.crumbs(dialog.location) + " › " + nameField.text.trim())
            color: Theme.textMuted
            font.pixelSize: 11
        }
    }
}

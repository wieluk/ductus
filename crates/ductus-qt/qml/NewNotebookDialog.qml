import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Ductus

// A notebook's name and where it is saved, or an existing folder turned into a notebook.
AppDialog {
    id: dialog

    property alias location: place.location
    /// The folder itself becomes the notebook; nothing new is created.
    property bool markOnly: false

    function show(where) {
        markOnly = false
        location = where
        place.browsing = false
        ownFolder.checked = true
        nameField.text = qsTr("New notebook")
        open()
    }

    function showForFolder(folder) {
        markOnly = true
        location = folder
        place.browsing = false
        nameField.text = folder.split("/").pop()
        open()
    }

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
        PlacePicker {
            id: place
            Layout.fillWidth: true
            title: dialog.markOnly ? qsTr("Folder") : qsTr("Saved in")
            changeable: !dialog.markOnly
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
                  : qsTr("Creates the folder “%1” with a first page.").arg(AppController.placeLabel(dialog.location) + " › " + nameField.text.trim())
            color: Theme.textMuted
            font.pixelSize: 11
        }
    }
}

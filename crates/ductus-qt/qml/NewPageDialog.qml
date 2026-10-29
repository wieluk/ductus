import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Ductus

// Name and place of a new page before it exists: a notebook, one of its sections or the Inbox.
AppDialog {
    id: dialog

    property string folder
    property var targets: []

    function show(target) {
        targets = AppController.pageTargets()
        folder = target.length > 0 ? target : AppController.defaultPageFolder()
        // A folder that is no notebook (opened from Folders) is offered as it is.
        if (!targets.some(t => t.path === folder))
            targets = [{ path: folder, label: folder.split("/").join(" › ") }].concat(targets)
        nameField.text = qsTr("Untitled page")
        open()
    }

    objectName: "newPageDialog"
    title: qsTr("New page")
    acceptText: qsTr("Create")
    acceptButton.enabled: nameField.valid
    onOpened: nameField.focusAll()
    onAccepted: AppController.addPage(folder, nameField.text)

    contentItem: ColumnLayout {
        spacing: 6
        Label {
            text: qsTr("Name")
            color: Theme.textMuted
            font.pixelSize: 12
        }
        NameField {
            id: nameField
            fieldName: "newPageName"
            Layout.fillWidth: true
            onAccepted: if (dialog.acceptButton.enabled) dialog.accept()
        }
        Label {
            Layout.topMargin: 8
            text: qsTr("Saved in")
            color: Theme.textMuted
            font.pixelSize: 12
        }
        AppComboBox {
            objectName: "newPageWhere"
            Layout.fillWidth: true
            model: dialog.targets
            textRole: "label"
            valueRole: "path"
            currentIndex: dialog.targets.findIndex(t => t.path === dialog.folder)
            onActivated: dialog.folder = currentValue
        }
        Label {
            Layout.fillWidth: true
            text: {
                AppController.libraryPath // displayName() follows it.
                return qsTr("Folder: %1").arg([AppController.displayName("")].concat(dialog.folder.split("/")).join(" › "))
            }
            color: Theme.textMuted
            font.pixelSize: 11
            elide: Text.ElideMiddle
        }
    }
}

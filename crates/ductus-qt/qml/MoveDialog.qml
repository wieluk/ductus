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

    function show(item, label, folder) {
        path = item
        name = label
        isFolder = folder
        place.location = AppController.parentOf(item)
        open()
    }

    // A folder can't go into itself.
    readonly property bool inside: isFolder && (place.location === path || place.location.startsWith(path + "/"))

    objectName: "moveDialog"
    title: qsTr("Move “%1”").arg(name)
    acceptText: qsTr("Move here")
    acceptButton.objectName: "moveConfirm"
    acceptButton.enabled: !inside && place.location !== AppController.parentOf(path)
    onAccepted: AppController.moveTo(path, place.location)

    contentItem: ColumnLayout {
        spacing: 6
        PlacePicker {
            id: place
            Layout.fillWidth: true
            title: qsTr("Move to")
            alwaysOpen: true
            excluded: dialog.isFolder ? dialog.path : ""
        }
        Label {
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            visible: dialog.inside || place.location === AppController.parentOf(dialog.path)
            text: dialog.inside ? qsTr("A folder can't go inside itself.")
                                : qsTr("It is in this folder already. Open the folder to move it into.")
            color: Theme.textMuted
            font.pixelSize: 11
        }
    }
}

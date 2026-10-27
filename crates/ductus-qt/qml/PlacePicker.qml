import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Ductus

// Where something is saved: the place in full, and the folders to step into to pick another.
ColumnLayout {
    id: picker

    property string location
    property string title: qsTr("Saved in")
    /// The folder list shows right away, as when moving; otherwise behind Change….
    property bool alwaysOpen: false
    property bool changeable: true
    property bool browsing: alwaysOpen
    /// A folder that is being moved is no place to move it to.
    property string excluded
    readonly property var folders: browsing ? AppController.subfolders(location) : []

    spacing: 6

    Label {
        Layout.topMargin: 8
        text: picker.title
        color: Theme.textMuted
        font.pixelSize: 12
    }
    RowLayout {
        Layout.fillWidth: true
        spacing: 6
        Label {
            objectName: "placeWhere"
            Layout.fillWidth: true
            text: {
                AppController.libraryPath // placeLabel() follows it.
                return AppController.placeLabel(picker.location)
            }
            color: Theme.text
            font.pixelSize: 13
            elide: Text.ElideMiddle
        }
        RibbonButton {
            objectName: "placeChange"
            visible: picker.changeable && !picker.alwaysOpen
            text: picker.browsing ? qsTr("Done") : qsTr("Change…")
            onClicked: picker.browsing = !picker.browsing
        }
    }
    // Tapping a folder opens it; the place shown above is where it goes.
    ColumnLayout {
        visible: picker.browsing
        Layout.fillWidth: true
        spacing: 2
        RibbonButton {
            objectName: "placeUp"
            enabled: picker.location.length > 0
            iconName: "arrow-up"
            text: qsTr("Up one folder")
            onClicked: picker.location = AppController.parentOf(picker.location)
        }
        ListView {
            id: folderList
            objectName: "placeFolders"
            Layout.fillWidth: true
            Layout.preferredHeight: picker.alwaysOpen ? 200 : 150
            clip: true
            model: picker.folders
            boundsBehavior: Flickable.StopAtBounds
            ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }
            delegate: ItemDelegate {
                required property var modelData
                objectName: "placeFolder_" + modelData.name
                width: ListView.view.width
                height: 32
                enabled: modelData.path !== picker.excluded
                text: modelData.notebook.length === 0 ? modelData.name
                      : modelData.notebook === modelData.name ? qsTr("%1 (notebook)").arg(modelData.name)
                      : qsTr("%1 (notebook “%2”)").arg(modelData.name).arg(modelData.notebook)
                font.pixelSize: 13
                rightPadding: 28
                onClicked: picker.location = modelData.path
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
}

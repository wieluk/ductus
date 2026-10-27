import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Ductus

// What an import found, its name and where it is saved, before anything is written.
AppDialog {
    id: dialog

    property string kind
    property int pages
    property int sections
    property bool finished: false
    /// The library changes between imports while name and place may not.
    property int showCount: 0
    readonly property string name: nameField.text.trim()
    readonly property string savedAs: {
        showCount
        return name.length > 0 ? AppController.uniqueName(place.location, name) : ""
    }

    function show(what, title, pageCount, sectionCount, location) {
        kind = what
        pages = pageCount
        sections = sectionCount
        finished = false
        nameField.text = title
        place.location = location
        place.browsing = false
        ++showCount
        open()
    }

    objectName: "importDialog"
    title: qsTr("Import notes")
    acceptText: qsTr("Import")
    acceptButton.objectName: "importConfirm"
    acceptButton.enabled: name.length > 0
    onOpened: {
        nameField.forceActiveFocus()
        nameField.selectAll()
    }
    onAccepted: {
        finished = true
        AppController.finishImport(place.location, name)
    }
    // Closed any other way: nothing is saved.
    onClosed: if (!finished) AppController.cancelImport()

    contentItem: ColumnLayout {
        spacing: 6
        Label {
            objectName: "importSummary"
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            text: {
                const parts = [dialog.kind === "notebook" ? qsTr("Notebook")
                               : dialog.kind === "section" ? qsTr("Section") : qsTr("Page")]
                if (dialog.sections > 0)
                    parts.push(qsTr("Sections: %1").arg(dialog.sections))
                if (dialog.kind !== "page")
                    parts.push(qsTr("Pages: %1").arg(dialog.pages))
                return parts.join(" · ")
            }
            color: Theme.text
            font.pixelSize: 13
        }
        Label {
            Layout.topMargin: 8
            text: qsTr("Name")
            color: Theme.textMuted
            font.pixelSize: 12
        }
        TextField {
            id: nameField
            objectName: "importName"
            Layout.fillWidth: true
            selectByMouse: true
            onAccepted: if (dialog.acceptButton.enabled) dialog.accept()
        }
        PlacePicker {
            id: place
            Layout.fillWidth: true
        }
        Label {
            objectName: "importNote"
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            text: dialog.savedAs.length > 0 && dialog.savedAs !== dialog.name
                  ? qsTr("“%1” already exists there, so this is saved as “%2”. Nothing is overwritten.")
                        .arg(dialog.name).arg(dialog.savedAs)
                  : qsTr("The original file is only read, never changed.")
            color: Theme.textMuted
            font.pixelSize: 11
        }
    }
}

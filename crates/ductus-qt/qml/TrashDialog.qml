import QtQuick
import QtQuick.Controls
import QtQuick.Controls.impl
import QtQuick.Layouts
import Ductus

// Deleted pages, PDFs and folders: put back where they were, or gone for good.
AppDialog {
    id: dialog

    property var rows: []

    function show() {
        rows = AppController.trashRows()
        open()
    }

    function reload() {
        rows = AppController.trashRows()
    }

    objectName: "trashDialog"
    title: qsTr("Trash")
    acceptText: qsTr("Close")
    rejectText: ""
    acceptButton.objectName: "trashClose"

    contentItem: ColumnLayout {
        spacing: 8
        Label {
            Layout.fillWidth: true
            text: qsTr("Deleted items stay here, on this device, until you delete them for good.")
            wrapMode: Text.WordWrap
            color: Theme.textMuted
            font.pixelSize: 12
        }
        ListView {
            id: list
            objectName: "trashList"
            Layout.fillWidth: true
            Layout.preferredHeight: Math.min(320, Math.max(64, contentHeight))
            clip: true
            model: dialog.rows
            boundsBehavior: Flickable.StopAtBounds
            ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }
            delegate: Item {
                id: item
                required property var modelData
                required property int index
                width: ListView.view.width
                height: 52
                RowLayout {
                    anchors.fill: parent
                    spacing: 8
                    IconImage {
                        Layout.preferredWidth: 18
                        Layout.preferredHeight: 18
                        source: Qt.resolvedUrl("icons/" + (item.modelData.kind === "folder" ? "folder"
                                                          : item.modelData.kind === "pdf" ? "pdf" : "page") + ".svg")
                        sourceSize: Qt.size(18, 18)
                        color: Theme.textMuted
                    }
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 1
                        Label {
                            Layout.fillWidth: true
                            text: item.modelData.name
                            elide: Text.ElideRight
                            color: Theme.text
                            font.pixelSize: 13
                        }
                        Label {
                            Layout.fillWidth: true
                            text: {
                                AppController.libraryPath // displayName() follows it.
                                const from = item.modelData.location.length > 0 ? item.modelData.location
                                                                                : AppController.displayName("")
                                return qsTr("From %1 · %2").arg(from)
                                    .arg(item.modelData.deleted.toLocaleString(Qt.locale(), Locale.ShortFormat))
                            }
                            elide: Text.ElideMiddle
                            color: Theme.textMuted
                            font.pixelSize: 11
                        }
                    }
                    RibbonButton {
                        objectName: "trashRestore" + item.index
                        text: qsTr("Restore")
                        onClicked: {
                            AppController.restoreFromTrash(item.modelData.id)
                            dialog.reload()
                        }
                    }
                    RibbonButton {
                        objectName: "trashPurge" + item.index
                        iconName: "delete"
                        tip: qsTr("Delete for good")
                        onClicked: {
                            AppController.purgeFromTrash(item.modelData.id)
                            dialog.reload()
                        }
                    }
                }
            }
            Label {
                anchors.centerIn: parent
                visible: list.count === 0
                text: qsTr("The trash is empty.")
                color: Theme.textMuted
                font.pixelSize: 13
            }
        }
        RibbonButton {
            id: emptyButton
            objectName: "trashEmpty"
            Layout.alignment: Qt.AlignLeft
            enabled: list.count > 0
            iconName: "delete"
            property bool confirming: false
            text: confirming ? qsTr("Delete everything for good?") : qsTr("Empty trash")
            onClicked: {
                if (!confirming) {
                    confirming = true
                    return
                }
                confirming = false
                AppController.emptyTrash()
                dialog.reload()
            }
        }
    }
    onClosed: emptyButton.confirming = false
}

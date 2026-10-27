import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Ductus

AppDialog {
    id: dialog
    objectName: "updateDialog"

    readonly property string action: Updater.restartNeeded ? "restart"
        : ["flatpak", "installer", "android"].includes(Updater.channel) ? "install" : "page"

    title: Updater.restartNeeded ? qsTr("Ductus %1 is installed").arg(Updater.latestVersion)
                                 : qsTr("Ductus %1 is available").arg(Updater.latestVersion)
    preferredWidth: 500
    // The bar has Cancel; hiding the dialog lets the download go on.
    rejectText: Updater.downloading ? qsTr("Hide") : qsTr("Later")
    acceptText: ({ restart: qsTr("Restart now"), install: qsTr("Install"),
                   page: qsTr("Open download page") })[action]
    acceptButton.objectName: "updateAccept"
    acceptButton.enabled: !Updater.busy
    acceptButton.visible: !Updater.downloading
    // Installing runs in the background; the dialog stays to show it.
    closePolicy: Popup.CloseOnEscape

    property bool acted: false

    onAboutToShow: acted = Updater.busy || Updater.restartNeeded
    onAccepted: {
        if (action === "restart") {
            Updater.restart()
            return
        }
        Updater.install()
        // Downloads and installs report here; the others hand over to another program.
        if (action === "install") {
            Qt.callLater(dialog.open)
            Qt.callLater(() => dialog.acted = true)
        }
    }
    onRejected: if (!Updater.busy) Updater.dismiss()

    contentItem: ColumnLayout {
        spacing: 10
        Label {
            Layout.fillWidth: true
            Layout.preferredWidth: dialog.width - dialog.leftPadding - dialog.rightPadding
            text: qsTr("You have version %1.").arg(Updater.currentVersion)
            color: Theme.textMuted
            font.pixelSize: 13
            wrapMode: Text.WordWrap
        }
        ScrollView {
            Layout.fillWidth: true
            Layout.preferredHeight: Math.min(220, notes.implicitHeight)
            visible: Updater.notes.length > 0 && !Updater.restartNeeded
            clip: true
            Label {
                id: notes
                objectName: "updateNotes"
                width: dialog.width - dialog.leftPadding - dialog.rightPadding - 16
                text: Updater.notes
                textFormat: Text.MarkdownText
                wrapMode: Text.WordWrap
                color: Theme.text
                font.pixelSize: 13
            }
        }
        TransferProgress {
            objectName: "updateProgress"
            Layout.fillWidth: true
            visible: Updater.downloading
            label: Updater.status
            value: Updater.progress
            onCancelled: Updater.cancel()
        }
        Label {
            objectName: "updateStatus"
            Layout.fillWidth: true
            Layout.preferredWidth: dialog.width - dialog.leftPadding - dialog.rightPadding
            visible: dialog.acted && text.length > 0 && !Updater.downloading
            text: Updater.status
            color: Theme.textMuted
            font.pixelSize: 12
            wrapMode: Text.WordWrap
        }
    }
}

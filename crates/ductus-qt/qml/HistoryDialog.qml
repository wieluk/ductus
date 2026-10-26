import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Ductus

// Earlier versions of a page or PDF: put one back, or open it as a page of its own.
AppDialog {
    id: dialog

    property string path
    property var versions: []

    function show(page) {
        path = page
        versions = AppController.pageVersions(page)
        open()
    }

    function ago(date) {
        const minutes = Math.round((Date.now() - date.getTime()) / 60000)
        if (minutes < 1)
            return qsTr("just now")
        if (minutes < 60)
            return qsTr("%n minute(s) ago", "", minutes)
        const hours = Math.round(minutes / 60)
        if (hours < 24)
            return qsTr("%n hour(s) ago", "", hours)
        return qsTr("%n day(s) ago", "", Math.round(hours / 24))
    }

    objectName: "historyDialog"
    title: qsTr("Earlier versions of “%1”").arg(path.split("/").pop())
    acceptText: qsTr("Close")
    rejectText: ""
    acceptButton.objectName: "historyClose"

    contentItem: ColumnLayout {
        spacing: 8
        Label {
            Layout.fillWidth: true
            text: qsTr("Ductus keeps a version from each session and every 10 minutes while you work. Restoring keeps the current state as a version too.")
            wrapMode: Text.WordWrap
            color: Theme.textMuted
            font.pixelSize: 12
        }
        ListView {
            id: list
            objectName: "historyList"
            Layout.fillWidth: true
            Layout.preferredHeight: Math.min(320, Math.max(64, contentHeight))
            clip: true
            model: dialog.versions
            boundsBehavior: Flickable.StopAtBounds
            ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }
            delegate: Item {
                id: item
                required property var modelData
                required property int index
                width: ListView.view.width
                height: 48
                RowLayout {
                    anchors.fill: parent
                    spacing: 8
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 1
                        Label {
                            Layout.fillWidth: true
                            text: item.modelData.date.toLocaleString(Qt.locale(), Locale.ShortFormat)
                            elide: Text.ElideRight
                            color: Theme.text
                            font.pixelSize: 13
                        }
                        Label {
                            text: dialog.ago(item.modelData.date)
                            color: Theme.textMuted
                            font.pixelSize: 11
                        }
                    }
                    RibbonButton {
                        objectName: "historyRestore" + item.index
                        text: qsTr("Restore")
                        onClicked: {
                            if (AppController.restoreVersion(dialog.path, item.modelData.saved))
                                dialog.close()
                        }
                    }
                    RibbonButton {
                        objectName: "historyCopy" + item.index
                        iconName: "copy"
                        tip: qsTr("Open as a copy beside it")
                        onClicked: {
                            if (AppController.copyVersion(dialog.path, item.modelData.saved).length > 0)
                                dialog.close()
                        }
                    }
                }
            }
            Label {
                anchors.centerIn: parent
                width: list.width
                visible: list.count === 0
                horizontalAlignment: Text.AlignHCenter
                wrapMode: Text.WordWrap
                text: qsTr("No earlier versions yet. One is kept the first time it changes in a session.")
                color: Theme.textMuted
                font.pixelSize: 13
            }
        }
    }
}

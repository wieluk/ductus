import QtQuick
import QtQuick.Controls
import QtQuick.Controls.impl
import QtQuick.Layouts
import Ductus

Item {
    id: box

    property var results: []

    function focusSearch() {
        field.forceActiveFocus()
        field.selectAll()
        if (field.text.length > 0)
            popup.open()
    }

    function run() {
        results = AppController.search(field.text)
        list.currentIndex = results.length > 0 ? 0 : -1
        if (field.text.trim().length > 0)
            popup.open()
        else
            popup.close()
    }

    function openResult(index) {
        if (index < 0 || index >= results.length)
            return
        const hit = results[index]
        popup.close()
        field.focus = false
        AppController.openSearchHit(hit)
    }

    implicitWidth: 240
    implicitHeight: 28

    // Searching while the index fills up shows more as it arrives.
    Connections {
        target: AppController
        function onIndexStatusChanged() {
            if (popup.opened)
                box.run()
        }
    }

    Timer {
        id: debounce
        interval: 150
        onTriggered: box.run()
    }

    Rectangle {
        anchors.fill: parent
        radius: 6
        color: field.activeFocus ? Theme.popup : Theme.titleHover
        border.width: field.activeFocus ? 1 : 0
        border.color: Theme.accent
    }

    IconImage {
        id: icon
        anchors.left: parent.left
        anchors.leftMargin: 8
        anchors.verticalCenter: parent.verticalCenter
        width: 16
        height: 16
        sourceSize: Qt.size(16, 16)
        source: Qt.resolvedUrl("icons/search.svg")
        color: field.activeFocus ? Theme.textMuted : Theme.titleTextMuted
    }

    TextField {
        id: field
        objectName: "searchField"
        anchors.left: icon.right
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        leftPadding: 6
        rightPadding: 8
        background: null
        font.pixelSize: 13
        color: activeFocus ? Theme.text : Theme.titleText
        placeholderText: qsTr("Search (Ctrl+F)")
        placeholderTextColor: activeFocus ? Theme.textMuted : Theme.titleTextMuted
        selectByMouse: true
        onTextEdited: debounce.restart()
        Keys.onDownPressed: list.currentIndex = Math.min(list.currentIndex + 1, box.results.length - 1)
        Keys.onUpPressed: list.currentIndex = Math.max(list.currentIndex - 1, 0)
        Keys.onReturnPressed: box.openResult(list.currentIndex)
        Keys.onEnterPressed: box.openResult(list.currentIndex)
        Keys.onEscapePressed: {
            popup.close()
            field.focus = false
        }
    }

    Popup {
        id: popup
        objectName: "searchResults"
        y: box.height + 6
        x: Math.min(0, box.width - width)
        width: 480
        height: Math.min(440, column.implicitHeight + 16)
        padding: 8
        closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutsideParent
        background: Rectangle {
            radius: 10
            color: Theme.popup
            border.width: 1
            border.color: Theme.ribbonBorder
        }

        contentItem: ColumnLayout {
            id: column
            spacing: 4

            ListView {
                id: list
                objectName: "searchList"
                Layout.fillWidth: true
                Layout.preferredHeight: Math.min(contentHeight, 380)
                clip: true
                model: box.results
                boundsBehavior: Flickable.StopAtBounds
                ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }
                delegate: AbstractButton {
                    id: result
                    required property var modelData
                    required property int index
                    objectName: "searchResult" + index
                    width: ListView.view.width
                    height: content.implicitHeight + 12
                    onClicked: box.openResult(index)
                    background: Rectangle {
                        radius: 6
                        color: result.ListView.isCurrentItem || result.hovered ? Theme.hover : "transparent"
                    }
                    contentItem: RowLayout {
                        id: content
                        spacing: 10
                        IconImage {
                            Layout.alignment: Qt.AlignTop
                            Layout.topMargin: 2
                            Layout.leftMargin: 6
                            Layout.preferredWidth: 18
                            Layout.preferredHeight: 18
                            sourceSize: Qt.size(18, 18)
                            color: Theme.textMuted
                            source: Qt.resolvedUrl("icons/" + ({
                                title: result.modelData.path.toLowerCase().endsWith(".pdf") ? "pdf" : "page",
                                text: "text", math: "math", ink: "pen", pdf: "pdf"
                            })[result.modelData.kind] + ".svg")
                        }
                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 1
                            RowLayout {
                                Layout.fillWidth: true
                                Label {
                                    text: result.modelData.title
                                    color: Theme.text
                                    font.pixelSize: 13
                                    font.weight: Font.DemiBold
                                    elide: Text.ElideRight
                                    Layout.maximumWidth: 240
                                }
                                Label {
                                    Layout.fillWidth: true
                                    text: result.modelData.location
                                          + (result.modelData.kind === "pdf"
                                             ? "  ·  " + qsTr("page %1").arg(result.modelData.page + 1) : "")
                                    color: Theme.textMuted
                                    font.pixelSize: 11
                                    elide: Text.ElideMiddle
                                }
                            }
                            Label {
                                Layout.fillWidth: true
                                visible: result.modelData.kind !== "title"
                                text: result.modelData.snippet
                                textFormat: Text.StyledText
                                color: Theme.textMuted
                                font.pixelSize: 12
                                wrapMode: Text.WordWrap
                                maximumLineCount: 2
                                elide: Text.ElideRight
                            }
                        }
                    }
                }
            }

            Label {
                objectName: "searchStatus"
                Layout.fillWidth: true
                Layout.leftMargin: 6
                visible: text.length > 0
                text: AppController.indexing ? AppController.indexStatus
                      : AppController.recognitionStatus.length > 0 ? AppController.recognitionStatus
                      : (box.results.length === 0 ? qsTr("Nothing found.") : "")
                color: Theme.textMuted
                font.pixelSize: 12
            }
        }
    }
}

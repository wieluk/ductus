import QtCore
import QtQuick
import QtQuick.Controls
import QtQuick.Controls.impl
import QtQuick.Layouts
import Ductus

Item {
    id: box

    property var results: []
    /// "page" (the open page), "names" (all page names) or "all" (names and contents).
    property string scope: libraryScope
    /// The scope used when no page is open, and for Ctrl+K.
    property string libraryScope: "all"
    readonly property bool onPage: scope === "page"
    /// In use: a compact title bar widens the field only then.
    readonly property bool active: field.activeFocus || field.text.length > 0
    readonly property var scopes: [
        { key: "page", icon: "page", label: qsTr("This page"),
          tip: qsTr("Text, formulas, PDF text and handwriting on the open page (Ctrl+F)") },
        { key: "names", icon: "file", label: qsTr("Page names"),
          tip: qsTr("The names of all pages and PDFs") },
        { key: "all", icon: "notebooks", label: qsTr("All pages"),
          tip: qsTr("Names and contents of all pages and PDFs (Ctrl+K)") }
    ]
    readonly property var current: scopes.find(s => s.key === scope) || scopes[2]

    /// `page`: search the open page if there is one (Ctrl+F), else the whole library.
    function focusSearch(page) {
        scope = page && AppController.currentPage.length > 0 ? "page" : libraryScope
        field.forceActiveFocus()
        field.selectAll()
        run()
    }

    function setScope(key) {
        scope = key
        if (key !== "page")
            libraryScope = key
        field.forceActiveFocus()
        run()
    }

    function run() {
        results = AppController.search(field.text, scope)
        list.currentIndex = results.length > 0 ? 0 : -1
        if (field.activeFocus)
            popup.open()
    }

    /// On this page the results stay open, and the next match is picked for Enter.
    function openResult(index, advance) {
        if (index < 0 || index >= results.length)
            return
        const hit = results[index]
        if (onPage) {
            list.currentIndex = advance ? (index + 1) % results.length : index
            AppController.openSearchHit(hit)
            return
        }
        popup.close()
        field.focus = false
        AppController.openSearchHit(hit)
    }

    implicitWidth: 240
    implicitHeight: 28

    Settings {
        category: "search"
        property alias scope: box.libraryScope
    }

    // Searching while the index fills up shows more as it arrives.
    Connections {
        target: AppController
        function onIndexStatusChanged() {
            if (popup.opened)
                box.run()
        }
        function onCurrentPageChanged() {
            if (!box.onPage)
                return
            if (AppController.currentPage.length === 0)
                box.scope = box.libraryScope
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

    // Always says what is searched; a narrow field is little more than its icon.
    AbstractButton {
        id: chip
        objectName: "searchScope"
        anchors.left: parent.left
        anchors.top: parent.top
        anchors.bottom: parent.bottom
        leftPadding: 8
        rightPadding: showLabel ? 4 : 8
        focusPolicy: Qt.NoFocus
        readonly property bool showLabel: box.width >= 150
        readonly property color tint: field.activeFocus ? Theme.textMuted : Theme.titleTextMuted
        ToolTip.visible: hovered && showLabel
        ToolTip.delay: 600
        ToolTip.text: box.current.tip
        onClicked: {
            field.forceActiveFocus()
            box.run()
        }
        contentItem: Row {
            spacing: 4
            IconImage {
                anchors.verticalCenter: parent.verticalCenter
                width: 16
                height: 16
                sourceSize: Qt.size(16, 16)
                source: Qt.resolvedUrl("icons/search.svg")
                color: chip.tint
            }
            Label {
                objectName: "searchScopeLabel"
                anchors.verticalCenter: parent.verticalCenter
                visible: chip.showLabel
                text: box.current.label
                font.pixelSize: 12
                font.weight: Font.DemiBold
                color: field.activeFocus ? Theme.accent : Theme.titleText
            }
            IconImage {
                anchors.verticalCenter: parent.verticalCenter
                visible: chip.showLabel
                width: 12
                height: 12
                sourceSize: Qt.size(12, 12)
                rotation: 90
                source: Qt.resolvedUrl("icons/chevron.svg")
                color: chip.tint
            }
        }
    }

    Rectangle {
        id: divider
        visible: chip.showLabel
        anchors.left: chip.right
        anchors.verticalCenter: parent.verticalCenter
        width: 1
        height: parent.height - 12
        color: chip.tint
        opacity: 0.4
    }

    TextField {
        id: field
        objectName: "searchField"
        anchors.left: chip.showLabel ? divider.right : chip.right
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
        onActiveFocusChanged: if (activeFocus) popup.open()
        Keys.onDownPressed: list.currentIndex = Math.min(list.currentIndex + 1, box.results.length - 1)
        Keys.onUpPressed: list.currentIndex = Math.max(list.currentIndex - 1, 0)
        Keys.onReturnPressed: box.openResult(list.currentIndex, true)
        Keys.onEnterPressed: box.openResult(list.currentIndex, true)
        Keys.onEscapePressed: {
            popup.close()
            field.focus = false
        }
        // A text field would take Ctrl+K as "delete to the end of the line".
        Keys.onPressed: event => {
            if (event.key === Qt.Key_K && event.modifiers === Qt.ControlModifier) {
                box.focusSearch(false)
                event.accepted = true
            }
        }
    }

    Popup {
        id: popup
        objectName: "searchResults"
        y: box.height + 6
        // Right-aligned with the field, but never off the window's left edge.
        x: Math.max(8 - box.mapToItem(null, 0, 0).x, Math.min(0, box.width - width))
        width: Math.min(480, (Overlay.overlay ? Overlay.overlay.width : 496) - 16)
        height: Math.min(480, column.implicitHeight + 16)
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

            RowLayout {
                Layout.fillWidth: true
                spacing: 4
                Repeater {
                    model: box.scopes
                    RibbonButton {
                        required property var modelData
                        objectName: "searchScope_" + modelData.key
                        iconName: modelData.icon
                        text: modelData.label
                        tip: modelData.tip
                        checked: box.scope === modelData.key
                        enabled: modelData.key !== "page" || AppController.currentPage.length > 0
                        implicitHeight: 30
                        icon.width: 16
                        icon.height: 16
                        onClicked: box.setScope(modelData.key)
                    }
                }
            }

            Rectangle {
                Layout.fillWidth: true
                implicitHeight: 1
                color: Theme.separator
                visible: box.results.length > 0
            }

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
                    text: modelData.title
                    width: ListView.view.width
                    height: content.implicitHeight + 12
                    focusPolicy: Qt.NoFocus
                    onClicked: box.openResult(index, false)
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
                            // On this page its name and place would only repeat.
                            RowLayout {
                                Layout.fillWidth: true
                                visible: !box.onPage || result.modelData.kind === "pdf"
                                Label {
                                    visible: !box.onPage
                                    text: result.modelData.title
                                    color: Theme.text
                                    font.pixelSize: 13
                                    font.weight: Font.DemiBold
                                    elide: Text.ElideRight
                                    Layout.maximumWidth: 240
                                }
                                Label {
                                    Layout.fillWidth: true
                                    text: [box.onPage ? "" : result.modelData.location,
                                           result.modelData.kind === "pdf"
                                           ? qsTr("page %1").arg(result.modelData.page + 1) : ""]
                                          .filter(t => t.length > 0).join("  ·  ")
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
                                color: box.onPage ? Theme.text : Theme.textMuted
                                font.pixelSize: box.onPage ? 13 : 12
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
                text: field.text.trim().length === 0 ? box.current.tip
                      : AppController.indexing ? AppController.indexStatus
                      : AppController.recognitionStatus.length > 0 ? AppController.recognitionStatus
                      : (box.results.length === 0 ? qsTr("Nothing found.") : "")
                color: Theme.textMuted
                font.pixelSize: 12
            }
        }
    }
}

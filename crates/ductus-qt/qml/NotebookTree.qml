import QtQuick
import QtQuick.Controls
import QtQuick.Controls.impl
import QtQuick.Layouts
import QtQml.Models
import Ductus

Rectangle {
    id: sidebar

    property string renaming: ""
    /// The row just shown from elsewhere (location bar, a new folder), highlighted briefly.
    property string flashPath: ""
    Timer {
        id: flashTimer
        interval: 1500
        onTriggered: sidebar.flashPath = ""
    }
    property bool onRight: false

    /// Row menus open now; their rows must not be rebuilt under them.
    property int openMenus: 0
    HoverHandler { id: sidebarHover }
    Binding {
        target: AppController
        property: "holdRecent"
        value: sidebarHover.hovered || sidebar.openMenus > 0
    }

    // Dragging a row with mouse or pen (touch scrolls).
    property string dragging: ""
    property string draggingName: ""
    property point dragAt
    readonly property Item dropTarget: {
        if (dragging.length === 0)
            return null
        const item = list.itemAt(dragAt.x + list.contentX, dragAt.y + list.contentY)
        return item && item.path !== undefined && item.path !== dragging && !item.quick ? item : null
    }

    /// Onto a folder or a notebook's add row: into it. Onto a page: its place, or its folder.
    function dropDragged() {
        const target = dropTarget
        const path = dragging
        dragging = ""
        if (!target)
            return
        if (target.isFolder || target.placeholder) {
            if (target.path !== AppController.parentOf(path))
                AppController.moveTo(path, target.path)
            return
        }
        const folder = AppController.parentOf(target.path)
        if (folder !== AppController.parentOf(path)) {
            AppController.moveTo(path, folder)
            return
        }
        AppController.moveBeside(path, target.path)
    }

    signal deleteRequested(string path, string name, bool isFolder)
    signal openFolderRequested()
    signal collapseRequested()
    /// "" for the default place.
    signal newPageRequested(string folder)
    signal newNotebookRequested(string location)
    signal makeNotebookRequested(string folder)
    signal moveRequested(string path, string name, bool isFolder)
    signal historyRequested(string path)
    signal exportRequested(string folder, string name)

    color: Theme.sidebar

    Connections {
        target: AppController
        function onRenameRequested(path) {
            // A row just added gets its field now, so the field sees renaming start and takes focus.
            list.forceLayout()
            sidebar.renaming = path
        }
        function onRevealRequested(path) {
            const i = AppController.tree.treeIndexOf(path)
            if (i >= 0)
                list.positionViewAtIndex(i, ListView.Contain)
            sidebar.flashPath = path
            flashTimer.restart()
        }
    }

    function hasFilter(bit) { return (AppController.treeFilter & bit) !== 0 }
    function toggleFilter(bit) { AppController.treeFilter = AppController.treeFilter ^ bit }
    function lastName(path) { return path.split(/[\\/]/).pop() }
    function shownKinds() {
        const kinds = []
        if (hasFilter(1)) kinds.push(qsTr("notes pages"))
        if (hasFilter(2)) kinds.push(qsTr("PDFs"))
        if (hasFilter(4)) kinds.push(qsTr("other files"))
        return kinds.join(", ")
    }

    readonly property bool focused: AppController.treeFocus.length > 0
    readonly property var crumbs: {
        const out = [{ name: lastName(AppController.libraryPath), path: "" }]
        const focus = AppController.treeFocus
        if (focus.length > 0) {
            const parts = focus.split("/")
            for (let i = 0; i < parts.length; ++i)
                out.push({ name: parts[i], path: parts.slice(0, i + 1).join("/") })
        }
        return out
    }

    Rectangle {
        x: sidebar.onRight ? 0 : parent.width - width
        height: parent.height
        width: 1
        color: Theme.sidebarBorder
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.leftMargin: sidebar.onRight ? 1 : 0
        anchors.rightMargin: sidebar.onRight ? 0 : 1
        spacing: 0

        RowLayout {
            Layout.fillWidth: true
            Layout.leftMargin: 14
            Layout.rightMargin: 6
            Layout.topMargin: 6
            Layout.bottomMargin: 2
            spacing: 0

            Repeater {
                model: [{ notebooks: true, label: qsTr("Notebooks") },
                        { notebooks: false, label: qsTr("Folders") }]
                delegate: AbstractButton {
                    id: viewButton
                    required property var modelData
                    readonly property bool active: AppController.notebookView === modelData.notebooks
                    objectName: modelData.notebooks ? "viewNotebooks" : "viewFolders"
                    implicitHeight: 28
                    implicitWidth: viewLabel.implicitWidth + 12
                    // A narrow sidebar shortens the names rather than pushing the buttons out.
                    Layout.fillWidth: true
                    Layout.maximumWidth: implicitWidth
                    focusPolicy: Qt.NoFocus
                    onClicked: AppController.notebookView = modelData.notebooks
                    // Hidden once pressed, until the pointer leaves.
                    property bool tipDismissed: false
                    onPressed: tipDismissed = true
                    onHoveredChanged: if (!hovered) tipDismissed = false
                    ToolTip.visible: hovered && !tipDismissed
                    ToolTip.delay: 600
                    ToolTip.text: modelData.notebooks
                                  ? qsTr("All notebooks in your own order, with your favorites")
                                  : qsTr("The folders on disk, with PDFs and other files")
                    contentItem: Label {
                        id: viewLabel
                        text: viewButton.modelData.label
                        elide: Text.ElideRight
                        horizontalAlignment: Text.AlignHCenter
                        verticalAlignment: Text.AlignVCenter
                        color: viewButton.active ? Theme.accent : Theme.textMuted
                        font.pixelSize: 12
                        font.weight: Font.DemiBold
                    }
                    background: Rectangle {
                        radius: Theme.radius
                        color: viewButton.active ? Theme.checked : (viewButton.hovered ? Theme.hover : "transparent")
                    }
                }
            }
            RibbonButton {
                objectName: AppController.tree.anyExpanded ? "collapseAll" : "expandAll"
                iconName: AppController.tree.anyExpanded ? "collapse-all" : "expand-all"
                tip: AppController.tree.anyExpanded ? qsTr("Collapse all") : qsTr("Expand all")
                implicitHeight: 32
                implicitWidth: 24
                padding: 4
                icon.width: 16
                icon.height: 16
                onClicked: AppController.tree.anyExpanded ? AppController.collapseAll() : AppController.expandAll()
            }
            Item { Layout.fillWidth: true }
            RibbonButton {
                objectName: "treeFilter"
                iconName: "filter"
                tip: qsTr("Show…")
                implicitHeight: 32
                implicitWidth: 28
                checked: AppController.treeFilter !== AppController.defaultTreeFilter
                onClicked: filterMenu.popup(this, 0, height)

                AppMenu {
                    id: filterMenu
                    AppMenuItem {
                        objectName: "filterNotes"
                        text: qsTr("Notes pages")
                        checkable: true
                        checked: sidebar.hasFilter(1)
                        onTriggered: sidebar.toggleFilter(1)
                    }
                    AppMenuItem {
                        objectName: "filterPdfs"
                        text: qsTr("PDFs")
                        checkable: true
                        checked: sidebar.hasFilter(2)
                        onTriggered: sidebar.toggleFilter(2)
                    }
                    AppMenuItem {
                        objectName: "filterOther"
                        text: qsTr("Other files")
                        checkable: true
                        checked: sidebar.hasFilter(4)
                        onTriggered: sidebar.toggleFilter(4)
                    }
                    AppMenuSeparator { visible: !AppController.notebookView }
                    AppMenuItem {
                        visible: !AppController.notebookView
                        height: visible ? implicitHeight : 0
                        objectName: "filterHideEmpty"
                        text: qsTr("Hide folders without these")
                        checkable: true
                        checked: sidebar.hasFilter(8)
                        onTriggered: sidebar.toggleFilter(8)
                    }
                }
            }
            RibbonButton {
                objectName: "sidebarNewPage"
                iconName: "page-add"
                tip: qsTr("New page…")
                implicitHeight: 32
                implicitWidth: 28
                onClicked: sidebar.newPageRequested("")
            }
            RibbonButton {
                objectName: "sidebarNewNotebook"
                iconName: "plus"
                tip: qsTr("New notebook…")
                implicitHeight: 32
                implicitWidth: 28
                onClicked: sidebar.newNotebookRequested(AppController.defaultNotebookLocation())
            }
        }

        // What the list is narrowed to: a folder under Folders, one notebook under Notebooks.
        Rectangle {
            objectName: "focusBar"
            visible: AppController.notebookView || sidebar.focused
            Layout.fillWidth: true
            Layout.leftMargin: 6
            Layout.rightMargin: 6
            Layout.bottomMargin: 2
            implicitHeight: 32
            radius: Theme.radius
            color: sidebar.focused ? Theme.checked : "transparent"

            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: 2
                anchors.rightMargin: 2
                spacing: 2

                RibbonButton {
                    objectName: "focusUp"
                    visible: !AppController.notebookView
                    iconName: "arrow-up"
                    tip: qsTr("Up one folder")
                    implicitHeight: 28
                    implicitWidth: 28
                    padding: 4
                    onClicked: AppController.treeFocus = AppController.parentOf(AppController.treeFocus)
                }
                ListView {
                    id: crumbList
                    visible: !AppController.notebookView
                    Layout.fillWidth: true
                    Layout.preferredHeight: 28
                    orientation: ListView.Horizontal
                    clip: true
                    boundsBehavior: Flickable.StopAtBounds
                    model: sidebar.crumbs
                    // The deepest folder is the one that matters.
                    onCountChanged: positionViewAtEnd()
                    onWidthChanged: positionViewAtEnd()
                    delegate: AbstractButton {
                        id: crumb
                        required property var modelData
                        required property int index
                        readonly property bool last: index === crumbList.count - 1
                        objectName: "crumb_" + modelData.path
                        height: 28
                        implicitWidth: crumbLabel.implicitWidth + 8
                        focusPolicy: Qt.NoFocus
                        enabled: !last
                        onClicked: AppController.treeFocus = modelData.path
                        contentItem: Label {
                            id: crumbLabel
                            text: (crumb.index > 0 ? "› " : "") + crumb.modelData.name
                            verticalAlignment: Text.AlignVCenter
                            color: crumb.last ? Theme.text : crumb.hovered ? Theme.accent : Theme.textMuted
                            font.pixelSize: 12
                            font.weight: crumb.last ? Font.DemiBold : Font.Normal
                        }
                        background: Rectangle {
                            radius: Theme.radius
                            color: crumb.hovered && !crumb.last ? Theme.hover : "transparent"
                        }
                    }
                }

                AbstractButton {
                    id: switcher
                    objectName: "notebookSwitcher"
                    visible: AppController.notebookView
                    Layout.fillWidth: true
                    implicitHeight: 28
                    focusPolicy: Qt.NoFocus
                    onClicked: switcherMenu.popup(this, 0, height)
                    contentItem: RowLayout {
                        spacing: 6
                        IconImage {
                            Layout.leftMargin: 6
                            Layout.preferredWidth: 16
                            Layout.preferredHeight: 16
                            source: Qt.resolvedUrl("icons/notebooks.svg")
                            sourceSize: Qt.size(16, 16)
                            color: Theme.accent
                        }
                        Label {
                            objectName: "notebookSwitcherLabel"
                            Layout.fillWidth: true
                            text: sidebar.focused ? AppController.notebookName(AppController.treeFocus)
                                                  : qsTr("All notebooks")
                            elide: Text.ElideRight
                            color: Theme.text
                            font.pixelSize: 13
                            font.weight: Font.DemiBold
                        }
                        IconImage {
                            Layout.preferredWidth: 14
                            Layout.preferredHeight: 14
                            source: Qt.resolvedUrl("icons/chevron.svg")
                            sourceSize: Qt.size(14, 14)
                            color: Theme.textMuted
                            rotation: 90
                        }
                    }
                    background: Rectangle {
                        radius: Theme.radius
                        color: switcher.hovered ? Theme.hover : "transparent"
                    }

                    AppMenu {
                        id: switcherMenu
                        property var notebooks: []
                        onAboutToShow: notebooks = AppController.notebookList()
                        AppMenuItem {
                            objectName: "switchAllNotebooks"
                            text: qsTr("All notebooks")
                            checkable: true
                            checked: !sidebar.focused
                            onTriggered: AppController.treeFocus = ""
                        }
                        AppMenuSeparator {}
                        Instantiator {
                            model: switcherMenu.notebooks
                            delegate: AppMenuItem {
                                required property var modelData
                                objectName: "switchNotebook_" + modelData.path
                                text: modelData.name
                                checkable: true
                                checked: AppController.treeFocus === modelData.path
                                onTriggered: AppController.treeFocus = modelData.path
                            }
                            onObjectAdded: (index, item) => switcherMenu.insertItem(index + 2, item)
                            onObjectRemoved: (index, item) => switcherMenu.removeItem(item)
                        }
                    }
                }

                RibbonButton {
                    objectName: "focusClear"
                    visible: sidebar.focused
                    iconName: "close"
                    tip: qsTr("Show everything")
                    implicitHeight: 28
                    implicitWidth: 28
                    padding: 6
                    onClicked: AppController.treeFocus = ""
                }
            }
        }

        ListView {
            id: list
            objectName: "notebookList"
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            boundsBehavior: Flickable.StopAtBounds
            // Mouse and pen drag rows; touch and the wheel scroll.
            acceptedButtons: Qt.NoButton
            model: AppController.tree
            spacing: 1
            ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }

            // A row dragged to an edge scrolls the list towards rows out of view.
            Timer {
                readonly property int edge: 32
                interval: 16
                repeat: true
                running: sidebar.dragging.length > 0
                         && (sidebar.dragAt.y < edge || sidebar.dragAt.y > list.height - edge)
                onTriggered: {
                    const step = sidebar.dragAt.y < edge ? -12 : 12
                    const end = list.originY + Math.max(0, list.contentHeight - list.height)
                    list.contentY = Math.max(list.originY, Math.min(end, list.contentY + step))
                }
            }

            section.property: AppController.tree.hasSections ? "section" : ""
            section.delegate: Item {
                id: heading
                required property string section
                width: ListView.view.width
                height: 26
                Label {
                    anchors.left: parent.left
                    anchors.bottom: parent.bottom
                    anchors.bottomMargin: 2
                    leftPadding: 14
                    text: ({ favorites: qsTr("Favorites"), recent: qsTr("Recent"),
                             notebooks: qsTr("Notebooks") })[heading.section] || ""
                    color: Theme.textMuted
                    font.pixelSize: 11
                    font.weight: Font.DemiBold
                    font.capitalization: Font.AllUppercase
                }
                AbstractButton {
                    objectName: "clearRecent"
                    visible: heading.section === "recent"
                    anchors.right: parent.right
                    anchors.rightMargin: 12
                    anchors.bottom: parent.bottom
                    height: 22
                    implicitWidth: clearLabel.implicitWidth + 12
                    focusPolicy: Qt.NoFocus
                    onClicked: AppController.clearRecent()
                    contentItem: Label {
                        id: clearLabel
                        text: qsTr("Clear")
                        horizontalAlignment: Text.AlignHCenter
                        verticalAlignment: Text.AlignVCenter
                        color: parent.hovered ? Theme.accent : Theme.textMuted
                        font.pixelSize: 11
                    }
                    background: Rectangle {
                        radius: Theme.radius
                        color: parent.hovered ? Theme.hover : "transparent"
                    }
                }
            }

            delegate: Item {
                id: row

                required property int index
                required property string name
                required property string path
                required property int depth
                required property bool isFolder
                required property bool isPdf
                required property string color
                required property bool hasChildren
                required property bool expanded
                required property bool conflict
                required property bool isCurrent
                required property bool isFile
                required property string subtitle
                required property bool pinned
                required property bool quick
                required property bool isNotebook
                required property string section
                required property string notebookName
                required property string kind
                required property string placeholder
                // Folders shows which folders are notebooks under another name.
                readonly property string caption: subtitle.length > 0 ? subtitle
                    : isNotebook && !AppController.notebookView && notebookName !== name
                      ? qsTr("Notebook \u201c%1\u201d").arg(notebookName) : ""

                readonly property bool isSubject: isFolder && depth === 0 && kind === ""
                // Folders narrows to any folder, Notebooks to one notebook.
                readonly property bool canFocus: !quick && !placeholder
                    && (AppController.notebookView ? isNotebook && depth === 0 : isFolder)
                readonly property color mark: color.length > 0 ? color : Theme.accent

                objectName: (placeholder === "section" ? "addSection_" : placeholder ? "empty_"
                             : section === "recent" ? "recent_" : quick ? "quick_" : "row_") + path
                Accessible.role: Accessible.ListItem
                Accessible.name: name
                width: ListView.view.width
                height: caption.length > 0 ? 42 : 34

                function activate() {
                    if (row.placeholder === "section")
                        AppController.newFolder(row.path, true)
                    else if (row.placeholder)
                        sidebar.newPageRequested(row.path)
                    else if (row.isFolder && row.quick)
                        AppController.reveal(row.path)
                    else if (row.isFolder)
                        AppController.toggleExpanded(row.path)
                    else if (row.isFile)
                        AppController.openExternally(row.path)
                    else
                        AppController.openPage(row.path)
                }

                HoverHandler { id: hover }
                DragHandler {
                    id: dragger
                    target: null
                    enabled: !row.quick && !row.placeholder && sidebar.renaming !== row.path
                    // Laptops report their pointer as a touchpad, and the pen arrives through it.
                    acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad | PointerDevice.Stylus
                    onActiveChanged: {
                        if (active) {
                            sidebar.dragging = row.path
                            sidebar.draggingName = row.isNotebook && row.notebookName.length > 0 ? row.notebookName : row.name
                        } else if (sidebar.dragging === row.path) {
                            sidebar.dropDragged()
                        }
                    }
                    onCentroidChanged: if (active) sidebar.dragAt = row.mapToItem(list, centroid.position)
                }
                readonly property bool isPage: !isFolder && !isFile && !placeholder
                TapHandler {
                    acceptedButtons: Qt.LeftButton
                    acceptedModifiers: Qt.NoModifier
                    onTapped: row.activate()
                    onDoubleTapped: if (row.isPage) AppController.keepTab(row.path)
                    // At the touch point: touch does not move the cursor.
                    onLongPressed: if (!row.placeholder) rowMenu.popup(point.position.x, point.position.y)
                }
                // Like a link in a browser: Ctrl or the middle button opens a new tab.
                TapHandler {
                    acceptedButtons: Qt.LeftButton
                    acceptedModifiers: Qt.ControlModifier
                    onTapped: row.isPage ? AppController.openInNewTab(row.path) : row.activate()
                }
                TapHandler {
                    acceptedButtons: Qt.MiddleButton
                    enabled: row.isPage
                    onTapped: AppController.openInNewTab(row.path)
                }
                TapHandler {
                    acceptedButtons: Qt.RightButton
                    enabled: !row.placeholder
                    onTapped: (eventPoint) => rowMenu.popup(eventPoint.position.x, eventPoint.position.y)
                }

                Rectangle {
                    anchors.fill: parent
                    anchors.leftMargin: 6
                    anchors.rightMargin: 6
                    radius: Theme.radius
                    color: row.isCurrent ? Theme.checked
                         : sidebar.dropTarget === row ? Theme.checked
                         : row.path === sidebar.flashPath && !row.quick && !row.placeholder ? Theme.checked
                         : hover.hovered ? Theme.hover : "transparent"
                    Behavior on color { ColorAnimation { duration: 250 } }
                }
                Rectangle {
                    visible: row.isCurrent
                    x: 6
                    width: 3
                    height: parent.height - 12
                    anchors.verticalCenter: parent.verticalCenter
                    radius: 1.5
                    color: row.mark
                }

                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: 12 + row.depth * 16
                    anchors.rightMargin: 8
                    spacing: 6

                    IconImage {
                        visible: !row.quick
                        Layout.preferredWidth: 14
                        Layout.preferredHeight: 14
                        source: row.isFolder && row.hasChildren ? Qt.resolvedUrl("icons/chevron.svg") : ""
                        sourceSize: Qt.size(14, 14)
                        color: Theme.textMuted
                        rotation: row.expanded ? 90 : 0
                    }
                    Rectangle {
                        visible: row.isSubject && !row.isNotebook
                        Layout.preferredWidth: 12
                        Layout.preferredHeight: 12
                        radius: 3
                        color: row.mark
                    }
                    IconImage {
                        visible: (!row.isSubject || row.isNotebook) && row.placeholder !== "page"
                        Layout.preferredWidth: 16
                        Layout.preferredHeight: 16
                        source: Qt.resolvedUrl(row.placeholder ? "icons/plus.svg"
                                               : row.isNotebook ? "icons/notebooks.svg"
                                               : row.kind === "section" ? "icons/section.svg"
                                               : row.kind === "group" ? "icons/section-group.svg"
                                               : row.isFolder ? "icons/folder.svg"
                                               : row.isPdf ? "icons/pdf.svg"
                                               : row.isFile ? "icons/file.svg" : "icons/page.svg")
                        sourceSize: Qt.size(16, 16)
                        color: row.isFolder ? row.mark : Theme.textMuted
                    }

                    ColumnLayout {
                        visible: sidebar.renaming !== row.path || row.quick || row.placeholder
                        Layout.fillWidth: true
                        spacing: 0
                        Label {
                            Layout.fillWidth: true
                            text: row.placeholder === "section" ? qsTr("New section")
                                  : row.placeholder ? qsTr("No pages yet. Tap to add one.") : row.name
                            elide: Text.ElideRight
                            color: row.isFile || row.placeholder ? Theme.textMuted : Theme.text
                            font.italic: row.placeholder === "page"
                            font.pixelSize: 13
                            font.weight: (row.isSubject && !row.quick) || row.isCurrent ? Font.DemiBold : Font.Normal
                        }
                        Label {
                            visible: row.caption.length > 0
                            Layout.fillWidth: true
                            text: row.caption
                            // The nearest folders tell same-named ones apart.
                            elide: Text.ElideLeft
                            color: Theme.textMuted
                            font.pixelSize: 11
                        }
                    }
                    TextField {
                        id: nameField
                        objectName: "renameField"
                        readonly property string problem: AppController.nameProblem(text)
                        visible: sidebar.renaming === row.path && !row.quick && !row.placeholder
                        Layout.fillWidth: true
                        text: row.name
                        font.pixelSize: 13
                        color: Theme.text
                        selectByMouse: true
                        ToolTip.visible: visible && problem.length > 0
                        ToolTip.text: problem
                        background: Rectangle {
                            radius: 4
                            color: Theme.popup
                            border.width: 1
                            border.color: nameField.problem.length > 0 ? Theme.danger : Theme.accent
                        }
                        onVisibleChanged: {
                            if (visible) {
                                forceActiveFocus()
                                selectAll()
                            } else if (activeFocus) {
                                // A hidden field must not keep typing and shortcuts (Ctrl+V).
                                sidebar.forceActiveFocus()
                            }
                        }
                        function commit() {
                            // Enter keeps a name that can't be used open for fixing.
                            if (sidebar.renaming !== row.path || problem.length > 0)
                                return
                            sidebar.renaming = ""
                            if (text.trim() === row.name)
                                return
                            AppController.rename(row.path, text)
                        }
                        onAccepted: commit()
                        onActiveFocusChanged: {
                            if (activeFocus)
                                return
                            // Clicking away from a name that can't be used keeps the old one.
                            if (problem.length > 0 && sidebar.renaming === row.path)
                                sidebar.renaming = ""
                            else
                                commit()
                        }
                        Keys.onEscapePressed: sidebar.renaming = ""
                    }

                    IconImage {
                        visible: row.conflict
                        Layout.preferredWidth: 16
                        Layout.preferredHeight: 16
                        source: Qt.resolvedUrl("icons/warning.svg")
                        sourceSize: Qt.size(16, 16)
                        color: "#e08a00"
                        HoverHandler { id: conflictHover }
                        ToolTip.visible: conflictHover.hovered
                        ToolTip.text: qsTr("Sync conflict copy")
                    }
                    RibbonButton {
                        objectName: "focus_" + row.path
                        visible: row.canFocus && hover.hovered && AppController.treeFocus !== row.path
                        iconName: AppController.notebookView ? "notebooks" : "folder-open"
                        tip: AppController.notebookView ? qsTr("Show only this notebook") : qsTr("Show only this folder")
                        implicitWidth: 26
                        implicitHeight: 26
                        padding: 3
                        onClicked: AppController.treeFocus = row.path
                    }
                    RibbonButton {
                        objectName: "more_" + row.path
                        visible: (hover.hovered || row.isCurrent || rowMenu.visible) && !row.placeholder
                        iconName: "more"
                        tip: qsTr("More")
                        implicitWidth: 26
                        implicitHeight: 26
                        padding: 3
                        onClicked: rowMenu.popup(this, 0, height)
                    }
                }

                AppMenu {
                    id: rowMenu
                    objectName: "rowMenu"

                    property int moves: 0
                    onAboutToShow: {
                        moves = AppController.moveOptions(row.path)
                        sidebar.openMenus++
                    }
                    onAboutToHide: sidebar.openMenus--
                    Component.onDestruction: if (visible) sidebar.openMenus--

                    AppMenuItem {
                        visible: row.isPage
                        height: visible ? implicitHeight : 0
                        text: qsTr("Open in new tab")
                        objectName: "menuOpenInNewTab"
                        onTriggered: AppController.openInNewTab(row.path)
                    }
                    AppMenuItem {
                        visible: row.isPage
                        height: visible ? implicitHeight : 0
                        text: qsTr("Open side by side")
                        objectName: "menuOpenBeside"
                        onTriggered: AppController.openBeside(row.path)
                    }
                    AppMenuItem {
                        visible: row.isFile
                        height: visible ? implicitHeight : 0
                        text: qsTr("Open with its program")
                        objectName: "menuOpenExternally"
                        onTriggered: AppController.openExternally(row.path)
                    }
                    AppMenuItem {
                        visible: row.canFocus && AppController.treeFocus !== row.path
                        height: visible ? implicitHeight : 0
                        text: AppController.notebookView ? qsTr("Show only this notebook") : qsTr("Show only this folder")
                        objectName: "menuFocus"
                        onTriggered: AppController.treeFocus = row.path
                    }
                    AppMenuItem {
                        visible: row.isFolder
                        height: visible ? implicitHeight : 0
                        text: qsTr("New page…")
                        objectName: "menuNewPage"
                        onTriggered: sidebar.newPageRequested(row.path)
                    }
                    AppMenuItem {
                        visible: row.isFolder && !AppController.notebookView
                        height: visible ? implicitHeight : 0
                        text: qsTr("New notebook inside…")
                        objectName: "menuNewNotebook"
                        onTriggered: sidebar.newNotebookRequested(row.path)
                    }
                    AppMenuItem {
                        visible: row.isFolder
                        height: visible ? implicitHeight : 0
                        text: AppController.notebookView ? qsTr("New section") : qsTr("New folder inside")
                        objectName: "menuNewFolder"
                        // Beside a section, inside a notebook or group.
                        onTriggered: AppController.notebookView
                                     ? AppController.newFolder(row.kind === "section" ? AppController.parentOf(row.path) : row.path, true)
                                     : AppController.newFolder(row.path)
                    }
                    AppMenuItem {
                        visible: row.isFolder && !row.isNotebook && !AppController.notebookView
                        height: visible ? implicitHeight : 0
                        text: qsTr("Make this a notebook…")
                        objectName: "menuMakeNotebook"
                        onTriggered: sidebar.makeNotebookRequested(row.path)
                    }
                    AppMenuItem {
                        visible: row.isNotebook && !row.quick
                        height: visible ? implicitHeight : 0
                        text: qsTr("No longer a notebook")
                        objectName: "menuUnmarkNotebook"
                        onTriggered: AppController.setNotebook(row.path, "")
                    }
                    AppMenuItem {
                        text: row.pinned ? qsTr("Remove from favorites") : qsTr("Add to favorites")
                        objectName: "menuPin"
                        onTriggered: AppController.setPinned(row.path, !row.pinned)
                    }
                    AppMenuItem {
                        visible: AppController.notebookView && (row.quick || row.isNotebook)
                        height: visible ? implicitHeight : 0
                        text: qsTr("Show in folders")
                        objectName: "menuReveal"
                        onTriggered: AppController.reveal(row.path)
                    }
                    AppMenuItem {
                        visible: !AppController.notebookView && row.isNotebook
                        height: visible ? implicitHeight : 0
                        text: qsTr("Show in notebooks")
                        objectName: "menuShowNotebook"
                        onTriggered: AppController.showInNotebooks(row.path)
                    }
                    AppMenuItem {
                        text: qsTr("Show in file manager")
                        objectName: "menuShowInFolder"
                        onTriggered: AppController.showInFolder(row.path)
                    }
                    AppMenuSeparator {}
                    AppMenuItem {
                        visible: !row.quick
                        height: visible ? implicitHeight : 0
                        text: qsTr("Rename")
                        objectName: "menuRename"
                        onTriggered: sidebar.renaming = row.path
                    }
                    MenuItem {
                        visible: row.isFolder && !row.quick
                        height: visible ? 40 : 0
                        objectName: "menuColors"
                        contentItem: Row {
                            spacing: 0
                            leftPadding: 6
                            Repeater {
                                model: Theme.colors
                                delegate: ColorSwatch {
                                    required property string modelData
                                    required property int index
                                    objectName: "folderColor" + index
                                    swatch: modelData
                                    selected: Qt.colorEqual(row.mark, modelData)
                                    onClicked: {
                                        AppController.setColor(row.path, modelData)
                                        rowMenu.close()
                                    }
                                }
                            }
                        }
                        background: null
                    }
                    AppMenuSeparator { visible: !row.quick; height: visible ? implicitHeight : 0 }
                    AppMenuItem {
                        visible: !row.quick
                        height: visible ? implicitHeight : 0
                        text: qsTr("Move up")
                        objectName: "menuMoveUp"
                        enabled: (rowMenu.moves & 1) !== 0
                        onTriggered: AppController.moveBy(row.path, -1)
                    }
                    AppMenuItem {
                        visible: !row.quick
                        height: visible ? implicitHeight : 0
                        text: qsTr("Move down")
                        objectName: "menuMoveDown"
                        enabled: (rowMenu.moves & 2) !== 0
                        onTriggered: AppController.moveBy(row.path, 1)
                    }
                    AppMenuItem {
                        visible: !row.quick
                        height: visible ? implicitHeight : 0
                        text: qsTr("Move to…")
                        objectName: "menuMoveTo"
                        onTriggered: sidebar.moveRequested(row.path, row.isNotebook && row.notebookName.length > 0
                                                                     ? row.notebookName : row.name, row.isFolder)
                    }
                    AppMenuItem {
                        visible: row.isFolder
                        height: visible ? implicitHeight : 0
                        text: qsTr("Export as PDF…")
                        objectName: "menuExportFolder"
                        onTriggered: sidebar.exportRequested(row.path, row.isNotebook && row.notebookName.length > 0
                                                                       ? row.notebookName : row.name)
                    }
                    AppMenuItem {
                        visible: !row.isFolder && !row.isFile
                        height: visible ? implicitHeight : 0
                        text: qsTr("Earlier versions…")
                        objectName: "menuHistory"
                        onTriggered: sidebar.historyRequested(row.path)
                    }
                    AppMenuSeparator {}
                    AppMenuItem {
                        text: qsTr("Delete…")
                        objectName: "menuDelete"
                        onTriggered: sidebar.deleteRequested(row.path, row.name, row.isFolder)
                    }
                }
            }

            ColumnLayout {
                readonly property int flags: AppController.treeFilter
                // Under Notebooks the filter only picks the pages inside them.
                readonly property bool filtered: !AppController.notebookView
                                                 && ((flags & 7) !== 7 && flags !== AppController.defaultTreeFilter
                                                     || (flags & 8) !== 0)
                anchors.centerIn: parent
                width: parent.width - 40
                visible: list.count === 0
                spacing: 8
                Label {
                    objectName: "emptyHint"
                    Layout.fillWidth: true
                    horizontalAlignment: Text.AlignHCenter
                    wrapMode: Text.WordWrap
                    color: Theme.textMuted
                    text: parent.filtered
                          ? qsTr("Nothing matches the filter: only %1 are shown.").arg(sidebar.shownKinds())
                          : AppController.notebookView
                          ? qsTr("No notebooks yet.\nCreate one with the + button, or make a folder a notebook under Folders.")
                          : qsTr("Nothing to show in this folder.")
                }
                RibbonButton {
                    objectName: "showEverything"
                    visible: parent.filtered
                    Layout.alignment: Qt.AlignHCenter
                    iconName: "filter"
                    text: qsTr("Show all files")
                    onClicked: AppController.treeFilter = 7
                }
            }
        }

        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 1
            color: Theme.sidebarBorder
        }
        RowLayout {
            Layout.fillWidth: true
            Layout.leftMargin: 6
            Layout.rightMargin: 6
            Layout.topMargin: 4
            Layout.bottomMargin: 4
            RibbonButton {
                objectName: "sidebarCollapse"
                iconName: "sidebar"
                tip: qsTr("Show or hide notebooks (%1)").arg("Ctrl+\\")
                implicitHeight: 30
                implicitWidth: 30
                onClicked: sidebar.collapseRequested()
            }
            Label {
                objectName: "libraryLabel"
                Layout.fillWidth: true
                text: AppController.libraryDisplayPath
                elide: Text.ElideMiddle
                color: Theme.textMuted
                font.pixelSize: 11
                ToolTip.visible: libraryHover.hovered
                ToolTip.text: AppController.libraryDisplayPath
                HoverHandler { id: libraryHover }
            }
            RibbonButton {
                objectName: "openLibrary"
                iconName: "folder-open"
                tip: qsTr("Open library folder…")
                implicitHeight: 30
                implicitWidth: 30
                onClicked: sidebar.openFolderRequested()
            }
        }
    }

    // What is being dragged, beside the pointer.
    Rectangle {
        readonly property point at: sidebar.dragging.length > 0 ? list.mapToItem(sidebar, sidebar.dragAt) : Qt.point(0, 0)
        objectName: "dragLabel"
        visible: sidebar.dragging.length > 0
        z: 10
        x: Math.min(at.x + 14, sidebar.width - width - 4)
        y: at.y + 10
        width: dragText.implicitWidth + 16
        height: dragText.implicitHeight + 8
        radius: Theme.radius
        color: Theme.popup
        border.width: 1
        border.color: Theme.accent
        Label {
            id: dragText
            anchors.centerIn: parent
            text: sidebar.draggingName
            color: Theme.text
            font.pixelSize: 12
        }
    }
}

import QtQuick

Item {
    id: root

    required property Window window
    readonly property int grip: 6

    component Grip: MouseArea {
        required property int edges
        hoverEnabled: true
        onPressed: root.window.startSystemResize(edges)
    }

    Grip { edges: Qt.LeftEdge; cursorShape: Qt.SizeHorCursor
        x: 0; y: root.grip; width: root.grip; height: parent.height - 2 * root.grip }
    Grip { edges: Qt.RightEdge; cursorShape: Qt.SizeHorCursor
        x: parent.width - root.grip; y: root.grip; width: root.grip; height: parent.height - 2 * root.grip }
    Grip { edges: Qt.TopEdge; cursorShape: Qt.SizeVerCursor
        x: root.grip; y: 0; width: parent.width - 2 * root.grip; height: root.grip }
    Grip { edges: Qt.BottomEdge; cursorShape: Qt.SizeVerCursor
        x: root.grip; y: parent.height - root.grip; width: parent.width - 2 * root.grip; height: root.grip }
    Grip { edges: Qt.TopEdge | Qt.LeftEdge; cursorShape: Qt.SizeFDiagCursor
        x: 0; y: 0; width: root.grip; height: root.grip }
    Grip { edges: Qt.TopEdge | Qt.RightEdge; cursorShape: Qt.SizeBDiagCursor
        x: parent.width - root.grip; y: 0; width: root.grip; height: root.grip }
    Grip { edges: Qt.BottomEdge | Qt.LeftEdge; cursorShape: Qt.SizeBDiagCursor
        x: 0; y: parent.height - root.grip; width: root.grip; height: root.grip }
    Grip { edges: Qt.BottomEdge | Qt.RightEdge; cursorShape: Qt.SizeFDiagCursor
        x: parent.width - root.grip; y: parent.height - root.grip; width: root.grip; height: root.grip }
}

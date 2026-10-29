import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Ductus

// A name for a page, folder or notebook, checked while typing: a dialog can't take one that fails.
ColumnLayout {
    id: box

    property alias text: field.text
    property alias fieldName: field.objectName
    /// Off for names that are only shown, not used for a file or folder.
    property bool fileName: true
    readonly property string problem: fileName ? AppController.nameProblem(field.text) : ""
    readonly property bool valid: field.text.trim().length > 0 && problem.length === 0

    signal accepted()

    function focusAll() {
        field.forceActiveFocus()
        field.selectAll()
    }

    spacing: 4

    TextField {
        id: field
        Layout.fillWidth: true
        selectByMouse: true
        onAccepted: if (box.valid) box.accepted()
    }
    Label {
        objectName: field.objectName + "Problem"
        Layout.fillWidth: true
        visible: box.problem.length > 0
        text: box.problem
        color: Theme.danger
        font.pixelSize: 12
        wrapMode: Text.WordWrap
    }
}

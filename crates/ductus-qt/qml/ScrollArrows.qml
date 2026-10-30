import QtQuick
import QtQuick.Controls
import Ductus

// Over a row that scrolls sideways: arrows at the ends show there is more, and scroll to it.
// The mouse wheel scrolls it sideways too.
Item {
    id: arrows

    required property Flickable target
    /// The background behind the row, which the ends fade into.
    property color color: Theme.ribbon
    property bool onTitleBar: false

    anchors.fill: target
    readonly property bool more: target.contentWidth > target.width + 1

    function scrollBy(delta) {
        const end = Math.max(0, target.contentWidth - target.width)
        slide.to = Math.max(0, Math.min(end, target.contentX + delta))
        slide.restart()
    }

    NumberAnimation {
        id: slide
        target: arrows.target
        property: "contentX"
        duration: 160
        easing.type: Easing.OutCubic
    }

    WheelHandler {
        target: null
        parent: arrows.target
        enabled: arrows.more
        acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
        onWheel: (event) => {
            const delta = Math.abs(event.angleDelta.x) > Math.abs(event.angleDelta.y) ? event.angleDelta.x
                                                                                      : event.angleDelta.y
            arrows.scrollBy(-delta)
        }
    }

    component End: Rectangle {
        id: end
        property bool atEnd: false
        visible: arrows.more && (atEnd ? arrows.target.contentX < arrows.target.contentWidth - arrows.target.width - 1
                                       : arrows.target.contentX > 1)
        width: 28
        height: arrows.height
        x: atEnd ? arrows.width - width : 0
        gradient: Gradient {
            orientation: Gradient.Horizontal
            GradientStop { position: end.atEnd ? 0 : 0.6; color: end.atEnd ? "transparent" : arrows.color }
            GradientStop { position: end.atEnd ? 0.4 : 1; color: end.atEnd ? arrows.color : "transparent" }
        }
        RibbonButton {
            objectName: end.atEnd ? "scrollRight" : "scrollLeft"
            anchors.verticalCenter: parent.verticalCenter
            x: end.atEnd ? parent.width - width : 0
            implicitWidth: 22
            implicitHeight: 28
            iconName: "chevron"
            onTitleBar: arrows.onTitleBar
            icon.width: 14
            icon.height: 14
            rotation: end.atEnd ? 0 : 180
            tip: end.atEnd ? qsTr("More") : qsTr("Back")
            onClicked: arrows.scrollBy((end.atEnd ? 1 : -1) * arrows.width * 0.7)
        }
    }

    End {}
    End { atEnd: true }
}

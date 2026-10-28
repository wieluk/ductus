pragma Singleton
import QtQuick

QtObject {
    property bool dark: false

    readonly property color accent: dark ? "#a395f0" : "#5a43c0"
    readonly property color titleBar: dark ? "#1c1a24" : "#5a43c0"
    readonly property color titleText: "#ffffff"
    readonly property color titleTextMuted: dark ? "#a9a5bd" : "#d9d2fb"
    readonly property color titleHover: dark ? "#2e2b3a" : "#6b55cf"
    readonly property color titleTabActive: dark ? "#2a2833" : "#f6f5f9"

    readonly property color ribbon: dark ? "#2a2833" : "#f6f5f9"
    readonly property color ribbonBorder: dark ? "#3b3847" : "#e0dde9"
    readonly property color text: dark ? "#ecebf2" : "#1f1d26"
    readonly property color textMuted: dark ? "#a29fb2" : "#6a6779"
    /// Under ink swatches on dark bars: pens show as they write on the light page.
    readonly property color swatchPaper: "#f4f3f8"
    // Still readable: disabled options must be legible, just quieter.
    readonly property color textDisabled: dark ? "#8b879c" : "#9e9bab"
    readonly property color hover: dark ? "#383545" : "#e9e6f3"
    readonly property color checked: dark ? "#4a4463" : "#ddd6f7"
    readonly property color separator: dark ? "#45414f" : "#d6d2e2"
    readonly property color popup: dark ? "#2f2c3a" : "#ffffff"
    /// Text fields and lists inside popups.
    readonly property color field: dark ? "#25232d" : "#ffffff"
    readonly property color overlayBg: "#d0181620"
    readonly property color sidebar: dark ? "#24222c" : "#efedf5"
    readonly property color sidebarBorder: dark ? "#34313f" : "#dedbe8"
    readonly property color desk: dark ? "#1b1a21" : "#dcdbe3"
    readonly property color danger: dark ? "#ff6b6b" : "#c62828"
    readonly property color toast: dark ? "#f2f1f7" : "#2a2733"
    readonly property color toastText: dark ? "#1f1d26" : "#ffffff"
    readonly property color toastAction: dark ? "#5a43c0" : "#b8a9ff"

    readonly property int titleHeight: 36
    readonly property int ribbonHeight: 48
    readonly property int buttonSize: 36
    readonly property int radius: 6
    readonly property int sidebarWidth: 280
}

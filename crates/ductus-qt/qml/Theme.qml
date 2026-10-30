pragma Singleton
import QtQuick

QtObject {
    property bool dark: false
    /// The app color the user picked; the greys take on its hue.
    property color base: "#5a43c0"
    /// The colors offered for the app and for folders.
    readonly property var colors: ["#5a43c0", "#1f4fd1", "#0a8a8a", "#1d8a3b", "#e07b00", "#d12f2f", "#c2185b", "#6a6779"]

    readonly property real hue: Math.max(0, base.hslHue)
    // A grey app color gives grey surfaces too.
    readonly property real chroma: Math.min(1, base.hslSaturation / 0.5)
    function tone(saturation, lightness) { return Qt.hsla(hue, saturation * chroma, lightness, 1) }
    function luminance(c) {
        const linear = v => v <= 0.03928 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4)
        return 0.2126 * linear(c.r) + 0.7152 * linear(c.g) + 0.0722 * linear(c.b)
    }
    // White title text stays readable: darker until it contrasts 3:1, which a yellow at half lightness doesn't.
    readonly property color accentLight: {
        let lightness = Math.min(base.hslLightness, 0.51)
        let c = Qt.hsla(hue, base.hslSaturation, lightness, 1)
        while (lightness > 0.05 && 1.05 / (luminance(c) + 0.05) < 3) {
            lightness -= 0.02
            c = Qt.hsla(hue, base.hslSaturation, lightness, 1)
        }
        return c
    }
    readonly property color accentDark: Qt.hsla(hue, Math.min(1, base.hslSaturation * 1.5), 0.76, 1)

    readonly property color accent: dark ? accentDark : accentLight
    readonly property color titleBar: dark ? tone(0.16, 0.122) : accentLight
    readonly property color titleText: "#ffffff"
    readonly property color titleTextMuted: dark ? tone(0.15, 0.694) : tone(0.84, 0.904)
    readonly property color titleHover: dark ? tone(0.15, 0.198)
                                             : Qt.hsla(hue, Math.min(1, base.hslSaturation * 1.12), accentLight.hslLightness + 0.065, 1)
    readonly property color titleTabActive: ribbon

    readonly property color ribbon: dark ? tone(0.12, 0.178) : tone(0.25, 0.969)
    readonly property color ribbonBorder: dark ? tone(0.12, 0.249) : tone(0.21, 0.89)
    readonly property color text: dark ? tone(0.21, 0.935) : tone(0.13, 0.131)
    readonly property color textMuted: dark ? tone(0.11, 0.661) : tone(0.08, 0.439)
    // Still readable: disabled options must be legible, just quieter.
    readonly property color textDisabled: dark ? tone(0.1, 0.571) : tone(0.09, 0.639)
    readonly property color hover: dark ? tone(0.13, 0.239) : tone(0.35, 0.927)
    readonly property color checked: dark ? tone(0.19, 0.327) : tone(0.67, 0.904)
    readonly property color separator: dark ? tone(0.1, 0.282) : tone(0.22, 0.855)
    readonly property color popup: dark ? tone(0.14, 0.2) : "#ffffff"
    /// Text fields and lists inside popups.
    readonly property color field: dark ? tone(0.12, 0.157) : "#ffffff"
    readonly property color overlayBg: "#d0181620"
    readonly property color sidebar: dark ? tone(0.13, 0.153) : tone(0.29, 0.945)
    readonly property color sidebarBorder: dark ? tone(0.12, 0.22) : tone(0.22, 0.884)
    readonly property color desk: dark ? tone(0.12, 0.116) : tone(0.12, 0.875)
    readonly property color danger: dark ? "#ff6b6b" : "#c62828"
    readonly property color toast: dark ? tone(0.27, 0.957) : tone(0.13, 0.176)
    readonly property color toastText: dark ? tone(0.13, 0.131) : "#ffffff"
    readonly property color toastAction: dark ? accentLight : accentDark

    readonly property int titleHeight: 36
    readonly property int ribbonHeight: 48
    readonly property int buttonSize: 36
    readonly property int radius: 6
    readonly property int sidebarWidth: 280
}

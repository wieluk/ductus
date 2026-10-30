#include "ductus-qt/cpp/winframe.h"

#include <QAbstractNativeEventFilter>
#include <QGuiApplication>
#include <QWindow>

#include <windows.h>

namespace {

// Snap and the Win+arrow keys only work on windows with a caption and a resizable frame.
constexpr LONG_PTR kFrameStyle = WS_CAPTION | WS_THICKFRAME | WS_MINIMIZEBOX | WS_MAXIMIZEBOX | WS_SYSMENU;

class Filter : public QAbstractNativeEventFilter
{
public:
    explicit Filter(HWND hwnd)
        : m_hwnd(hwnd)
    {
    }

    bool nativeEventFilter(const QByteArray &, void *message, qintptr *result) override
    {
        const MSG *msg = static_cast<const MSG *>(message);
        if (msg->hwnd != m_hwnd)
            return false;
        if (msg->message == WM_NCCALCSIZE && msg->wParam == TRUE) {
            // The whole window is client area: the frame is there for Windows, not to be seen.
            if (IsZoomed(m_hwnd)) {
                // Maximized, Windows pushes the frame past the screen's edges.
                auto *params = reinterpret_cast<NCCALCSIZE_PARAMS *>(msg->lParam);
                const UINT dpi = GetDpiForWindow(m_hwnd);
                const int padding = GetSystemMetricsForDpi(SM_CXPADDEDBORDER, dpi);
                const int x = GetSystemMetricsForDpi(SM_CXFRAME, dpi) + padding;
                const int y = GetSystemMetricsForDpi(SM_CYFRAME, dpi) + padding;
                params->rgrc[0].left += x;
                params->rgrc[0].right -= x;
                params->rgrc[0].top += y;
                params->rgrc[0].bottom -= y;
            }
            *result = 0;
            return true;
        }
        if (msg->message == WM_STYLECHANGING && msg->wParam == GWL_STYLE) {
            // Qt sets the styles of a frameless window again now and then.
            reinterpret_cast<STYLESTRUCT *>(msg->lParam)->styleNew |= kFrameStyle;
        }
        return false;
    }

private:
    HWND m_hwnd;
};

} // namespace

void WinFrame::install(QWindow *window)
{
    // UI tests run offscreen, without native windows.
    if (!window || QGuiApplication::platformName() != u"windows")
        return;
    const HWND hwnd = reinterpret_cast<HWND>(window->winId());
    qApp->installNativeEventFilter(new Filter(hwnd));
    const LONG_PTR style = GetWindowLongPtrW(hwnd, GWL_STYLE);
    SetWindowLongPtrW(hwnd, GWL_STYLE, (style & ~LONG_PTR(WS_POPUP)) | kFrameStyle);
    SetWindowPos(hwnd, nullptr, 0, 0, 0, 0,
                 SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE);
}

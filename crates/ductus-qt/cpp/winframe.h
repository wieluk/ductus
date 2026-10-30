// Include guards: the build reaches headers through two include paths.
#ifndef DUCTUS_WINFRAME_H
#define DUCTUS_WINFRAME_H

#include <QtGlobal>

class QWindow;

namespace WinFrame {

/// Windows: our own title bar, with Snap and Win+arrow keys kept. The window keeps a native frame
/// whose visible parts are all inside its client area.
void install(QWindow *window);

} // namespace WinFrame

namespace WinPen {

/// +1 if the pen touched down with the native message of this time, -1 if it lifted, else 0.
int contactChangeAt(quint64 timestamp);

} // namespace WinPen

#endif

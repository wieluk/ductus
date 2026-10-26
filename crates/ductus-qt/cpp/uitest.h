// Include guards: the build reaches headers through two include paths.
#ifndef DUCTUS_UITEST_H
#define DUCTUS_UITEST_H

#include <QString>

class QQuickWindow;

/// Replays a tests/ui script against the window (DUCTUS_UI_SCRIPT).
void startUiTest(QQuickWindow *window, const QString &scriptPath, const QString &shotDir);

#endif

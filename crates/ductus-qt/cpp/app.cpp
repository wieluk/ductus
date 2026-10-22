#include "ductus-qt/cpp/app.h"
#include "ductus-qt/cpp/uitest.h"

#include <QApplication>
#include <QQmlApplicationEngine>
#include <QQuickStyle>
#include <QQuickWindow>
#include <QSettings>

std::int32_t run_app()
{
    static int argc = 1;
    static char arg0[] = "ductus";
    static char *argv[] = {arg0, nullptr};
    // A widgets application only for the native print dialog.
    QApplication app(argc, argv);
    QApplication::setApplicationName(QStringLiteral("Ductus"));
    QApplication::setApplicationVersion(QStringLiteral(DUCTUS_VERSION));
    QApplication::setOrganizationDomain(QStringLiteral("ductus.app"));
    QApplication::setDesktopFileName(QStringLiteral("app.ductus.Ductus"));
    // Tests and portable setups keep settings in a folder of their choice.
    const QString configDir = qEnvironmentVariable("DUCTUS_CONFIG_DIR");
    if (!configDir.isEmpty()) {
        QSettings::setDefaultFormat(QSettings::IniFormat);
        QSettings::setPath(QSettings::IniFormat, QSettings::UserScope, configDir);
    }
    // Our own look on every platform; the controls are restyled in QML.
    QQuickStyle::setStyle(QStringLiteral("Basic"));

    QQmlApplicationEngine engine;
    QObject::connect(&engine, &QQmlApplicationEngine::objectCreationFailed, &app,
                     [] { QCoreApplication::exit(1); }, Qt::QueuedConnection);
    engine.loadFromModule("Ductus", "Main");
    if (engine.rootObjects().isEmpty())
        return 1;

    const QString script = qEnvironmentVariable("DUCTUS_UI_SCRIPT");
    if (!script.isEmpty()) {
        auto *window = qobject_cast<QQuickWindow *>(engine.rootObjects().first());
        const QString shots = qEnvironmentVariable("DUCTUS_UI_SHOTS", QStringLiteral("ui-shots"));
        startUiTest(window, script, shots);
    }
    return app.exec();
}

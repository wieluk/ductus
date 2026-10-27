#include "ductus-qt/cpp/app.h"
#include "ductus-qt/cpp/translations.h"
#include "ductus-qt/cpp/uitest.h"

#include <QApplication>
#include <QQmlApplicationEngine>
#include <QQuickStyle>
#include <QQuickWindow>
#include <QSettings>
#include <QStyleHints>

#include <string>


bool softwareRendering()
{
#if defined(Q_OS_ANDROID)
    // Android still shows frames through OpenGL ES, which fails with Qt's software renderer.
    return false;
#else
    return QSettings().value(QStringLiteral("display/software"), false).toBool();
#endif
}

std::int32_t run_app(rust::Str program)
{
    // Qt for Android looks for its plugins next to argv[0], the app's library.
    static std::string arg0 = program.empty() ? std::string("ductus") : std::string(program);
    static int argc = 1;
    static char *argv[] = {arg0.data(), nullptr};
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
    // Some graphics drivers draw Qt Quick garbled: Qt's software renderer is slower but right.
    if (softwareRendering())
        QQuickWindow::setGraphicsApi(QSGRendererInterface::Software);
    // Our own look on every platform; the controls are restyled in QML.
    QQuickStyle::setStyle(QStringLiteral("Basic"));
    const QString script = qEnvironmentVariable("DUCTUS_UI_SCRIPT");
    // The offscreen platform turns hover off; tests need it like a desktop has it.
    if (!script.isEmpty())
        QGuiApplication::styleHints()->setUseHoverEffects(true);

    Translations::apply();
    QQmlApplicationEngine engine;
    QObject::connect(&engine, &QQmlApplicationEngine::objectCreationFailed, &app,
                     [] { QCoreApplication::exit(1); }, Qt::QueuedConnection);
    engine.loadFromModule("Ductus", "Main");
    if (engine.rootObjects().isEmpty())
        return 1;

    if (!script.isEmpty()) {
        auto *window = qobject_cast<QQuickWindow *>(engine.rootObjects().first());
        const QString shots = qEnvironmentVariable("DUCTUS_UI_SHOTS", QStringLiteral("ui-shots"));
        startUiTest(window, script, shots);
    }
    return app.exec();
}

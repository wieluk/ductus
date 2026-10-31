#include "ductus-qt/cpp/app.h"
#include "ductus-qt/cpp/appcontroller.h"
#include "ductus-qt/cpp/translations.h"
#include "ductus-qt/cpp/uitest.h"

#include <QApplication>
#include <QDir>
#include <QQmlApplicationEngine>
#include <QQuickStyle>
#include <QQuickWindow>
#include <QSettings>
#include <QStyleHints>
#include <QTimer>
#include <QUrl>
#if defined(Q_OS_ANDROID)
#include <QJniObject>
#include <QtCore/qcoreapplication_platform.h>
#include <QtCore/private/qjnihelpers_p.h>
#else
#include "ductus-qt/cpp/singleinstance.h"
#endif
#if defined(Q_OS_WIN)
#include "ductus-qt/cpp/winframe.h"
#endif

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

namespace {

#if defined(Q_OS_ANDROID)
/// The file an "Open with" intent points to, or "".
QString viewedFile(const QJniObject &intent)
{
    if (!intent.isValid()
        || intent.callObjectMethod<jstring>("getAction").toString() != u"android.intent.action.VIEW")
        return {};
    const QJniObject uri = intent.callObjectMethod("getData", "()Landroid/net/Uri;");
    return uri.isValid() ? uri.callObjectMethod<jstring>("toString").toString() : QString();
}

/// Intents that reach the running app: its activity is the only one (launchMode singleTask).
class IntentFiles : public QtAndroidPrivate::NewIntentListener
{
public:
    explicit IntentFiles(AppController *controller) : m_controller(controller) {}
    bool handleNewIntent(JNIEnv *, jobject intent) override
    {
        const QString file = viewedFile(QJniObject(intent));
        if (file.isEmpty())
            return false;
        // Called on Android's thread.
        QMetaObject::invokeMethod(m_controller, [c = m_controller, file] { Q_EMIT c->openRequested(file); },
                                  Qt::QueuedConnection);
        return true;
    }

private:
    AppController *m_controller;
};
#endif

/// Paths from the command line don't depend on the folder Ductus was started in.
QStringList openable(const rust::Vec<rust::String> &files)
{
    QStringList out;
    for (const rust::String &file : files) {
        const QString arg = QString::fromUtf8(file.data(), qsizetype(file.size()));
        const QUrl url(arg);
        out << (url.scheme().size() > 1 ? arg : QDir::current().absoluteFilePath(arg));
    }
    return out;
}

} // namespace

std::int32_t run_app(rust::Str program, rust::Vec<rust::String> files)
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
    QStringList opened = openable(files);
#if !defined(Q_OS_ANDROID)
    // Two copies would each save the same open pages over the other's.
    SingleInstance instance;
    if (!instance.claim(opened))
        return 0;
#endif
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
#if defined(Q_OS_WIN)
    WinFrame::install(qobject_cast<QQuickWindow *>(engine.rootObjects().first()));
#endif
    auto *controller = engine.singletonInstance<AppController *>("Ductus", "AppController");
#if defined(Q_OS_ANDROID)
    static IntentFiles intentFiles(controller);
    QtAndroidPrivate::registerNewIntentListener(&intentFiles);
    const QJniObject activity = QNativeInterface::QAndroidApplication::context();
    if (const QString file = viewedFile(activity.callObjectMethod("getIntent", "()Landroid/content/Intent;"));
        !file.isEmpty())
        opened << file;
#endif
    // Once the window is up and the library open.
    QTimer::singleShot(0, controller, [controller, opened] {
        for (const QString &file : opened)
            Q_EMIT controller->openRequested(file);
    });
#if !defined(Q_OS_ANDROID)
    auto *mainWindow = qobject_cast<QQuickWindow *>(engine.rootObjects().first());
    QObject::connect(&instance, &SingleInstance::activationRequested, mainWindow,
                     [mainWindow, controller](const QString &token, const QStringList &forwarded) {
        // Qt's Wayland shell reads the launcher's token from here when the window asks to come forward.
        if (!token.isEmpty())
            qputenv("XDG_ACTIVATION_TOKEN", token.toUtf8());
        mainWindow->setWindowStates(mainWindow->windowStates() & ~Qt::WindowMinimized);
        mainWindow->show();
        mainWindow->raise();
        mainWindow->requestActivate();
        for (const QString &file : forwarded)
            Q_EMIT controller->openRequested(file);
    });
#endif

    if (!script.isEmpty()) {
        auto *window = qobject_cast<QQuickWindow *>(engine.rootObjects().first());
        const QString shots = qEnvironmentVariable("DUCTUS_UI_SHOTS", QStringLiteral("ui-shots"));
        startUiTest(window, script, shots);
    }
    return app.exec();
}

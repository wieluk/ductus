#include "ductus-qt/cpp/updater.h"
#include "ductus-qt/cpp/rustconv.h"
#include "ductus-qt/src/ffi.cxxqt.h"

#include <QCoreApplication>
#include <QDesktopServices>
#include <QDir>
#include <QFileInfo>
#include <QProcess>
#include <QSettings>
#include <QStandardPaths>
#include <QSysInfo>
#include <QUrl>
#if defined(Q_OS_LINUX) && !defined(Q_OS_ANDROID)
#include <QDBusConnection>
#include <QDBusMessage>
#include <QDBusObjectPath>
#include <QDBusPendingCallWatcher>
#include <QDBusPendingReply>
#include <QDBusReply>
#endif
#if defined(Q_OS_WIN)
#include <shlobj.h>
#endif

namespace {
#if defined(Q_OS_LINUX) && !defined(Q_OS_ANDROID)
// The sandbox updates itself through this portal; it never reaches the host's flatpak.
const QString kPortal = QStringLiteral("org.freedesktop.portal.Flatpak");
const QString kPortalPath = QStringLiteral("/org/freedesktop/portal/Flatpak");
const QString kMonitor = QStringLiteral("org.freedesktop.portal.Flatpak.UpdateMonitor");
#endif

/// Where updates are downloaded to; empty if the system names no such place.
QString downloadDir(bool android)
{
    // The package installer only reads files our FileProvider shares, which covers the app's files.
    const QString base = QStandardPaths::writableLocation(android ? QStandardPaths::AppDataLocation
                                                                  : QStandardPaths::TempLocation);
    return base.isEmpty() ? QString() : base + QStringLiteral("/ductus-update");
}

/// Program Files as Windows knows it, not as an environment variable says.
bool installedForEveryone()
{
#if defined(Q_OS_WIN)
    PWSTR path = nullptr;
    QString programFiles;
    if (SUCCEEDED(SHGetKnownFolderPath(FOLDERID_ProgramFiles, 0, nullptr, &path)))
        programFiles = QDir::fromNativeSeparators(QString::fromWCharArray(path));
    CoTaskMemFree(path);
    return !programFiles.isEmpty()
           && QCoreApplication::applicationDirPath().startsWith(programFiles + u'/', Qt::CaseInsensitive);
#else
    return false;
#endif
}

/// UI test runs check only when their script asks, so a check never races the script.
bool testRun()
{
#ifdef DUCTUS_TEST_HOOKS
    return qEnvironmentVariableIsSet("DUCTUS_UI_SCRIPT");
#else
    return false;
#endif
}

/// Tests stop once the verified package is on disk.
bool installSkipped()
{
#ifdef DUCTUS_TEST_HOOKS
    return qEnvironmentVariableIsSet("DUCTUS_UPDATE_NO_INSTALL");
#else
    return false;
#endif
}

/// The APK built for this device's processor.
QString apkSuffix()
{
    return QStringLiteral("-android-%1.apk").arg(QSysInfo::buildCpuArchitecture());
}
}

Updater::Updater(QObject *parent)
    : QObject(parent)
{
    m_handle.emplace(open_updater());
#ifdef DUCTUS_TEST_HOOKS
    // Tests point at a release of their own.
    m_feed = qEnvironmentVariable("DUCTUS_UPDATE_FEED");
    m_channel = qEnvironmentVariable("DUCTUS_UPDATE_CHANNEL");
#endif
    if (m_channel.isEmpty()) {
#ifdef DUCTUS_DEVELOPMENT_BUILD
        const bool development = m_feed.isEmpty();
#else
        const bool development = false;
#endif
#if defined(Q_OS_ANDROID)
        const bool android = true;
#else
        const bool android = false;
#endif
        // The Windows installer leaves its uninstaller beside the program; the portable zip doesn't.
        const bool installed =
#if defined(Q_OS_WIN)
            QFileInfo::exists(QCoreApplication::applicationDirPath() + QStringLiteral("/unins000.exe"));
#else
            false;
#endif
        if (development)
            m_channel = QStringLiteral("none");
        else if (qEnvironmentVariableIsSet("FLATPAK_ID"))
            m_channel = QStringLiteral("flatpak");
        else if (installed)
            m_channel = QStringLiteral("installer");
        else if (android)
            m_channel = QStringLiteral("android");
        else
            m_channel = QStringLiteral("manual");
    }

    m_poll.setInterval(250);
    connect(&m_poll, &QTimer::timeout, this, &Updater::poll);
    m_daily.setInterval(24 * 60 * 60 * 1000);
    connect(&m_daily, &QTimer::timeout, this, [this] {
        if (autoCheck() && !m_busy) {
            m_automatic = true;
            check();
        }
    });
    if (m_channel != u"none") {
        m_daily.start();
        // A moment after start, so it doesn't compete with opening the library.
        if (autoCheck() && !testRun()) {
            QTimer::singleShot(5000, this, [this] {
                m_automatic = true;
                check();
            });
        }
    }
}

Updater::~Updater() = default;

QString Updater::currentVersion() const
{
    return QCoreApplication::applicationVersion();
}

bool Updater::autoCheck() const
{
    return QSettings().value(QStringLiteral("updates/auto"), true).toBool();
}

void Updater::setAutoCheck(bool on)
{
    if (on == autoCheck())
        return;
    QSettings().setValue(QStringLiteral("updates/auto"), on);
    Q_EMIT autoCheckChanged();
}

bool Updater::available() const
{
    return !m_latest.isEmpty() && update_is_newer(toStd(m_latest), toStd(currentVersion()));
}

void Updater::setStatus(const QString &status, double progress)
{
    m_status = status;
    m_progress = progress;
    Q_EMIT changed();
}

void Updater::check()
{
    if (m_busy || m_channel == u"none")
        return;
    m_busy = true;
    m_task = Task::Check;
    setStatus(tr("Looking for updates…"));
    (*m_handle)->check(toStd(m_feed), toStd(currentVersion()));
    m_poll.start();
}

void Updater::poll()
{
    const UpdateState state = (*m_handle)->state();
    if (m_task == Task::Download && state.total > 0)
        setStatus(tr("Downloading version %1…").arg(m_latest),
                  static_cast<double>(state.received) / static_cast<double>(state.total));
    if (state.busy)
        return;
    m_poll.stop();
    const Task task = m_task;
    m_task = Task::None;
    if (task == Task::Check)
        finishCheck(toQ(state.error));
    else if (task == Task::Download)
        finishDownload(toQ(state.file), toQ(state.error));
}

void Updater::finishCheck(const QString &error)
{
    const bool automatic = m_automatic;
    m_automatic = false;
    m_busy = false;
    if (!error.isEmpty()) {
        setStatus(tr("Couldn't look for updates: %1.").arg(error));
        return;
    }
    const UpdateState state = (*m_handle)->state();
    m_latest = toQ(state.latest);
    m_notes = toQ(state.notes);
    m_page = toQ(state.page);
    if (available()) {
        setStatus(tr("Version %1 is available.").arg(m_latest));
        if (automatic && QSettings().value(QStringLiteral("updates/dismissed")).toString() != m_latest)
            Q_EMIT updateFound();
    } else {
        setStatus(tr("Ductus is up to date."));
    }
}

void Updater::install()
{
    if (m_busy || !available())
        return;
    if (m_channel == u"flatpak") {
        flatpakUpdate();
    } else if (m_channel == u"installer" || m_channel == u"android") {
        const bool android = m_channel == u"android";
        const QString suffix = android ? apkSuffix() : QStringLiteral("-windows-x64-setup.exe");
        if (toQ((*m_handle)->asset_url(toStd(suffix))).isEmpty()) {
            openReleasePage();
            setStatus(tr("Version %1 has no package for this device; its release page is open.").arg(m_latest));
            return;
        }
        const QString dir = downloadDir(android);
        if (dir.isEmpty()) {
            openReleasePage();
            return;
        }
        // Earlier downloads are done with.
        QDir(dir).removeRecursively();
        m_busy = true;
        m_cancelled = false;
        m_task = Task::Download;
        setStatus(tr("Downloading version %1…").arg(m_latest), 0);
        (*m_handle)->download(toStd(suffix), toStd(dir));
        m_poll.start();
    } else {
        openReleasePage();
    }
}

void Updater::cancel()
{
    if (!downloading())
        return;
    m_cancelled = true;
    if (m_task == Task::Download)
        (*m_handle)->cancel();
    else
        finishFlatpak(tr("The update was cancelled."));
}

void Updater::openReleasePage()
{
    // The feed names the page; only GitHub's are opened.
    const QUrl page(m_page.isEmpty() ? toQ((*m_handle)->state().page) : m_page);
    if (page.scheme() == u"https" && page.host() == u"github.com")
        QDesktopServices::openUrl(page);
}

void Updater::finishDownload(const QString &file, const QString &error)
{
    m_busy = false;
    if (m_cancelled) {
        setStatus(tr("The update was cancelled."));
        return;
    }
    if (!error.isEmpty() || file.isEmpty()) {
        setStatus(tr("The update couldn't be downloaded: %1.").arg(error.isEmpty() ? tr("no file") : error));
        return;
    }
    if (m_channel == u"android")
        openApk(file);
    else
        runInstaller(file);
}

void Updater::openApk(const QString &file)
{
    if (installSkipped()) {
        setStatus(tr("Version %1 is downloaded.").arg(m_latest), 1);
        return;
    }
    // Android replaces the app in place, since the release key signed both; it asks first.
    if (!QDesktopServices::openUrl(QUrl::fromLocalFile(file))) {
        setStatus(tr("Android's installer didn't open. Download version %1 from its release page instead.").arg(m_latest));
        openReleasePage();
        return;
    }
    setStatus(tr("Confirm the update in Android's installer. Ductus closes while it installs."));
}

void Updater::runInstaller(const QString &file)
{
    if (installSkipped()) {
        setStatus(tr("Version %1 is downloaded.").arg(m_latest), 1);
        return;
    }
    // Installed for everyone (under Program Files) or only for this user.
    const bool allUsers = installedForEveryone();
    const QStringList args{QStringLiteral("/SILENT"), QStringLiteral("/SUPPRESSMSGBOXES"),
                           QStringLiteral("/NORESTART"), QStringLiteral("/CLOSEAPPLICATIONS"),
                           allUsers ? QStringLiteral("/ALLUSERS") : QStringLiteral("/CURRENTUSER")};
    if (!QProcess::startDetached(file, args)) {
        setStatus(tr("The installer didn't start. Run it yourself: %1").arg(QDir::toNativeSeparators(file)));
        return;
    }
    // The installer replaces the program and starts it again; this one quits (and saves).
    setStatus(tr("Installing version %1…").arg(m_latest));
    QTimer::singleShot(500, qApp, &QCoreApplication::quit);
}

void Updater::flatpakUpdate()
{
#if defined(Q_OS_LINUX) && !defined(Q_OS_ANDROID)
    QDBusConnection bus = QDBusConnection::sessionBus();
    const QDBusReply<QDBusObjectPath> monitor =
        bus.call(QDBusMessage::createMethodCall(kPortal, kPortalPath, kPortal, QStringLiteral("CreateUpdateMonitor"))
                 << QVariantMap());
    if (!monitor.isValid()) {
        setStatus(tr("Flatpak couldn't install version %1%2.").arg(m_latest, QStringLiteral(" (%1)").arg(monitor.error().message())));
        return;
    }
    m_monitor = monitor.value().path();
    bus.connect(kPortal, m_monitor, kMonitor, QStringLiteral("Progress"), this, SLOT(flatpakProgress(QVariantMap)));
    m_busy = true;
    m_task = Task::Flatpak;
    setStatus(tr("Updating…"));
    auto *watcher = new QDBusPendingCallWatcher(
        bus.asyncCall(QDBusMessage::createMethodCall(kPortal, m_monitor, kMonitor, QStringLiteral("Update"))
                      << QString() << QVariantMap()),
        this);
    connect(watcher, &QDBusPendingCallWatcher::finished, this, [this](QDBusPendingCallWatcher *w) {
        w->deleteLater();
        if (w->isError() && m_task == Task::Flatpak)
            finishFlatpak(tr("Flatpak couldn't install version %1%2.").arg(m_latest, QStringLiteral(" (%1)").arg(w->error().message())));
    });
#endif
}

void Updater::flatpakProgress(const QVariantMap &info)
{
    if (m_task != Task::Flatpak)
        return;
    // The portal's states: running, nothing to do, done, failed.
    const QString error = info.value(QStringLiteral("error_message")).toString();
    const QString reason = error.isEmpty() ? QString() : QStringLiteral(" (%1)").arg(error);
    switch (info.value(QStringLiteral("status")).toUInt()) {
    case 0:
        setStatus(tr("Updating…"), info.value(QStringLiteral("progress")).toUInt() / 100.0);
        break;
    case 1:
        finishFlatpak(tr("The Ductus repository doesn't offer version %1 yet%2. Try again later.").arg(m_latest, reason));
        break;
    case 2:
        m_restartNeeded = true;
        finishFlatpak(tr("Version %1 is installed. Restart Ductus to use it.").arg(m_latest));
        break;
    default:
        finishFlatpak(tr("Flatpak couldn't install version %1%2.").arg(m_latest, reason));
    }
}

void Updater::finishFlatpak(const QString &status)
{
#if defined(Q_OS_LINUX) && !defined(Q_OS_ANDROID)
    QDBusConnection bus = QDBusConnection::sessionBus();
    bus.disconnect(kPortal, m_monitor, kMonitor, QStringLiteral("Progress"), this, SLOT(flatpakProgress(QVariantMap)));
    bus.asyncCall(QDBusMessage::createMethodCall(kPortal, m_monitor, kMonitor, QStringLiteral("Close")));
#endif
    m_monitor.clear();
    m_task = Task::None;
    m_busy = false;
    setStatus(status);
}

void Updater::dismiss()
{
    if (!m_latest.isEmpty())
        QSettings().setValue(QStringLiteral("updates/dismissed"), m_latest);
}

void Updater::restart()
{
    // The portal starts the new version in a sandbox of its own, which outlives this one.
    if (m_channel == u"flatpak")
        QProcess::startDetached(QStringLiteral("flatpak-spawn"), {QStringLiteral("--latest-version"), QStringLiteral("ductus")});
    else
        QProcess::startDetached(QCoreApplication::applicationFilePath(), {});
    QCoreApplication::quit();
}

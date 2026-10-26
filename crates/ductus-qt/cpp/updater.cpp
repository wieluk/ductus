#include "ductus-qt/cpp/updater.h"
#include "ductus-qt/src/ffi.cxxqt.h"

#include <QCoreApplication>
#include <QDesktopServices>
#include <QDir>
#include <QFileInfo>
#include <QRegularExpression>
#include <QSettings>
#include <QStandardPaths>
#include <QUrl>

namespace {
QString toQ(const rust::String &s)
{
    return QString::fromUtf8(s.data(), static_cast<qsizetype>(s.size()));
}

std::string toStd(const QString &s)
{
    return s.toStdString();
}

QString flatpakId()
{
    return qEnvironmentVariable("FLATPAK_ID", QStringLiteral("app.ductus.Ductus"));
}

/// Installs Ductus from its Flatpak repository, which installs then follow for updates.
const char *const kFlatpakRef = "https://wieluk.github.io/ductus/repo/ductus.flatpakref";
}

Updater::Updater(QObject *parent)
    : QObject(parent)
{
    m_handle.emplace(open_updater());
    // Tests point at a release of their own.
    m_feed = qEnvironmentVariable("DUCTUS_UPDATE_FEED");
    m_channel = qEnvironmentVariable("DUCTUS_UPDATE_CHANNEL");
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
    m_flatpak.setProcessChannelMode(QProcess::MergedChannels);
    connect(&m_flatpak, &QProcess::readyRead, this, [this] {
        const QString text = QString::fromUtf8(m_flatpak.readAll());
        m_flatpakOutput += text;
        // flatpak redraws one progress line ending in a percentage.
        static const QRegularExpression percent(QStringLiteral("(\\d{1,3})%"));
        auto matches = percent.globalMatch(text);
        double last = -1;
        while (matches.hasNext())
            last = matches.next().captured(1).toInt() / 100.0;
        if (last >= 0)
            setStatus(tr("Updating…"), qMin(1.0, last));
    });
    connect(&m_flatpak, &QProcess::finished, this, [this](int exitCode) { flatpakFinished(exitCode); });
    connect(&m_flatpak, &QProcess::errorOccurred, this, [this](QProcess::ProcessError error) {
        if (error == QProcess::FailedToStart)
            flatpakFinished(-1);
    });

    if (m_channel != u"none") {
        m_daily.start();
        // A moment after start, so it doesn't compete with opening the library.
        // UI test runs stay offline unless they bring a feed.
        const bool offlineTest = qEnvironmentVariableIsSet("DUCTUS_UI_SCRIPT") && m_feed.isEmpty();
        if (autoCheck() && !offlineTest) {
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
    (*m_handle)->check(toStd(m_feed));
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
        if (automatic)
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
    } else if (m_channel == u"installer") {
        m_busy = true;
        m_task = Task::Download;
        setStatus(tr("Downloading version %1…").arg(m_latest), 0);
        const QString dir = QStandardPaths::writableLocation(QStandardPaths::TempLocation) + QStringLiteral("/ductus-update");
        (*m_handle)->download("-windows-x64-setup.exe", toStd(dir));
        m_poll.start();
    } else if (m_channel == u"android") {
        const QString apk = toQ((*m_handle)->asset_url("-android-arm64.apk"));
        QDesktopServices::openUrl(QUrl(apk.isEmpty() ? m_page : apk));
        setStatus(tr("Your browser downloads version %1. Open the download to install it.").arg(m_latest));
    } else {
        openReleasePage();
    }
}

void Updater::cancel()
{
    if (m_task == Task::Download)
        (*m_handle)->cancel();
    else if (m_task == Task::Flatpak)
        m_flatpak.kill();
}

void Updater::openReleasePage()
{
    QDesktopServices::openUrl(QUrl(m_page.isEmpty() ? toQ((*m_handle)->state().page) : m_page));
}

void Updater::finishDownload(const QString &file, const QString &error)
{
    m_busy = false;
    if (!error.isEmpty() || file.isEmpty()) {
        setStatus(tr("The update couldn't be downloaded: %1.").arg(error.isEmpty() ? tr("no file") : error));
        return;
    }
    runInstaller(file);
}

void Updater::runInstaller(const QString &file)
{
    // Tests stop here: the verified installer is on disk.
    if (qEnvironmentVariableIsSet("DUCTUS_UPDATE_NO_INSTALL")) {
        setStatus(tr("Version %1 is downloaded.").arg(m_latest), 1);
        return;
    }
    // Installed for everyone (under Program Files) or only for this user.
    const QString programFiles = QDir::fromNativeSeparators(qEnvironmentVariable("ProgramFiles"));
    const bool allUsers = !programFiles.isEmpty()
                          && QCoreApplication::applicationDirPath().startsWith(programFiles, Qt::CaseInsensitive);
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
    m_busy = true;
    m_task = Task::Flatpak;
    m_flatpakOutput.clear();
    setStatus(tr("Updating…"));
    // flatpak runs outside the sandbox, on the host.
    m_flatpak.start(QStringLiteral("flatpak-spawn"),
                    {QStringLiteral("--host"), QStringLiteral("flatpak"), QStringLiteral("update"),
                     QStringLiteral("-y"), QStringLiteral("--noninteractive"), flatpakId()});
}

void Updater::flatpakFinished(int exitCode)
{
    if (m_task != Task::Flatpak)
        return;
    m_task = Task::None;
    m_busy = false;
    // An install from a file has no repository, and flatpak then reports nothing to do.
    QProcess info;
    info.start(QStringLiteral("flatpak-spawn"),
               {QStringLiteral("--host"), QStringLiteral("flatpak"), QStringLiteral("info"), flatpakId()});
    info.waitForFinished(15000);
    static const QRegularExpression versionLine(QStringLiteral("^\\s*Version:\\s*(\\S+)"),
                                                QRegularExpression::MultilineOption);
    const QString installed = versionLine.match(QString::fromUtf8(info.readAllStandardOutput())).captured(1);
    if (exitCode == 0 && installed == m_latest) {
        m_restartNeeded = true;
        setStatus(tr("Version %1 is installed. Restart Ductus to use it.").arg(m_latest));
        return;
    }
    const QString detail = m_flatpakOutput.trimmed().section(QLatin1Char('\n'), -1);
    setStatus(tr("Flatpak didn't install version %1%2. Updates come from the Ductus repository; "
                 "install Ductus from it once:\nflatpak install --user %3")
                  .arg(m_latest, detail.isEmpty() ? QString() : QStringLiteral(" (%1)").arg(detail),
                       QString::fromLatin1(kFlatpakRef)));
}

void Updater::restart()
{
    if (m_channel == u"flatpak")
        QProcess::startDetached(QStringLiteral("flatpak-spawn"),
                                {QStringLiteral("--host"), QStringLiteral("flatpak"), QStringLiteral("run"), flatpakId()});
    else
        QProcess::startDetached(QCoreApplication::applicationFilePath(), {});
    QCoreApplication::quit();
}

#include "ductus-qt/cpp/updater.h"
#include "ductus-qt/src/ffi.cxxqt.h"

#include <QCoreApplication>
#include <QDesktopServices>
#include <QDir>
#include <QFileInfo>
#include <QRegularExpression>
#include <QSettings>
#include <QStandardPaths>
#include <QSysInfo>
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

/// The Flatpak repository that installs follow for updates.
const char *const kFlatpakRepo = "https://wieluk.github.io/ductus/repo/ductus.flatpakrepo";

/// Runs flatpak on the host, outside the sandbox. Empty when it fails.
QString hostFlatpak(const QStringList &args)
{
    QProcess process;
    process.start(QStringLiteral("flatpak-spawn"), QStringList{QStringLiteral("--host"), QStringLiteral("flatpak")} + args);
    if (!process.waitForFinished(15000) || process.exitStatus() != QProcess::NormalExit || process.exitCode() != 0)
        return {};
    return QString::fromUtf8(process.readAllStandardOutput()).trimmed();
}

/// Bundles without a repository get a remote with no URL or a file:// one, which never updates.
bool followsOnlineRepository(const QString &origin)
{
    const QStringList lines = hostFlatpak({QStringLiteral("remotes"), QStringLiteral("--columns=name,url")})
                                  .split(QLatin1Char('\n'));
    for (const QString &line : lines) {
        const QStringList columns = line.simplified().split(QLatin1Char(' '));
        if (columns.size() >= 2 && columns[0] == origin
            && (columns[1].startsWith(u"https://") || columns[1].startsWith(u"http://")))
            return true;
    }
    return false;
}

QString installedCommit()
{
    return hostFlatpak({QStringLiteral("info"), QStringLiteral("--show-commit"), flatpakId()});
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
        // UI test runs check only when their script asks, so a check never races the script.
        if (autoCheck() && !qEnvironmentVariableIsSet("DUCTUS_UI_SCRIPT")) {
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
        // The package installer only reads files our FileProvider shares, which covers the app's files.
        const QString dir = QStandardPaths::writableLocation(android ? QStandardPaths::AppDataLocation
                                                                     : QStandardPaths::TempLocation)
                            + QStringLiteral("/ductus-update");
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
    if (m_task == Task::Download) {
        (*m_handle)->cancel();
        return;
    }
    // flatpak-spawn passes SIGTERM on to flatpak on the host; SIGKILL would end only flatpak-spawn.
    m_flatpak.terminate();
    QTimer::singleShot(5000, this, [this] {
        if (m_task == Task::Flatpak)
            m_flatpak.kill();
    });
}

void Updater::openReleasePage()
{
    QDesktopServices::openUrl(QUrl(m_page.isEmpty() ? toQ((*m_handle)->state().page) : m_page));
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
    if (qEnvironmentVariableIsSet("DUCTUS_UPDATE_NO_INSTALL")) {
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
    // Without an online repository flatpak succeeds with nothing to do, forever.
    const QString origin = hostFlatpak({QStringLiteral("info"), QStringLiteral("--show-origin"), flatpakId()});
    if (!origin.isEmpty() && !followsOnlineRepository(origin)) {
        // Uninstalling keeps settings, and unlike --reinstall it accepts a repository build older than this one.
        setStatus(tr("This copy was installed from a file that doesn't follow the Ductus repository, so it can't "
                     "update. Install it from the repository once (notes and settings stay):\n"
                     "flatpak remote-add --user --if-not-exists ductus %1 && flatpak uninstall -y %2 && "
                     "flatpak install --user -y ductus %2")
                      .arg(QString::fromLatin1(kFlatpakRepo), flatpakId()));
        return;
    }
    m_flatpakCommit = installedCommit();
    m_busy = true;
    m_cancelled = false;
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
    const QString commit = installedCommit();
    if (exitCode == 0 && !commit.isEmpty() && commit != m_flatpakCommit) {
        m_restartNeeded = true;
        setStatus(tr("Version %1 is installed. Restart Ductus to use it.").arg(m_latest));
        return;
    }
    if (m_cancelled) {
        setStatus(tr("The update was cancelled."));
        return;
    }
    // flatpak prints an unreachable repository as a warning and then says there's nothing to do.
    static const QRegularExpression lineBreak(QStringLiteral("[\\r\\n]"));
    const QStringList lines = m_flatpakOutput.split(lineBreak, Qt::SkipEmptyParts);
    QString detail = lines.isEmpty() ? QString() : lines.last().trimmed();
    for (const QString &line : lines) {
        if (line.contains(u"error", Qt::CaseInsensitive) || line.contains(u"warning", Qt::CaseInsensitive)) {
            detail = line.trimmed();
            break;
        }
    }
    const QString reason = detail.isEmpty() ? QString() : QStringLiteral(" (%1)").arg(detail);
    if (exitCode == 0)
        setStatus(tr("The Ductus repository doesn't offer version %1 yet%2. Try again later.").arg(m_latest, reason));
    else
        setStatus(tr("Flatpak couldn't install version %1%2.").arg(m_latest, reason));
}

void Updater::dismiss()
{
    if (!m_latest.isEmpty())
        QSettings().setValue(QStringLiteral("updates/dismissed"), m_latest);
}

void Updater::restart()
{
    // Backgrounded on the host, so flatpak-spawn returns and this sandbox can end with us.
    if (m_channel == u"flatpak")
        QProcess::startDetached(QStringLiteral("flatpak-spawn"),
                                {QStringLiteral("--host"), QStringLiteral("sh"), QStringLiteral("-c"),
                                 QStringLiteral("flatpak run '%1' >/dev/null 2>&1 &").arg(flatpakId())});
    else
        QProcess::startDetached(QCoreApplication::applicationFilePath(), {});
    QCoreApplication::quit();
}

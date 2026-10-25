#include "ductus-qt/cpp/appcontroller.h"
#include "ductus-qt/cpp/platformfiles.h"
#include "ductus-qt/cpp/inkcanvas.h"
#include "ductus-qt/cpp/models.h"
#include "ductus-qt/src/ffi.cxxqt.h"

#include <QAbstractTextDocumentLayout>
#include <QBuffer>
#include <QClipboard>
#include <QDesktopServices>
#include <QRandomGenerator>
#include <QTemporaryFile>
#if defined(Q_OS_LINUX) && !defined(Q_OS_ANDROID)
#include <QDBusConnection>
#include <QDBusMessage>
#include <QDBusPendingCallWatcher>
#include <QDBusPendingReply>
#endif
#include <QPdfWriter>
#include <QTextDocument>
#include <QDir>
#include <QLocale>
#include <QFileInfo>
#include <QGuiApplication>
#include <QImage>
#include <QPainter>
#ifndef DUCTUS_NO_PRINTING
#include <QPrintDialog>
#include <QPrinter>
#endif
#include <QSettings>
#include <QStandardPaths>
#include <QUrl>

#include <algorithm>

namespace {

QString toQ(const rust::String &s)
{
    return QString::fromUtf8(s.data(), static_cast<qsizetype>(s.size()));
}

std::string toStd(const QString &s)
{
    return s.toStdString();
}

QString defaultLibraryPath()
{
    // Tests and portable setups can point the app elsewhere.
    const QString env = qEnvironmentVariable("DUCTUS_LIBRARY");
    if (!env.isEmpty())
        return env;
    const QString saved = QSettings().value(QStringLiteral("library/path")).toString();
    if (!saved.isEmpty())
        return saved;
    // On Android, notes stay inside the app until it may use shared storage.
    return PlatformFiles::hasSharedAccess() ? PlatformFiles::sharedLibraryPath()
                                            : PlatformFiles::privateLibraryPath();
}

}

int NotebookModel::rowCount(const QModelIndex &parent) const
{
    return parent.isValid() ? 0 : static_cast<int>(m_rows.size());
}

QVariant NotebookModel::data(const QModelIndex &index, int role) const
{
    if (!index.isValid() || index.row() >= m_rows.size())
        return {};
    const Row &row = m_rows.at(index.row());
    switch (role) {
    case NameRole:
        return row.name;
    case PathRole:
        return row.path;
    case DepthRole:
        return row.depth;
    case IsFolderRole:
        return row.isFolder;
    case IsPdfRole:
        return row.isPdf;
    case ColorRole:
        return row.color;
    case HasChildrenRole:
        return row.hasChildren;
    case ExpandedRole:
        return row.expanded;
    case ConflictRole:
        return row.conflict;
    case CurrentRole:
        return !row.isFolder && !row.quick && row.path == m_current;
    case IsFileRole:
        return row.isFile;
    case SubtitleRole:
        return row.subtitle;
    case PinnedRole:
        return row.pinned;
    case QuickRole:
        return row.quick;
    case IsNotebookRole:
        return row.isNotebook;
    case SectionRole:
        return row.section;
    case NotebookNameRole:
        return row.notebookName;
    case PlaceholderRole:
        return row.placeholder;
    default:
        return {};
    }
}

QHash<int, QByteArray> NotebookModel::roleNames() const
{
    return {
        {NameRole, "name"},
        {PathRole, "path"},
        {DepthRole, "depth"},
        {IsFolderRole, "isFolder"},
        {IsPdfRole, "isPdf"},
        {ColorRole, "color"},
        {HasChildrenRole, "hasChildren"},
        {ExpandedRole, "expanded"},
        {ConflictRole, "conflict"},
        {CurrentRole, "isCurrent"},
        {IsFileRole, "isFile"},
        {SubtitleRole, "subtitle"},
        {PinnedRole, "pinned"},
        {QuickRole, "quick"},
        {IsNotebookRole, "isNotebook"},
        {SectionRole, "section"},
        {NotebookNameRole, "notebookName"},
        {PlaceholderRole, "placeholder"},
    };
}

void NotebookModel::setRows(QList<Row> rows)
{
    // Same rows in the same places: update in place so views keep their
    // scroll position and delegates.
    bool sameShape = rows.size() == m_rows.size();
    for (qsizetype i = 0; sameShape && i < rows.size(); ++i)
        sameShape = rows[i].path == m_rows[i].path && rows[i].section == m_rows[i].section;
    if (sameShape) {
        m_rows = std::move(rows);
        if (!m_rows.isEmpty())
            Q_EMIT dataChanged(index(0), index(static_cast<int>(m_rows.size()) - 1));
        // Names may have changed (a notebook renamed): the location bar follows.
        Q_EMIT rowsChanged();
        return;
    }
    beginResetModel();
    m_rows = std::move(rows);
    endResetModel();
    Q_EMIT rowsChanged();
}

int NotebookModel::treeIndexOf(const QString &path) const
{
    for (qsizetype i = 0; i < m_rows.size(); ++i)
        if (!m_rows[i].quick && m_rows[i].path == path)
            return static_cast<int>(i);
    return -1;
}

void NotebookModel::setCurrent(const QString &path)
{
    if (path == m_current)
        return;
    m_current = path;
    if (!m_rows.isEmpty())
        Q_EMIT dataChanged(index(0), index(static_cast<int>(m_rows.size()) - 1), {CurrentRole});
}

AppController::AppController(QObject *parent)
    : QObject(parent)
{
    const QStringList expanded =
        QSettings().value(QStringLiteral("tree/expanded")).toStringList();
    m_expanded = QSet<QString>(expanded.begin(), expanded.end());
    m_treeFilter = QSettings().value(QStringLiteral("tree/filter"), 3).toInt();
    m_notebookFilter = QSettings().value(QStringLiteral("tree/notebookFilter"), 1).toInt();
    m_notebookView = QSettings().value(QStringLiteral("tree/notebooks"), false).toBool();
    m_treeFocus = QSettings().value(QStringLiteral("tree/focus")).toString();

    m_saveTimer.setSingleShot(true);
    m_saveTimer.setInterval(QSettings().value(QStringLiteral("saving/delay"), 1500).toInt());
    connect(&m_saveTimer, &QTimer::timeout, this, &AppController::saveNow);

    m_refreshTimer.setSingleShot(true);
    m_refreshTimer.setInterval(250);
    connect(&m_refreshTimer, &QTimer::timeout, this, &AppController::refreshTree);

    connect(&m_watcher, &QFileSystemWatcher::directoryChanged, this, [this] {
        m_refreshTimer.start();
        refreshIndex();
    });
    m_recognizer.emplace(open_recognizer());
    m_converter.emplace(open_converter());
    m_convertPoll.setInterval(200);
    connect(&m_convertPoll, &QTimer::timeout, this, &AppController::pollConversion);
    m_recognizeTimer.setSingleShot(true);
    m_recognizeTimer.setInterval(2000);
    connect(&m_recognizeTimer, &QTimer::timeout, this, &AppController::recognizeOpenPage);
    m_recognitionPoll.setInterval(500);
    connect(&m_recognitionPoll, &QTimer::timeout, this, &AppController::pollRecognition);
    m_indexTimer.setInterval(400);
    connect(&m_indexTimer, &QTimer::timeout, this, &AppController::pollIndex);
    connect(&m_watcher, &QFileSystemWatcher::fileChanged, this, &AppController::onFileChanged);
    connect(qApp, &QCoreApplication::aboutToQuit, this, [this] {
        if (m_canvas)
            m_canvas->finishTextEditing();
        saveNow();
    });
    connect(QGuiApplication::clipboard(), &QClipboard::dataChanged, this, [this] {
        if (!m_awaitingSnip)
            return;
        const QImage image = QGuiApplication::clipboard()->image();
        if (image.isNull())
            return;
        m_awaitingSnip = false;
        // Hand the snip over as a file, like the portal does.
        QTemporaryFile file(QDir::tempPath() + QStringLiteral("/ductus-snip-XXXXXX.png"));
        file.setAutoRemove(false);
        if (file.open() && image.save(&file, "PNG"))
            Q_EMIT screenshotTaken(file.fileName());
        else
            Q_EMIT screenshotFailed(tr("The screenshot could not be read."));
    });
    // Only open folders are watched; catch up on the rest when we come back.
    connect(qApp, &QGuiApplication::applicationStateChanged, this, [this](Qt::ApplicationState state) {
        if (state == Qt::ApplicationActive) {
            // Back from the system page that gives access to shared storage.
            if (m_awaitingShared && PlatformFiles::hasSharedAccess()) {
                m_awaitingShared = false;
                moveToSharedStorage();
            }
            m_refreshTimer.start();
            refreshIndex();
        }
    });
}

AppController::~AppController() = default;

bool AppController::attempt(const std::function<void()> &action)
{
    try {
        action();
        return true;
    } catch (const rust::Error &e) {
        Q_EMIT errorOccurred(QString::fromUtf8(e.what()));
        return false;
    }
}

QString AppController::libraryPath() const
{
    return m_library ? toQ((*m_library)->root()) : QString();
}

QString AppController::currentPage() const
{
    return m_library ? toQ((*m_library)->open_page()) : QString();
}

QString AppController::currentTitle() const
{
    const QString page = currentPage();
    return page.section(QLatin1Char('/'), -1);
}

QString AppController::parentOf(const QString &path) const
{
    const qsizetype slash = path.lastIndexOf(QLatin1Char('/'));
    return slash < 0 ? QString() : path.left(slash);
}

QString AppController::currentFolder() const
{
    const QString folder = parentOf(currentPage());
    return inFocus(folder) ? folder : m_treeFocus;
}

bool AppController::inFocus(const QString &path) const
{
    return m_treeFocus.isEmpty() || path == m_treeFocus
           || path.startsWith(m_treeFocus + QLatin1Char('/'));
}

void AppController::setCanvas(InkCanvas *canvas)
{
    if (canvas == m_canvas)
        return;
    if (m_canvas)
        disconnect(m_canvas, nullptr, this, nullptr);
    m_canvas = canvas;
    if (m_canvas) {
        connect(m_canvas, &InkCanvas::edited, this, &AppController::onCanvasEdited);
        openLibrary(defaultLibraryPath());
    }
    Q_EMIT canvasChanged();
}

void AppController::openLibrary(const QString &pathOrUrl)
{
    saveNow();
    const QString path = PlatformFiles::folderPath(pathOrUrl);
    if (path.isEmpty()) {
        Q_EMIT errorOccurred(tr("Ductus can only keep notes in a folder on this device's storage."));
        return;
    }
    if (!PlatformFiles::insideApp(path) && !PlatformFiles::hasSharedAccess()) {
        Q_EMIT errorOccurred(tr("Allow Ductus to use your files first, then choose the folder again."));
        PlatformFiles::requestSharedAccess();
        return;
    }
    const bool ok = attempt([&] { m_library.emplace(open_library(toStd(path), toStd(dataDir()))); });
    if (!ok)
        return;
    if (!qEnvironmentVariableIsSet("DUCTUS_LIBRARY"))
        QSettings().setValue(QStringLiteral("library/path"), path);
    const QString starter = ensureStarterContent();
    refreshTree();
    // The search index lives in the cache: it can always be rebuilt.
    m_search.reset();
    QString cache = qEnvironmentVariable("DUCTUS_CACHE_DIR");
    if (cache.isEmpty())
        cache = QStandardPaths::writableLocation(QStandardPaths::CacheLocation);
    attempt([&] { m_search.emplace(open_search(toStd(cache), toStd(path), toStd(dataDir()))); });
    refreshIndex();
    Q_EMIT libraryChanged();

    // A closed page stays closed; a new library starts on its starter page.
    QString last = starter.isEmpty() ? rememberedPage() : starter;
    if (!last.isEmpty() && !QFileInfo::exists(toQ((*m_library)->page_file(toStd(last)))))
        last.clear();
    if (!last.isEmpty()) {
        openPage(last);
    } else {
        if (m_canvas) {
            m_canvas->engine().unload();
            m_canvas->documentReplaced();
            m_savedRevision = m_canvas->revision();
        }
        setDirty(false);
        Q_EMIT currentPageChanged();
    }
}

bool AppController::notesInsideApp() const
{
    return PlatformFiles::insideApp(libraryPath());
}

bool AppController::storageQuestionPending() const
{
    return notesInsideApp() && !qEnvironmentVariableIsSet("DUCTUS_LIBRARY")
           && !QSettings().value(QStringLiteral("library/storageAsked"), false).toBool();
}

void AppController::storageQuestionAnswered()
{
    QSettings().setValue(QStringLiteral("library/storageAsked"), true);
}

void AppController::useSharedStorage()
{
    if (PlatformFiles::hasSharedAccess()) {
        moveToSharedStorage();
        return;
    }
    m_awaitingShared = true;
    PlatformFiles::requestSharedAccess();
}

void AppController::moveToSharedStorage()
{
    const QString from = libraryPath();
    const QString to = PlatformFiles::sharedLibraryPath();
    if (!m_library || from == to || !notesInsideApp())
        return;
    if (m_canvas)
        m_canvas->finishTextEditing();
    saveNow();
    const QString page = currentPage();
    m_search.reset();
    m_library.reset();
    if (!attempt([&] { relocate_library(toStd(from), toStd(to), toStd(dataDir())); })) {
        openLibrary(from);
        return;
    }
    openLibrary(to);
    if (!page.isEmpty())
        openPage(page);
    Q_EMIT notice(tr("Your notes are now in Documents/Ductus, where backups and sync apps find them."));
}

QString AppController::ensureStarterContent()
{
    QString page;
    attempt([&] {
        if (!(*m_library)->is_empty() || !m_canvas)
            return;
        const rust::String subject = (*m_library)->create_folder("", toStd(tr("My Notebook")));
        (*m_library)->set_color(toStd(toQ(subject)), "#5a43c0");
        (*m_library)->set_notebook(toStd(toQ(subject)), subject);
        page = toQ((*m_library)->create_page(toStd(toQ(subject)), toStd(tr("First page")),
                                             m_canvas->engine()));
        m_expanded.insert(toQ(subject));
        storeExpanded();
    });
    return page;
}

void AppController::rememberPage(const QString &path)
{
    QSettings settings;
    QVariantMap pages = settings.value(QStringLiteral("library/lastPages")).toMap();
    if (path.isEmpty())
        pages.remove(libraryPath());
    else
        pages.insert(libraryPath(), path);
    settings.setValue(QStringLiteral("library/lastPages"), pages);
}

QString AppController::rememberedPage() const
{
    QSettings settings;
    const QVariantMap pages = settings.value(QStringLiteral("library/lastPages")).toMap();
    if (pages.contains(libraryPath()))
        return pages.value(libraryPath()).toString();
    // Versions before per-library pages kept one; it is offered once, to the first library opened.
    const QString old = settings.value(QStringLiteral("library/lastPage")).toString();
    settings.remove(QStringLiteral("library/lastPage"));
    return old;
}

void AppController::closePage()
{
    if (!m_library || !m_canvas || currentPage().isEmpty())
        return;
    m_canvas->finishTextEditing();
    saveNow();
    (*m_library)->close_page();
    m_canvas->engine().unload();
    m_canvas->documentReplaced();
    m_savedRevision = m_canvas->revision();
    setDirty(false);
    rememberPage(QString());
    refreshTree();
    rewatch();
    Q_EMIT currentPageChanged();
}

void AppController::refreshTree()
{
    if (!m_library)
        return;
    // The focus folder was renamed or deleted elsewhere, or belongs to
    // another library.
    if (!m_treeFocus.isEmpty() && !QFileInfo(libraryPath() + QLatin1Char('/') + m_treeFocus).isDir()) {
        m_treeFocus.clear();
        QSettings().remove(QStringLiteral("tree/focus"));
        Q_EMIT treeViewChanged();
    }
    QStringList expanded(m_expanded.begin(), m_expanded.end());
    attempt([&] {
        const rust::Vec<TreeRow> rows =
            (*m_library)->rows(toStd(expanded.join(QLatin1Char('\n'))),
                               static_cast<std::uint8_t>(treeFilter()), m_notebookView,
                               toStd(m_treeFocus), toStd(m_keepVisible));
        QList<NotebookModel::Row> out;
        out.reserve(static_cast<qsizetype>(rows.size()));
        for (const TreeRow &r : rows) {
            out.append({toQ(r.name), toQ(r.path), int(r.depth), r.is_folder, r.is_pdf,
                        toQ(r.color), r.has_children, r.expanded, r.conflict, r.is_file,
                        toQ(r.subtitle), r.pinned, r.quick, r.is_notebook, toQ(r.section),
                        toQ(r.notebook_name), r.placeholder});
        }
        m_tree.setRows(std::move(out));
    });
    m_tree.setCurrent(currentPage());
    rewatch();
}

void AppController::rewatch()
{
    if (!m_library)
        return;
    // The root and the open folders: a library can be a huge existing folder
    // tree, so collapsed folders are only checked when the app is activated.
    const QString root = libraryPath();
    QSet<QString> wanted{root};
    for (const QString &folder : std::as_const(m_expanded)) {
        const QString dir = root + QLatin1Char('/') + folder;
        if (QFileInfo(dir).isDir())
            wanted.insert(dir);
    }
    const QString page = currentPage();
    if (!page.isEmpty())
        wanted.insert(toQ((*m_library)->page_file(toStd(page))));
    // The page file is always re-added: an atomic save replaces it.
    const QStringList files = m_watcher.files();
    if (!files.isEmpty())
        m_watcher.removePaths(files);
    QStringList stale;
    for (const QString &dir : m_watcher.directories())
        if (!wanted.remove(dir))
            stale << dir;
    if (!stale.isEmpty())
        m_watcher.removePaths(stale);
    if (!wanted.isEmpty())
        m_watcher.addPaths(QStringList(wanted.begin(), wanted.end()));
}

void AppController::openPage(const QString &path)
{
    if (!m_library || !m_canvas)
        return;
    m_canvas->finishTextEditing();
    saveNow();
    const bool ok = attempt([&] { (*m_library)->load_into(toStd(path), m_canvas->engine()); });
    if (!ok)
        return;
    m_canvas->documentReplaced();
    m_canvas->setPicturesAllowed((*m_library)->accepts_pictures());
    m_savedRevision = m_canvas->revision();
    setDirty(false);
    rememberPage(path);
    attempt([&] { (*m_library)->add_recent(toStd(path)); });
    expandTo(path);
    // The open page is always in the sidebar.
    if (!inFocus(path)) {
        m_treeFocus.clear();
        QSettings().remove(QStringLiteral("tree/focus"));
        Q_EMIT treeViewChanged();
    }
    refreshTree();
    Q_EMIT currentPageChanged();
}

void AppController::expandTo(const QString &path)
{
    QString folder = parentOf(path);
    bool changed = false;
    while (!folder.isEmpty()) {
        if (!m_expanded.contains(folder)) {
            m_expanded.insert(folder);
            changed = true;
        }
        folder = parentOf(folder);
    }
    if (changed)
        storeExpanded();
}

void AppController::storeExpanded()
{
    QStringList list(m_expanded.begin(), m_expanded.end());
    list.sort();
    QSettings().setValue(QStringLiteral("tree/expanded"), list);
}

void AppController::toggleExpanded(const QString &path)
{
    if (!m_expanded.remove(path))
        m_expanded.insert(path);
    storeExpanded();
    refreshTree();
}

QString AppController::newFolder(const QString &parent)
{
    if (!m_library)
        return {};
    QString path;
    attempt([&] {
        const rust::String name = (*m_library)->unique_name(toStd(parent), toStd(tr("New folder")));
        path = toQ((*m_library)->create_folder(toStd(parent), name));
    });
    if (path.isEmpty())
        return {};
    // Shown even while empty folders are hidden, to be named right away.
    m_keepVisible = path;
    expandTo(path);
    refreshTree();
    Q_EMIT renameRequested(path);
    Q_EMIT revealRequested(path);
    return path;
}

QString AppController::addNotebook(const QString &location, const QString &name, bool ownFolder)
{
    if (!m_library || name.trimmed().isEmpty())
        return {};
    QString folder = location;
    QString page;
    const bool ok = attempt([&] {
        if (ownFolder) {
            const rust::String unique = (*m_library)->unique_name(toStd(location), toStd(name.trimmed()));
            folder = toQ((*m_library)->create_folder(toStd(location), unique));
        }
        (*m_library)->set_notebook(toStd(folder), toStd(name));
    });
    if (!ok || folder.isEmpty())
        return {};
    if (ownFolder)
        page = createPage(folder);
    m_expanded.insert(folder);
    storeExpanded();
    if (!page.isEmpty())
        openPage(page);
    else
        refreshTree();
    Q_EMIT revealRequested(folder);
    return folder;
}

QString AppController::addPage(const QString &folder, const QString &name)
{
    if (!m_library || !m_canvas)
        return {};
    // The Inbox appears with its first page.
    if (folder == inboxFolder() && !QFileInfo(libraryPath() + QLatin1Char('/') + folder).isDir()) {
        if (!attempt([&] {
                (*m_library)->create_folder("", toStd(folder));
                (*m_library)->set_notebook(toStd(folder), toStd(folder));
            }))
            return {};
    }
    const QString path = createPage(folder, name);
    if (!path.isEmpty())
        openPage(path);
    return path;
}

QString AppController::inboxFolder() const
{
    return tr("Inbox");
}

QString AppController::defaultPageFolder() const
{
    const QString page = currentPage();
    if (!page.isEmpty() && !notebookOf(page).isEmpty())
        return parentOf(page);
    return inboxFolder();
}

QVariantList AppController::pageTargets() const
{
    QVariantList out;
    if (!m_library)
        return out;
    const QString inbox = inboxFolder();
    bool hasInbox = false;
    const QStringList lines = toQ((*m_library)->page_targets()).split(QLatin1Char('\n'), Qt::SkipEmptyParts);
    for (const QString &line : lines) {
        const QString path = line.section(QLatin1Char('\t'), 0, 0);
        hasInbox = hasInbox || path == inbox;
        out.append(QVariantMap{{QStringLiteral("path"), path},
                               {QStringLiteral("label"), line.section(QLatin1Char('\t'), 1)}});
    }
    if (!hasInbox)
        out.prepend(QVariantMap{{QStringLiteral("path"), inbox}, {QStringLiteral("label"), inbox}});
    return out;
}

QVariantList AppController::subfolders(const QString &folder) const
{
    QVariantList out;
    if (!m_library)
        return out;
    const QStringList lines = toQ((*m_library)->subfolders(toStd(folder))).split(QLatin1Char('\n'), Qt::SkipEmptyParts);
    for (const QString &line : lines) {
        const QString name = line.section(QLatin1Char('\t'), 0, 0);
        out.append(QVariantMap{{QStringLiteral("name"), name},
                               {QStringLiteral("path"), folder.isEmpty() ? name : folder + QLatin1Char('/') + name},
                               {QStringLiteral("notebook"), line.section(QLatin1Char('\t'), 1)}});
    }
    return out;
}

void AppController::setNotebook(const QString &folder, const QString &name)
{
    if (m_library && attempt([&] { (*m_library)->set_notebook(toStd(folder), toStd(name)); }))
        refreshTree();
}

QString AppController::notebookName(const QString &folder) const
{
    return m_library ? toQ((*m_library)->notebook_name(toStd(folder))) : QString();
}

QString AppController::notebookOf(const QString &path) const
{
    return m_library ? toQ((*m_library)->notebook_of(toStd(path))) : QString();
}

QString AppController::dataDir()
{
    const QString dir = qEnvironmentVariable("DUCTUS_DATA_DIR");
    if (!dir.isEmpty())
        return dir;
    // Local, not roaming, on Windows: history and models are large and belong to this device.
    const QString local = QStandardPaths::writableLocation(QStandardPaths::AppLocalDataLocation);
    const QString roaming = QStandardPaths::writableLocation(QStandardPaths::AppDataLocation);
    if (local != roaming) {
        // Versions before kept them in the roaming folder.
        for (const QString &name : {QStringLiteral("libraries"), QStringLiteral("models")}) {
            const QString from = roaming + QLatin1Char('/') + name;
            const QString to = local + QLatin1Char('/') + name;
            if (QFileInfo(from).isDir() && !QFileInfo::exists(to) && QDir().mkpath(local))
                QDir().rename(from, to);
        }
    }
    return local;
}

void AppController::showLibraryData()
{
    if (m_library)
        QDesktopServices::openUrl(QUrl::fromLocalFile(toQ((*m_library)->data_dir())));
}

QString AppController::displayName(const QString &path) const
{
    return (path.isEmpty() ? libraryPath() : path).section(QLatin1Char('/'), -1);
}

void AppController::showInNotebooks(const QString &path)
{
    setNotebookView(true);
    if (!inFocus(path))
        setTreeFocus(QString());
    m_expanded.insert(path);
    storeExpanded();
    refreshTree();
    Q_EMIT revealRequested(path);
}

QString AppController::createPage(const QString &parent, const QString &baseName)
{
    if (!m_library || !m_canvas)
        return {};
    QString path;
    attempt([&] {
        const QString base = baseName.trimmed().isEmpty() ? tr("Untitled page") : baseName.trimmed();
        const rust::String name = (*m_library)->unique_name(toStd(parent), toStd(base));
        // New pages copy the open page's style, or use the style from the settings.
        QSettings settings;
        if (settings.value(QStringLiteral("pages/useDefault"), false).toBool()) {
            path = toQ((*m_library)->create_page_styled(
                toStd(parent), name,
                toStd(settings.value(QStringLiteral("pages/kind"), QStringLiteral("endless")).toString()),
                settings.value(QStringLiteral("pages/landscape"), false).toBool(),
                toStd(settings.value(QStringLiteral("pages/pattern"), QStringLiteral("dotted")).toString())));
        } else {
            path = toQ((*m_library)->create_page(toStd(parent), name, m_canvas->engine()));
        }
    });
    return path;
}

QString AppController::rename(const QString &path, const QString &name)
{
    if (!m_library)
        return {};
    saveNow();
    QString renamed;
    attempt([&] { renamed = toQ((*m_library)->rename(toStd(path), toStd(name))); });
    if (renamed.isEmpty())
        return {};
    if (m_keepVisible == path)
        m_keepVisible = renamed;
    // Keep expansion state for a renamed folder and everything inside it.
    QSet<QString> updated;
    for (const QString &e : std::as_const(m_expanded)) {
        if (e == path)
            updated.insert(renamed);
        else if (e.startsWith(path + QLatin1Char('/')))
            updated.insert(renamed + e.mid(path.size()));
        else
            updated.insert(e);
    }
    m_expanded = updated;
    storeExpanded();
    if (!currentPage().isEmpty())
        rememberPage(currentPage());
    refreshTree();
    refreshIndex();
    Q_EMIT currentPageChanged();
    return renamed;
}

QString AppController::moveTo(const QString &path, const QString &parent)
{
    if (!m_library || parentOf(path) == parent)
        return {};
    saveNow();
    QString moved;
    if (!attempt([&] { moved = toQ((*m_library)->move_to(toStd(path), toStd(parent), -1)); }))
        return {};
    // Expansion follows the moved folder and everything inside it; its new place opens.
    QSet<QString> updated;
    for (const QString &e : std::as_const(m_expanded)) {
        if (e == path)
            updated.insert(moved);
        else if (e.startsWith(path + QLatin1Char('/')))
            updated.insert(moved + e.mid(path.size()));
        else
            updated.insert(e);
    }
    for (QString folder = parent; !folder.isEmpty(); folder = parentOf(folder))
        updated.insert(folder);
    m_expanded = updated;
    storeExpanded();
    m_keepVisible = moved;
    if (!currentPage().isEmpty())
        rememberPage(currentPage());
    refreshTree();
    refreshIndex();
    Q_EMIT currentPageChanged();
    const QString notebook = notebookName(notebookOf(moved));
    Q_EMIT notice(tr("Moved to %1.").arg(notebook.isEmpty() ? displayName(parent) : notebook));
    return moved;
}

void AppController::remove(const QString &path)
{
    if (!m_library)
        return;
    const QString open = currentPage();
    const bool openInside = open == path || open.startsWith(path + QLatin1Char('/'));
    if (openInside)
        saveNow();
    // Where to go instead: the next page beside it, else the one before.
    QString neighbour;
    if (openInside) {
        const QString parent = parentOf(path);
        QStringList pages;
        for (const NotebookModel::Row &row : m_tree.rows()) {
            if (!row.quick && !row.placeholder && !row.isFolder && !row.isFile
                && (row.path == path || parentOf(row.path) == parent) && !pages.contains(row.path))
                pages << row.path;
        }
        const qsizetype at = pages.indexOf(path);
        if (at >= 0 && at + 1 < pages.size())
            neighbour = pages.at(at + 1);
        else if (at > 0)
            neighbour = pages.at(at - 1);
    }
    QString name;
    for (const NotebookModel::Row &row : m_tree.rows())
        if (row.path == path)
            name = row.isNotebook && !row.notebookName.isEmpty() ? row.notebookName : row.name;
    if (name.isEmpty())
        name = path.section(QLatin1Char('/'), -1);
    QString id;
    if (!attempt([&] { id = toQ((*m_library)->remove(toStd(path))); }))
        return;
    refreshTree();
    refreshIndex();
    if (openInside) {
        if (!neighbour.isEmpty()) {
            openPage(neighbour);
        } else {
            unloadPage();
        }
    }
    Q_EMIT trashed(name, id);
}

void AppController::unloadPage()
{
    m_canvas->engine().unload();
    m_canvas->documentReplaced();
    m_savedRevision = m_canvas->revision();
    setDirty(false);
    rememberPage(QString());
    refreshTree();
    rewatch();
    Q_EMIT currentPageChanged();
}

QVariantList AppController::trashRows()
{
    QVariantList out;
    if (!m_library)
        return out;
    rust::Vec<TrashRow> rows;
    if (!attempt([&] { rows = (*m_library)->trash_rows(); }))
        return out;
    for (const TrashRow &row : rows) {
        out << QVariantMap{
            {QStringLiteral("id"), toQ(row.id)},
            {QStringLiteral("name"), toQ(row.name)},
            {QStringLiteral("location"), toQ(row.location)},
            {QStringLiteral("kind"), toQ(row.kind)},
            {QStringLiteral("deleted"), QDateTime::fromMSecsSinceEpoch(qint64(row.deleted))},
        };
    }
    return out;
}

QString AppController::restoreFromTrash(const QString &id)
{
    if (!m_library)
        return {};
    QString rel;
    if (!attempt([&] { rel = toQ((*m_library)->restore(toStd(id))); }))
        return {};
    m_keepVisible = rel;
    refreshTree();
    refreshIndex();
    // A page that comes back opens; a folder shows where it is.
    if (QFileInfo(libraryPath() + QLatin1Char('/') + rel).isDir())
        reveal(rel);
    else
        openPage(rel);
    Q_EMIT notice(tr("\u201c%1\u201d is back.").arg(rel.section(QLatin1Char('/'), -1)));
    return rel;
}

void AppController::purgeFromTrash(const QString &id)
{
    if (m_library)
        attempt([&] { (*m_library)->purge(toStd(id)); });
}

void AppController::emptyTrash()
{
    if (m_library && attempt([&] { (*m_library)->empty_trash(); }))
        Q_EMIT notice(tr("The trash is empty."));
}

QVariantList AppController::pageVersions(const QString &path)
{
    QVariantList out;
    if (!m_library || path.isEmpty())
        return out;
    rust::Vec<std::uint64_t> saved;
    if (!attempt([&] { saved = (*m_library)->versions(toStd(path)); }))
        return out;
    for (const std::uint64_t ms : saved) {
        out << QVariantMap{
            {QStringLiteral("saved"), double(ms)},
            {QStringLiteral("date"), QDateTime::fromMSecsSinceEpoch(qint64(ms))},
        };
    }
    return out;
}

bool AppController::restoreVersion(const QString &path, double saved)
{
    if (!m_library)
        return false;
    const bool isOpen = path == currentPage();
    if (isOpen) {
        m_canvas->finishTextEditing();
        saveNow();
    }
    if (!attempt([&] { (*m_library)->restore_version(toStd(path), std::uint64_t(saved)); }))
        return false;
    if (isOpen)
        openPage(path);
    refreshIndex();
    Q_EMIT notice(tr("The earlier version is back. The one it replaced is kept as a version too."));
    return true;
}

QString AppController::copyVersion(const QString &path, double saved)
{
    if (!m_library)
        return {};
    const QDateTime when = QDateTime::fromMSecsSinceEpoch(qint64(saved));
    QString stem = path.section(QLatin1Char('/'), -1);
    if (stem.endsWith(QStringLiteral(".pdf"), Qt::CaseInsensitive))
        stem.chop(4);
    const QString name = tr("%1 (%2)").arg(stem, when.toString(QStringLiteral("yyyy-MM-dd HH.mm")));
    QString rel;
    if (!attempt([&] { rel = toQ((*m_library)->copy_version(toStd(path), std::uint64_t(saved), toStd(name))); }))
        return {};
    m_keepVisible = rel;
    refreshTree();
    refreshIndex();
    openPage(rel);
    return rel;
}

void AppController::setColor(const QString &path, const QString &color)
{
    if (m_library && attempt([&] { (*m_library)->set_color(toStd(path), toStd(color)); }))
        refreshTree();
}

QStringList AppController::moveSiblings(const QString &path, bool *notebookList) const
{
    const auto notebook = std::find_if(m_tree.rows().cbegin(), m_tree.rows().cend(), [&](const auto &row) {
        return !row.quick && row.path == path && row.isNotebook && row.depth == 0;
    });
    *notebookList = m_notebookView && notebook != m_tree.rows().cend();
    QStringList siblings;
    if (*notebookList) {
        for (const NotebookModel::Row &row : m_tree.rows())
            if (!row.quick && row.isNotebook && row.depth == 0 && !siblings.contains(row.path))
                siblings << row.path;
        return siblings;
    }
    const QString parent = parentOf(path);
    for (const NotebookModel::Row &row : m_tree.rows())
        if (!row.quick && !row.placeholder && parentOf(row.path) == parent && !siblings.contains(row.path))
            siblings << row.path;
    return siblings;
}

int AppController::moveOptions(const QString &path) const
{
    bool notebookList = false;
    const QStringList siblings = moveSiblings(path, &notebookList);
    const qsizetype index = siblings.indexOf(path);
    if (index < 0)
        return 0;
    return (index > 0 ? 1 : 0) | (index + 1 < siblings.size() ? 2 : 0);
}

void AppController::moveBy(const QString &path, int delta)
{
    if (!m_library)
        return;
    bool notebookList = false;
    const QStringList siblings = moveSiblings(path, &notebookList);
    if (notebookList) {
        if (attempt([&] {
                (*m_library)->move_notebook(toStd(path), delta, toStd(m_treeFocus));
            }))
            refreshTree();
        return;
    }
    const qsizetype index = siblings.indexOf(path);
    const qsizetype target = index + delta;
    if (index < 0 || target < 0 || target >= siblings.size())
        return;
    const QString parent = parentOf(path);
    if (attempt([&] { (*m_library)->move_to(toStd(path), toStd(parent), int(target)); }))
        refreshTree();
}

void AppController::onCanvasEdited()
{
    if (!m_canvas || m_canvas->revision() == m_savedRevision)
        return;
    setDirty(true);
    if (autosaveDelay() > 0)
        m_saveTimer.start();
}

void AppController::setDirty(bool dirty)
{
    if (dirty == m_dirty)
        return;
    m_dirty = dirty;
    Q_EMIT unsavedChanged();
}

void AppController::setAutosaveDelay(int ms)
{
    if (ms == autosaveDelay())
        return;
    m_saveTimer.setInterval(ms);
    QSettings().setValue(QStringLiteral("saving/delay"), ms);
    if (ms > 0 && m_dirty)
        m_saveTimer.start();
    Q_EMIT autosaveDelayChanged();
}

void AppController::saveNow()
{
    m_saveTimer.stop();
    if (!m_dirty || !m_library || !m_canvas)
        return;
    if (attempt([&] { (*m_library)->save_from(m_canvas->engine()); })) {
        m_savedRevision = m_canvas->revision();
        setDirty(false);
        m_lastSavedFile = toQ((*m_library)->page_file(toStd(currentPage())));
        m_lastSavedTime = QFileInfo(m_lastSavedFile).lastModified();
        if (m_search) {
            (*m_search)->reindex(toStd(currentPage()));
            m_indexTimer.start();
        }
        if (recognizerKind() != u"off")
            m_recognizeTimer.start();
        // An atomic save replaces the file, which ends the watch on it.
        QTimer::singleShot(0, this, &AppController::rewatch);
    }
}

void AppController::onFileChanged(const QString &file)
{
    if (!m_library || !m_canvas)
        return;
    const QString page = currentPage();
    if (page.isEmpty() || file != toQ((*m_library)->page_file(toStd(page))))
        return;
    // Our own saves trigger this too.
    if (file == m_lastSavedFile && QFileInfo(file).lastModified() == m_lastSavedTime) {
        QTimer::singleShot(0, this, &AppController::rewatch);
        return;
    }
    if (!QFileInfo::exists(file)) {
        // Deleted or renamed elsewhere; the directory watch refreshes the tree.
        QTimer::singleShot(0, this, &AppController::rewatch);
        return;
    }
    if (m_dirty) {
        Q_EMIT notice(tr("This page was also changed elsewhere. Your version is kept."));
        saveNow();
        return;
    }
    if (attempt([&] { (*m_library)->load_into(toStd(page), m_canvas->engine()); })) {
        m_canvas->documentReplaced();
        m_savedRevision = m_canvas->revision();
        Q_EMIT notice(tr("Page updated from another device."));
    }
    QTimer::singleShot(0, this, &AppController::rewatch);
}

QString AppController::pdfProblem() const
{
    return toQ(pdf_support_problem());
}

QString AppController::importPdf(const QString &fileOrUrl, bool annotate)
{
    if (!m_library)
        return {};
    const QString file = PlatformFiles::readable(fileOrUrl);
    if (file.isEmpty()) {
        Q_EMIT errorOccurred(tr("%1 can't be read.").arg(PlatformFiles::displayName(fileOrUrl)));
        return {};
    }
    QString path;
    const QString parent = currentFolder();
    attempt([&] { path = toQ((*m_library)->import_pdf(toStd(parent), toStd(file), annotate)); });
    if (path.isEmpty())
        return {};
    openPage(path);
    refreshIndex();
    Q_EMIT notice(annotate ? tr("PDF added. Your ink is saved into the PDF itself.")
                           : tr("PDF imported as a page."));
    return path;
}

bool AppController::exportPdf(const QString &fileOrUrl)
{
    if (!m_library || !m_canvas)
        return false;
    const QString target = PlatformFiles::writable(fileOrUrl);
    m_canvas->finishTextEditing();
    const QByteArray overlay = textOverlay();
    bool ok = attempt([&] {
        (*m_library)->export_pdf(toStd(target), m_canvas->engine(), bytes(overlay));
    });
    if (ok && !PlatformFiles::deliver(target, fileOrUrl)) {
        Q_EMIT errorOccurred(tr("The PDF couldn't be saved there."));
        ok = false;
    }
    if (ok)
        Q_EMIT notice(tr("Exported to %1").arg(PlatformFiles::displayName(fileOrUrl)));
    return ok;
}

void AppController::setTreeFilter(int flags)
{
    // Showing nothing at all would look like an empty library.
    if (!(flags & 7))
        flags |= 3;
    if (flags == treeFilter())
        return;
    (m_notebookView ? m_notebookFilter : m_treeFilter) = flags;
    m_keepVisible.clear();
    QSettings().setValue(m_notebookView ? QStringLiteral("tree/notebookFilter") : QStringLiteral("tree/filter"),
                         flags);
    Q_EMIT treeViewChanged();
    refreshTree();
}

void AppController::setNotebookView(bool on)
{
    if (on == m_notebookView)
        return;
    m_notebookView = on;
    QSettings().setValue(QStringLiteral("tree/notebooks"), on);
    Q_EMIT treeViewChanged();
    refreshTree();
}

void AppController::setTreeFocus(const QString &folder)
{
    if (folder == m_treeFocus)
        return;
    m_treeFocus = folder;
    if (folder.isEmpty())
        QSettings().remove(QStringLiteral("tree/focus"));
    else
        QSettings().setValue(QStringLiteral("tree/focus"), folder);
    Q_EMIT treeViewChanged();
    refreshTree();
}

void AppController::setPinned(const QString &path, bool pinned)
{
    if (m_library && attempt([&] { (*m_library)->set_pinned(toStd(path), pinned); }))
        refreshTree();
}

void AppController::reveal(const QString &path)
{
    if (!m_library)
        return;
    setNotebookView(false);
    if (!inFocus(path))
        setTreeFocus(QString());
    expandTo(path);
    refreshTree();
    Q_EMIT revealRequested(path);
}

void AppController::openExternally(const QString &path)
{
    if (!m_library)
        return;
    const QString file = toQ((*m_library)->absolute_path(toStd(path)));
    if (!QDesktopServices::openUrl(QUrl::fromLocalFile(file)))
        Q_EMIT errorOccurred(tr("No program is set up to open %1.").arg(QFileInfo(file).fileName()));
}

void AppController::showInFolder(const QString &path)
{
    if (!m_library)
        return;
    const QFileInfo info(toQ((*m_library)->absolute_path(toStd(path))));
    const QString folder = info.isDir() ? info.absoluteFilePath() : info.absolutePath();
    QDesktopServices::openUrl(QUrl::fromLocalFile(folder));
}

QString AppController::openFile(const QString &fileOrUrl)
{
    if (!m_library)
        return {};
    const QString readable = PlatformFiles::readable(fileOrUrl);
    if (readable.isEmpty()) {
        Q_EMIT errorOccurred(tr("%1 can't be read.").arg(PlatformFiles::displayName(fileOrUrl)));
        return {};
    }
    const QFileInfo info(readable);
    const QString suffix = info.suffix().toLower();
    if (suffix != u"pdf" && suffix != u"svg") {
        Q_EMIT errorOccurred(tr("Ductus opens notes pages (.svg) and PDFs."));
        return {};
    }
    const QString root = QFileInfo(libraryPath()).canonicalFilePath() + QLatin1Char('/');
    const QString file = info.canonicalFilePath();
    if (file.startsWith(root)) {
        QString rel = file.mid(root.size());
        if (suffix == u"svg")
            rel.chop(4);
        openPage(rel);
        return {};
    }
    if (suffix == u"pdf")
        return QStringLiteral("pdf");
    QString path;
    attempt([&] {
        path = toQ((*m_library)->import_page(toStd(currentFolder()), toStd(info.absoluteFilePath())));
    });
    if (!path.isEmpty()) {
        openPage(path);
        Q_EMIT notice(tr("Page copied into your library."));
    }
    return {};
}

void AppController::refreshIndex()
{
    if (!m_search)
        return;
    (*m_search)->refresh();
    m_indexTimer.start();
}

void AppController::pollIndex()
{
    if (!m_search) {
        m_indexTimer.stop();
        return;
    }
    const bool busy = (*m_search)->indexing();
    const rust::Vec<std::uint64_t> p = (*m_search)->index_progress();
    QString status;
    if (busy && p.size() == 2 && p[1] > 0)
        status = tr("Indexing %1 of %2 files…").arg(p[0]).arg(p[1]);
    else if (busy)
        status = tr("Indexing…");
    if (busy != m_indexing || status != m_indexStatus) {
        m_indexing = busy;
        m_indexStatus = status;
        Q_EMIT indexStatusChanged();
    }
    if (!busy)
        m_indexTimer.stop();
}

QVariantList AppController::search(const QString &query, int limit)
{
    QVariantList out;
    if (!m_search || query.trimmed().isEmpty())
        return out;
    const rust::Vec<SearchHit> hits = (*m_search)->search(toStd(query), static_cast<std::size_t>(limit));
    for (const SearchHit &h : hits) {
        QString snippet = toQ(h.snippet).toHtmlEscaped();
        snippet.replace(QChar(1), QStringLiteral("<b>")).replace(QChar(2), QStringLiteral("</b>"));
        // A notebook's own name tells more than the folders it is kept in.
        const QString path = toQ(h.path);
        QString location = toQ(h.location);
        const QString notebook = notebookOf(path);
        if (!notebook.isEmpty() && notebook != path) {
            const QString section = parentOf(path).mid(notebook.size() + 1);
            location = notebookName(notebook);
            if (!section.isEmpty())
                location += QStringLiteral(" › ") + QString(section).replace(QLatin1Char('/'), QStringLiteral(" › "));
        }
        out.append(QVariantMap{
            {QStringLiteral("path"), path},
            {QStringLiteral("title"), toQ(h.title)},
            {QStringLiteral("location"), location},
            {QStringLiteral("kind"), toQ(h.kind)},
            {QStringLiteral("page"), static_cast<qint64>(h.page)},
            {QStringLiteral("x"), h.x},
            {QStringLiteral("y"), h.y},
            {QStringLiteral("w"), h.w},
            {QStringLiteral("h"), h.h},
            {QStringLiteral("snippet"), snippet},
        });
    }
    return out;
}

void AppController::openSearchHit(const QVariantMap &hit)
{
    if (!m_canvas)
        return;
    const QString path = hit.value(QStringLiteral("path")).toString();
    if (path != currentPage())
        openPage(path);
    if (path != currentPage())
        return;
    const double w = hit.value(QStringLiteral("w")).toDouble();
    if (w >= 0) {
        m_canvas->focusDocRect(QRectF(hit.value(QStringLiteral("x")).toDouble(),
                                      hit.value(QStringLiteral("y")).toDouble(), w,
                                      hit.value(QStringLiteral("h")).toDouble()));
    } else {
        m_canvas->focusSheet(hit.value(QStringLiteral("page")).toInt());
    }
}

QString AppController::recognizerKind() const
{
    // Tests: every line reads as this text.
    const QString fixed = qEnvironmentVariable("DUCTUS_TEST_RECOGNIZER");
    if (!fixed.isEmpty())
        return QStringLiteral("fixed:") + fixed;
    return QSettings().value(QStringLiteral("hwr/backend"), QStringLiteral("off")).toString();
}

void AppController::recognizeNow()
{
    if (recognizerKind() == u"off") {
        Q_EMIT notice(tr("Choose a handwriting recognizer in Settings › Handwriting first."));
        return;
    }
    saveNow();
    recognizeOpenPage();
}

void AppController::recognizeOpenPage()
{
    if (!m_library || !m_canvas || !m_recognizer)
        return;
    const QString kind = recognizerKind();
    if (kind == u"off")
        return;
    QSettings settings;
    QString url = settings.value(QStringLiteral("hwr/url")).toString();
    QString model = settings.value(QStringLiteral("hwr/model")).toString();
    if (kind == u"builtin") {
        url = Models::dir();
        model = textModel();
        if (!Models::installed(model)) {
            setRecognitionStatus(tr("Download a handwriting model in Settings › Handwriting."));
            return;
        }
    }
    (*m_recognizer)->configure(toStd(kind), toStd(url), toStd(model),
                               toStd(settings.value(QStringLiteral("hwr/key")).toString()));
    const QString lang = settings.value(QStringLiteral("hwr/lang"), QStringLiteral("auto")).toString();
    const QString page = currentPage();
    const rust::Vec<LineJob> jobs = (*m_library)->recognition_jobs(m_canvas->engine());
    for (const LineJob &job : jobs) {
        (*m_recognizer)->submit(toStd(page), job.key, job.x, job.y, job.w, job.h,
                                rust::Slice<const std::uint8_t>(job.png.data(), job.png.size()),
                                toStd(lang));
    }
    if (!jobs.empty())
        m_recognitionPoll.start();
    pollRecognition();
}

QString AppController::textModel() const
{
    const QString lang = QSettings().value(QStringLiteral("hwr/lang"), QStringLiteral("auto")).toString();
    if (lang == u"de" || lang == u"en")
        return QStringLiteral("text-") + lang;
    if (Models::installed(QStringLiteral("text-de")))
        return QStringLiteral("text-de");
    if (Models::installed(QStringLiteral("text-en")) || QLocale::system().language() != QLocale::German)
        return QStringLiteral("text-en");
    return QStringLiteral("text-de");
}

void AppController::convertSelection(const QString &kind)
{
    if (!m_canvas || m_converting)
        return;
    const bool math = kind == u"math";
    if (kind == u"page")
        m_canvas->selectAll();
    QString recognizer = QStringLiteral("builtin");
    QString model = math ? QStringLiteral("math") : textModel();
    // Tests: every line reads as this text.
    const QString fixed = qEnvironmentVariable("DUCTUS_TEST_RECOGNIZER");
    if (!fixed.isEmpty())
        recognizer = QStringLiteral("fixed:") + fixed;
    else if (!Models::installed(model)) {
        Q_EMIT modelsNeeded(model);
        return;
    }
    rust::Vec<LineJob> jobs = m_canvas->engine().selection_jobs(math);
    if (jobs.empty()) {
        Q_EMIT notice(tr("Select some handwriting first."));
        return;
    }
    m_convertMath = math;
    m_converting = true;
    Q_EMIT convertingChanged();
    Q_EMIT notice(tr("Reading the handwriting…"));
    (*m_converter)->start(toStd(recognizer), toStd(Models::dir()), toStd(model), std::move(jobs));
    m_convertPoll.start();
}

void AppController::pollConversion()
{
    const ConvertState state = (*m_converter)->state();
    if (state.busy)
        return;
    m_convertPoll.stop();
    m_converting = false;
    Q_EMIT convertingChanged();
    const QString error = toQ(state.error);
    if (!error.isEmpty())
        Q_EMIT errorOccurred(tr("The handwriting couldn't be read: %1").arg(error));
    else if (!m_canvas || !m_canvas->replaceInk(toQ(state.result), m_convertMath))
        Q_EMIT notice(tr("No handwriting was recognized."));
}

void AppController::setRecognitionStatus(const QString &status)
{
    if (status != m_recognitionStatus) {
        m_recognitionStatus = status;
        Q_EMIT recognitionStatusChanged();
    }
}

void AppController::pollRecognition()
{
    if (!m_recognizer || !m_library)
        return;
    const rust::Vec<RecognitionResult> results = (*m_recognizer)->take_results();
    bool openPageChanged = false;
    for (const RecognitionResult &r : results) {
        bool open = false;
        attempt([&] {
            open = (*m_library)->apply_recognition(r.page, r.key, r.text, r.x, r.y, r.w, r.h, r.lang);
        });
        if (open)
            openPageChanged = true;
        else if (m_search)
            (*m_search)->reindex(r.page);
    }
    if (openPageChanged) {
        // The text is stored with the page (and indexed when it is saved).
        setDirty(true);
        saveNow();
    }
    const std::size_t left = (*m_recognizer)->pending();
    const QString error = toQ((*m_recognizer)->last_error());
    if (left > 0)
        setRecognitionStatus(tr("Reading handwriting: %n line(s) left", nullptr, int(left)));
    else
        setRecognitionStatus(error.isEmpty() ? QString() : tr("Handwriting: %1").arg(error));
    if (left == 0)
        m_recognitionPoll.stop();
}

void AppController::takeScreenshot()
{
    // Tests supply the picture.
    const QString injected = qEnvironmentVariable("DUCTUS_TEST_SCREENSHOT");
    if (!injected.isEmpty()) {
        QTimer::singleShot(200, this, [this, injected] {
            Q_EMIT screenshotTaken(QFileInfo(injected).absoluteFilePath());
        });
        return;
    }
#if defined(Q_OS_LINUX) && !defined(Q_OS_ANDROID)
    QDBusConnection bus = QDBusConnection::sessionBus();
    if (!bus.isConnected()) {
        Q_EMIT screenshotFailed(tr("Taking screenshots needs a desktop session. Take one with your "
                                   "system's tool and press Ctrl+V."));
        return;
    }
    // The portal answers on a request object whose path we can predict, so
    // we listen there before asking (no race with a quick answer).
    const QString token = QStringLiteral("ductus%1").arg(QRandomGenerator::global()->generate());
    const QString sender = bus.baseService().mid(1).replace(QLatin1Char('.'), QLatin1Char('_'));
    if (!m_screenshotRequest.isEmpty())
        bus.disconnect(QString(), m_screenshotRequest, QStringLiteral("org.freedesktop.portal.Request"),
                       QStringLiteral("Response"), this,
                       SLOT(onScreenshotResponse(uint, QVariantMap)));
    m_screenshotRequest = QStringLiteral("/org/freedesktop/portal/desktop/request/%1/%2").arg(sender, token);
    bus.connect(QStringLiteral("org.freedesktop.portal.Desktop"), m_screenshotRequest,
                QStringLiteral("org.freedesktop.portal.Request"), QStringLiteral("Response"), this,
                SLOT(onScreenshotResponse(uint, QVariantMap)));
    QDBusMessage call = QDBusMessage::createMethodCall(
        QStringLiteral("org.freedesktop.portal.Desktop"), QStringLiteral("/org/freedesktop/portal/desktop"),
        QStringLiteral("org.freedesktop.portal.Screenshot"), QStringLiteral("Screenshot"));
    call << QString() << QVariantMap{{QStringLiteral("interactive"), true},
                                     {QStringLiteral("handle_token"), token}};
    auto *watcher = new QDBusPendingCallWatcher(bus.asyncCall(call), this);
    connect(watcher, &QDBusPendingCallWatcher::finished, this, [this](QDBusPendingCallWatcher *w) {
        w->deleteLater();
        if (w->isError())
            Q_EMIT screenshotFailed(tr("Your desktop can't take screenshots for Ductus (%1). Take one with "
                                       "your system's tool and press Ctrl+V.")
                                        .arg(w->error().message()));
    });
#elif defined(Q_OS_WIN)
    // The snipping overlay puts its result on the clipboard.
    m_awaitingSnip = true;
    if (!QDesktopServices::openUrl(QUrl(QStringLiteral("ms-screenclip:")))) {
        m_awaitingSnip = false;
        Q_EMIT screenshotFailed(tr("The snipping tool didn't start. Press Win+Shift+S, then Ctrl+V."));
    } else {
        // Give up quietly if nothing arrives.
        QTimer::singleShot(120000, this, [this] {
            if (m_awaitingSnip) {
                m_awaitingSnip = false;
                Q_EMIT screenshotCancelled();
            }
        });
    }
#else
    Q_EMIT screenshotFailed(tr("Take a screenshot with your system's tool, then press Ctrl+V."));
#endif
}

void AppController::onScreenshotResponse(uint response, const QVariantMap &results)
{
    if (response == 0 && results.contains(QStringLiteral("uri")))
        Q_EMIT screenshotTaken(results.value(QStringLiteral("uri")).toString());
    else if (response == 1)
        Q_EMIT screenshotCancelled();
    else
        Q_EMIT screenshotFailed(tr("The screenshot didn't work."));
}

rust::Slice<const std::uint8_t> AppController::bytes(const QByteArray &data)
{
    return {reinterpret_cast<const std::uint8_t *>(data.constData()),
            static_cast<std::size_t>(data.size())};
}

QByteArray AppController::textOverlay()
{
    const rust::Vec<TextRow> texts = m_canvas->engine().texts_of_page();
    const rust::Vec<double> areas = (*m_library)->export_areas(m_canvas->engine());
    if (texts.empty() || areas.size() < 4)
        return {};
    // One page per exported page with only the text boxes, drawn over them; 72 dpi makes a unit a point.
    QByteArray out;
    QBuffer buffer(&out);
    buffer.open(QIODevice::WriteOnly);
    QPdfWriter writer(&buffer);
    writer.setResolution(72);
    writer.setPageMargins(QMarginsF());
    constexpr double pointsPerUnit = 0.75; // Document units are CSS pixels.
    auto pageSize = [&](std::size_t i) {
        return QPageSize(QSizeF(areas[i + 2] * pointsPerUnit, areas[i + 3] * pointsPerUnit),
                         QPageSize::Point, QString(), QPageSize::ExactMatch);
    };
    writer.setPageSize(pageSize(0));
    QPainter painter;
    if (!painter.begin(&writer))
        return {};
    for (std::size_t i = 0; i + 3 < areas.size(); i += 4) {
        if (i > 0) {
            writer.setPageSize(pageSize(i));
            writer.newPage();
        }
        const QRectF area(areas[i], areas[i + 1], areas[i + 2], areas[i + 3]);
        painter.save();
        painter.scale(pointsPerUnit, pointsPerUnit);
        painter.translate(-area.topLeft());
        painter.setClipRect(area);
        for (const TextRow &t : texts) {
            const QRectF box(t.x, t.y, t.width, t.height);
            if (!box.intersects(area))
                continue;
            QTextDocument doc;
            doc.setDefaultFont(textFont());
            doc.setDocumentMargin(0);
            doc.setMarkdown(toQ(t.markdown));
            doc.setTextWidth(t.width / t.scale);
            painter.save();
            painter.translate(box.topLeft());
            painter.scale(t.scale, t.scale);
            doc.drawContents(&painter);
            painter.restore();
        }
        painter.restore();
    }
    painter.end();
    return out;
}

QFont AppController::textFont()
{
    // The same font the text boxes use on screen; 16 px is 12 pt.
    QFont font = QGuiApplication::font();
    font.setPixelSize(16);
    return font;
}

#ifndef DUCTUS_NO_PRINTING
bool AppController::paintPages(QPrinter &printer)
{
    std::optional<rust::Box<PrintJob>> job;
    m_canvas->finishTextEditing();
    const QByteArray overlay = textOverlay();
    if (!attempt([&] { job.emplace((*m_library)->print_job(m_canvas->engine(), bytes(overlay))); }))
        return false;
    const int count = static_cast<int>((*job)->print_page_count());
    const int from = printer.fromPage() > 0 ? printer.fromPage() : 1;
    const int to = printer.toPage() > 0 ? qMin(printer.toPage(), count) : count;
    QPainter painter;
    if (!painter.begin(&printer)) {
        Q_EMIT errorOccurred(tr("Printing could not start."));
        return false;
    }
    // Pages are rendered at up to 200 dpi; the printer scales them up.
    const double factor = qMin(1.0, 200.0 / printer.resolution());
    bool ok = true;
    for (int i = from - 1; ok && i < to; ++i) {
        if (i > from - 1)
            printer.newPage();
        const double aspect = (*job)->print_page_aspect(static_cast<std::size_t>(i));
        QSizeF area(printer.width(), printer.height());
        painter.save();
        // Landscape pages turn sideways on portrait paper (and vice versa).
        if ((aspect > 1) != (area.width() > area.height())) {
            painter.translate(area.width(), 0);
            painter.rotate(90);
            area.transpose();
        }
        QSizeF size(area.width(), area.width() / aspect);
        if (size.height() > area.height())
            size = QSizeF(area.height() * aspect, area.height());
        const QRectF target(QPointF((area.width() - size.width()) / 2, 0), size);
        const int w = qMax(1, qRound(size.width() * factor));
        const int h = qMax(1, qRound(size.height() * factor));
        ok = attempt([&] {
            const rust::Vec<std::uint8_t> pixels =
                (*job)->print_page_pixels(static_cast<std::size_t>(i), w, h);
            const QImage image(pixels.data(), w, h, w * 4, QImage::Format_RGBA8888_Premultiplied);
            painter.drawImage(target, image);
        });
        painter.restore();
    }
    painter.end();
    return ok;
}

void AppController::print()
{
    if (!m_library || !m_canvas || currentPage().isEmpty())
        return;
    saveNow();
    QPrinter printer(QPrinter::HighResolution);
    printer.setDocName(currentTitle());
    // Four numbers per printed page; the range starts at page 1, not 0.
    const int pages = qMax(1, static_cast<int>((*m_library)->export_areas(m_canvas->engine()).size() / 4));
    QPrintDialog dialog(&printer);
    dialog.setOption(QAbstractPrintDialog::PrintPageRange);
    dialog.setMinMax(1, pages);
    printer.setFromTo(1, pages);
    if (dialog.exec() == QDialog::Accepted && paintPages(printer))
        Q_EMIT notice(tr("Sent to the printer."));
}

bool AppController::printToFile(const QString &fileOrUrl)
{
    if (!m_library || !m_canvas || currentPage().isEmpty())
        return false;
    QPrinter printer(QPrinter::HighResolution);
    printer.setOutputFormat(QPrinter::PdfFormat);
    const QString target = PlatformFiles::writable(fileOrUrl);
    printer.setOutputFileName(target);
    return paintPages(printer) && PlatformFiles::deliver(target, fileOrUrl);
}
#else
void AppController::print()
{
    Q_EMIT notice(tr("Printing isn't available here. Export the page as a PDF and print it from another app."));
}

bool AppController::printToFile(const QString &)
{
    return false;
}
#endif

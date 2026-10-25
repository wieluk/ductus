// Include guards, not #pragma once: the build reaches headers through two include paths.
#ifndef DUCTUS_APPCONTROLLER_H
#define DUCTUS_APPCONTROLLER_H

#include <QAbstractListModel>
#include <functional>
#include <optional>
#include <QDateTime>
#include <QFont>
#include <QFileSystemWatcher>
#include <QPointer>
#include <QSet>
#include <QTimer>
#include <QtQml/qqmlregistration.h>

#include "ductus-qt/cpp/inkcanvas.h"
#include "rust/cxx.h"

struct LibraryHandle;
struct SearchHandle;
struct RecognitionHandle;
struct ConvertHandle;
struct TreeRow;
class QPrinter;

class NotebookModel : public QAbstractListModel
{
    Q_OBJECT
    QML_ANONYMOUS
    /// Notebooks view: Favorites, Recent and Notebooks headings.
    Q_PROPERTY(bool hasSections READ hasSections NOTIFY rowsChanged)

public:
    enum Role {
        NameRole = Qt::UserRole + 1,
        PathRole,
        DepthRole,
        IsFolderRole,
        IsPdfRole,
        ColorRole,
        HasChildrenRole,
        ExpandedRole,
        ConflictRole,
        CurrentRole,
        IsFileRole,
        SubtitleRole,
        PinnedRole,
        QuickRole,
        IsNotebookRole,
        SectionRole,
        NotebookNameRole,
        PlaceholderRole,
    };

    struct Row {
        QString name;
        QString path;
        int depth = 0;
        bool isFolder = false;
        bool isPdf = false;
        QString color;
        bool hasChildren = false;
        bool expanded = false;
        bool conflict = false;
        bool isFile = false;
        QString subtitle;
        bool pinned = false;
        bool quick = false;
        bool isNotebook = false;
        QString section;
        QString notebookName;
        bool placeholder = false;
    };

    using QAbstractListModel::QAbstractListModel;

    int rowCount(const QModelIndex &parent = {}) const override;
    QVariant data(const QModelIndex &index, int role) const override;
    QHash<int, QByteArray> roleNames() const override;

    void setRows(QList<Row> rows);
    void setCurrent(const QString &path);
    const QList<Row> &rows() const { return m_rows; }
    bool hasSections() const { return !m_rows.isEmpty() && !m_rows.first().section.isEmpty(); }
    Q_INVOKABLE int treeIndexOf(const QString &path) const;

Q_SIGNALS:
    void rowsChanged();

private:
    QList<Row> m_rows;
    QString m_current;
};

class AppController : public QObject
{
    Q_OBJECT
    QML_ELEMENT
    QML_SINGLETON

    Q_PROPERTY(QString libraryPath READ libraryPath NOTIFY libraryChanged)
    /// Android: the notes are in the app's own folder, which uninstalling removes.
    Q_PROPERTY(bool notesInsideApp READ notesInsideApp NOTIFY libraryChanged)
    Q_PROPERTY(NotebookModel *tree READ tree CONSTANT)
    Q_PROPERTY(QString currentPage READ currentPage NOTIFY currentPageChanged)
    Q_PROPERTY(QString currentTitle READ currentTitle NOTIFY currentPageChanged)
    Q_PROPERTY(InkCanvas *canvas READ canvas WRITE setCanvas NOTIFY canvasChanged)
    Q_PROPERTY(QString pdfProblem READ pdfProblem CONSTANT)
    /// The shown view's: 1 notes, 2 PDFs, 4 other files, 8 hide folders without those.
    Q_PROPERTY(int treeFilter READ treeFilter WRITE setTreeFilter NOTIFY treeViewChanged)
    Q_PROPERTY(int defaultTreeFilter READ defaultTreeFilter NOTIFY treeViewChanged)
    Q_PROPERTY(bool indexing READ indexing NOTIFY indexStatusChanged)
    Q_PROPERTY(QString indexStatus READ indexStatus NOTIFY indexStatusChanged)
    Q_PROPERTY(QString recognitionStatus READ recognitionStatus NOTIFY recognitionStatusChanged)
    /// Selected handwriting is being turned into text or a formula.
    Q_PROPERTY(bool converting READ converting NOTIFY convertingChanged)
    Q_PROPERTY(bool notebookView READ notebookView WRITE setNotebookView NOTIFY treeViewChanged)
    /// "" for the whole library.
    Q_PROPERTY(QString treeFocus READ treeFocus WRITE setTreeFocus NOTIFY treeViewChanged)
    /// Milliseconds after the last edit; 0 saves only on Ctrl+S, page switches and quitting.
    Q_PROPERTY(int autosaveDelay READ autosaveDelay WRITE setAutosaveDelay NOTIFY autosaveDelayChanged)
    Q_PROPERTY(bool unsaved READ unsaved NOTIFY unsavedChanged)

public:
    explicit AppController(QObject *parent = nullptr);
    ~AppController() override;

    QString libraryPath() const;
    NotebookModel *tree() { return &m_tree; }
    QString currentPage() const;
    QString currentTitle() const;
    InkCanvas *canvas() const { return m_canvas; }
    QString pdfProblem() const;
    bool indexing() const { return m_indexing; }
    QString recognitionStatus() const { return m_recognitionStatus; }
    bool converting() const { return m_converting; }
    QString indexStatus() const { return m_indexStatus; }
    int treeFilter() const { return m_notebookView ? m_notebookFilter : m_treeFilter; }
    int defaultTreeFilter() const { return m_notebookView ? 1 : 3; }
    void setTreeFilter(int flags);
    bool notebookView() const { return m_notebookView; }
    void setNotebookView(bool on);
    QString treeFocus() const { return m_treeFocus; }
    void setTreeFocus(const QString &folder);
    int autosaveDelay() const { return m_saveTimer.interval(); }
    void setAutosaveDelay(int ms);
    bool unsaved() const { return m_dirty; }
    void setCanvas(InkCanvas *canvas);

    Q_INVOKABLE void openLibrary(const QString &pathOrUrl);
    bool notesInsideApp() const;
    /// Asked once on Android: may the notes live in Documents, outside the app?
    Q_INVOKABLE bool storageQuestionPending() const;
    Q_INVOKABLE void storageQuestionAnswered();
    /// Moves the notes to Documents/Ductus, asking for access first where needed.
    Q_INVOKABLE void useSharedStorage();
    Q_INVOKABLE void openPage(const QString &path);
    /// A notebook called `name` in `location`: in a new folder of that name, or `location` itself.
    /// A new folder gets a first page, which opens.
    Q_INVOKABLE QString addNotebook(const QString &location, const QString &name, bool ownFolder);
    /// A page in `folder` (a notebook or one of its sections), which opens.
    Q_INVOKABLE QString addPage(const QString &folder, const QString &name);
    /// Where a new page goes unless chosen otherwise: beside the open page, else the Inbox.
    Q_INVOKABLE QString defaultPageFolder() const;
    /// Notebooks and their sections: {path, label}; the Inbox comes first even before it exists.
    Q_INVOKABLE QVariantList pageTargets() const;
    /// {name, path, notebook} of each folder in `folder`.
    Q_INVOKABLE QVariantList subfolders(const QString &folder) const;
    /// "" makes the folder a plain folder again.
    Q_INVOKABLE void setNotebook(const QString &folder, const QString &name);
    Q_INVOKABLE QString notebookName(const QString &folder) const;
    /// The notebook a page or folder belongs to, or "".
    Q_INVOKABLE QString notebookOf(const QString &path) const;
    Q_INVOKABLE QString newFolder(const QString &parent);
    /// The folder's name, or the library's for "".
    Q_INVOKABLE QString displayName(const QString &path) const;
    Q_INVOKABLE void showInNotebooks(const QString &path);
    /// History and trash of the open library, in the app's data folder.
    Q_INVOKABLE void showLibraryData();
    /// The app's own data on this device: each library's history and trash, and the models.
    static QString dataDir();
    Q_INVOKABLE QString rename(const QString &path, const QString &name);
    /// Into another folder, at its end; returns the new path ("" when nothing moved).
    Q_INVOKABLE QString moveTo(const QString &path, const QString &parent);
    /// Moves it to the trash; `trashed` follows so the change can be undone.
    Q_INVOKABLE void remove(const QString &path);
    /// Newest first: id, name, location, kind, deleted (a date).
    Q_INVOKABLE QVariantList trashRows();
    Q_INVOKABLE QString restoreFromTrash(const QString &id);
    Q_INVOKABLE void purgeFromTrash(const QString &id);
    Q_INVOKABLE void emptyTrash();
    /// Earlier versions of a page or PDF, newest first: saved (ms), date.
    Q_INVOKABLE QVariantList pageVersions(const QString &path);
    Q_INVOKABLE bool restoreVersion(const QString &path, double saved);
    /// The version as a page of its own beside the original; it opens.
    Q_INVOKABLE QString copyVersion(const QString &path, double saved);
    Q_INVOKABLE void setColor(const QString &path, const QString &color);
    Q_INVOKABLE void toggleExpanded(const QString &path);
    /// `delta`: -1 up, +1 down.
    Q_INVOKABLE void moveBy(const QString &path, int delta);
    /// 1: it can move up, 2: down (both: 3).
    Q_INVOKABLE int moveOptions(const QString &path) const;
    Q_INVOKABLE QString parentOf(const QString &path) const;
    Q_INVOKABLE QString currentFolder() const;
    Q_INVOKABLE bool inFocus(const QString &path) const;
    Q_INVOKABLE void saveNow();
    Q_INVOKABLE void closePage();
    Q_INVOKABLE QString importPdf(const QString &fileOrUrl, bool annotate);
    Q_INVOKABLE bool exportPdf(const QString &fileOrUrl);
    /// Returns "pdf" for a PDF from outside the library, which needs the import choice first.
    Q_INVOKABLE QString openFile(const QString &fileOrUrl);
    Q_INVOKABLE void openExternally(const QString &path);
    Q_INVOKABLE void showInFolder(const QString &path);
    Q_INVOKABLE void setPinned(const QString &path, bool pinned);
    Q_INVOKABLE void reveal(const QString &path);
    Q_INVOKABLE void print();
    /// Without a dialog, for tests.
    Q_INVOKABLE bool printToFile(const QString &fileOrUrl);
    Q_INVOKABLE void takeScreenshot();
    Q_INVOKABLE QVariantList search(const QString &query, int limit = 50);
    Q_INVOKABLE void openSearchHit(const QVariantMap &hit);
    Q_INVOKABLE void recognizeNow();
    /// Replaces the selected handwriting with text ("text"), a formula ("math"), or all of
    /// the page's handwriting with text ("page"), read by the built-in recognizer.
    Q_INVOKABLE void convertSelection(const QString &kind);

Q_SIGNALS:
    void libraryChanged();
    void currentPageChanged();
    void canvasChanged();
    void errorOccurred(const QString &message);
    void notice(const QString &message);
    /// Something went to the trash; `trashId` brings it back.
    void trashed(const QString &name, const QString &trashId);
    void renameRequested(const QString &path);
    void treeViewChanged();
    void autosaveDelayChanged();
    void unsavedChanged();
    void revealRequested(const QString &path);
    void screenshotTaken(const QString &file);
    void indexStatusChanged();
    void recognitionStatusChanged();
    void convertingChanged();
    /// A built-in model has to be downloaded first.
    void modelsNeeded(const QString &id);
    void screenshotCancelled();
    void screenshotFailed(const QString &message);

private Q_SLOTS:
    void onScreenshotResponse(uint response, const QVariantMap &results);

private:
    bool attempt(const std::function<void()> &action);
    /// Empties the canvas after the open page went away.
    void unloadPage();
    void moveToSharedStorage();
    /// What `path` moves among; `notebookList` is set when that is the list of notebooks.
    QStringList moveSiblings(const QString &path, bool *notebookList) const;
    void refreshTree();
    void rewatch();
    void onCanvasEdited();
    void onFileChanged(const QString &file);
    void expandTo(const QString &path);
    void storeExpanded();
    /// The starter page it created in an empty library, else "".
    QString ensureStarterContent();
    /// Each library reopens its own last page; "" forgets it.
    void rememberPage(const QString &path);
    QString rememberedPage() const;
    QString inboxFolder() const;
    QString createPage(const QString &parent, const QString &baseName = {});
    bool paintPages(QPrinter &printer);
    QByteArray textOverlay();
    static rust::Slice<const std::uint8_t> bytes(const QByteArray &data);

public:
    static QFont textFont();

private:

    std::optional<rust::Box<LibraryHandle>> m_library;
    std::optional<rust::Box<SearchHandle>> m_search;
    QTimer m_indexTimer;
    bool m_indexing = false;
    QString m_indexStatus;
    void refreshIndex();
    void pollIndex();
    std::optional<rust::Box<RecognitionHandle>> m_recognizer;
    QTimer m_recognizeTimer;
    QTimer m_recognitionPoll;
    QString m_recognitionStatus;
    /// "off", "server" or "fixed:<text>" (tests).
    QString recognizerKind() const;
    QString textModel() const;
    void pollConversion();
    std::optional<rust::Box<ConvertHandle>> m_converter;
    QTimer m_convertPoll;
    bool m_converting = false;
    bool m_convertMath = false;
    void recognizeOpenPage();
    void pollRecognition();
    void setRecognitionStatus(const QString &status);
    NotebookModel m_tree;
    QPointer<InkCanvas> m_canvas;
    QSet<QString> m_expanded;
    int m_treeFilter = 3;
    int m_notebookFilter = 1;
    /// Shown whatever the filter, until it changes.
    QString m_keepVisible;
    bool m_awaitingSnip = false;
    bool m_awaitingShared = false;
    QString m_screenshotRequest;
    bool m_notebookView = false;
    QString m_treeFocus;
    quint64 m_savedRevision = 0;
    bool m_dirty = false;
    void setDirty(bool dirty);
    QTimer m_saveTimer;
    QTimer m_refreshTimer;
    QFileSystemWatcher m_watcher;
    /// To tell our own writes from other programs' changes.
    QString m_lastSavedFile;
    QDateTime m_lastSavedTime;
};

#endif

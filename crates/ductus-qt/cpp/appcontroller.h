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
    Q_PROPERTY(bool hasQuick READ hasQuick NOTIFY rowsChanged)

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
    };

    using QAbstractListModel::QAbstractListModel;

    int rowCount(const QModelIndex &parent = {}) const override;
    QVariant data(const QModelIndex &index, int role) const override;
    QHash<int, QByteArray> roleNames() const override;

    void setRows(QList<Row> rows);
    void setCurrent(const QString &path);
    const QList<Row> &rows() const { return m_rows; }
    bool hasQuick() const { return !m_rows.isEmpty() && m_rows.first().quick; }
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
    Q_PROPERTY(NotebookModel *tree READ tree CONSTANT)
    Q_PROPERTY(QString currentPage READ currentPage NOTIFY currentPageChanged)
    Q_PROPERTY(QString currentTitle READ currentTitle NOTIFY currentPageChanged)
    Q_PROPERTY(InkCanvas *canvas READ canvas WRITE setCanvas NOTIFY canvasChanged)
    Q_PROPERTY(QString pdfProblem READ pdfProblem CONSTANT)
    /// 1 notes, 2 PDFs, 4 other files, 8 hide folders without those.
    Q_PROPERTY(int treeFilter READ treeFilter WRITE setTreeFilter NOTIFY treeViewChanged)
    Q_PROPERTY(bool indexing READ indexing NOTIFY indexStatusChanged)
    Q_PROPERTY(QString indexStatus READ indexStatus NOTIFY indexStatusChanged)
    Q_PROPERTY(QString recognitionStatus READ recognitionStatus NOTIFY recognitionStatusChanged)
    /// Selected handwriting is being turned into text or a formula.
    Q_PROPERTY(bool converting READ converting NOTIFY convertingChanged)
    Q_PROPERTY(bool notebookView READ notebookView WRITE setNotebookView NOTIFY treeViewChanged)
    /// "" for the whole library.
    Q_PROPERTY(QString treeFocus READ treeFocus WRITE setTreeFocus NOTIFY treeViewChanged)

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
    int treeFilter() const { return m_treeFilter; }
    void setTreeFilter(int flags);
    bool notebookView() const { return m_notebookView; }
    void setNotebookView(bool on);
    QString treeFocus() const { return m_treeFocus; }
    void setTreeFocus(const QString &folder);
    void setCanvas(InkCanvas *canvas);

    Q_INVOKABLE void openLibrary(const QString &pathOrUrl);
    Q_INVOKABLE void openPage(const QString &path);
    Q_INVOKABLE QString newSubject();
    /// With a first page: an empty folder is no notebook.
    Q_INVOKABLE QString newFolder(const QString &parent);
    Q_INVOKABLE QString newPage(const QString &parent);
    Q_INVOKABLE QString rename(const QString &path, const QString &name);
    Q_INVOKABLE void remove(const QString &path);
    Q_INVOKABLE void setColor(const QString &path, const QString &color);
    Q_INVOKABLE void toggleExpanded(const QString &path);
    /// `delta`: -1 up, +1 down.
    Q_INVOKABLE void moveBy(const QString &path, int delta);
    Q_INVOKABLE QString parentOf(const QString &path) const;
    Q_INVOKABLE QString currentFolder() const;
    Q_INVOKABLE bool inFocus(const QString &path) const;
    Q_INVOKABLE void saveNow();
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
    void renameRequested(const QString &path);
    void treeViewChanged();
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
    void refreshTree();
    void rewatch();
    void onCanvasEdited();
    void onFileChanged(const QString &file);
    void expandTo(const QString &path);
    void storeExpanded();
    void ensureStarterContent();
    QString createFolder(const QString &parent, const QString &baseName);
    QString createPage(const QString &parent);
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
    bool m_awaitingSnip = false;
    QString m_screenshotRequest;
    bool m_notebookView = false;
    QString m_treeFocus;
    quint64 m_savedRevision = 0;
    bool m_dirty = false;
    QTimer m_saveTimer;
    QTimer m_refreshTimer;
    QFileSystemWatcher m_watcher;
    /// To tell our own writes from other programs' changes.
    QString m_lastSavedFile;
    QDateTime m_lastSavedTime;
};

#endif

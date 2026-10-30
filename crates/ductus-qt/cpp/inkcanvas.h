// Include guards: the build reaches headers through two include paths.
#ifndef DUCTUS_INKCANVAS_H
#define DUCTUS_INKCANVAS_H

#include <QAbstractListModel>
#include <QColor>
#include <QImage>
#include <QElapsedTimer>
#include <QHash>
#include <QPointF>
#include <QPointer>
#include <QQuickItem>
#include <QTimer>
#include <QtQml/qqmlregistration.h>

#include "rust/cxx.h"

struct CanvasEngine;
class QSGImageNode;
class QTabletEvent;
class QQuickWindow;
class QMimeData;

class TextBoxModel : public QAbstractListModel
{
    Q_OBJECT
    QML_ANONYMOUS

public:
    enum Role {
        IdRole = Qt::UserRole + 1,
        MarkdownRole,
        BoxXRole,
        BoxYRole,
        BoxWidthRole,
        BoxHeightRole,
        ZoomScaleRole,
        DocScaleRole,
        SelectedRole,
    };

    struct Row {
        qint64 id = 0;
        QString markdown;
        QRectF box;
        double zoomScale = 1;
        double docScale = 1;
        bool selected = false;
        bool operator==(const Row &o) const
        {
            return id == o.id && markdown == o.markdown && box == o.box
                && zoomScale == o.zoomScale && docScale == o.docScale && selected == o.selected;
        }
    };

    using QAbstractListModel::QAbstractListModel;
    int rowCount(const QModelIndex &parent = {}) const override;
    QVariant data(const QModelIndex &index, int role) const override;
    QHash<int, QByteArray> roleNames() const override;
    void setRows(QList<Row> rows);

private:
    QList<Row> m_rows;
};

class InkCanvas : public QQuickItem
{
    Q_OBJECT
    QML_ELEMENT

    Q_PROPERTY(Tool tool READ tool WRITE setTool NOTIFY toolChanged)
    Q_PROPERTY(QColor penColor READ penColor WRITE setPenColor NOTIFY penColorChanged)
    Q_PROPERTY(qreal penWidth READ penWidth WRITE setPenWidth NOTIFY penWidthChanged)
    /// "solid", "dashed" or "dotted".
    Q_PROPERTY(QString penPattern READ penPattern WRITE setPenPattern NOTIFY penPatternChanged)
    Q_PROPERTY(QColor highlighterColor READ highlighterColor WRITE setHighlighterColor NOTIFY highlighterChanged)
    Q_PROPERTY(qreal highlighterWidth READ highlighterWidth WRITE setHighlighterWidth NOTIFY highlighterChanged)
    Q_PROPERTY(bool fingerDrawing READ fingerDrawing WRITE setFingerDrawing NOTIFY fingerDrawingChanged)
    Q_PROPERTY(bool canUndo READ canUndo NOTIFY historyChanged)
    Q_PROPERTY(bool canRedo READ canRedo NOTIFY historyChanged)
    Q_PROPERTY(qreal zoom READ zoom NOTIFY viewChanged)
    /// "endless", or a paper size name ("a4", "a5", "letter").
    Q_PROPERTY(QString pageKind READ pageKind NOTIFY pageStyleChanged)
    Q_PROPERTY(bool pageLandscape READ pageLandscape NOTIFY pageStyleChanged)
    /// "blank", "lined", "grid" or "dotted".
    Q_PROPERTY(QString pagePattern READ pagePattern NOTIFY pageStyleChanged)
    /// 0 narrow, 1 normal, 2 wide.
    Q_PROPERTY(int pageSpacing READ pageSpacing WRITE setPageSpacing NOTIFY pageStyleChanged)
    /// 0 light, 1 normal, 2 firm.
    Q_PROPERTY(int pressureResponse READ pressureResponse WRITE setPressureResponse NOTIFY inputSettingsChanged)
    /// 0 = off … 3 = strong.
    Q_PROPERTY(int smoothing READ smoothing WRITE setSmoothing NOTIFY inputSettingsChanged)
    Q_PROPERTY(bool eraserWholeStrokes READ eraserWholeStrokes WRITE setEraserWholeStrokes NOTIFY eraserWholeStrokesChanged)
    Q_PROPERTY(int eraserSize READ eraserSize WRITE setEraserSize NOTIFY eraserSizeChanged)
    Q_PROPERTY(bool shapeAssist READ shapeAssist WRITE setShapeAssist NOTIFY shapeAssistChanged)
    Q_PROPERTY(int selectionCount READ selectionCount NOTIFY selectionChanged)
    Q_PROPERTY(TextBoxModel *texts READ texts CONSTANT)
    Q_PROPERTY(QRectF mathDraftRect READ mathDraftRect NOTIFY mathDraftChanged)
    Q_PROPERTY(bool selectionHasInk READ selectionHasInk NOTIFY selectionChanged)
    /// Flipping or rotating would change something.
    Q_PROPERTY(bool selectionTurns READ selectionTurns NOTIFY selectionChanged)
    Q_PROPERTY(QRectF selectionRect READ selectionRect NOTIFY selectionChanged)
    Q_PROPERTY(int holdDelay READ holdDelay WRITE setHoldDelay NOTIFY inputSettingsChanged)
    /// Touch is ignored this long (ms) after pen activity, so a resting palm doesn't scroll.
    Q_PROPERTY(int palmRejection READ palmRejection WRITE setPalmRejection NOTIFY inputSettingsChanged)
    Q_PROPERTY(bool penTipVisible READ penTipVisible WRITE setPenTipVisible NOTIFY inputSettingsChanged)
    /// Radius of the pen-tip dot in logical pixels.
    Q_PROPERTY(qreal penTipSize READ penTipSize WRITE setPenTipSize NOTIFY inputSettingsChanged)
    Q_PROPERTY(QColor deskColor READ deskColor WRITE setDeskColor NOTIFY deskColorChanged)

public:
    /// Text and Math aren't engine tools: they place boxes.
    enum Tool { Pen, Highlighter, Eraser, Lasso, Text, Math, Hand };
    Q_ENUM(Tool)

    explicit InkCanvas(QQuickItem *parent = nullptr);
    ~InkCanvas() override;

    Tool tool() const;
    void setTool(Tool tool);
    QColor penColor() const { return m_penColor; }
    void setPenColor(const QColor &color);
    qreal penWidth() const;
    void setPenWidth(qreal width);
    QString penPattern() const { return m_penPattern; }
    void setPenPattern(const QString &pattern);
    QColor highlighterColor() const { return m_highlighterColor; }
    void setHighlighterColor(const QColor &color);
    qreal highlighterWidth() const { return m_highlighterWidth; }
    void setHighlighterWidth(qreal width);
    bool fingerDrawing() const;
    void setFingerDrawing(bool on);
    bool canUndo() const;
    bool canRedo() const;
    qreal zoom() const;
    QString pageKind() const;
    bool pageLandscape() const;
    QString pagePattern() const;
    int pageSpacing() const;
    void setPageSpacing(int level);
    QColor deskColor() const { return m_deskColor; }
    int pressureResponse() const { return m_pressureResponse; }
    void setPressureResponse(int response);
    int smoothing() const { return m_smoothing; }
    void setSmoothing(int level);
    void setDeskColor(const QColor &color);
    int holdDelay() const { return m_holdDelay; }
    void setHoldDelay(int ms);
    int palmRejection() const { return m_palmRejection; }
    void setPalmRejection(int ms);
    bool penTipVisible() const { return m_penTipVisible; }
    void setPenTipVisible(bool on);
    qreal penTipSize() const { return m_penTipSize; }
    void setPenTipSize(qreal radius);
    bool eraserWholeStrokes() const { return m_eraserWholeStrokes; }
    int eraserSize() const { return m_eraserSize; }
    void setEraserSize(int radius);
    void setEraserWholeStrokes(bool on);
    bool shapeAssist() const { return m_shapeAssist; }
    void setShapeAssist(bool on);
    int selectionCount() const { return m_selectionCount; }
    TextBoxModel *texts() { return &m_texts; }
    QRectF mathDraftRect() const { return m_mathDraftRect; }
    bool selectionHasInk() const { return m_selectionHasInk; }
    bool selectionTurns() const { return m_selectionTurns; }
    QRectF selectionRect() const { return m_selectionRect; }

    CanvasEngine &engine() { return *m_engine; }
    quint64 revision() const;
    void documentReplaced();

    Q_INVOKABLE void undo();
    Q_INVOKABLE void redo();
    Q_INVOKABLE void clear();
    Q_INVOKABLE void resetView();
    Q_INVOKABLE void zoomBy(qreal factor);
    Q_INVOKABLE void setPageStyle(const QString &kind, bool landscape, const QString &pattern);
    Q_INVOKABLE void selectAll();
    /// Replaces the selected ink with recognized text or a LaTeX formula (one undo step).
    bool replaceInk(const QString &content, bool math, const QString &strokes);
    Q_INVOKABLE void clearSelection();
    Q_INVOKABLE void deleteSelection();
    Q_INVOKABLE void copySelection();
    Q_INVOKABLE void cutSelection();
    /// A picture copied after the last ink copy wins.
    Q_INVOKABLE void paste();
    Q_INVOKABLE bool insertImage(const QImage &image, QPointF at = QPointF(qQNaN(), qQNaN()));
    Q_INVOKABLE bool insertImageFile(const QString &fileOrUrl, QPointF at = QPointF(qQNaN(), qQNaN()));
    /// Its first pages as pictures, like a printout; they arrive once rendered.
    Q_INVOKABLE bool insertPdfFile(const QString &fileOrUrl);
    Q_INVOKABLE void recolorSelection(const QColor &color);
    Q_INVOKABLE void flipSelection(bool horizontal);
    Q_INVOKABLE void rotateSelection(int quarterTurns);
    Q_INVOKABLE void reorderSelection(int mode);
    Q_INVOKABLE QPointF toDoc(QPointF itemPoint) const;
    Q_INVOKABLE QPointF toScreen(QPointF docPoint) const;
    void finishTextEditing() { Q_EMIT textCommitRequested(); }
    Q_INVOKABLE qint64 addText(QPointF docTopLeft, qreal docWidth, qreal docHeight,
                               const QString &markdown);
    Q_INVOKABLE bool setText(qint64 id, const QString &markdown, qreal docHeight);
    Q_INVOKABLE void setTextHeight(qint64 id, qreal docHeight);
    Q_INVOKABLE void beginMath(QPointF position, const QString &syntax);
    Q_INVOKABLE QStringList editMath(qint64 id);
    Q_INVOKABLE QString setMathDraft(const QString &source, const QString &syntax);
    Q_INVOKABLE bool commitMath();
    Q_INVOKABLE void cancelMath();
    void focusDocRect(const QRectF &rect);
    void focusSheet(int index);

Q_SIGNALS:
    void toolChanged();
    void penColorChanged();
    void penWidthChanged();
    void highlighterChanged();
    void penPatternChanged();
    void fingerDrawingChanged();
    void historyChanged();
    void viewChanged();
    void pageStyleChanged();
    void deskColorChanged();
    void inputSettingsChanged();
    void eraserWholeStrokesChanged();
    void eraserSizeChanged();
    void shapeAssistChanged();
    void selectionChanged();
    void textRequested(QPointF position, qint64 id);
    void mathRequested(QPointF position, qint64 id);
    void mathDraftChanged();
    void textCommitRequested();
    void notice(const QString &message, bool error);
    void shapeRecognized(const QString &kind);
    void edited();
    /// Pen, mouse or finger went down on it: its half of a split view becomes the active one.
    void pressed();

protected:
    bool event(QEvent *event) override;
    bool eventFilter(QObject *watched, QEvent *event) override;
    void itemChange(ItemChange change, const ItemChangeData &value) override;
    void mousePressEvent(QMouseEvent *event) override;
    void mouseMoveEvent(QMouseEvent *event) override;
    void mouseReleaseEvent(QMouseEvent *event) override;
    void hoverMoveEvent(QHoverEvent *event) override;
    void hoverLeaveEvent(QHoverEvent *event) override;
    void touchEvent(QTouchEvent *event) override;
    void wheelEvent(QWheelEvent *event) override;
    void dragEnterEvent(QDragEnterEvent *event) override;
    void dragMoveEvent(QDragMoveEvent *event) override;
    void dropEvent(QDropEvent *event) override;
    void geometryChange(const QRectF &newGeometry, const QRectF &oldGeometry) override;
    QSGNode *updatePaintNode(QSGNode *oldNode, UpdatePaintNodeData *data) override;
    void releaseResources() override;

private:
    bool handleTablet(QTabletEvent *event);
    void forwardAsMouse(QTabletEvent *event);
    /// False where a control, popup or dialog is on top.
    bool ownsScenePoint(QPointF scenePosition) const;
    QQuickItem *inputItemAt(QQuickItem *item, QPointF scenePosition) const;
    void afterEdit();
    void trackHold(QPointF position);
    void stopHold();
    void onHold();
    void updateSelection();
    void pollPrintout();
    void refreshTexts();
    bool insertFromMime(const QMimeData *mime, QPointF at);
    void viewMoved();
    void showPenTip(QPointF position);
    void hidePenTip();
    void hideCursor(bool hide);

    rust::Box<CanvasEngine> m_engine;
    QColor m_penColor;
    QColor m_deskColor;
    QString m_penPattern = QStringLiteral("solid");
    QColor m_highlighterColor = QColor::fromRgbF(1.0f, 0.85f, 0.1f);
    qreal m_highlighterWidth = 18;
    int m_pressureResponse = 1;
    int m_smoothing = 2;
    quint64 m_revision = 0;

    QPointF m_tipPosition;
    bool m_tipVisible = false;
    QColor m_tipColor;
    QElapsedTimer m_penSeen;
    bool m_cursorHidden = false;

    bool m_mousePanning = false;
    Qt::MouseButton m_panButton = Qt::MiddleButton;
    /// The hand tool's pen on empty paper scrolls the page.
    bool m_tabletPanning = false;
    QPointF m_lastPanPosition;
    bool m_touchDrawing = false;
    bool m_pinching = false;
    qreal m_pinchStartZoom = 1;
    QPointF m_pinchStartOffset;
    QPointF m_pinchStartCenter;
    qreal m_pinchStartDistance = 1;
    QPointF m_lastTouchPosition;
    // A finger tap with the text or formula tool places a box, as a pen tap does.
    QPointF m_tapStart;
    QElapsedTimer m_tapTime;
    bool m_tapCandidate = false;
    QTimer m_settleTimer;

    bool m_shapeAssist = true;
    int m_holdDelay = 500;
    int m_palmRejection = 600;
    bool m_penTipVisible = true;
    qreal m_penTipSize = 2.5;
    bool m_eraserWholeStrokes = true;
    int m_eraserSize = 8;
    bool m_tipEraser = false;
    bool m_tipBuiltEraser = false;
    qreal m_tipBuiltRadius = 0;
    QTimer m_holdTimer;
    QTimer m_markTimer;
    QPointF m_holdAnchor;
    bool m_textTool = false;
    bool m_mathTool = false;
    QRectF m_mathDraftRect;
    TextBoxModel m_texts;
    /// The system clipboard changed after our last ink copy.
    bool m_systemClipboardNewer = true;
    int m_selectionCount = 0;
    bool m_selectionHasInk = false;
    bool m_selectionTurns = false;
    QTimer m_printoutPoll;
    QString m_printoutName;
    QRectF m_selectionRect;

    // Only touched in updatePaintNode (render thread, GUI thread blocked).
    struct TileNode {
        QSGImageNode *node = nullptr;
        quint64 generation = 0;
    };
    QHash<quint64, TileNode> m_tileNodes;
    QSGNode *m_baseLayer = nullptr;
    QSGNode *m_overlayLayer = nullptr;
    QSGNode *m_tipLayer = nullptr;
    QSGImageNode *m_tipNode = nullptr;
    /// A stroke that started on the canvas keeps the pen until release.
    /// Every canvas: the pen events of a window reach all of them.
    static inline QList<InkCanvas *> s_canvases;
    /// Another canvas of this window takes the pen there, or forwards it as a mouse.
    bool otherTakesPen(QPointF scenePosition) const;
    bool m_tabletGrab = false;
    /// A pen press elsewhere is forwarded as mouse input until release.
    bool m_tabletForward = false;
    bool m_mouseForwarded = false;
    Qt::MouseButton m_forwardButton = Qt::LeftButton;
    QPointer<QQuickWindow> m_filteredWindow;
};

#endif

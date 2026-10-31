#include "ductus-qt/cpp/inkcanvas.h"
#include "ductus-qt/cpp/platformfiles.h"
#include "ductus-qt/src/ffi.cxxqt.h"

#include <QBuffer>
#include <QClipboard>
#include <QFile>
#include <QFileInfo>
#include <QGuiApplication>
#include <QImage>
#include <QImageReader>
#include <QMimeData>
#include <QUrl>
#include <QLineF>
#include <QNativeGestureEvent>
#include <QPainter>
#include <QPointingDevice>
#include <QQuickWindow>
#include <QSGImageNode>
#include <QSGTransformNode>
#include <QTabletEvent>
#include <QTouchEvent>
#include <QWheelEvent>
#include <private/qhighdpiscaling_p.h>
#include <qpa/qwindowsysteminterface.h>
#ifdef Q_OS_WIN
#include "ductus-qt/cpp/winframe.h"
#endif
#ifdef DUCTUS_WAYLAND
#include <QtWaylandClient/private/qwaylanddisplay_p.h>
#include <QtWaylandClient/private/qwaylandinputdevice_p.h>
#include <QtWaylandClient/private/qwaylandintegration_p.h>
#endif

#include <algorithm>
#include <cmath>
#include <string>

namespace {

constexpr double kWheelZoomStep = 1.15;
constexpr double kWheelScrollStep = 50.0;
// A finger that moves less than this, this quickly, tapped rather than scrolled.
constexpr double kTapSlop = 12.0;
constexpr qint64 kTapTime = 500;
constexpr double kMinZoom = 0.1;
constexpr double kMaxZoom = 10.0;
/// Larger pictures are scaled down to this many pixels on their long side.
constexpr int kMaxPictureSide = 4096;
/// Movement (logical px) that still counts as resting.
constexpr qreal kHoldSlop = 3.0;

PointerSample sampleAt(QPointF position, quint64 timestamp)
{
    return PointerSample{position.x(), position.y(), 0.0, false, 0.0, 0.0,
                         static_cast<std::uint32_t>(timestamp)};
}

/// Packs layer and tile indices (each within ±2^30) into one hash key.
quint64 tileKey(const TileInfo &t)
{
    constexpr quint64 mask = 0x7FFFFFFF;
    // One layer per hash: the position is the key.
    return ((quint64(quint32(t.x)) & mask) << 31) | (quint64(quint32(t.y)) & mask);
}

// The Qt button a pen's side button arrives as depends on the platform. Linux reports the lower one
// (the only one on most pens) as middle and the upper as right. Windows knows one barrel button: right.
// Android passes its own flags on, so stylus buttons arrive as ExtraButton3/4, and Samsung's S Pen
// as the secondary (right) button.
#if defined(Q_OS_LINUX) && !defined(Q_OS_ANDROID)
const Qt::MouseButtons firstPenButton = Qt::MiddleButton;
const Qt::MouseButtons secondPenButton = Qt::RightButton;
#else
const Qt::MouseButtons firstPenButton = Qt::RightButton | Qt::ExtraButton3;
const Qt::MouseButtons secondPenButton = Qt::MiddleButton | Qt::ExtraButton4;
#endif

std::uint8_t engineTool(InkCanvas::Tool tool)
{
    return tool == InkCanvas::Hand ? 4 : static_cast<std::uint8_t>(tool);
}

// On Wayland a pen gets the mouse's cursor, which Qt only updates while the mouse is over the window.
void setPenCursor(QWindow *window, const QCursor &cursor)
{
#ifdef DUCTUS_WAYLAND
    auto *integration = QtWaylandClient::QWaylandIntegration::instance();
    if (!integration || !QGuiApplication::platformName().startsWith(u"wayland"))
        return;
    for (QtWaylandClient::QWaylandInputDevice *seat : integration->display()->inputDevices())
        seat->setCursor(&cursor, {}, qCeil(window->devicePixelRatio()));
#else
    Q_UNUSED(window);
    Q_UNUSED(cursor);
#endif
}

#ifdef Q_OS_WIN
// Qt on Windows reads a pen's buttons from the mouse, which lags behind the pen: a lifted pen still
// holds the left button until it moves on, or for good once it leaves. A tap on a button then never
// clicks and the next touch is no press. Windows' own pen-down and pen-up messages decide instead.
QEvent::Type penContactType(const QTabletEvent *event)
{
    static bool down = false;
    const int change = WinPen::contactChangeAt(event->timestamp());
    const bool contact = change != 0 ? change > 0
                         : down       ? event->type() != QEvent::TabletRelease
                                      : event->type() == QEvent::TabletPress;
    const QEvent::Type type = contact == down ? QEvent::TabletMove
                              : contact       ? QEvent::TabletPress
                                              : QEvent::TabletRelease;
    down = contact;
    return type;
}

// Returned as a prvalue: tablet events can't be copied or moved.
QTabletEvent penEventAs(const QTabletEvent *event, QEvent::Type type)
{
    const bool contact = type != QEvent::TabletRelease
        && (type == QEvent::TabletPress || event->buttons() != Qt::NoButton);
    const Qt::MouseButtons held = event->buttons() ? event->buttons() : Qt::MouseButtons(Qt::LeftButton);
    const Qt::MouseButton button = type == QEvent::TabletMove ? Qt::NoButton
                                   : held & Qt::RightButton   ? Qt::RightButton
                                                              : Qt::LeftButton;
    return QTabletEvent(type, event->pointingDevice(), event->position(), event->globalPosition(),
                        contact ? event->pressure() : 0.0, event->xTilt(), event->yTilt(),
                        event->tangentialPressure(), event->rotation(), event->z(), event->modifiers(),
                        button, contact ? held : Qt::NoButton);
}
#endif

bool isMouse(const QPointerEvent *event)
{
    const auto *device = event->pointingDevice();
    return device
        && (device->type() == QInputDevice::DeviceType::Mouse
            || device->type() == QInputDevice::DeviceType::TouchPad);
}

}

InkCanvas::InkCanvas(QQuickItem *parent)
    : QQuickItem(parent)
    , m_engine(new_canvas_engine())
    , m_penColor(QColor::fromRgbF(0.1f, 0.1f, 0.12f))
{
    s_canvases << this;
    setFlag(ItemHasContents);
    setAcceptedMouseButtons(Qt::LeftButton | Qt::MiddleButton);
    setAcceptHoverEvents(true);
    setAcceptTouchEvents(true);
    setFlag(ItemAcceptsDrops);
    connect(QGuiApplication::clipboard(), &QClipboard::dataChanged, this,
            [this] { m_systemClipboardNewer = true; });
    setCursor(Qt::CrossCursor);
    m_engine->set_eraser_whole_strokes(m_eraserWholeStrokes);

    m_settleTimer.setSingleShot(true);
    m_settleTimer.setInterval(150);
    connect(&m_settleTimer, &QTimer::timeout, this, [this] {
        m_engine->settle();
        repaint();
    });

    m_markTimer.setSingleShot(true);
    m_markTimer.setInterval(2500);
    connect(&m_markTimer, &QTimer::timeout, this, [this] {
        m_engine->clear_mark();
        repaint();
    });

    m_holdTimer.setSingleShot(true);
    m_holdTimer.setInterval(m_holdDelay);
    connect(&m_holdTimer, &QTimer::timeout, this, &InkCanvas::onHold);

    m_printoutPoll.setInterval(50);
    connect(&m_printoutPoll, &QTimer::timeout, this, &InkCanvas::pollPrintout);

    m_penSeen.invalidate();
}

InkCanvas::~InkCanvas()
{
    s_canvases.removeAll(this);
    hideCursor(false);
}

InkCanvas::Tool InkCanvas::tool() const
{
    if (m_textTool)
        return Text;
    if (m_mathTool)
        return Math;
    // The engine counts the hand as 4, after the lasso.
    const std::uint8_t engineTool = m_engine->tool();
    return engineTool == 4 ? Hand : static_cast<Tool>(engineTool);
}

void InkCanvas::setTool(Tool tool)
{
    if (tool == this->tool())
        return;
    m_textTool = tool == Text;
    m_mathTool = tool == Math;
    if (!m_textTool && !m_mathTool)
        m_engine->set_tool(engineTool(tool));
    else
        clearSelection();
    setCursor(tool == Hand ? Qt::OpenHandCursor : Qt::CrossCursor);
    Q_EMIT toolChanged();
}

int TextBoxModel::rowCount(const QModelIndex &parent) const
{
    return parent.isValid() ? 0 : static_cast<int>(m_rows.size());
}

QVariant TextBoxModel::data(const QModelIndex &index, int role) const
{
    if (!index.isValid() || index.row() >= m_rows.size())
        return {};
    const Row &row = m_rows.at(index.row());
    switch (role) {
    case IdRole:
        return row.id;
    case MarkdownRole:
        return row.markdown;
    case BoxXRole:
        return row.box.x();
    case BoxYRole:
        return row.box.y();
    case BoxWidthRole:
        return row.box.width();
    case BoxHeightRole:
        return row.box.height();
    case ZoomScaleRole:
        return row.zoomScale;
    case DocScaleRole:
        return row.docScale;
    case SelectedRole:
        return row.selected;
    case StackRole:
        return row.stack;
    default:
        return {};
    }
}

QHash<int, QByteArray> TextBoxModel::roleNames() const
{
    return {{IdRole, "textId"},       {MarkdownRole, "markdown"},   {BoxXRole, "boxX"},
            {BoxYRole, "boxY"},       {BoxWidthRole, "boxWidth"},   {BoxHeightRole, "boxHeight"},
            {ZoomScaleRole, "zoomScale"}, {DocScaleRole, "docScale"}, {SelectedRole, "selected"},
            {StackRole, "stack"}};
}

void TextBoxModel::setRows(QList<Row> rows)
{
    if (rows == m_rows)
        return;
    // Same boxes in the same order: update in place, so an editor keeps its state.
    bool sameShape = rows.size() == m_rows.size();
    for (qsizetype i = 0; sameShape && i < rows.size(); ++i)
        sameShape = rows[i].id == m_rows[i].id;
    if (sameShape) {
        m_rows = std::move(rows);
        Q_EMIT dataChanged(index(0), index(static_cast<int>(m_rows.size()) - 1));
        return;
    }
    beginResetModel();
    m_rows = std::move(rows);
    endResetModel();
}

void InkCanvas::repaint()
{
    update();
    Q_EMIT repainted();
}

void InkCanvas::refreshTexts()
{
    const int bands = static_cast<int>(m_engine->bands());
    if (bands != m_bands) {
        m_bands = bands;
        Q_EMIT bandsChanged();
    }
    const rust::Vec<double> m = m_engine->math_draft_rect();
    const QRectF draft = m.size() == 4 ? QRectF(m[0], m[1], m[2], m[3]) : QRectF();
    if (draft != m_mathDraftRect) {
        m_mathDraftRect = draft;
        Q_EMIT mathDraftChanged();
    }
    const rust::Vec<TextRow> rows = m_engine->text_rows();
    const double zoom = m_engine->zoom();
    QList<TextBoxModel::Row> out;
    out.reserve(static_cast<qsizetype>(rows.size()));
    for (const TextRow &r : rows) {
        out.append({static_cast<qint64>(r.id),
                    QString::fromUtf8(r.markdown.data(), static_cast<qsizetype>(r.markdown.size())),
                    QRectF(r.x, r.y, r.width, r.height), r.scale, r.scale / zoom, r.selected, r.z});
    }
    m_texts.setRows(std::move(out));
}

QPointF InkCanvas::toScreen(QPointF docPoint) const
{
    const qreal zoom = m_engine->zoom();
    return {docPoint.x() * zoom + m_engine->offset_x(), docPoint.y() * zoom + m_engine->offset_y()};
}

QPointF InkCanvas::toDoc(QPointF itemPoint) const
{
    const rust::Vec<double> p = m_engine->to_doc(itemPoint.x(), itemPoint.y());
    return {p[0], p[1]};
}

qint64 InkCanvas::addText(QPointF docTopLeft, qreal docWidth, qreal docHeight,
                          const QString &markdown)
{
    const qint64 id = static_cast<qint64>(m_engine->add_text(
        docTopLeft.x(), docTopLeft.y(), docWidth, docHeight, markdown.toStdString()));
    afterEdit();
    return id;
}

bool InkCanvas::setText(qint64 id, const QString &markdown, qreal docHeight)
{
    const bool changed =
        m_engine->set_text(static_cast<std::uint64_t>(id), markdown.toStdString(), docHeight);
    afterEdit();
    return changed;
}

void InkCanvas::beginMath(QPointF position, const QString &syntax)
{
    // The formula takes the pen's color, so it matches the notes around it.
    m_engine->begin_math(position.x(), position.y(), syntax.toStdString(), m_penColor.redF(),
                         m_penColor.greenF(), m_penColor.blueF());
    afterEdit();
}

QStringList InkCanvas::editMath(qint64 id)
{
    const rust::Vec<rust::String> found = m_engine->edit_math(static_cast<std::uint64_t>(id));
    QStringList out;
    for (const rust::String &s : found)
        out << QString::fromUtf8(s.data(), static_cast<qsizetype>(s.size()));
    afterEdit();
    return out;
}

QString InkCanvas::setMathDraft(const QString &source, const QString &syntax)
{
    const rust::String problem = m_engine->set_math_draft(source.toStdString(), syntax.toStdString());
    afterEdit();
    return QString::fromUtf8(problem.data(), static_cast<qsizetype>(problem.size()));
}

bool InkCanvas::commitMath()
{
    const bool changed = m_engine->commit_math();
    afterEdit();
    return changed;
}

void InkCanvas::focusDocRect(const QRectF &rect)
{
    m_engine->focus_rect(rect.x(), rect.y(), rect.width(), rect.height());
    m_markTimer.start();
    viewMoved();
}

void InkCanvas::focusSheet(int index)
{
    if (index < 0)
        return;
    m_engine->focus_sheet(static_cast<std::size_t>(index));
    viewMoved();
}

void InkCanvas::cancelMath()
{
    m_engine->cancel_math();
    afterEdit();
}

void InkCanvas::setTextHeight(qint64 id, qreal docHeight)
{
    m_engine->set_text_height(static_cast<std::uint64_t>(id), docHeight);
    refreshTexts();
    // A taller box may reach items stacked on it.
    repaint();
}

void InkCanvas::setPenColor(const QColor &color)
{
    if (color == m_penColor)
        return;
    m_penColor = color;
    m_engine->set_pen_color(color.redF(), color.greenF(), color.blueF(), color.alphaF());
    Q_EMIT penColorChanged();
    repaint();
}

void InkCanvas::setPenPattern(const QString &pattern)
{
    if (pattern == m_penPattern)
        return;
    m_penPattern = pattern;
    m_engine->set_pen_pattern(pattern.toStdString());
    Q_EMIT penPatternChanged();
}

void InkCanvas::setHighlighterColor(const QColor &color)
{
    if (color == m_highlighterColor)
        return;
    m_highlighterColor = color;
    m_engine->set_highlighter(color.redF(), color.greenF(), color.blueF(), m_highlighterWidth);
    Q_EMIT highlighterChanged();
}

void InkCanvas::setHighlighterWidth(qreal width)
{
    if (qFuzzyCompare(width, m_highlighterWidth))
        return;
    m_highlighterWidth = width;
    m_engine->set_highlighter(m_highlighterColor.redF(), m_highlighterColor.greenF(),
                              m_highlighterColor.blueF(), width);
    Q_EMIT highlighterChanged();
}

qreal InkCanvas::penWidth() const
{
    return m_engine->pen_width();
}

void InkCanvas::setPenWidth(qreal width)
{
    if (qFuzzyCompare(width, penWidth()))
        return;
    m_engine->set_pen_width(width);
    Q_EMIT penWidthChanged();
}

bool InkCanvas::fingerDrawing() const
{
    return m_engine->finger_draws();
}

void InkCanvas::setFingerDrawing(bool on)
{
    if (on == fingerDrawing())
        return;
    m_engine->set_finger_draws(on);
    Q_EMIT fingerDrawingChanged();
}

bool InkCanvas::canUndo() const
{
    return m_engine->can_undo();
}

bool InkCanvas::canRedo() const
{
    return m_engine->can_redo();
}

qreal InkCanvas::zoom() const
{
    return m_engine->zoom();
}

void InkCanvas::undo()
{
    m_engine->undo();
    afterEdit();
}

void InkCanvas::redo()
{
    m_engine->redo();
    afterEdit();
}

void InkCanvas::clear()
{
    m_engine->clear();
    afterEdit();
}

void InkCanvas::resetView()
{
    m_engine->reset_view();
    viewMoved();
}

void InkCanvas::zoomBy(qreal factor)
{
    m_engine->zoom_around(width() / 2, height() / 2, m_engine->zoom() * factor);
    viewMoved();
}

void InkCanvas::setEraserWholeStrokes(bool on)
{
    if (on == m_eraserWholeStrokes)
        return;
    m_eraserWholeStrokes = on;
    m_engine->set_eraser_whole_strokes(on);
    Q_EMIT eraserWholeStrokesChanged();
}

void InkCanvas::setEraserSize(int radius)
{
    radius = qBound(2, radius, 80);
    if (radius == m_eraserSize)
        return;
    m_eraserSize = radius;
    m_engine->set_eraser_size(radius);
    repaint();
    Q_EMIT eraserSizeChanged();
}

void InkCanvas::setShapeAssist(bool on)
{
    if (on == m_shapeAssist)
        return;
    m_shapeAssist = on;
    if (!on)
        stopHold();
    Q_EMIT shapeAssistChanged();
}

bool InkCanvas::replaceInk(const QString &content, bool math, const QString &strokes)
{
    const rust::Vec<double> added = m_engine->replace_ink(content.toStdString(), math, strokes.toStdString());
    if (added.empty())
        return false;
    afterEdit();
    // The result opens in its editor, so a misread is fixed right away.
    const auto id = static_cast<qint64>(added[0]);
    const QPointF end = toScreen(QPointF(added[1], added[2])) - QPointF(2, 2);
    setTool(math ? Math : Text);
    if (math)
        Q_EMIT mathRequested(end, id);
    else
        Q_EMIT textRequested(end, id);
    return true;
}

void InkCanvas::selectAll()
{
    setTool(Lasso);
    m_engine->select_all();
    afterEdit();
}

void InkCanvas::clearSelection()
{
    m_engine->clear_selection();
    afterEdit();
}

void InkCanvas::deleteSelection()
{
    m_engine->delete_selection();
    afterEdit();
}

void InkCanvas::copySelection()
{
    m_engine->copy_selection();
    m_systemClipboardNewer = false;
}

void InkCanvas::cutSelection()
{
    m_engine->cut_selection();
    m_systemClipboardNewer = false;
    afterEdit();
}

static bool hasPicture(const QMimeData *mime)
{
    if (mime->hasImage())
        return true;
    const QList<QUrl> urls = mime->urls();
    return std::any_of(urls.begin(), urls.end(), [](const QUrl &url) {
        return url.isLocalFile() && !QImageReader::imageFormat(url.toLocalFile()).isEmpty();
    });
}

void InkCanvas::paste()
{
    // A picture copied after the last ink copy wins; if it fails, no ink is pasted either.
    const QMimeData *mime = QGuiApplication::clipboard()->mimeData();
    if ((m_systemClipboardNewer || !m_engine->can_paste()) && mime && hasPicture(mime)) {
        insertFromMime(mime, QPointF(qQNaN(), qQNaN()));
        return;
    }
    if (!m_engine->can_paste())
        return;
    setTool(Lasso);
    m_engine->paste();
    afterEdit();
}

bool InkCanvas::insertImage(const QImage &image, QPointF at)
{
    if (image.isNull())
        return false;
    QImage picture = image;
    if (qMax(picture.width(), picture.height()) > kMaxPictureSide)
        picture = picture.scaled(kMaxPictureSide, kMaxPictureSide, Qt::KeepAspectRatio,
                                 Qt::SmoothTransformation);
    QByteArray png;
    QBuffer buffer(&png);
    buffer.open(QIODevice::WriteOnly);
    if (!picture.save(&buffer, "PNG"))
        return false;
    if (std::isnan(at.x()))
        at = QPointF(width() / 2, height() / 2);
    const bool ok = m_engine->insert_picture(
        rust::Slice<const std::uint8_t>(reinterpret_cast<const std::uint8_t *>(png.constData()),
                                        static_cast<std::size_t>(png.size())),
        at.x(), at.y());
    if (ok) {
        Q_EMIT toolChanged();
        afterEdit();
    }
    return ok;
}

bool InkCanvas::insertImageFile(const QString &fileOrUrl, QPointF at)
{
    const QString file = PlatformFiles::readable(fileOrUrl);
    QImageReader reader(file);
    reader.setAutoTransform(true); // Photos taken sideways.
    const QImage image = reader.read();
    if (image.isNull()) {
        Q_EMIT notice(tr("%1 isn't a picture Ductus can read.").arg(PlatformFiles::displayName(fileOrUrl)), true);
        return false;
    }
    return insertImage(image, at);
}

bool InkCanvas::insertPdfFile(const QString &fileOrUrl)
{
    if (m_engine->printout_busy())
        return false;
    QFile file(PlatformFiles::readable(fileOrUrl));
    const QByteArray pdf = file.open(QIODevice::ReadOnly) ? file.readAll() : QByteArray();
    m_engine->start_printout(rust::Slice<const std::uint8_t>(
        reinterpret_cast<const std::uint8_t *>(pdf.constData()), static_cast<std::size_t>(pdf.size())));
    m_printoutName = PlatformFiles::displayName(fileOrUrl);
    m_printoutPoll.start();
    return true;
}

void InkCanvas::pollPrintout()
{
    const PrintoutState state = m_engine->take_printout(width() / 2, height() / 2);
    if (!state.done)
        return;
    m_printoutPoll.stop();
    if (state.error) {
        Q_EMIT notice(tr("%1 isn't a PDF Ductus can read.").arg(m_printoutName), true);
        return;
    }
    Q_EMIT toolChanged();
    afterEdit();
    if (state.placed < state.total)
        Q_EMIT notice(tr("The first %1 of %2 pages are in. For the whole PDF, import it as note pages.")
                          .arg(state.placed)
                          .arg(state.total),
                      false);
}

bool InkCanvas::insertFromMime(const QMimeData *mime, QPointF at)
{
    if (!mime)
        return false;
    if (mime->hasImage())
        return insertImage(qvariant_cast<QImage>(mime->imageData()), at);
    for (const QUrl &url : mime->urls()) {
        if (url.isLocalFile() && !QImageReader::imageFormat(url.toLocalFile()).isEmpty())
            return insertImageFile(url.toLocalFile(), at);
    }
    return false;
}

void InkCanvas::dragEnterEvent(QDragEnterEvent *event)
{
    if (hasPicture(event->mimeData()))
        event->acceptProposedAction();
    else
        event->ignore();
}

void InkCanvas::dragMoveEvent(QDragMoveEvent *event)
{
    event->acceptProposedAction();
}

void InkCanvas::dropEvent(QDropEvent *event)
{
    if (insertFromMime(event->mimeData(), event->position()))
        event->acceptProposedAction();
}

void InkCanvas::recolorSelection(const QColor &color)
{
    m_engine->recolor_selection(color.redF(), color.greenF(), color.blueF(), color.alphaF());
    afterEdit();
}

void InkCanvas::flipSelection(bool horizontal)
{
    m_engine->flip_selection(horizontal);
    afterEdit();
}

void InkCanvas::rotateSelection(int quarterTurns)
{
    m_engine->rotate_selection(quarterTurns);
    afterEdit();
}

void InkCanvas::reorderSelection(int mode)
{
    m_engine->reorder_selection(static_cast<std::uint8_t>(mode));
    afterEdit();
}

void InkCanvas::trackHold(QPointF position)
{
    if (!m_shapeAssist)
        return;
    // Pens jitter a little even when held still.
    if (!m_holdTimer.isActive() || QLineF(position, m_holdAnchor).length() > kHoldSlop) {
        m_holdAnchor = position;
        m_holdTimer.start();
    }
}

void InkCanvas::stopHold()
{
    m_holdTimer.stop();
}

void InkCanvas::onHold()
{
    if (!m_engine->is_drawing())
        return;
    const QString kind = QString::fromStdString(std::string(m_engine->hold()));
    if (!kind.isEmpty()) {
        repaint();
        Q_EMIT shapeRecognized(kind);
    }
}

void InkCanvas::updateSelection()
{
    const int count = static_cast<int>(m_engine->selection_count());
    const rust::Vec<double> r = m_engine->selection_rect();
    const QRectF rect = r.size() == 4 ? QRectF(r[0], r[1], r[2], r[3]) : QRectF();
    const bool ink = count > 0 && m_engine->selection_has_ink();
    const bool turns = count > 0 && m_engine->selection_turns();
    if (count != m_selectionCount || rect != m_selectionRect || ink != m_selectionHasInk
        || turns != m_selectionTurns) {
        m_selectionCount = count;
        m_selectionRect = rect;
        m_selectionHasInk = ink;
        m_selectionTurns = turns;
        Q_EMIT selectionChanged();
    }
}

void InkCanvas::afterEdit()
{
    updateSelection();
    refreshTexts();
    if (m_engine->revision() != m_revision) {
        m_revision = m_engine->revision();
        Q_EMIT historyChanged();
        Q_EMIT pageStyleChanged();
        Q_EMIT edited();
    }
    repaint();
}

quint64 InkCanvas::revision() const
{
    return m_engine->revision();
}

void InkCanvas::documentReplaced()
{
    // A printout still rendering was meant for the page before.
    m_engine->cancel_printout();
    m_printoutPoll.stop();
    m_revision = m_engine->revision();
    updateSelection();
    refreshTexts();
    Q_EMIT historyChanged();
    Q_EMIT pageStyleChanged();
    Q_EMIT viewChanged();
    repaint();
}

QString InkCanvas::pageKind() const
{
    return QString::fromStdString(std::string(m_engine->page_kind()));
}

bool InkCanvas::pageLandscape() const
{
    return m_engine->page_landscape();
}

QString InkCanvas::pagePattern() const
{
    return QString::fromStdString(std::string(m_engine->page_pattern()));
}

int InkCanvas::pageSpacing() const
{
    return m_engine->page_spacing();
}

void InkCanvas::setPageSpacing(int level)
{
    if (level == pageSpacing())
        return;
    m_engine->set_page_spacing(static_cast<std::uint8_t>(qBound(0, level, 2)));
    afterEdit();
}

void InkCanvas::setPageStyle(const QString &kind, bool landscape, const QString &pattern)
{
    const std::string k = (kind.isEmpty() ? pageKind() : kind).toStdString();
    const std::string p = (pattern.isEmpty() ? pagePattern() : pattern).toStdString();
    m_engine->set_page_style(k, landscape, p);
    afterEdit();
}

void InkCanvas::setPressureResponse(int response)
{
    response = qBound(0, response, 2);
    if (response == m_pressureResponse)
        return;
    m_pressureResponse = response;
    constexpr double gammas[] = {0.6, 1.0, 1.6};
    m_engine->set_pressure_gamma(gammas[response]);
    Q_EMIT inputSettingsChanged();
}

void InkCanvas::setSmoothing(int level)
{
    level = qBound(0, level, 3);
    if (level == m_smoothing)
        return;
    m_smoothing = level;
    m_engine->set_smoothing_level(static_cast<std::uint8_t>(level));
    Q_EMIT inputSettingsChanged();
}

void InkCanvas::setDeskColor(const QColor &color)
{
    if (color == m_deskColor)
        return;
    m_deskColor = color;
    m_engine->set_desk_color(color.redF(), color.greenF(), color.blueF());
    Q_EMIT deskColorChanged();
    repaint();
}

void InkCanvas::viewMoved()
{
    updateSelection();
    refreshTexts();
    Q_EMIT viewChanged();
    m_settleTimer.start();
    repaint();
}

bool InkCanvas::event(QEvent *event)
{
    switch (event->type()) {
    case QEvent::NativeGesture: {
        auto *gesture = static_cast<QNativeGestureEvent *>(event);
        if (gesture->gestureType() == Qt::ZoomNativeGesture) {
            const QPointF p = gesture->position();
            m_engine->zoom_around(p.x(), p.y(), m_engine->zoom() * (1.0 + gesture->value()));
            viewMoved();
            return true;
        }
        break;
    }
    default:
        break;
    }
    return QQuickItem::event(event);
}

void InkCanvas::itemChange(ItemChange change, const ItemChangeData &value)
{
    // Qt Quick does not deliver tablet events to items, so the canvas reads them from its window.
    if (change == ItemSceneChange) {
        if (m_filteredWindow)
            m_filteredWindow->removeEventFilter(this);
        m_filteredWindow = value.window;
        if (m_filteredWindow)
            m_filteredWindow->installEventFilter(this);
    }
    QQuickItem::itemChange(change, value);
}

bool InkCanvas::eventFilter(QObject *watched, QEvent *event)
{
    switch (event->type()) {
    case QEvent::TabletPress:
    case QEvent::TabletMove:
    case QEvent::TabletRelease:
        if (watched == window()) {
            auto *tablet = static_cast<QTabletEvent *>(event);
            if (otherTakesPen(tablet->scenePosition())) {
                hidePenTip();
                return false;
            }
#ifdef Q_OS_WIN
            if (const QEvent::Type type = penContactType(tablet); type != tablet->type()) {
                QTabletEvent fixed = penEventAs(tablet, type);
                fixed.setTimestamp(tablet->timestamp());
                tablet->accept();
                return handleTablet(&fixed);
            }
#endif
            return handleTablet(tablet);
        }
        break;
    case QEvent::MouseMove:
        // Some systems move the mouse along with the pen; only a mouse on its own counts.
        if (m_cursorHidden && isMouse(static_cast<QMouseEvent *>(event))
            && m_penSeen.elapsed() > m_palmRejection)
            hideCursor(false);
        break;
    default:
        break;
    }
    return QQuickItem::eventFilter(watched, event);
}

bool InkCanvas::otherTakesPen(QPointF scenePosition) const
{
    if (m_tabletGrab || m_tabletForward)
        return false;
    const InkCanvas *forwarder = nullptr;
    for (const InkCanvas *other : std::as_const(s_canvases)) {
        if (other->window() != window())
            continue;
        if (other != this
            && (other->m_tabletGrab || other->m_tabletForward || other->ownsScenePoint(scenePosition)))
            return true;
        if (!forwarder && other->isVisible())
            forwarder = other;
    }
    // Off every page only one of them turns the pen into a mouse.
    return !ownsScenePoint(scenePosition) && forwarder && forwarder != this;
}

bool InkCanvas::ownsScenePoint(QPointF scenePosition) const
{
    if (!isVisible() || !isEnabled() || !contains(mapFromScene(scenePosition)))
        return false;
    const QQuickItem *hit = inputItemAt(window()->contentItem(), scenePosition);
    return !hit || hit == this;
}

// The topmost item that takes pointer input there; layers without input are skipped.
QQuickItem *InkCanvas::inputItemAt(QQuickItem *item, QPointF scenePosition) const
{
    if (!item->isVisible() || !item->isEnabled())
        return nullptr;
    if (item->inherits("QQuickOverlay")) {
        // It spans the window while a popup is open: only popups cover what lies beneath.
        // An open menu takes the press that closes it; the pen editor stays open to writing.
        for (const QQuickItem *child : item->childItems()) {
            if (!child->isVisible())
                continue;
            const QObject *popup = child->parent();
            if ((popup && popup->inherits("QQuickMenu"))
                || child->contains(child->mapFromScene(scenePosition)))
                return item;
        }
        return nullptr;
    }
    const bool inside = item->contains(item->mapFromScene(scenePosition));
    if (item->clip() && !inside)
        return nullptr;
    // In painting order, like Qt Quick: the canvas is declared after the layers above it, at z -1.
    QList<QQuickItem *> children = item->childItems();
    const auto byZ = [](const QQuickItem *a, const QQuickItem *b) { return a->z() < b->z(); };
    // Most lists already are: this runs on every pen move.
    if (!std::is_sorted(children.cbegin(), children.cend(), byZ))
        std::stable_sort(children.begin(), children.end(), byZ);
    for (auto it = children.crbegin(); it != children.crend(); ++it) {
        if (QQuickItem *hit = inputItemAt(*it, scenePosition))
            return hit;
    }
    if (!inside)
        return nullptr;
    if (item == this || item->acceptedMouseButtons() != Qt::NoButton || item->acceptTouchEvents())
        return item;
    // Items that only have a TapHandler or DragHandler take input too.
    for (const QObject *child : item->children()) {
        if (child->inherits("QQuickPointerHandler") && !child->inherits("QQuickHoverHandler"))
            return item;
    }
    return nullptr;
}

void InkCanvas::forwardAsMouse(QTabletEvent *event)
{
    m_mouseForwarded = true;
    QEvent::Type type = QEvent::MouseMove;
    if (event->type() == QEvent::TabletPress) {
        type = QEvent::MouseButtonPress;
        // The tip with a barrel button held is a right click.
        m_forwardButton = event->buttons() & (firstPenButton | secondPenButton) ? Qt::RightButton
                                                                                : Qt::LeftButton;
    } else if (event->type() == QEvent::TabletRelease) {
        type = QEvent::MouseButtonRelease;
    }
    const bool down = m_tabletForward && type != QEvent::MouseButtonRelease;
    // It expects native pixels like a platform sends; Qt's own are scaled on 125-200% screens.
    QWindowSystemInterface::handleMouseEvent(window(), event->timestamp(),
                                             QHighDpi::toNativeLocalPosition(event->position(), window()),
                                             QHighDpi::toNativeGlobalPosition(event->globalPosition(), window()),
                                             down ? Qt::MouseButtons(m_forwardButton) : Qt::NoButton,
                                             type == QEvent::MouseMove ? Qt::NoButton : m_forwardButton,
                                             type);
}

bool InkCanvas::handleTablet(QTabletEvent *event)
{
    const QPointF scenePosition = event->scenePosition();
    const bool press = event->type() == QEvent::TabletPress;
    // Outside the page the pen is a mouse: Qt's own fallback loses taps to stylus-aware handlers.
    if (m_tabletForward || (!m_tabletGrab && !ownsScenePoint(scenePosition))) {
        hidePenTip();
        hideCursor(false);
        if (press)
            m_tabletForward = true;
        forwardAsMouse(event);
        // One event behind, since the forwarded mouse event is queued.
        setPenCursor(window(), window()->cursor());
        if (event->type() == QEvent::TabletRelease)
            m_tabletForward = false;
        event->accept();
        return true;
    }
    event->accept();
    if (press)
        Q_EMIT pressed();
    m_penSeen.restart();
    hideCursor(true);
    // Otherwise the control the pen last pointed at stays hovered, with its tooltip.
    if (m_mouseForwarded) {
        m_mouseForwarded = false;
        QWindowSystemInterface::handleLeaveEvent(window());
    }

    const QPointF position = mapFromScene(scenePosition);
    const PointerSample sample{position.x(),
                               position.y(),
                               event->pressure(),
                               true,
                               event->xTilt(),
                               event->yTilt(),
                               static_cast<std::uint32_t>(event->timestamp())};
    const bool eraserEnd = event->pointerType() == QPointingDevice::PointerType::Eraser;
    const Tool penTool = this->penTool(event);
    m_tipEraser = penTool == Eraser;

    if (press && penTool == Text) {
        m_tabletGrab = true;
        Q_EMIT textRequested(position,
                             static_cast<qint64>(m_engine->text_at(position.x(), position.y())));
    } else if (press && penTool == Math) {
        m_tabletGrab = true;
        Q_EMIT mathRequested(position,
                             static_cast<qint64>(m_engine->math_at(position.x(), position.y())));
    } else if (press) {
        m_tabletGrab = true;
        // Drawing ends typing in a text box.
        forceActiveFocus();
        if (m_engine->pointer_down(eraserEnd ? 1 : 0, sample, engineTool(penTool))) {
            trackHold(position);
        } else {
            m_tabletPanning = true;
            m_lastPanPosition = position;
        }
        hidePenTip();
    } else if (event->type() == QEvent::TabletMove) {
        if (m_tabletPanning) {
            const QPointF delta = position - m_lastPanPosition;
            m_lastPanPosition = position;
            m_engine->pan_by(delta.x(), delta.y());
            viewMoved();
        } else if (m_engine->is_drawing()) {
            m_engine->pointer_move(rust::Slice<const PointerSample>(&sample, 1));
            trackHold(position);
            updateSelection();
            if (m_engine->has_selection_drag())
                refreshTexts();
        } else {
            showPenTip(position);
        }
    } else {
        m_tabletGrab = false;
        m_tabletPanning = false;
        stopHold();
        if (m_engine->is_drawing()) {
            m_engine->pointer_move(rust::Slice<const PointerSample>(&sample, 1));
            m_engine->pointer_up();
            afterEdit();
        }
        showPenTip(position);
    }
    repaint();
    return true;
}

void InkCanvas::mousePressEvent(QMouseEvent *event)
{
    if (!isMouse(event)) {
        event->ignore();
        return;
    }
    Q_EMIT pressed();
    hidePenTip();
    if (event->button() == Qt::MiddleButton) {
        m_mousePanning = true;
        m_panButton = Qt::MiddleButton;
        m_lastPanPosition = event->position();
    } else if (event->button() == Qt::LeftButton && m_textTool) {
        const QPointF p = event->position();
        Q_EMIT textRequested(p, static_cast<qint64>(m_engine->text_at(p.x(), p.y())));
    } else if (event->button() == Qt::LeftButton && m_mathTool) {
        const QPointF p = event->position();
        Q_EMIT mathRequested(p, static_cast<qint64>(m_engine->math_at(p.x(), p.y())));
    } else if (event->button() == Qt::LeftButton) {
        forceActiveFocus();
        if (m_engine->pointer_down(2, sampleAt(event->position(), event->timestamp()), 255)) {
            trackHold(event->position());
        } else {
            m_mousePanning = true;
            m_panButton = Qt::LeftButton;
            m_lastPanPosition = event->position();
        }
    }
    event->accept();
    repaint();
}

void InkCanvas::mouseMoveEvent(QMouseEvent *event)
{
    if (m_mousePanning) {
        const QPointF delta = event->position() - m_lastPanPosition;
        m_lastPanPosition = event->position();
        m_engine->pan_by(delta.x(), delta.y());
        viewMoved();
    } else if (m_engine->is_drawing()) {
        const PointerSample sample = sampleAt(event->position(), event->timestamp());
        m_engine->pointer_move(rust::Slice<const PointerSample>(&sample, 1));
        trackHold(event->position());
        updateSelection();
        if (m_engine->has_selection_drag())
            refreshTexts();
        repaint();
    }
}

void InkCanvas::mouseReleaseEvent(QMouseEvent *event)
{
    if (m_mousePanning && event->button() == m_panButton) {
        m_mousePanning = false;
    } else if (m_engine->is_drawing()) {
        stopHold();
        m_engine->pointer_up();
        afterEdit();
    }
}

void InkCanvas::hoverMoveEvent(QHoverEvent *event)
{
    if (isMouse(event) && (!m_penSeen.isValid() || m_penSeen.elapsed() > m_palmRejection))
        hidePenTip();
}

void InkCanvas::hoverLeaveEvent(QHoverEvent *)
{
    hidePenTip();
}

void InkCanvas::touchEvent(QTouchEvent *event)
{
    event->accept();
    if (event->type() == QEvent::TouchBegin)
        Q_EMIT pressed();
    // Palm rejection: a hand resting on the screen while writing must not pan.
    if (m_penSeen.isValid() && m_penSeen.elapsed() < m_palmRejection) {
        if (m_touchDrawing) {
            m_engine->pointer_cancel();
            m_touchDrawing = false;
        }
        return;
    }
    if (event->type() == QEvent::TouchCancel) {
        if (m_touchDrawing)
            m_engine->pointer_cancel();
        m_touchDrawing = false;
        m_pinching = false;
        repaint();
        return;
    }

    QList<QEventPoint> active;
    for (const QEventPoint &point : event->points()) {
        if (point.state() != QEventPoint::Released)
            active.append(point);
    }

    if (active.size() >= 2) {
        m_tapCandidate = false;
        // A second finger turns an accidental finger stroke into navigation.
        if (m_touchDrawing) {
            m_engine->pointer_cancel();
            m_touchDrawing = false;
        }
        const QPointF a = active[0].position();
        const QPointF b = active[1].position();
        const QPointF center = (a + b) / 2;
        const qreal distance = qMax<qreal>(QLineF(a, b).length(), 1);
        if (!m_pinching) {
            m_pinching = true;
            m_pinchStartZoom = m_engine->zoom();
            m_pinchStartOffset = QPointF(m_engine->offset_x(), m_engine->offset_y());
            m_pinchStartCenter = center;
            m_pinchStartDistance = distance;
        } else {
            const qreal zoom = qBound(kMinZoom, m_pinchStartZoom * distance / m_pinchStartDistance,
                                      kMaxZoom);
            const QPointF anchor = (m_pinchStartCenter - m_pinchStartOffset) / m_pinchStartZoom;
            const QPointF offset = center - anchor * zoom;
            m_engine->set_view(zoom, offset.x(), offset.y());
            viewMoved();
        }
    } else if (active.size() == 1) {
        const QEventPoint &point = active.first();
        if (m_pinching) {
            // Back to one finger: continue as a pan from here.
            m_pinching = false;
            m_lastTouchPosition = point.position();
        } else if (point.state() == QEventPoint::Pressed) {
            m_touchDrawing = m_engine->pointer_down(3, sampleAt(point.position(), point.timestamp()), 255);
            if (m_touchDrawing)
                trackHold(point.position());
            m_lastTouchPosition = point.position();
            m_tapCandidate = !m_touchDrawing && (m_textTool || m_mathTool);
            m_tapStart = point.position();
            m_tapTime.start();
        } else if (m_touchDrawing) {
            const PointerSample sample = sampleAt(point.position(), point.timestamp());
            m_engine->pointer_move(rust::Slice<const PointerSample>(&sample, 1));
            trackHold(point.position());
        } else {
            if (QLineF(point.position(), m_tapStart).length() > kTapSlop)
                m_tapCandidate = false;
            const QPointF delta = point.position() - m_lastTouchPosition;
            m_lastTouchPosition = point.position();
            m_engine->pan_by(delta.x(), delta.y());
            viewMoved();
        }
    } else {
        if (m_touchDrawing) {
            stopHold();
            m_engine->pointer_up();
            afterEdit();
        }
        const bool tap = m_tapCandidate && !m_pinching && m_tapTime.elapsed() < kTapTime;
        m_tapCandidate = false;
        m_touchDrawing = false;
        m_pinching = false;
        if (tap) {
            const QPointF p = m_tapStart;
            if (m_textTool)
                Q_EMIT textRequested(p, static_cast<qint64>(m_engine->text_at(p.x(), p.y())));
            else if (m_mathTool)
                Q_EMIT mathRequested(p, static_cast<qint64>(m_engine->math_at(p.x(), p.y())));
        }
    }
    repaint();
}

void InkCanvas::wheelEvent(QWheelEvent *event)
{
    const QPoint pixels = event->pixelDelta();
    const QPoint degrees = event->angleDelta();
    if (event->modifiers() & Qt::ControlModifier) {
        const double notches = !degrees.isNull() ? degrees.y() / 120.0 : pixels.y() / 100.0;
        const QPointF p = event->position();
        m_engine->zoom_around(p.x(), p.y(), m_engine->zoom() * std::pow(kWheelZoomStep, notches));
    } else if (!pixels.isNull()) {
        m_engine->pan_by(pixels.x(), pixels.y());
    } else {
        m_engine->pan_by(degrees.x() / 120.0 * kWheelScrollStep,
                         degrees.y() / 120.0 * kWheelScrollStep);
    }
    event->accept();
    viewMoved();
}

void InkCanvas::setHoldDelay(int ms)
{
    ms = qBound(200, ms, 2000);
    if (ms == m_holdDelay)
        return;
    m_holdDelay = ms;
    m_holdTimer.setInterval(ms);
    Q_EMIT inputSettingsChanged();
}

void InkCanvas::setPenButtonTool(int which, int tool)
{
    tool = tool >= Pen && tool <= Hand ? tool : -1;
    if (tool == m_penButtonTools[which])
        return;
    m_penButtonTools[which] = tool;
    Q_EMIT inputSettingsChanged();
}

InkCanvas::Tool InkCanvas::penTool(const QTabletEvent *event) const
{
    int choice = -1;
    if (event->pointerType() == QPointingDevice::PointerType::Eraser)
        choice = m_penButtonTools[2];
    else if (event->buttons() & firstPenButton)
        choice = m_penButtonTools[0];
    else if (event->buttons() & secondPenButton)
        choice = m_penButtonTools[1];
    return choice < 0 ? tool() : static_cast<Tool>(choice);
}

void InkCanvas::setPalmRejection(int ms)
{
    ms = qBound(0, ms, 3000);
    if (ms == m_palmRejection)
        return;
    m_palmRejection = ms;
    Q_EMIT inputSettingsChanged();
}

void InkCanvas::setPenTipVisible(bool on)
{
    if (on == m_penTipVisible)
        return;
    m_penTipVisible = on;
    if (!on)
        hidePenTip();
    Q_EMIT inputSettingsChanged();
}

void InkCanvas::setPenTipSize(qreal radius)
{
    radius = qBound(1.0, radius, 8.0);
    if (qFuzzyCompare(radius, m_penTipSize))
        return;
    m_penTipSize = radius;
    repaint();
    Q_EMIT inputSettingsChanged();
}

void InkCanvas::showPenTip(QPointF position)
{
    if (!m_penTipVisible)
        return;
    m_tipPosition = position;
    m_tipVisible = true;
    repaint();
}

void InkCanvas::hideCursor(bool hide)
{
    // Item cursors only follow mouse events, which a pen on the page doesn't send.
    if (hide == m_cursorHidden)
        return;
    m_cursorHidden = hide;
    if (hide)
        QGuiApplication::setOverrideCursor(Qt::BlankCursor);
    else
        QGuiApplication::restoreOverrideCursor();
    if (hide && window())
        setPenCursor(window(), QCursor(Qt::BlankCursor));
}

void InkCanvas::hidePenTip()
{
    if (m_tipVisible) {
        m_tipVisible = false;
        repaint();
    }
}

void InkCanvas::geometryChange(const QRectF &newGeometry, const QRectF &oldGeometry)
{
    QQuickItem::geometryChange(newGeometry, oldGeometry);
    const qreal oldWidth = oldGeometry.width();
    const qreal newWidth = newGeometry.width();
    if (window() && oldWidth <= 0 && newWidth > 0) {
        // A page loaded while hidden (the other half of a split) is laid out for its real size.
        m_engine->set_viewport(newWidth, newGeometry.height(), window()->effectiveDevicePixelRatio());
        m_engine->reset_view();
        viewMoved();
    } else if (oldWidth > 0 && newWidth > 0 && !qFuzzyCompare(oldWidth, newWidth)
               && pageKind() != QLatin1String("endless")) {
        // Paper and PDFs stay centred when the canvas narrows or widens, e.g. beside another page.
        m_engine->pan_by((newWidth - oldWidth) / 2, 0);
        viewMoved();
    }
    repaint();
}

QSGNode *InkCanvas::updatePaintNode(QSGNode *oldNode, UpdatePaintNodeData *)
{
    return paintLayer(oldNode, 0, m_pageNodes);
}

QSGNode *InkCanvas::paintLayer(QSGNode *oldNode, int layer, LayerNodes &nodes)
{
    QQuickWindow *win = window();
    if (!win)
        return oldNode;
    QSGNode *root = oldNode;
    if (!root) {
        // Layers are transform nodes: the software renderer only passes transform and clip to later-added children of stateful nodes.
        root = new QSGNode;
        nodes.tileRoot = new QSGTransformNode;
        root->appendChildNode(nodes.tileRoot);
        if (layer == OverlayLayer) {
            nodes.tipRoot = new QSGTransformNode;
            root->appendChildNode(nodes.tipRoot);
        }
        nodes.tiles.clear();
        nodes.tip = nullptr;
    }
    m_engine->set_viewport(width(), height(), win->effectiveDevicePixelRatio());
    const rust::Vec<TileInfo> tiles = m_engine->frame(static_cast<std::uint8_t>(layer));
    const int size = static_cast<int>(m_engine->tile_size());

    // Only changed tiles are replaced, so renderers keep their batches.
    QHash<quint64, TileNode> visible;
    visible.reserve(static_cast<qsizetype>(tiles.size()));
    for (const TileInfo &tile : tiles) {
        const quint64 key = tileKey(tile);
        QSGNode *parent = nodes.tileRoot;
        TileNode entry = nodes.tiles.take(key);
        if (!entry.node || entry.generation != tile.generation) {
            const rust::Slice<const std::uint8_t> pixels =
                m_engine->tile_pixels(tile.layer, tile.x, tile.y);
            const QImage image(pixels.data(), size, size, size * 4,
                               QImage::Format_RGBA8888_Premultiplied);
            // Deep copy: the engine may redraw this buffer before the upload happens.
            QSGTexture *texture = win->createTextureFromImage(
                image.copy(), layer == 0 ? QQuickWindow::CreateTextureOptions()
                                         : QQuickWindow::TextureHasAlphaChannel);
            QSGImageNode *node = win->createImageNode();
            node->setTexture(texture);
            node->setOwnsTexture(true);
            node->setFiltering(QSGTexture::Linear);
            // Nodes own their texture, so deleting the old node frees its pixels.
            if (entry.node) {
                parent->insertChildNodeBefore(node, entry.node);
                parent->removeChildNode(entry.node);
                delete entry.node;
            } else {
                parent->appendChildNode(node);
            }
            entry.node = node;
            entry.generation = tile.generation;
        }
        const QRectF rect(tile.rect_x, tile.rect_y, tile.rect_w, tile.rect_h);
        if (entry.node->rect() != rect)
            entry.node->setRect(rect);
        visible.insert(key, entry);
    }
    for (const TileNode &stale : std::as_const(nodes.tiles)) {
        stale.node->parent()->removeChildNode(stale.node);
        delete stale.node;
    }
    nodes.tiles = std::move(visible);
    if (layer != OverlayLayer)
        return root;

    const qreal tipRadius = m_tipEraser ? m_eraserSize : m_penTipSize;
    if (nodes.tip && (!m_tipVisible || m_tipColor != m_penColor || m_tipBuiltEraser != m_tipEraser
                      || m_tipBuiltRadius != tipRadius)) {
        nodes.tipRoot->removeChildNode(nodes.tip);
        delete nodes.tip;
        nodes.tip = nullptr;
    }
    if (m_tipVisible) {
        if (!nodes.tip) {
            const qreal dpr = win->effectiveDevicePixelRatio();
            const int px = qCeil(tipRadius * 2 * dpr) + 2;
            QImage dot(px, px, QImage::Format_ARGB32_Premultiplied);
            dot.fill(Qt::transparent);
            QPainter painter(&dot);
            painter.setRenderHint(QPainter::Antialiasing);
            if (m_tipEraser) {
                // A ring as large as what the eraser removes.
                painter.setPen(QPen(QColor(255, 255, 255, 220), 3 * dpr));
                painter.drawEllipse(QRectF(1.5 * dpr, 1.5 * dpr, px - 3 * dpr, px - 3 * dpr));
                painter.setPen(QPen(QColor(60, 60, 70, 230), 1.2 * dpr));
                painter.drawEllipse(QRectF(1.5 * dpr, 1.5 * dpr, px - 3 * dpr, px - 3 * dpr));
            } else {
                painter.setPen(QPen(QColor(255, 255, 255, 200), 1));
                painter.setBrush(m_penColor);
                painter.drawEllipse(QRectF(1, 1, px - 2, px - 2));
            }
            painter.end();
            nodes.tip = win->createImageNode();
            nodes.tip->setTexture(
                win->createTextureFromImage(dot, QQuickWindow::TextureHasAlphaChannel));
            nodes.tip->setOwnsTexture(true);
            m_tipColor = m_penColor;
            m_tipBuiltEraser = m_tipEraser;
            m_tipBuiltRadius = tipRadius;
            nodes.tipRoot->appendChildNode(nodes.tip);
        }
        const qreal r = tipRadius + 1;
        nodes.tip->setRect(QRectF(m_tipPosition.x() - r, m_tipPosition.y() - r, 2 * r, 2 * r));
    }
    return root;
}

void InkCanvas::releaseResources()
{
    // The scene graph deletes the node tree.
    m_pageNodes = {};
}


InkLayer::InkLayer(QQuickItem *parent)
    : QQuickItem(parent)
{
    setFlag(ItemHasContents, true);
}

void InkLayer::setCanvas(InkCanvas *canvas)
{
    if (m_canvas == canvas)
        return;
    disconnect(m_repaint);
    m_canvas = canvas;
    if (m_canvas)
        m_repaint = connect(m_canvas, &InkCanvas::repainted, this, &QQuickItem::update);
    m_rebuild = true;
    Q_EMIT canvasChanged();
    update();
}

void InkLayer::setSheet(int layer)
{
    if (m_layer == layer)
        return;
    m_layer = layer;
    m_rebuild = true;
    Q_EMIT sheetChanged();
    update();
}

QSGNode *InkLayer::updatePaintNode(QSGNode *oldNode, UpdatePaintNodeData *)
{
    // A node tree built for another canvas or layer starts over.
    if (m_rebuild || !m_canvas) {
        m_rebuild = false;
        delete oldNode;
        oldNode = nullptr;
        m_nodes = {};
    }
    if (!m_canvas)
        return nullptr;
    return m_canvas->paintLayer(oldNode, m_layer, m_nodes);
}

void InkLayer::releaseResources()
{
    m_nodes = {};
}

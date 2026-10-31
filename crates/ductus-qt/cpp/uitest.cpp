#include "ductus-qt/cpp/uitest.h"
#include "ductus-qt/cpp/inkcanvas.h"
#include "ductus-qt/src/ffi.cxxqt.h"

#include <QClipboard>
#include <QCoreApplication>
#include <QCursor>
#include <QGuiApplication>
#include <QImage>
#include <QDir>
#include <QFile>
#include <QRegularExpression>
#include <QFileInfo>
#include <QKeyEvent>
#include <QKeySequence>
#include <QPointingDevice>
#include <QQmlEngine>
#include <QQuickItem>
#include <QQuickWindow>
#include <QTest>
// The GUI helpers need QT_GUI_LIB, which this build doesn't define.
#include <QtTest/qtest_gui.h>
#include <QTimer>
#include <private/qhighdpiscaling_p.h>
#include <qpa/qwindowsysteminterface.h>

#include <cstdio>

namespace {

// Never freed: Qt keeps the registered device for the application's lifetime.
const QPointingDevice *stylus(QPointingDevice::PointerType type)
{
    auto make = [](qint64 id, QPointingDevice::PointerType pointer) {
        using Cap = QInputDevice::Capability;
        auto *device = new QPointingDevice(
            QStringLiteral("Test Stylus"), id, QInputDevice::DeviceType::Stylus, pointer,
            Cap::Position | Cap::Pressure | Cap::XTilt | Cap::YTilt | Cap::Hover, 1, 3,
            QString(), QPointingDeviceUniqueId::fromNumericId(id));
        QWindowSystemInterface::registerInputDevice(device);
        return device;
    };
    static const QPointingDevice *pen = make(4201, QPointingDevice::PointerType::Pen);
    static const QPointingDevice *eraser = make(4202, QPointingDevice::PointerType::Eraser);
    return type == QPointingDevice::PointerType::Eraser ? eraser : pen;
}

// Laptops report their mouse this way.
const QPointingDevice *touchpad()
{
    static const QPointingDevice *device = [] {
        auto *d = new QPointingDevice(QStringLiteral("Test Touchpad"), 4203,
                                      QInputDevice::DeviceType::TouchPad,
                                      QPointingDevice::PointerType::Generic,
                                      QInputDevice::Capability::Position, 1, 3);
        QWindowSystemInterface::registerInputDevice(d);
        return d;
    }();
    return device;
}

QPointingDevice *touchscreen()
{
    static QPointingDevice *device = QTest::createTouchDevice();
    return device;
}

// The one button a pen sends of those the canvas accepts for a side button.
Qt::MouseButton lowestButton(Qt::MouseButtons buttons)
{
    const int bits = buttons.toInt();
    return Qt::MouseButton(bits & -bits);
}

QPointF parsePoint(const QString &token)
{
    const QStringList xy = token.split(QLatin1Char(','));
    return xy.size() == 2 ? QPointF(xy[0].toDouble(), xy[1].toDouble()) : QPointF();
}

QList<QPointF> densify(const QList<QPointF> &corners, qreal step = 4)
{
    QList<QPointF> out;
    for (qsizetype i = 0; i + 1 < corners.size(); ++i) {
        const QPointF a = corners[i];
        const QPointF b = corners[i + 1];
        const int n = qMax(1, qCeil(QLineF(a, b).length() / step));
        for (int k = 0; k < n; ++k)
            out.append(a + (b - a) * (qreal(k) / n));
    }
    if (!corners.isEmpty())
        out.append(corners.last());
    return out;
}

class Runner
{
public:
    Runner(QQuickWindow *window, QString shotDir)
        : m_window(window)
        , m_shotDir(std::move(shotDir))
    {
    }

    bool run(const QString &line)
    {
        QStringList args = line.split(QLatin1Char(' '), Qt::SkipEmptyParts);
        if (args.isEmpty() || args.first().startsWith(QLatin1Char('#')))
            return true;
        const QString cmd = args.takeFirst();
        const QString rest = line.section(QLatin1Char(' '), 1).trimmed();
        std::fprintf(stderr, "[uitest] %s\n", qPrintable(line));

        if (cmd == u"wait") {
            QTest::qWait(args.value(0).toInt());
        } else if (cmd == u"size") {
            m_window->resize(args.value(0).toInt(), args.value(1).toInt());
            QTest::qWait(200);
        } else if (cmd == u"pen" || cmd == u"eraser") {
            stroke(cmd == u"eraser" ? QPointingDevice::PointerType::Eraser
                                    : QPointingDevice::PointerType::Pen,
                   args);
                } else if (cmd == u"click") {
            QTest::mouseClick(m_window, Qt::LeftButton, {}, parsePoint(args.value(0)).toPoint());
        } else if (cmd == u"drag-item" || cmd == u"touchpad-drag-item") {
            // `drag-item A | B`: the mouse from item A's middle to item B's.
            QQuickItem *from = findItem(m_window->contentItem(), rest.section(QLatin1Char('|'), 0, 0).trimmed());
            QQuickItem *to = findItem(m_window->contentItem(), rest.section(QLatin1Char('|'), 1).trimmed());
            if (!from || !to)
                return fail(QStringLiteral("drag-item: no such items in ") + rest);
            const QPoint a = from->mapToScene(QPointF(from->width() / 2, from->height() / 2)).toPoint();
            const QPoint b = to->mapToScene(QPointF(to->width() / 2, to->height() / 2)).toPoint();
            if (cmd == u"touchpad-drag-item")
                touchpadDrag(a, b);
            else
                mouseDrag(a, b);
        } else if (cmd == u"mouse") {
            // `mouse x,y x,y`: a left-button drag between two window points.
            mouseDrag(parsePoint(args.value(0)).toPoint(), parsePoint(args.value(1)).toPoint());
        } else if (cmd == u"click-item" || cmd == u"right-click-item") {
            const Qt::MouseButton button = cmd == u"click-item" ? Qt::LeftButton : Qt::RightButton;
            if (!clickItem(rest, button))
                return fail(QStringLiteral("no visible item named ") + rest);
        } else if (cmd == u"pen-tap") {
            QQuickItem *item = findItem(m_window->contentItem(), rest);
            if (!item)
                return fail(QStringLiteral("no visible item named ") + rest);
            const QPointF at = item->mapToScene(QPointF(item->width() / 2, item->height() / 2));
            const auto *pen = stylus(QPointingDevice::PointerType::Pen);
            tablet(QEvent::TabletMove, pen, at, 0, Qt::NoButton);
            QTest::qWait(30);
            tablet(QEvent::TabletPress, pen, at, 0.5, Qt::LeftButton);
            QTest::qWait(30);
            tablet(QEvent::TabletRelease, pen, at, 0, Qt::LeftButton);
            QTest::qWait(80);
        } else if (cmd == u"type") {
            // A field that just appeared may get focus a moment later.
            for (int i = 0; i < 40; ++i) {
                const QQuickItem *focus = m_window->activeFocusItem();
                if (focus && (focus->flags() & QQuickItem::ItemAcceptsInputMethod))
                    break;
                QTest::qWait(50);
            }
            // QTest::keyClicks only exists for widgets.
            for (const QChar c : rest) {
                const int key = c == QLatin1Char(' ') ? Qt::Key_Space : 0;
                QKeyEvent press(QEvent::KeyPress, key, Qt::NoModifier, QString(c));
                QKeyEvent release(QEvent::KeyRelease, key, Qt::NoModifier, QString(c));
                QCoreApplication::sendEvent(m_window, &press);
                QCoreApplication::sendEvent(m_window, &release);
            }
            QTest::qWait(50);
        } else if (cmd == u"expect-file" || cmd == u"expect-no-file") {
            const QString path = qEnvironmentVariable("DUCTUS_LIBRARY") + QLatin1Char('/') + rest;
            const bool want = cmd == u"expect-file";
            // Autosave runs shortly after edits.
            for (int i = 0; i < 100 && QFileInfo::exists(path) != want; ++i)
                QTest::qWait(100);
            if (QFileInfo::exists(path) != want)
                return fail((want ? QStringLiteral("missing file ") : QStringLiteral("unexpected file ")) + path);
        } else if (cmd == u"expect-in-file" || cmd == u"expect-not-in-file") {
            const QString path = qEnvironmentVariable("DUCTUS_LIBRARY") + QLatin1Char('/')
                + rest.section(QLatin1Char('|'), 0, 0).trimmed();
            const QString text = rest.section(QLatin1Char('|'), 1).trimmed();
            const bool want = cmd == u"expect-in-file";
            auto contains = [&] {
                QFile file(path);
                return file.open(QIODevice::ReadOnly) && QString::fromUtf8(file.readAll()).contains(text);
            };
            for (int i = 0; i < 100 && contains() != want; ++i)
                QTest::qWait(100);
            if (contains() != want)
                return fail(path + (want ? QStringLiteral(" doesn't contain ") : QStringLiteral(" contains ")) + text);
        } else if (cmd == u"expect-pdf-text") {
            const QString path = qEnvironmentVariable("DUCTUS_LIBRARY") + QLatin1Char('/')
                + rest.section(QLatin1Char('|'), 0, 0).trimmed();
            const QString text = rest.section(QLatin1Char('|'), 1).trimmed();
            const rust::String found = pdf_text(path.toStdString());
            const QString all = QString::fromUtf8(found.data(), static_cast<qsizetype>(found.size()));
            if (!all.contains(text))
                return fail(path + QStringLiteral(" has no text ") + text + QStringLiteral(" (found: ") + all.left(200) + QLatin1Char(')'));
        } else if (cmd == u"export-folder") {
            // `export-folder FOLDER | FILE`, past the file dialog like export-pdf.
            const QString root = qEnvironmentVariable("DUCTUS_LIBRARY") + QLatin1Char('/');
            bool ok = false;
            QMetaObject::invokeMethod(controller(), "exportFolder", Q_RETURN_ARG(bool, ok),
                                      Q_ARG(QString, rest.section(QLatin1Char('|'), 0, 0).trimmed()),
                                      Q_ARG(QString, root + rest.section(QLatin1Char('|'), 1).trimmed()));
            if (!ok)
                return fail(QStringLiteral("export-folder failed"));
        } else if (cmd == u"expect-pdf-pages") {
            // Counts page objects: enough for files Ductus wrote itself.
            const QString path = qEnvironmentVariable("DUCTUS_LIBRARY") + QLatin1Char('/')
                + rest.section(QLatin1Char('|'), 0, 0).trimmed();
            const int expected = rest.section(QLatin1Char('|'), 1).trimmed().toInt();
            int pages = -1;
            for (int tries = 0; tries < 50 && pages != expected; ++tries) {
                if (tries > 0)
                    QTest::qWait(200);
                QFile file(path);
                if (!file.open(QIODevice::ReadOnly))
                    continue;
                static const QRegularExpression page(QStringLiteral("/Type\\s*/Page(?![s\\w])"));
                pages = int(QString::fromLatin1(file.readAll()).count(page));
            }
            if (pages != expected)
                return fail(QStringLiteral("%1 has %2 pages, expected %3").arg(path).arg(pages).arg(expected));
        } else if (cmd == u"copy-file") {
            // Replaces DST the way sync tools do: write elsewhere, then rename over it.
            const QString root = qEnvironmentVariable("DUCTUS_LIBRARY") + QLatin1Char('/');
            const QString src = root + rest.section(QLatin1Char('|'), 0, 0).trimmed();
            const QString dst = root + rest.section(QLatin1Char('|'), 1).trimmed();
            const QString tmp = dst + QStringLiteral(".synctmp");
            QFile::remove(tmp);
            if (!QFile::copy(src, tmp) || !QFile::rename(tmp, dst + QStringLiteral(".x"))
                || (QFile::exists(dst) && !QFile::remove(dst))
                || !QFile::rename(dst + QStringLiteral(".x"), dst))
                return fail(QStringLiteral("could not copy ") + src + QStringLiteral(" to ") + dst);
            QTest::qWait(100);
        } else if (cmd == u"clipboard-image") {
            const QImage image(rest);
            if (image.isNull())
                return fail(QStringLiteral("cannot read ") + rest);
            QGuiApplication::clipboard()->setImage(image);
            QTest::qWait(100);
        } else if (cmd == u"setenv") {
            qputenv(args.value(0).toUtf8(), rest.section(QLatin1Char(' '), 1).toUtf8());
        } else if (cmd == u"put-file") {
            const QString src = rest.section(QLatin1Char('|'), 0, 0).trimmed();
            const QString dst = qEnvironmentVariable("DUCTUS_LIBRARY") + QLatin1Char('/')
                + rest.section(QLatin1Char('|'), 1).trimmed();
            QDir().mkpath(QFileInfo(dst).absolutePath());
            if (!QFile::copy(src, dst))
                return fail(QStringLiteral("could not copy ") + src + QStringLiteral(" to ") + dst);
            QTest::qWait(100);
        } else if (cmd == u"import-pdf" || cmd == u"export-pdf") {
            // File dialogs can't be scripted, so these call the controller directly.
            QString result;
            bool ok = false;
            if (cmd == u"import-pdf" && args.value(0) == u"printout") {
                QQuickItem *canvas = findItem(m_window->contentItem(), QStringLiteral("canvas"));
                const QString file = QFileInfo(rest.section(QLatin1Char(' '), 1)).absoluteFilePath();
                if (canvas)
                    QMetaObject::invokeMethod(canvas, "insertPdfFile", Q_RETURN_ARG(bool, ok), Q_ARG(QString, file));
            } else if (cmd == u"import-pdf") {
                const bool annotate = args.value(0) == u"annotate";
                const QString file = QFileInfo(rest.section(QLatin1Char(' '), 1)).absoluteFilePath();
                QString folder;
                QMetaObject::invokeMethod(controller(), "defaultPageFolder", Q_RETURN_ARG(QString, folder));
                QMetaObject::invokeMethod(controller(), "importPdf", Q_RETURN_ARG(QString, result),
                                          Q_ARG(QString, file), Q_ARG(bool, annotate), Q_ARG(QString, folder));
                ok = !result.isEmpty();
            } else {
                const QString file = qEnvironmentVariable("DUCTUS_LIBRARY") + QLatin1Char('/') + rest;
                QMetaObject::invokeMethod(controller(), "exportPdf", Q_RETURN_ARG(bool, ok),
                                          Q_ARG(QString, file));
            }
            if (!ok)
                return fail(cmd + QStringLiteral(" failed"));
            QTest::qWait(300);
        } else if (cmd == u"import-notes") {
            // Read on a worker thread: waits until it asks where to save.
            const QString file = QFileInfo(rest).absoluteFilePath();
            QMetaObject::invokeMethod(controller(), "importNotes", Q_ARG(QString, file));
            for (int i = 0; i < 300 && controller()->property("importing").toBool(); ++i)
                QTest::qWait(100);
            if (controller()->property("importing").toBool())
                return fail(QStringLiteral("import-notes timed out"));
            QTest::qWait(300);
        } else if (cmd == u"print-pdf") {
            bool ok = false;
            const QString file = qEnvironmentVariable("DUCTUS_LIBRARY") + QLatin1Char('/') + rest;
            QMetaObject::invokeMethod(controller(), "printToFile", Q_RETURN_ARG(bool, ok), Q_ARG(QString, file));
            if (!ok)
                return fail(QStringLiteral("printing failed"));
        } else if (cmd == u"expect-cursor") {
            const QCursor *cursor = QGuiApplication::overrideCursor();
            const QString actual = cursor && cursor->shape() == Qt::BlankCursor ? QStringLiteral("hidden")
                                                                                : QStringLiteral("shown");
            if (actual != rest)
                return fail(QStringLiteral("cursor is ") + actual);
        } else if (cmd == u"expect-prop") {
            // Waits a while for it: a list adds a new row's item with its next frame, and work such
            // as a screenshot or a PDF printout finishes later.
            const QString expected = rest.section(QLatin1Char(' '), 2);
            QString actual;
            for (int i = 0; i < 150; ++i) {
                QQuickItem *item = findItem(m_window->contentItem(), args.value(0));
                actual = item ? item->property(args.value(1).toUtf8().constData()).toString()
                              : QStringLiteral("(no item)");
                if (actual == expected)
                    break;
                QTest::qWait(20);
            }
            if (actual != expected)
                return fail(QStringLiteral("%1.%2 is %3, expected %4")
                                .arg(args.value(0), args.value(1), actual, expected));
        } else if (cmd == u"shot") {
            // `shot name`: a screenshot for looking at, in the shots folder.
            QTest::qWait(50);
            QDir().mkpath(m_shotDir);
            m_window->grabWindow().save(m_shotDir + QLatin1Char('/') + rest + QStringLiteral(".png"));
        } else if (cmd == u"expect-hidden") {
            if (findItem(m_window->contentItem(), rest))
                return fail(rest + QStringLiteral(" is visible"));
        } else if (cmd == u"expect-text") {
            const QString name = args.value(0);
            const QString text = rest.section(QLatin1Char(' '), 1);
            // Lists create their rows a frame or two later.
            QString actual;
            for (int i = 0; i < 20; ++i) {
                QQuickItem *item = findItem(m_window->contentItem(), name);
                actual = item ? item->property("text").toString() : QString();
                if (item && actual.contains(text))
                    break;
                QTest::qWait(100);
            }
            if (!actual.contains(text))
                return fail(QStringLiteral("%1 shows \"%2\", expected \"%3\"").arg(name, actual, text));
        } else if (cmd == u"touch") {
            touchDrag(args);
        } else if (cmd == u"pinch") {
            pinch(parsePoint(args.value(0)), args.value(1).toDouble(), args.value(2).toDouble());
        } else if (cmd == u"key") {
            QTest::keySequence(m_window, QKeySequence::fromString(args.value(0)));
        } else {
            return fail(QStringLiteral("unknown command ") + cmd);
        }
        return true;
    }

private:
    bool fail(const QString &message)
    {
        std::fprintf(stderr, "[uitest] ERROR: %s\n", qPrintable(message));
        QDir().mkpath(m_shotDir);
        m_window->grabWindow().save(m_shotDir + QStringLiteral("/failure.png"));
        return false;
    }

    QObject *controller()
    {
        return qmlEngine(m_window)->singletonInstance<QObject *>(QStringLiteral("Ductus"),
                                                                 QStringLiteral("AppController"));
    }

    void mouseDrag(QPoint a, QPoint b)
    {
        QTest::mouseMove(m_window, a);
        QTest::mousePress(m_window, Qt::LeftButton, {}, a);
        for (int i = 1; i <= 20; ++i)
            QTest::mouseMove(m_window, a + (b - a) * i / 20, 15);
        QTest::mouseRelease(m_window, Qt::LeftButton, {}, b);
        QTest::qWait(100);
    }

    void touchpadDrag(QPoint a, QPoint b)
    {
        auto send = [this](QPointF pos, Qt::MouseButtons buttons, Qt::MouseButton button, QEvent::Type type) {
            QWindowSystemInterface::handleMouseEvent(m_window, m_time += 15, touchpad(),
                                                     QHighDpi::toNativeLocalPosition(pos, m_window),
                                                     QHighDpi::toNativeGlobalPosition(m_window->mapToGlobal(pos), m_window),
                                                     buttons, button, type);
            QWindowSystemInterface::flushWindowSystemEvents();
        };
        send(a, Qt::LeftButton, Qt::LeftButton, QEvent::MouseButtonPress);
        for (int i = 1; i <= 20; ++i) {
            send(a + QPointF(b - a) * i / 20, Qt::LeftButton, Qt::NoButton, QEvent::MouseMove);
            QTest::qWait(15);
        }
        send(b, Qt::NoButton, Qt::LeftButton, QEvent::MouseButtonRelease);
        QTest::qWait(100);
    }

    // Through the platform input path, as a real tablet's events arrive.
    void tablet(QEvent::Type type, const QPointingDevice *device, QPointF pos, qreal pressure,
                Qt::MouseButtons held)
    {
        const Qt::MouseButtons buttons = type == QEvent::TabletRelease ? Qt::NoButton : held;
        // Platforms report native pixels; Qt scales them, as on a 150% Windows screen.
        QWindowSystemInterface::handleTabletEvent(m_window, m_time += 5, device,
                                                  QHighDpi::toNativeLocalPosition(pos, m_window),
                                                  QHighDpi::toNativeGlobalPosition(m_window->mapToGlobal(pos), m_window),
                                                  buttons, pressure,
                                                  0, 0, 0, 0, 0, Qt::NoModifier);
        QWindowSystemInterface::flushWindowSystemEvents();
    }

    // `pen [p=0.2-1.0] [button=1|2] x,y x,y ...`: a stroke with a pressure ramp, maybe with the first
    // or second side button held, sent as this platform's pen reports it.
    void stroke(QPointingDevice::PointerType type, const QStringList &args)
    {
        qreal p0 = 0.8, p1 = 0.8;
        Qt::MouseButtons held = Qt::LeftButton;
        QList<QPointF> corners;
        for (const QString &arg : args) {
            if (arg == u"button=1") {
                held |= lowestButton(firstPenButton);
            } else if (arg == u"button=2") {
                held |= lowestButton(secondPenButton);
            } else if (arg.startsWith(u"p=")) {
                const QStringList range = arg.mid(2).split(QLatin1Char('-'));
                p0 = range.value(0).toDouble();
                p1 = range.size() > 1 ? range.value(1).toDouble() : p0;
            } else {
                corners.append(parsePoint(arg));
            }
        }
        const QList<QPointF> points = densify(corners);
        if (points.isEmpty())
            return;
        const auto *device = stylus(type);
        tablet(QEvent::TabletPress, device, points.first(), p0, held);
        for (qsizetype i = 1; i < points.size(); ++i) {
            const qreal f = qreal(i) / qMax<qsizetype>(1, points.size() - 1);
            tablet(QEvent::TabletMove, device, points[i], p0 + (p1 - p0) * f, held);
            if (i % 4 == 0)
                QTest::qWait(8);
        }
        tablet(QEvent::TabletRelease, device, points.last(), 0, held);
        QTest::qWait(30);
    }

    void touchDrag(const QStringList &args)
    {
        QList<QPointF> corners;
        for (const QString &arg : args)
            corners.append(parsePoint(arg));
        const QList<QPointF> points = densify(corners, 8);
        if (points.isEmpty())
            return;
        QTest::touchEvent(m_window, touchscreen()).press(0, points.first().toPoint());
        for (const QPointF &p : points.mid(1)) {
            QTest::touchEvent(m_window, touchscreen()).move(0, p.toPoint());
            QTest::qWait(4);
        }
        QTest::touchEvent(m_window, touchscreen()).release(0, points.last().toPoint());
        QTest::qWait(30);
    }

    void pinch(QPointF center, qreal d0, qreal d1)
    {
        auto at = [center](qreal d, int side) { return (center + QPointF(side * d / 2, 0)).toPoint(); };
        QTest::touchEvent(m_window, touchscreen()).press(0, at(d0, -1)).press(1, at(d0, 1));
        for (int i = 1; i <= 12; ++i) {
            const qreal d = d0 + (d1 - d0) * i / 12;
            QTest::touchEvent(m_window, touchscreen()).move(0, at(d, -1)).move(1, at(d, 1));
            QTest::qWait(8);
        }
        QTest::touchEvent(m_window, touchscreen()).release(0, at(d1, -1)).release(1, at(d1, 1));
        QTest::qWait(30);
    }

    // Not findChild: Repeater delegates and popup contents have no QObject parent.
    static QQuickItem *findItem(QQuickItem *root, const QString &name)
    {
        if (root->objectName() == name && root->isVisible())
            return root;
        for (QQuickItem *child : root->childItems())
            if (QQuickItem *found = findItem(child, name))
                return found;
        return nullptr;
    }

    // An item scrolled out of sight (far down a settings section, say) comes into view first.
    static void reveal(QQuickItem *item)
    {
        for (QQuickItem *view = item->parentItem(); view; view = view->parentItem()) {
            if (!view->inherits("QQuickFlickable"))
                continue;
            auto *content = view->property("contentItem").value<QQuickItem *>();
            if (!content)
                continue;
            const QRectF r = item->mapRectToItem(content, QRectF(0, 0, item->width(), item->height()));
            const qreal y = view->property("contentY").toReal();
            if (r.top() >= y && r.bottom() <= y + view->height())
                continue;
            const qreal end = qMax(0.0, view->property("contentHeight").toReal() - view->height());
            view->setProperty("contentY", qBound(0.0, r.center().y() - view->height() / 2, end));
        }
    }

    bool clickItem(const QString &name, Qt::MouseButton button)
    {
        QQuickItem *item = findItem(m_window->contentItem(), name);
        if (!item)
            return false;
        reveal(item);
        const QPointF center = item->mapToScene(QPointF(item->width() / 2, item->height() / 2));
        QTest::mouseClick(m_window, button, {}, center.toPoint());
        QTest::qWait(60);
        return true;
    }

    QQuickWindow *m_window;
    QString m_shotDir;
    quint64 m_time = 1000;
};

}

void startUiTest(QQuickWindow *window, const QString &scriptPath, const QString &shotDir)
{
    QTimer::singleShot(800, window, [window, scriptPath, shotDir] {
        QFile file(scriptPath);
        int status = 0;
        if (!file.open(QIODevice::ReadOnly | QIODevice::Text)) {
            std::fprintf(stderr, "[uitest] ERROR: cannot open %s\n", qPrintable(scriptPath));
            status = 2;
        } else {
            Runner runner(window, shotDir);
            const QStringList lines = QString::fromUtf8(file.readAll()).split(QLatin1Char('\n'));
            for (const QString &line : lines) {
                if (!runner.run(line.trimmed())) {
                    status = 1;
                    break;
                }
            }
        }
        std::fprintf(stderr, "[uitest] finished with status %d\n", status);
        QCoreApplication::exit(status);
    });
}

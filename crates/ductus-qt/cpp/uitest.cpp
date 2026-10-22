#include "ductus-qt/cpp/uitest.h"
#include "ductus-qt/src/ffi.cxxqt.h"

#include <QClipboard>
#include <QCoreApplication>
#include <QCursor>
#include <QGuiApplication>
#include <QImage>
#include <QDir>
#include <QFile>
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

QPointingDevice *touchscreen()
{
    static QPointingDevice *device = QTest::createTouchDevice();
    return device;
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
            for (int i = 0; i < 40 && QFileInfo::exists(path) != want; ++i)
                QTest::qWait(100);
            if (QFileInfo::exists(path) != want)
                return fail((want ? QStringLiteral("missing file ") : QStringLiteral("unexpected file ")) + path);
        } else if (cmd == u"expect-in-file") {
            const QString path = qEnvironmentVariable("DUCTUS_LIBRARY") + QLatin1Char('/')
                + rest.section(QLatin1Char('|'), 0, 0).trimmed();
            const QString text = rest.section(QLatin1Char('|'), 1).trimmed();
            auto contains = [&] {
                QFile file(path);
                return file.open(QIODevice::ReadOnly) && QString::fromUtf8(file.readAll()).contains(text);
            };
            for (int i = 0; i < 40 && !contains(); ++i)
                QTest::qWait(100);
            if (!contains())
                return fail(path + QStringLiteral(" doesn't contain ") + text);
        } else if (cmd == u"expect-pdf-text") {
            const QString path = qEnvironmentVariable("DUCTUS_LIBRARY") + QLatin1Char('/')
                + rest.section(QLatin1Char('|'), 0, 0).trimmed();
            const QString text = rest.section(QLatin1Char('|'), 1).trimmed();
            const rust::String found = pdf_text(path.toStdString());
            const QString all = QString::fromUtf8(found.data(), static_cast<qsizetype>(found.size()));
            if (!all.contains(text))
                return fail(path + QStringLiteral(" has no text ") + text + QStringLiteral(" (found: ") + all.left(200) + QLatin1Char(')'));
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
            if (cmd == u"import-pdf") {
                const bool annotate = args.value(0) == u"annotate";
                const QString file = QFileInfo(rest.section(QLatin1Char(' '), 1)).absoluteFilePath();
                QMetaObject::invokeMethod(controller(), "importPdf", Q_RETURN_ARG(QString, result),
                                          Q_ARG(QString, file), Q_ARG(bool, annotate));
                ok = !result.isEmpty();
            } else {
                const QString file = qEnvironmentVariable("DUCTUS_LIBRARY") + QLatin1Char('/') + rest;
                QMetaObject::invokeMethod(controller(), "exportPdf", Q_RETURN_ARG(bool, ok),
                                          Q_ARG(QString, file));
            }
            if (!ok)
                return fail(cmd + QStringLiteral(" failed"));
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
            QQuickItem *item = findItem(m_window->contentItem(), args.value(0));
            const QString actual = item ? item->property(args.value(1).toUtf8().constData()).toString()
                                        : QStringLiteral("(no item)");
            const QString expected = rest.section(QLatin1Char(' '), 2);
            if (actual != expected)
                return fail(QStringLiteral("%1.%2 is %3, expected %4")
                                .arg(args.value(0), args.value(1), actual, expected));
        } else if (cmd == u"expect-text") {
            const QString name = args.value(0);
            const QString text = rest.section(QLatin1Char(' '), 1);
            QQuickItem *item = findItem(m_window->contentItem(), name);
            const QString actual = item ? item->property("text").toString() : QString();
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

    // Through the platform input path, as a real tablet's events arrive.
    void tablet(QEvent::Type type, const QPointingDevice *device, QPointF pos, qreal pressure,
                Qt::MouseButton button)
    {
        const Qt::MouseButtons buttons =
            type == QEvent::TabletRelease ? Qt::NoButton : Qt::MouseButtons(button);
        QWindowSystemInterface::handleTabletEvent(m_window, m_time += 5, device, pos,
                                                  m_window->mapToGlobal(pos), buttons, pressure,
                                                  0, 0, 0, 0, 0, Qt::NoModifier);
        QWindowSystemInterface::flushWindowSystemEvents();
    }

    // `pen [p=0.2-1.0] x,y x,y ...`: a stroke with a pressure ramp.
    void stroke(QPointingDevice::PointerType type, const QStringList &args)
    {
        qreal p0 = 0.8, p1 = 0.8;
        QList<QPointF> corners;
        for (const QString &arg : args) {
            if (arg.startsWith(u"p=")) {
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
        tablet(QEvent::TabletPress, device, points.first(), p0, Qt::LeftButton);
        for (qsizetype i = 1; i < points.size(); ++i) {
            const qreal f = qreal(i) / qMax<qsizetype>(1, points.size() - 1);
            tablet(QEvent::TabletMove, device, points[i], p0 + (p1 - p0) * f, Qt::LeftButton);
            if (i % 4 == 0)
                QTest::qWait(8);
        }
        tablet(QEvent::TabletRelease, device, points.last(), 0, Qt::LeftButton);
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

    bool clickItem(const QString &name, Qt::MouseButton button)
    {
        QQuickItem *item = findItem(m_window->contentItem(), name);
        if (!item)
            return false;
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

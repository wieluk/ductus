// Include guards: the build reaches headers through two include paths.
#ifndef DUCTUS_MODELS_H
#define DUCTUS_MODELS_H

#include <QObject>
#include <QTimer>
#include <QVariantList>
#include <QtQml/qqmlregistration.h>
#include <optional>

#include "rust/cxx.h"

struct ModelsHandle;

/// The built-in recognition models: downloaded on request, never bundled.
class Models : public QObject
{
    Q_OBJECT
    QML_ELEMENT
    QML_SINGLETON
    /// [{ id, title, size, installed }]
    Q_PROPERTY(QVariantList entries READ entries NOTIFY changed)
    /// The model being downloaded, or "".
    Q_PROPERTY(QString busy READ busy NOTIFY changed)
    Q_PROPERTY(double progress READ progress NOTIFY changed)
    Q_PROPERTY(QString error READ error NOTIFY changed)
    /// [{ code, name, model }]: what handwriting can be read in, "auto" first.
    Q_PROPERTY(QVariantList languages READ languages CONSTANT)

public:
    explicit Models(QObject *parent = nullptr);
    ~Models() override;

    static QString dir();
    static bool installed(const QString &id);
    /// The model that reads handwriting in the `hwr/lang` language `lang`.
    Q_INVOKABLE static QString textModel(const QString &lang);

    QVariantList entries() const;
    QString busy() const { return m_busy; }
    double progress() const { return m_progress; }
    QString error() const { return m_error; }
    QVariantList languages() const;

    Q_INVOKABLE void download(const QString &id);
    Q_INVOKABLE void remove(const QString &id);
    Q_INVOKABLE void cancel();

Q_SIGNALS:
    void changed();

private:
    void poll();

    std::optional<rust::Box<ModelsHandle>> m_handle;
    QTimer m_poll;
    QString m_busy;
    double m_progress = 0;
    QString m_error;
};

#endif

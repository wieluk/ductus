#include "ductus-qt/cpp/models.h"
#include "ductus-qt/src/ffi.cxxqt.h"

#include <QStandardPaths>
#include <QVariantMap>

namespace {
QString toQ(const rust::String &s)
{
    return QString::fromUtf8(s.data(), static_cast<qsizetype>(s.size()));
}
} // namespace

Models::Models(QObject *parent)
    : QObject(parent)
{
    m_handle.emplace(open_models());
    m_poll.setInterval(300);
    connect(&m_poll, &QTimer::timeout, this, &Models::poll);
}

Models::~Models() = default;

QString Models::dir()
{
    const QString custom = qEnvironmentVariable("DUCTUS_MODELS_DIR");
    if (!custom.isEmpty())
        return custom;
    return QStandardPaths::writableLocation(QStandardPaths::AppDataLocation) + QStringLiteral("/models");
}

bool Models::installed(const QString &id)
{
    for (const ModelRow &row : model_rows(dir().toStdString())) {
        if (toQ(row.id) == id)
            return row.installed;
    }
    return false;
}

QVariantList Models::entries() const
{
    QVariantList out;
    for (const ModelRow &row : model_rows(dir().toStdString())) {
        out.append(QVariantMap{{QStringLiteral("id"), toQ(row.id)},
                               {QStringLiteral("title"), toQ(row.title)},
                               {QStringLiteral("size"), static_cast<double>(row.size)},
                               {QStringLiteral("installed"), row.installed}});
    }
    return out;
}

void Models::download(const QString &id)
{
    if (!m_busy.isEmpty())
        return;
    (*m_handle)->install(dir().toStdString(), id.toStdString());
    m_busy = id;
    m_progress = 0;
    m_error.clear();
    m_poll.start();
    Q_EMIT changed();
}

void Models::remove(const QString &id)
{
    m_error = toQ(remove_model(dir().toStdString(), id.toStdString()));
    Q_EMIT changed();
}

void Models::cancel()
{
    (*m_handle)->cancel();
}

void Models::poll()
{
    const ModelsState state = (*m_handle)->state();
    m_progress = state.total > 0 ? static_cast<double>(state.received) / static_cast<double>(state.total) : 0;
    if (state.busy.empty()) {
        m_poll.stop();
        m_busy.clear();
        m_error = toQ(state.error);
    }
    Q_EMIT changed();
}

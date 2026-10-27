#include "ductus-qt/cpp/models.h"
#include "ductus-qt/cpp/appcontroller.h"
#include "ductus-qt/src/ffi.cxxqt.h"

#include <QCoreApplication>
#include <QLocale>
#include <QSettings>
#include <QVariantMap>

namespace {
QString toQ(const rust::String &s)
{
    return QString::fromUtf8(s.data(), static_cast<qsizetype>(s.size()));
}
// For lupdate: the names Rust gives (crates/ductus-hwr/src/builtin.rs), translated below.
[[maybe_unused]] const char *const languageNames[] = {
    QT_TRANSLATE_NOOP("Language", "Arabic"),
    QT_TRANSLATE_NOOP("Language", "Belarusian"),
    QT_TRANSLATE_NOOP("Language", "Bulgarian"),
    QT_TRANSLATE_NOOP("Language", "Catalan"),
    QT_TRANSLATE_NOOP("Language", "Chinese"),
    QT_TRANSLATE_NOOP("Language", "Croatian"),
    QT_TRANSLATE_NOOP("Language", "Czech"),
    QT_TRANSLATE_NOOP("Language", "Danish"),
    QT_TRANSLATE_NOOP("Language", "Dutch"),
    QT_TRANSLATE_NOOP("Language", "English"),
    QT_TRANSLATE_NOOP("Language", "Estonian"),
    QT_TRANSLATE_NOOP("Language", "Finnish"),
    QT_TRANSLATE_NOOP("Language", "French"),
    QT_TRANSLATE_NOOP("Language", "German"),
    QT_TRANSLATE_NOOP("Language", "Greek"),
    QT_TRANSLATE_NOOP("Language", "Hindi"),
    QT_TRANSLATE_NOOP("Language", "Hungarian"),
    QT_TRANSLATE_NOOP("Language", "Icelandic"),
    QT_TRANSLATE_NOOP("Language", "Indonesian"),
    QT_TRANSLATE_NOOP("Language", "Italian"),
    QT_TRANSLATE_NOOP("Language", "Japanese"),
    QT_TRANSLATE_NOOP("Language", "Kazakh"),
    QT_TRANSLATE_NOOP("Language", "Korean"),
    QT_TRANSLATE_NOOP("Language", "Latvian"),
    QT_TRANSLATE_NOOP("Language", "Lithuanian"),
    QT_TRANSLATE_NOOP("Language", "Macedonian"),
    QT_TRANSLATE_NOOP("Language", "Marathi"),
    QT_TRANSLATE_NOOP("Language", "Mongolian"),
    QT_TRANSLATE_NOOP("Language", "Nepali"),
    QT_TRANSLATE_NOOP("Language", "Norwegian"),
    QT_TRANSLATE_NOOP("Language", "Persian"),
    QT_TRANSLATE_NOOP("Language", "Polish"),
    QT_TRANSLATE_NOOP("Language", "Portuguese"),
    QT_TRANSLATE_NOOP("Language", "Romanian"),
    QT_TRANSLATE_NOOP("Language", "Russian"),
    QT_TRANSLATE_NOOP("Language", "Serbian"),
    QT_TRANSLATE_NOOP("Language", "Slovak"),
    QT_TRANSLATE_NOOP("Language", "Slovenian"),
    QT_TRANSLATE_NOOP("Language", "Spanish"),
    QT_TRANSLATE_NOOP("Language", "Swedish"),
    QT_TRANSLATE_NOOP("Language", "Tamil"),
    QT_TRANSLATE_NOOP("Language", "Telugu"),
    QT_TRANSLATE_NOOP("Language", "Thai"),
    QT_TRANSLATE_NOOP("Language", "Turkish"),
    QT_TRANSLATE_NOOP("Language", "Ukrainian"),
    QT_TRANSLATE_NOOP("Language", "Urdu"),
};
[[maybe_unused]] const char *const modelTitles[] = {
    QT_TRANSLATE_NOOP("Model", "Handwriting: Latin script, Chinese, Japanese"),
    QT_TRANSLATE_NOOP("Model", "Formulas"),
    QT_TRANSLATE_NOOP("Model", "Handwriting: Greek"),
    QT_TRANSLATE_NOOP("Model", "Handwriting: Russian, Ukrainian, Belarusian"),
    QT_TRANSLATE_NOOP("Model", "Handwriting: Cyrillic"),
    QT_TRANSLATE_NOOP("Model", "Handwriting: Arabic script"),
    QT_TRANSLATE_NOOP("Model", "Handwriting: Korean"),
    QT_TRANSLATE_NOOP("Model", "Handwriting: Devanagari"),
    QT_TRANSLATE_NOOP("Model", "Handwriting: Thai"),
    QT_TRANSLATE_NOOP("Model", "Handwriting: Tamil"),
    QT_TRANSLATE_NOOP("Model", "Handwriting: Telugu"),
};
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
    return AppController::dataDir() + QStringLiteral("/models");
}

bool Models::installed(const QString &id)
{
    for (const ModelRow &row : model_rows(dir().toStdString())) {
        if (toQ(row.id) == id)
            return row.installed;
    }
    return false;
}

QStringList Models::textModels(const QString &langs)
{
    QStringList out;
    for (const rust::String &id : text_models(langs.toStdString()))
        out.append(toQ(id));
    return out;
}

QString Models::defaultLanguages()
{
    return toQ(locale_language(QLocale::system().name().toStdString()));
}

QString Models::languages()
{
    return QSettings().value(QStringLiteral("hwr/lang"), defaultLanguages()).toString();
}

QVariantList Models::languageList() const
{
    QVariantList out;
    for (const LanguageRow &row : ::languages()) {
        const QString name = QCoreApplication::translate("Language", std::string(row.english).c_str());
        const QString native = toQ(row.native);
        out.append(QVariantMap{
            {QStringLiteral("code"), toQ(row.code)},
            {QStringLiteral("name"), native.isEmpty() || native == name ? name : name + QStringLiteral(" (") + native + u')'},
            {QStringLiteral("model"), toQ(row.model)}});
    }
    return out;
}

QVariantList Models::entries() const
{
    QVariantList out;
    for (const ModelRow &row : model_rows(dir().toStdString())) {
        out.append(QVariantMap{{QStringLiteral("id"), toQ(row.id)},
                               {QStringLiteral("title"), QCoreApplication::translate("Model", std::string(row.title).c_str())},
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

#include "ductus-qt/cpp/translations.h"

#include <QCoreApplication>
#include <QDir>
#include <QLibraryInfo>
#include <QLocale>
#include <QQmlEngine>
#include <QSettings>
#include <QTranslator>
#include <QVariantMap>

namespace {
// build.rs compiles i18n/ductus_<code>.ts into these.
const QString dir = QStringLiteral(":/i18n");

QTranslator *own = nullptr;
QTranslator *qt = nullptr;

QString nativeName(const QLocale &locale)
{
    QString name = locale.nativeLanguageName();
    if (!name.isEmpty())
        name[0] = name[0].toUpper();
    return name;
}

/// The language "System" shows the app in: the system's if there's a translation, else English.
QString systemName()
{
    QTranslator probe;
    if (probe.load(QLocale::system(), QStringLiteral("ductus"), QStringLiteral("_"), dir))
        return nativeName(QLocale(probe.language()));
    return QStringLiteral("English");
}

QString saved()
{
    return QSettings().value(QStringLiteral("app/language")).toString();
}
} // namespace

Translations::Translations(QObject *parent)
    : QObject(parent)
{
}

void Translations::apply()
{
    for (QTranslator **t : {&own, &qt}) {
        if (*t) {
            QCoreApplication::removeTranslator(*t);
            delete *t;
            *t = nullptr;
        }
    }
    const QString code = saved();
    const QLocale locale = code.isEmpty() ? QLocale::system() : QLocale(code);
    QLocale::setDefault(locale);
    own = new QTranslator;
    if (own->load(locale, QStringLiteral("ductus"), QStringLiteral("_"), dir))
        QCoreApplication::installTranslator(own);
    // Qt's own words (standard buttons, file dialogs), where the platform ships them.
    qt = new QTranslator;
    if (qt->load(locale, QStringLiteral("qtbase"), QStringLiteral("_"),
                 QLibraryInfo::path(QLibraryInfo::TranslationsPath)))
        QCoreApplication::installTranslator(qt);
}

QVariantList Translations::languages() const
{
    QVariantList out{QVariantMap{{QStringLiteral("code"), QString()},
                                 {QStringLiteral("name"), tr("System (%1)").arg(systemName())}},
                     QVariantMap{{QStringLiteral("code"), QStringLiteral("en")},
                                 {QStringLiteral("name"), QStringLiteral("English")}}};
    QList<QPair<QString, QString>> found;
    for (const QString &file : QDir(dir).entryList({QStringLiteral("ductus_*.qm")})) {
        const QString code = file.mid(7, file.size() - 10);
        found.append({nativeName(QLocale(code)), code});
    }
    std::sort(found.begin(), found.end());
    for (const auto &[name, code] : found)
        out.append(QVariantMap{{QStringLiteral("code"), code}, {QStringLiteral("name"), name}});
    return out;
}

QString Translations::language() const
{
    return saved();
}

void Translations::setLanguage(const QString &code)
{
    if (code == saved())
        return;
    QSettings().setValue(QStringLiteral("app/language"), code);
    apply();
    if (QQmlEngine *engine = qmlEngine(this))
        engine->retranslate();
    Q_EMIT languageChanged();
}

// Include guards: the build reaches headers through two include paths.
#ifndef DUCTUS_TRANSLATIONS_H
#define DUCTUS_TRANSLATIONS_H

#include <QObject>
#include <QVariantList>
#include <QtQml/qqmlregistration.h>

/// The app's own language: the system's, or one picked in the settings (app/language).
class Translations : public QObject
{
    Q_OBJECT
    QML_ELEMENT
    QML_SINGLETON
    /// [{ code, name }]: "" (the system's) first, then each translation by its own name.
    Q_PROPERTY(QVariantList languages READ languages NOTIFY languageChanged)
    /// "" follows the system.
    Q_PROPERTY(QString language READ language WRITE setLanguage NOTIFY languageChanged)

public:
    explicit Translations(QObject *parent = nullptr);

    /// Loads the saved language's translation; before the QML engine starts, and on each change.
    static void apply();

    QVariantList languages() const;
    QString language() const;
    void setLanguage(const QString &code);

Q_SIGNALS:
    void languageChanged();
};

#endif

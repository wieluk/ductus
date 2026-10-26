// Include guards: the build reaches headers through two include paths.
#ifndef DUCTUS_UPDATER_H
#define DUCTUS_UPDATER_H

#include <QObject>
#include <QProcess>
#include <QTimer>
#include <QtQml/qqmlregistration.h>
#include <optional>

#include "rust/cxx.h"

struct UpdateHandle;

/// Channels: flatpak (host flatpak update), installer (signed Windows installer), android (signed APK handed to the package installer), manual (release page), none (development builds).
class Updater : public QObject
{
    Q_OBJECT
    QML_ELEMENT
    QML_SINGLETON
    Q_PROPERTY(QString channel READ channel CONSTANT)
    Q_PROPERTY(QString currentVersion READ currentVersion CONSTANT)
    Q_PROPERTY(bool autoCheck READ autoCheck WRITE setAutoCheck NOTIFY autoCheckChanged)
    Q_PROPERTY(bool busy READ busy NOTIFY changed)
    Q_PROPERTY(bool available READ available NOTIFY changed)
    Q_PROPERTY(QString latestVersion READ latestVersion NOTIFY changed)
    Q_PROPERTY(QString notes READ notes NOTIFY changed)
    Q_PROPERTY(QString status READ status NOTIFY changed)
    /// 0 to 1 while a download runs, -1 when there's nothing to count.
    Q_PROPERTY(double progress READ progress NOTIFY changed)
    Q_PROPERTY(bool restartNeeded READ restartNeeded NOTIFY changed)

public:
    explicit Updater(QObject *parent = nullptr);
    ~Updater() override;

    QString channel() const { return m_channel; }
    QString currentVersion() const;
    bool autoCheck() const;
    void setAutoCheck(bool on);
    bool busy() const { return m_busy; }
    bool available() const;
    QString latestVersion() const { return m_latest; }
    QString notes() const { return m_notes; }
    QString status() const { return m_status; }
    double progress() const { return m_progress; }
    bool restartNeeded() const { return m_restartNeeded; }

    Q_INVOKABLE void check();
    Q_INVOKABLE void install();
    Q_INVOKABLE void cancel();
    Q_INVOKABLE void openReleasePage();
    Q_INVOKABLE void restart();
    /// "Later": the automatic check stops offering this version.
    Q_INVOKABLE void dismiss();

Q_SIGNALS:
    void changed();
    void autoCheckChanged();
    void updateFound();

private:
    enum class Task { None, Check, Download, Flatpak };
    void poll();
    void setStatus(const QString &status, double progress = -1);
    void finishCheck(const QString &error);
    void finishDownload(const QString &file, const QString &error);
    void runInstaller(const QString &file);
    void openApk(const QString &file);
    void flatpakUpdate();
    void flatpakFinished(int exitCode);

    std::optional<rust::Box<UpdateHandle>> m_handle;
    QString m_channel;
    QString m_feed;
    QString m_latest;
    QString m_notes;
    QString m_page;
    QString m_status;
    double m_progress = -1;
    bool m_busy = false;
    bool m_automatic = false;
    bool m_restartNeeded = false;
    Task m_task = Task::None;
    QTimer m_poll;
    QTimer m_daily;
    QProcess m_flatpak;
    QString m_flatpakOutput;
    QString m_flatpakCommit;
};

#endif

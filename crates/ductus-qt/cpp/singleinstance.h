#ifndef DUCTUS_SINGLEINSTANCE_H
#define DUCTUS_SINGLEINSTANCE_H

#include <QLocalServer>
#include <QObject>

/// One Ductus per user and settings folder: starting it again brings the running window forward,
/// and hands it the files to open.
class SingleInstance : public QObject
{
    Q_OBJECT

public:
    using QObject::QObject;

    /// False when another copy runs: it was asked to come forward and open `files`, and this one
    /// should quit.
    bool claim(const QStringList &files);

Q_SIGNALS:
    /// Another start asked for the window; `token` lets Wayland raise it.
    void activationRequested(const QString &token, const QStringList &files);

private:
    static QString serverName();
    QLocalServer m_server;
};

#endif

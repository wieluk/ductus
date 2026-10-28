#ifndef DUCTUS_SINGLEINSTANCE_H
#define DUCTUS_SINGLEINSTANCE_H

#include <QLocalServer>
#include <QObject>

/// One Ductus per user and settings folder: starting it again brings the running window forward.
class SingleInstance : public QObject
{
    Q_OBJECT

public:
    using QObject::QObject;

    /// False when another copy runs: it was asked to come forward, and this one should quit.
    bool claim();

Q_SIGNALS:
    /// Another start asked for the window; `token` lets Wayland raise it.
    void activationRequested(const QString &token);

private:
    static QString serverName();
    QLocalServer m_server;
};

#endif

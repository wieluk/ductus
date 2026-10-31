#include "ductus-qt/cpp/singleinstance.h"

#include <QCryptographicHash>
#include <QDir>
#include <QLocalSocket>
#include <QStandardPaths>

#include <memory>

QString SingleInstance::serverName()
{
    // Tests and portable setups have their own settings folder, and their own instance.
    QString name = QStringLiteral("ductus");
    const QString configDir = qEnvironmentVariable("DUCTUS_CONFIG_DIR");
    if (!configDir.isEmpty())
        name += QLatin1Char('-')
                + QString::fromLatin1(QCryptographicHash::hash(configDir.toUtf8(), QCryptographicHash::Sha1)
                                          .toHex()
                                          .left(12));
#if defined(Q_OS_WIN)
    // Named pipes are machine-wide.
    return name + QLatin1Char('-') + qEnvironmentVariable("USERNAME");
#else
    // A Flatpak sandbox shares only its app folder in the runtime directory with its other copies.
    QString dir = QStandardPaths::writableLocation(QStandardPaths::RuntimeLocation);
    const QString flatpak = qEnvironmentVariable("FLATPAK_ID");
    if (!flatpak.isEmpty())
        dir += QStringLiteral("/app/") + flatpak;
    if (dir.isEmpty() || !QDir(dir).exists())
        return name;
    return dir + QLatin1Char('/') + name + QStringLiteral(".socket");
#endif
}

bool SingleInstance::claim(const QStringList &files)
{
    const QString name = serverName();
    QLocalSocket running;
    running.connectToServer(name);
    if (running.waitForConnected(500)) {
        // The launcher's token, so the window may come to the front on Wayland.
        QByteArray token = qgetenv("XDG_ACTIVATION_TOKEN");
        if (token.isEmpty())
            token = qgetenv("DESKTOP_STARTUP_ID");
        // A line each for the token and the files, then an empty line.
        running.write(token + '\n');
        for (const QString &file : files)
            running.write(file.toUtf8() + '\n');
        running.write("\n");
        running.waitForBytesWritten(1000);
        running.disconnectFromServer();
        // Shows up in logs, so a start that seems to do nothing can be told apart from a crash.
        qInfo("Ductus is already running (%s); it was asked to come to the front.", qPrintable(name));
        return false;
    }
    // Nobody answers: a copy that crashed may have left its socket behind.
    QLocalServer::removeServer(name);
    m_server.setSocketOptions(QLocalServer::UserAccessOption);
    if (!m_server.listen(name))
        return true;
    connect(&m_server, &QLocalServer::newConnection, this, [this] {
        while (QLocalSocket *socket = m_server.nextPendingConnection()) {
            connect(socket, &QLocalSocket::disconnected, socket, &QObject::deleteLater);
            // The first line is the token, which may be empty; an empty line after it ends the files.
            auto lines = std::make_shared<QStringList>();
            connect(socket, &QLocalSocket::readyRead, this, [this, socket, lines] {
                while (socket->canReadLine()) {
                    const QString line = QString::fromUtf8(socket->readLine().chopped(1));
                    if (!lines->isEmpty() && line.isEmpty()) {
                        Q_EMIT activationRequested(lines->first(), lines->mid(1));
                        lines->clear();
                    } else {
                        lines->append(line);
                    }
                }
            });
        }
    });
    return true;
}

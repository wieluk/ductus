#include "ductus-qt/cpp/platformfiles.h"

#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QStandardPaths>
#include <QUrl>

#if defined(Q_OS_ANDROID)
#include <QCoreApplication>
#include <QtCore/qcoreapplication_platform.h>
#include <QJniEnvironment>
#include <QJniObject>
#endif

namespace {

bool isLocal(const QString &fileOrUrl)
{
    const QUrl url(fileOrUrl);
    return url.isLocalFile() || url.scheme().isEmpty() || url.scheme().size() == 1; // C:/…
}

QString localFile(const QString &fileOrUrl)
{
    const QUrl url(fileOrUrl);
    return url.isLocalFile() ? url.toLocalFile() : fileOrUrl;
}

QString cacheFile(const QString &folder, const QString &name)
{
    const QString dir = QStandardPaths::writableLocation(QStandardPaths::CacheLocation)
                        + QLatin1Char('/') + folder;
    QDir().mkpath(dir);
    return dir + QLatin1Char('/') + (name.isEmpty() ? QStringLiteral("file") : name);
}

#if defined(Q_OS_ANDROID)
int sdkVersion()
{
    return QJniObject::getStaticField<jint>("android/os/Build$VERSION", "SDK_INT");
}

QJniObject context()
{
    return QNativeInterface::QAndroidApplication::context();
}
#endif

} // namespace

namespace PlatformFiles {

QString displayName(const QString &fileOrUrl)
{
    // Qt names content:// files by the name the providing app gives them.
    return QFileInfo(localFile(fileOrUrl)).fileName();
}

QString readable(const QString &fileOrUrl)
{
    if (isLocal(fileOrUrl))
        return localFile(fileOrUrl);
    QFile source(fileOrUrl);
    if (!source.open(QIODevice::ReadOnly))
        return {};
    const QString copy = cacheFile(QStringLiteral("opened"), displayName(fileOrUrl));
    QFile::remove(copy);
    QFile target(copy);
    if (!target.open(QIODevice::WriteOnly) || target.write(source.readAll()) < 0)
        return {};
    return copy;
}

QString writable(const QString &fileOrUrl)
{
    if (isLocal(fileOrUrl))
        return localFile(fileOrUrl);
    const QString copy = cacheFile(QStringLiteral("saved"), displayName(fileOrUrl));
    QFile::remove(copy);
    return copy;
}

bool deliver(const QString &written, const QString &fileOrUrl)
{
    if (isLocal(fileOrUrl))
        return true;
    QFile source(written);
    QFile target(fileOrUrl);
    const bool ok = source.open(QIODevice::ReadOnly)
                    && target.open(QIODevice::WriteOnly | QIODevice::Truncate)
                    && target.write(source.readAll()) >= 0;
    source.close();
    QFile::remove(written);
    return ok;
}

QString folderPath(const QString &pathOrUrl)
{
    if (isLocal(pathOrUrl))
        return localFile(pathOrUrl);
    // content://com.android.externalstorage.documents/tree/primary%3ADocuments%2FNotes
    const QUrl url(pathOrUrl);
    if (url.host() != u"com.android.externalstorage.documents")
        return {};
    const QStringList parts = url.path(QUrl::FullyDecoded).split(QLatin1Char('/'), Qt::SkipEmptyParts);
    const qsizetype tree = parts.indexOf(QStringLiteral("tree"));
    if (tree < 0 || tree + 1 >= parts.size())
        return {};
    // After "tree" the folder's id; a "document" part may repeat it.
    const QString id = parts.mid(tree + 1).join(QLatin1Char('/')).section(QStringLiteral("/document/"), 0, 0);
    const QString volume = id.section(QLatin1Char(':'), 0, 0);
    const QString rest = id.section(QLatin1Char(':'), 1);
    const QString base = volume == u"primary" ? QStringLiteral("/storage/emulated/0")
                                               : QStringLiteral("/storage/") + volume;
    return rest.isEmpty() ? base : base + QLatin1Char('/') + rest;
}

#if defined(Q_OS_ANDROID)

bool hasSharedAccess()
{
    if (sdkVersion() >= 30)
        return QJniObject::callStaticMethod<jboolean>("android/os/Environment", "isExternalStorageManager", "()Z");
    const jint granted = context().callMethod<jint>(
        "checkSelfPermission", "(Ljava/lang/String;)I",
        QJniObject::fromString(QStringLiteral("android.permission.WRITE_EXTERNAL_STORAGE")).object<jstring>());
    return granted == 0;
}

void requestSharedAccess()
{
    QJniObject activity = context();
    if (sdkVersion() >= 30) {
        const QString package = activity.callObjectMethod("getPackageName", "()Ljava/lang/String;").toString();
        const QJniObject uri = QJniObject::callStaticObjectMethod(
            "android/net/Uri", "parse", "(Ljava/lang/String;)Landroid/net/Uri;",
            QJniObject::fromString(QStringLiteral("package:") + package).object<jstring>());
        QJniObject intent("android/content/Intent", "(Ljava/lang/String;Landroid/net/Uri;)V",
                          QJniObject::fromString(QStringLiteral("android.settings.MANAGE_APP_ALL_FILES_ACCESS_PERMISSION"))
                              .object<jstring>(),
                          uri.object());
        activity.callMethod<void>("startActivity", "(Landroid/content/Intent;)V", intent.object());
        return;
    }
    QJniEnvironment env;
    const QStringList permissions{QStringLiteral("android.permission.READ_EXTERNAL_STORAGE"),
                                  QStringLiteral("android.permission.WRITE_EXTERNAL_STORAGE")};
    jobjectArray array = env->NewObjectArray(jsize(permissions.size()), env.findClass("java/lang/String"), nullptr);
    for (qsizetype i = 0; i < permissions.size(); ++i)
        env->SetObjectArrayElement(array, jsize(i), QJniObject::fromString(permissions.at(i)).object<jstring>());
    activity.callMethod<void>("requestPermissions", "([Ljava/lang/String;I)V", array, jint(1));
    env->DeleteLocalRef(array);
}

QString sharedLibraryPath()
{
    const QJniObject dir = QJniObject::callStaticObjectMethod(
        "android/os/Environment", "getExternalStoragePublicDirectory", "(Ljava/lang/String;)Ljava/io/File;",
        QJniObject::fromString(QStringLiteral("Documents")).object<jstring>());
    return dir.callObjectMethod("getAbsolutePath", "()Ljava/lang/String;").toString() + QStringLiteral("/Ductus");
}

#else

bool hasSharedAccess()
{
    return true;
}

void requestSharedAccess() {}

QString sharedLibraryPath()
{
    return QStandardPaths::writableLocation(QStandardPaths::DocumentsLocation) + QStringLiteral("/Ductus");
}

#endif

bool insideApp(const QString &path)
{
#if defined(Q_OS_ANDROID)
    const QString app = QStandardPaths::writableLocation(QStandardPaths::AppDataLocation);
    return path.contains(QStringLiteral("/Android/data/")) || path.startsWith(app);
#else
    Q_UNUSED(path);
    return false;
#endif
}

QString privateLibraryPath()
{
    // On Android: the app's own folder in external storage; elsewhere the same as shared.
    return QStandardPaths::writableLocation(QStandardPaths::DocumentsLocation) + QStringLiteral("/Ductus");
}

} // namespace PlatformFiles

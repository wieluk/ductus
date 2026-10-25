// Include guards, not #pragma once: the build reaches headers through two include paths.
#ifndef DUCTUS_PLATFORMFILES_H
#define DUCTUS_PLATFORMFILES_H

#include <QString>

// Files that the system's pickers hand over. On Android they are content:// links that only
// Qt's file API can read and write, so the Rust side gets a copy in the cache instead.
namespace PlatformFiles {

/// A local path to read `fileOrUrl` from; "" when it can't be read.
QString readable(const QString &fileOrUrl);
/// A local path to write to; `deliver` then puts the result where `fileOrUrl` points.
QString writable(const QString &fileOrUrl);
bool deliver(const QString &written, const QString &fileOrUrl);
/// The name a person knows the file by.
QString displayName(const QString &fileOrUrl);
/// A folder picked in the system's folder picker as a path, or "" if it has none.
QString folderPath(const QString &pathOrUrl);

/// Whether Ductus may keep notes in shared storage (Documents), where sync apps and backups
/// reach them. Always true outside Android.
bool hasSharedAccess();
/// Asks for it: on Android 11 and later, the system's "All files access" page opens.
void requestSharedAccess();
/// Documents/Ductus in shared storage.
QString sharedLibraryPath();
/// The library folder inside the app, removed with it: where notes go without shared access.
QString privateLibraryPath();
/// Whether a folder is the app's own (Android only), which uninstalling removes.
bool insideApp(const QString &path);

} // namespace PlatformFiles

#endif

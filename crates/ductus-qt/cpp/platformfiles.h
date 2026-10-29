// Include guards: the build reaches headers through two include paths.
#ifndef DUCTUS_PLATFORMFILES_H
#define DUCTUS_PLATFORMFILES_H

#include <QString>

// Picked files. Android hands over content:// links only Qt can open, so Rust gets a cached copy.
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

/// Whether notes may go to shared storage (Documents). Always true outside Android.
bool hasSharedAccess();
/// Asks for it: on Android 11 and later, the system's "All files access" page opens.
void requestSharedAccess();
/// Documents/Ductus in shared storage.
QString sharedLibraryPath();
/// The library folder inside the app, removed with it: where notes go without shared access.
QString privateLibraryPath();
/// Whether a folder is the app's own (Android only), which uninstalling removes.
bool insideApp(const QString &path);
/// A folder as people see it: native separators, and on Android relative to the storage it is on.
QString displayPath(const QString &path);
/// Android: the app goes to the background, as Back does on an app's first screen.
void moveToBack();

} // namespace PlatformFiles

#endif

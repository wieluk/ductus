#!/usr/bin/env bash
# Builds Ductus.app and dist/Ductus-<version>-macos-<arch>.dmg for this Mac's architecture.
# Needs Rust, Xcode's command-line tools and Qt 6.10 for macOS (QMAKE or QT_ROOT_DIR).
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
cd "$root"
export QMAKE=${QMAKE:-${QT_ROOT_DIR:?set QMAKE or QT_ROOT_DIR}/bin/qmake}
version=${VERSION:-$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -1)}
case $(uname -m) in
  arm64) arch=arm64 pdfium=mac-arm64 ;;
  x86_64) arch=x86_64 pdfium=mac-x64 ;;
  *) echo "unsupported architecture $(uname -m)" >&2; exit 1 ;;
esac
build=$root/target/macos
app=$build/Ductus.app

cargo build --release --locked -p ductus-qt
build-aux/fetch-pdfium.sh "$pdfium" target/pdfium
# Offscreen tests before bundling: the bundle only carries the Cocoa platform plugin.
build-aux/ui-tests.sh target/release/ductus

rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources" "$app/Contents/Frameworks"
cp target/release/ductus "$app/Contents/MacOS/"
cp target/pdfium/lib/libpdfium.dylib "$app/Contents/Frameworks/"
sed "s/@VERSION@/$version/g" build-aux/macos/Info.plist > "$app/Contents/Info.plist"
iconset=$build/ductus.iconset
rm -rf "$iconset" && mkdir -p "$iconset"
for size in 16 32 128 256 512; do
  for scale in 1 2; do
    suffix=$([ $scale = 2 ] && echo @2x || true)
    sips -z $((size * scale)) $((size * scale)) data/icons/app.ductus.Ductus-1024.png \
      --out "$iconset/icon_${size}x${size}$suffix.png" > /dev/null
  done
done
iconutil -c icns "$iconset" -o "$app/Contents/Resources/ductus.icns"

"$(dirname "$QMAKE")/macdeployqt" "$app" -qmldir=crates/ductus-qt/qml
# Ad-hoc: Apple silicon refuses unsigned code; notarizing needs a developer account.
codesign --force --deep --sign - "$app"

# A script that only waits ends the app with status 0, proving the bundle starts.
smoke=$build/smoke
rm -rf "$smoke" && mkdir -p "$smoke"
printf 'wait 1500\n' > "$smoke/start.uitest"
DUCTUS_UI_SCRIPT=$smoke/start.uitest DUCTUS_UI_SHOTS=$smoke/shots DUCTUS_LIBRARY=$smoke/Notes \
  DUCTUS_CONFIG_DIR=$smoke/config DUCTUS_CACHE_DIR=$smoke/cache \
  perl -e 'alarm shift; exec @ARGV' 120 "$app/Contents/MacOS/ductus"

mkdir -p dist
hdiutil create -volname Ductus -srcfolder "$app" -ov -format UDZO "dist/Ductus-$version-macos-$arch.dmg"

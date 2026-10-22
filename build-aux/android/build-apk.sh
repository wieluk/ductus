#!/usr/bin/env bash
# Builds dist/Ductus-<version>-android-arm64.apk.
# Needs Rust (aarch64-linux-android), JDK 17, Android SDK 35 + NDK r27c and Qt 6.10 for Android
# plus the same desktop Qt: ANDROID_SDK_ROOT, ANDROID_NDK_ROOT, QT_ANDROID, QT_HOST.
# Without ANDROID_KEYSTORE (+ _PASSWORD, ANDROID_KEY_ALIAS) a throwaway key signs it.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
cd "$root"
: "${ANDROID_SDK_ROOT:?}" "${ANDROID_NDK_ROOT:?}" "${QT_ANDROID:?}" "${QT_HOST:?}"
version=${VERSION:-$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -1)}
api=28
target=aarch64-linux-android
abi=arm64-v8a
build=$root/target/android
apk_dir=$build/apk
mkdir -p "$build" dist

toolchain=$ANDROID_NDK_ROOT/toolchains/llvm/prebuilt/linux-x86_64
export CC_aarch64_linux_android=$toolchain/bin/$target$api-clang
export CXX_aarch64_linux_android=$toolchain/bin/$target$api-clang++
export AR_aarch64_linux_android=$toolchain/bin/llvm-ar
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER=$CC_aarch64_linux_android
# Explicit, or cxx-qt picks the build machine's linker.
export CARGO_TARGET_AARCH64_LINUX_ANDROID_RUSTFLAGS="-C link-arg=-fuse-ld=lld"
export QMAKE=$QT_ANDROID/bin/qmake
# The desktop rcc compresses with zstd, which Qt for Android lacks: the library would not load.
export CXX_QT_AUTORCC_OPTIONS=--no-zstd

# Qt's loader opens lib<name>_<abi>.so and calls its `main`.
cargo rustc --release --locked -p ductus-qt --lib --crate-type cdylib --target $target
rm -rf "$apk_dir"
mkdir -p "$apk_dir/libs/$abi"
"$toolchain/bin/llvm-strip" --strip-unneeded -o "$apk_dir/libs/$abi/libductus_$abi.so" \
  "target/$target/release/libductus_qt.so"
# Android refuses to load a library that needs a symbol Qt for Android doesn't have.
missing=$(comm -23 \
  <("$toolchain/bin/llvm-nm" -D --undefined-only "$apk_dir/libs/$abi/libductus_$abi.so" \
    | awk '$2 ~ /^qt_/ {print $2}' | sort -u) \
  <("$toolchain/bin/llvm-nm" -D --defined-only "$QT_ANDROID/lib/libQt6Core_$abi.so" \
    | awk '{sub(/@.*/, "", $3); print $3}' | sort -u))
if [ -n "$missing" ]; then
  echo "Qt for Android lacks: $missing" >&2
  exit 1
fi

pdfium=$build/pdfium
build-aux/fetch-pdfium.sh android-arm64 "$pdfium"

# Android counts versions with one number: 1.2.3 becomes 10203.
IFS=. read -r major minor patch <<<"${version%%[-+]*}"
code=$((major * 10000 + minor * 100 + patch))
[ "$code" -gt 0 ] || code=1

# What CMake would write for a qt_add_executable target.
cat > "$build/deployment-settings.json" <<EOF
{
  "description": "Written by build-aux/android/build-apk.sh for androiddeployqt.",
  "qt": { "$abi": "$QT_ANDROID" },
  "qtDataDirectory": { "$abi": "." },
  "qtLibExecsDirectory": { "$abi": "libexec" },
  "qtLibsDirectory": { "$abi": "lib" },
  "qtPluginsDirectory": { "$abi": "plugins" },
  "qtQmlDirectory": { "$abi": "qml" },
  "sdk": "$ANDROID_SDK_ROOT",
  "sdkBuildToolsRevision": "35.0.0",
  "ndk": "$ANDROID_NDK_ROOT",
  "toolchain-prefix": "llvm",
  "tool-prefix": "llvm",
  "useLLVM": true,
  "ndk-host": "linux-x86_64",
  "abi": "$abi",
  "architectures": { "$abi": "$target" },
  "android-extra-libs": "$pdfium/lib/libpdfium.so",
  "android-package-source-directory": "$root/build-aux/android/package",
  "android-package-name": "app.ductus.Ductus",
  "android-app-name": "Ductus",
  "android-version-code": "$code",
  "android-version-name": "$version",
  "android-min-sdk-version": "$api",
  "android-target-sdk-version": "35",
  "android-compile-sdk-version": "35",
  "permissions": [],
  "application-binary": "ductus",
  "qml-import-paths": "",
  "qml-root-path": "$root/crates/ductus-qt/qml",
  "qml-importscanner-binary": "$QT_HOST/libexec/qmlimportscanner",
  "rcc-binary": "$QT_HOST/libexec/rcc",
  "extraPrefixDirs": [],
  "zstdCompression": false,
  "generate-java-qtquickview-contents": false,
  "stdcpp-path": "$toolchain/sysroot/usr/lib/"
}
EOF

sign=()
if [ -n "${ANDROID_KEYSTORE:-}" ]; then
  sign=(--sign "$ANDROID_KEYSTORE" "${ANDROID_KEY_ALIAS:?}" --storepass "${ANDROID_KEYSTORE_PASSWORD:?}")
else
  echo "warning: no ANDROID_KEYSTORE; signing with a throwaway key (updates won't install over it)" >&2
  keystore=$build/throwaway.keystore
  [ -f "$keystore" ] || keytool -genkeypair -keystore "$keystore" -alias ductus -keyalg RSA \
    -keysize 2048 -validity 10000 -storepass throwaway -keypass throwaway \
    -dname "CN=Ductus test build" >/dev/null
  sign=(--sign "$keystore" ductus --storepass throwaway)
fi

"$QT_HOST/bin/androiddeployqt" --input "$build/deployment-settings.json" --output "$apk_dir" \
  --android-platform android-35 --jdk "${JAVA_HOME:?}" --gradle --release "${sign[@]}"

apk=$(find "$apk_dir/build/outputs/apk/release" -name '*-signed.apk' | head -1)
[ -n "$apk" ] || { echo "no signed APK was produced" >&2; exit 1; }
cp "$apk" "dist/Ductus-$version-android-arm64.apk"
echo "dist/Ductus-$version-android-arm64.apk"

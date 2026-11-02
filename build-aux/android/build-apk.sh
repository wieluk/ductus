#!/usr/bin/env bash
# Builds dist/Ductus-<version>-android-arm64.apk (ABI=x86_64: -android-x86_64.apk, for emulators).
#   build-apk.sh [build|sign]   (default: both; signing alone needs no build tools or code)
# Needs Rust (aarch64- or x86_64-linux-android), JDK 17, Android SDK 35 + NDK r27c and Qt 6.10
# for Android plus the same desktop Qt: ANDROID_SDK_ROOT, ANDROID_NDK_ROOT, QT_ANDROID, QT_HOST.
# Without ANDROID_KEYSTORE (+ _PASSWORD, ANDROID_KEY_ALIAS) a throwaway key signs it.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
cd "$root"
step=${1:-all}
: "${ANDROID_SDK_ROOT:?}" "${JAVA_HOME:?}"
# apksigner runs the `java` on PATH; a newer one warns about its native access.
export PATH="$JAVA_HOME/bin:$PATH"
version=${VERSION:-$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -1)}
api=28
abi=${ABI:-arm64-v8a}
case $abi in
  arm64-v8a) target=aarch64-linux-android; platform=android-arm64; arch=arm64 ;;
  x86_64) target=x86_64-linux-android; platform=android-x64; arch=x86_64 ;;
  *) echo "unknown ABI $abi" >&2; exit 1 ;;
esac
target_env=${target//-/_}
build=$root/target/android
apk_dir=$build/apk
unsigned=$build/ductus-$arch-unsigned.apk
mkdir -p "$build" dist

if [ "$step" != sign ]; then
: "${ANDROID_NDK_ROOT:?}" "${QT_ANDROID:?}" "${QT_HOST:?}"

toolchain=$ANDROID_NDK_ROOT/toolchains/llvm/prebuilt/linux-x86_64
export "CC_$target_env=$toolchain/bin/$target$api-clang"
export "CXX_$target_env=$toolchain/bin/$target$api-clang++"
export "AR_$target_env=$toolchain/bin/llvm-ar"
cargo_target=CARGO_TARGET_${target_env^^}
export "${cargo_target}_LINKER=$toolchain/bin/$target$api-clang"
# Explicit, or cxx-qt picks the build machine's linker.
export "${cargo_target}_RUSTFLAGS=-C link-arg=-fuse-ld=lld"
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
    | awk '$2 ~ /^qt_/ {sub(/@.*/, "", $2); print $2}' | sort -u) \
  <("$toolchain/bin/llvm-nm" -D --defined-only "$QT_ANDROID/lib/libQt6Core_$abi.so" \
    | awk '{sub(/@.*/, "", $3); print $3}' | sort -u))
if [ -n "$missing" ]; then
  echo "Qt for Android lacks: $missing" >&2
  exit 1
fi

pdfium=$build/pdfium-$arch
build-aux/fetch-pdfium.sh "$platform" "$pdfium"
onnxruntime=$build/onnxruntime-$arch
build-aux/fetch-onnxruntime.sh "$platform" "$onnxruntime"
# The licenses of what the APK bundles.
mkdir -p "$apk_dir/assets/licenses"
cp LICENSE "$apk_dir/assets/licenses/ductus.txt"
cp "$pdfium/LICENSE" "$apk_dir/assets/licenses/pdfium.txt"
cp "$onnxruntime/LICENSE" "$apk_dir/assets/licenses/onnxruntime.txt"

# Android counts versions with one number: 1.2.3 becomes 10203.
IFS=. read -r major minor patch <<<"${version%%[-+]*}"
code=$((major * 10000 + minor * 100 + patch))
[ "$code" -gt 0 ] || code=1

# Our QML module is compiled into the library. A qmldir without a plugin lets the import scanner
# resolve `import Ductus`, and androiddeployqt then deploys nothing for it.
mkdir -p "$build/qml-imports/Ductus"
echo "module Ductus" > "$build/qml-imports/Ductus/qmldir"

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
  "android-extra-libs": "$pdfium/lib/libpdfium.so,$onnxruntime/lib/libonnxruntime.so",
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
  "qml-import-paths": "$build/qml-imports",
  "qml-root-path": "$root/crates/ductus-qt/qml",
  "qml-importscanner-binary": "$QT_HOST/libexec/qmlimportscanner",
  "rcc-binary": "$QT_HOST/libexec/rcc",
  "extraPrefixDirs": [],
  "zstdCompression": false,
  "generate-java-qtquickview-contents": false,
  "stdcpp-path": "$toolchain/sysroot/usr/lib/"
}
EOF

# Gradle names each deprecated feature instead of only counting them.
export GRADLE_OPTS="${GRADLE_OPTS:+$GRADLE_OPTS }-Dorg.gradle.warning.mode=all"
"$QT_HOST/bin/androiddeployqt" --input "$build/deployment-settings.json" --output "$apk_dir" \
  --android-platform android-35 --jdk "$JAVA_HOME" --gradle --release
cp "$(find "$apk_dir/build/outputs/apk/release" -name '*.apk' | head -1)" "$unsigned"
fi
[ "$step" = build ] && exit 0

# Passwords go through the environment, never on a command line.
if [ -n "${ANDROID_KEYSTORE:-}" ]; then
  keystore=$ANDROID_KEYSTORE alias=${ANDROID_KEY_ALIAS:?}
  : "${ANDROID_KEYSTORE_PASSWORD:?}"
else
  echo "No ANDROID_KEYSTORE: signing with a throwaway key (updates won't install over it)." >&2
  keystore=$build/throwaway.keystore alias=ductus
  export ANDROID_KEYSTORE_PASSWORD=throwaway
  [ -f "$keystore" ] || keytool -genkeypair -keystore "$keystore" -alias ductus -keyalg RSA \
    -keysize 2048 -validity 10000 -storepass throwaway -keypass throwaway \
    -dname "CN=Ductus test build" >/dev/null
fi
tools=$ANDROID_SDK_ROOT/build-tools/35.0.0
apk=dist/Ductus-$version-android-$arch.apk
# Libraries aligned to 16 KB pages, which newer devices use.
"$tools/zipalign" -f -P 16 4 "$unsigned" "$build/aligned.apk"
"$tools/apksigner" sign --ks "$keystore" --ks-key-alias "$alias" --ks-pass env:ANDROID_KEYSTORE_PASSWORD \
  --key-pass env:ANDROID_KEYSTORE_PASSWORD --out "$apk" "$build/aligned.apk"
rm "$build/aligned.apk"
echo "$apk"

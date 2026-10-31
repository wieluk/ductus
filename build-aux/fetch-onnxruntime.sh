#!/usr/bin/env bash
# Downloads the pinned ONNX Runtime library for PLATFORM into DEST/lib, checked against its digest.
#   fetch-onnxruntime.sh linux-x64|linux-arm64|win-x64|android-arm64|android-x64 DEST
# The handwriting recognizer loads it at runtime (crates/ductus-hwr/src/runtime.rs).
set -euo pipefail

platform=$1
dest=$2
# renovate: datasource=github-releases depName=microsoft/onnxruntime
version=1.31.1
base=https://github.com/microsoft/onnxruntime/releases/download/v$version
case $platform in
  linux-x64) file=onnxruntime-linux-x64-$version.tgz; sha=c849e36a1ef3e22d4ff217b9d1ce4a3cf5b9467b975610913ae959ad5b16bdc9 ;;
  linux-arm64) file=onnxruntime-linux-aarch64-$version.tgz; sha=5317505e2da99be65f8e2775ba3c2afd6d2961fd78aba1bd8f56159f48820c78 ;;
  win-x64) file=onnxruntime-win-x64-$version.zip; sha=5869e26bb25314375f2d5ef1877e3ea06b1543d6c710e6e4a1ec2f7177ab4b2c ;;
  android-arm64 | android-x64) file=onnxruntime-android-$version.aar; sha=d4d50660300ed2c4f38ffdfd5ab5bfa3a22f995f282dda24f42ea3ea55467b49
    base=https://repo1.maven.org/maven2/com/microsoft/onnxruntime/onnxruntime-android/$version ;;
  *) echo "unknown platform $platform" >&2; exit 1 ;;
esac
[ -f "$dest/.$platform-$version" ] && exit 0
mkdir -p "$dest/lib"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
curl -sSfL -o "$tmp/$file" "$base/$file"
echo "$sha  $tmp/$file" | sha256sum -c - > /dev/null
case $platform in
  linux-*)
    tar -xzf "$tmp/$file" -C "$tmp"
    cp "$tmp/${file%.tgz}/lib/libonnxruntime.so.$version" "$dest/lib/libonnxruntime.so"
    cp "$tmp/${file%.tgz}/LICENSE" "$dest/LICENSE" ;;
  win-x64)
    unzip -q "$tmp/$file" -d "$tmp"
    cp "$tmp/${file%.zip}/lib/onnxruntime.dll" "$dest/lib/"
    cp "$tmp/${file%.zip}/LICENSE" "$dest/LICENSE" ;;
  android-*)
    abi=$([ "$platform" = android-arm64 ] && echo arm64-v8a || echo x86_64)
    unzip -q -j "$tmp/$file" "jni/$abi/libonnxruntime.so" -d "$dest/lib" ;;
esac
touch "$dest/.$platform-$version"

#!/usr/bin/env bash
# Downloads the pinned ONNX Runtime library for PLATFORM into DEST/lib, checked against its digest.
#   fetch-onnxruntime.sh linux-x64|linux-arm64|win-x64|android-arm64|android-x64 DEST
# The handwriting recognizer loads it at runtime (crates/ductus-hwr/src/runtime.rs).
set -euo pipefail

platform=$1
dest=$2
# renovate: datasource=github-releases depName=microsoft/onnxruntime
version=1.31.0
base=https://github.com/microsoft/onnxruntime/releases/download/v$version
case $platform in
  linux-x64) file=onnxruntime-linux-x64-$version.tgz; sha=cc5c72baf5ae5c8238a6841f0897227be2d02826b9cf98eaf02fdefaeeb03a57 ;;
  linux-arm64) file=onnxruntime-linux-aarch64-$version.tgz; sha=c5b8b3cca31f3d643a3b313b8f42e2d0f16a4fcefd02cda343d97bd5306afaab ;;
  win-x64) file=onnxruntime-win-x64-$version.zip; sha=3958d8a44984160692c1b4ffd0de3cb48f8c39f9d34bb31e433db06e30b82195 ;;
  android-arm64 | android-x64) file=onnxruntime-android-$version.aar; sha=72683787a80d0608a3121b967d8e1239aef48286826661c953211626690540a7
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

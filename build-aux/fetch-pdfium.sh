#!/usr/bin/env bash
# Downloads the pinned pdfium build for PLATFORM into DEST, checked against its digest.
#   fetch-pdfium.sh linux-x64|linux-arm64|win-x64|android-arm64|android-x64 DEST
set -euo pipefail

platform=$1
dest=$2
# renovate: datasource=github-releases depName=bblanchon/pdfium-binaries
build=8111
case $platform in
  linux-x64) sha=bdc60126399f39fe768566a18d4be055a7b3681bc9e248e8a6656b4f6cae0aa2 ;;
  linux-arm64) sha=042de247138f96341534cccf24ed21c2550a7245b936bb4a60d2340120000f05 ;;
  win-x64) sha=3452d11031050ec65e57f180215b57b9e6d465e3fbedafa8a77ad90885c7d23e ;;
  android-arm64) sha=727cb2645ece75d28d0b72f2ec05b82858ca3bfb6712cd333ffc248807016e2c ;;
  android-x64) sha=e2c31cffe42ee3a34e2b9d6c6c254ddf5da1541ce2d82216171e9354c86f0a7c ;;
  *) echo "unknown platform $platform" >&2; exit 1 ;;
esac
[ -f "$dest/.$platform-$build" ] && exit 0
mkdir -p "$dest"
curl -sSL -o "$dest/pdfium.tgz" \
  "https://github.com/bblanchon/pdfium-binaries/releases/download/chromium%2F$build/pdfium-$platform.tgz"
echo "$sha  $dest/pdfium.tgz" | sha256sum -c - > /dev/null
tar -xzf "$dest/pdfium.tgz" -C "$dest"
rm "$dest/pdfium.tgz"
touch "$dest/.$platform-$build"

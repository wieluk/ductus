#!/usr/bin/env bash
# Downloads the pinned pdfium build for PLATFORM into DEST, checked against its digest.
#   fetch-pdfium.sh linux-x64|linux-arm64|win-x64|android-arm64|android-x64 DEST
set -euo pipefail

platform=$1
dest=$2
# renovate: datasource=github-releases depName=bblanchon/pdfium-binaries
build=8125
case $platform in
  linux-x64) sha=e1c3ce6e270cb5e0c20816b379ad9f2c1309972697c505f508de92bf5e7d6659 ;;
  linux-arm64) sha=8b8818f2ec57d1b213d952ca6d7f0ffeca9d0110487d0e20ad83ba918bc92efa ;;
  win-x64) sha=9a964261760cff52570aabf038ab346d3a5034dd8b30d0d5fe8861235d356b8b ;;
  android-arm64) sha=0495e57163429937caaee1bdd9b07924a7098dbd51d1b97645d124e349f097b4 ;;
  android-x64) sha=bd68707f29f2316c20f9d5c793b8731796a4a65408ac4d3c30bc606390354e41 ;;
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

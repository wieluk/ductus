#!/usr/bin/env bash
# Downloads the pinned pdfium build for PLATFORM into DEST, checked against its digest.
#   fetch-pdfium.sh linux-x64|linux-arm64|win-x64|android-arm64 DEST
set -euo pipefail

platform=$1
dest=$2
# renovate: datasource=github-releases depName=bblanchon/pdfium-binaries
build=8086
case $platform in
  linux-x64) sha=588577cf52dabc1a444988bac841920df54cc2f141801424de97ab04f4fbb935 ;;
  linux-arm64) sha=e7e2fe4686925618330103cb167950aca5a84bb00fd977a41b86be59dd1480a2 ;;
  win-x64) sha=1fd8af952832dbb0eb16d9249f68fe09e5f5ebf7c3dd9f6066ea2720cc28487d ;;
  android-arm64) sha=f6ea29495d64795b9af61e41768030795b45f82a21415498ee4ab2c61e135756 ;;
  *) echo "unknown platform $platform" >&2; exit 1 ;;
esac
[ -f "$dest/.$platform" ] && exit 0
mkdir -p "$dest"
curl -sSL -o "$dest/pdfium.tgz" \
  "https://github.com/bblanchon/pdfium-binaries/releases/download/chromium%2F$build/pdfium-$platform.tgz"
echo "$sha  $dest/pdfium.tgz" | sha256sum -c - > /dev/null
tar -xzf "$dest/pdfium.tgz" -C "$dest"
rm "$dest/pdfium.tgz"
touch "$dest/.$platform"

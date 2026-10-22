#!/usr/bin/env bash
# Downloads the pinned pdfium build for PLATFORM into DEST, checked against its digest.
#   fetch-pdfium.sh linux-x64|linux-arm64|win-x64|android-arm64 DEST
set -euo pipefail

platform=$1
dest=$2
# renovate: datasource=github-releases depName=bblanchon/pdfium-binaries
build=8111
case $platform in
  linux-x64) sha=1c1894a12d4d96ee01400c3874444126254180c5c7d2ab628576b67f8120320a ;;
  linux-arm64) sha=58c87a95a40a24c914a9c755fae798843f46f06adf9e5e9f2c893302d4bdb040 ;;
  win-x64) sha=51621bcc1d9be9ea448f4b84b0516862b928e3968d3fca899af1a6d6e2848d6d ;;
  android-arm64) sha=e9bab2929443d3d881166af9ec1d191c76ecdd37effb07a58743a7d7b507e149 ;;
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

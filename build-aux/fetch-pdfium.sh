#!/usr/bin/env bash
# Downloads the pinned pdfium build for PLATFORM into DEST, checked against its digest.
#   fetch-pdfium.sh linux-x64|linux-arm64|win-x64|mac-arm64|mac-x64|android-arm64 DEST
set -euo pipefail

platform=$1
dest=$2
case $platform in
  linux-x64) sha=588577cf52dabc1a444988bac841920df54cc2f141801424de97ab04f4fbb935 ;;
  linux-arm64) sha=e7e2fe4686925618330103cb167950aca5a84bb00fd977a41b86be59dd1480a2 ;;
  win-x64) sha=1fd8af952832dbb0eb16d9249f68fe09e5f5ebf7c3dd9f6066ea2720cc28487d ;;
  mac-arm64) sha=e98679e052c07edbb5a627980902abb823d4b3f35744d877bd21668bd9fc13ab ;;
  mac-x64) sha=933a85a138f6027243c56bff8676375c33ceeb767401389415ffc44d689ca85d ;;
  android-arm64) sha=f6ea29495d64795b9af61e41768030795b45f82a21415498ee4ab2c61e135756 ;;
  *) echo "unknown platform $platform" >&2; exit 1 ;;
esac
[ -f "$dest/.$platform" ] && exit 0
mkdir -p "$dest"
curl -sSL -o "$dest/pdfium.tgz" \
  "https://github.com/bblanchon/pdfium-binaries/releases/download/chromium%2F8086/pdfium-$platform.tgz"
if command -v sha256sum > /dev/null; then sum=(sha256sum); else sum=(shasum -a 256); fi
echo "$sha  $dest/pdfium.tgz" | "${sum[@]}" -c - > /dev/null
tar -xzf "$dest/pdfium.tgz" -C "$dest"
rm "$dest/pdfium.tgz"
touch "$dest/.$platform"

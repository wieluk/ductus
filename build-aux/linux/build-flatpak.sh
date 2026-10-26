#!/usr/bin/env bash
# Builds dist/Ductus-<version>-linux-x86_64.flatpak.
#   build-flatpak.sh [binary|package]   (default: both)
# Compiles in the KDE SDK (so CI can cache target/) instead of inside flatpak-builder.
# FLATPAK_GPG_KEY_ID signs the build and makes the bundle follow the update repository.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
cd "$root"
step=${1:-all}
version=${VERSION:-$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -1)}
key=${FLATPAK_GPG_KEY_ID:-}
build=target/flatpak

# CI's container has the SDK system-wide; elsewhere it goes into the user installation.
if flatpak info --system org.kde.Sdk//6.10 > /dev/null 2>&1; then
  scope=--system
else
  scope=--user
  flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
  flatpak info --user org.kde.Sdk//6.10 > /dev/null 2>&1 \
    || flatpak install --user -y --noninteractive flathub org.kde.Platform//6.10 org.kde.Sdk//6.10
fi

if [ "$step" != package ]; then
  build-aux/sdk-run.sh cargo build --release --locked -p ductus-qt
  build-aux/fetch-pdfium.sh linux-x64 target/pdfium
fi
[ "$step" = binary ] && exit 0

signing=()
[ -n "$key" ] && signing=(--gpg-sign="$key")
# shellcheck disable=SC2086 # FLATPAK_ARGS is a list of flags.
flatpak-builder "$scope" --install-deps-from=flathub --force-clean ${FLATPAK_ARGS:-} \
  ${signing[@]+"${signing[@]}"} --state-dir="$build/state" --repo="$build/repo" \
  "$build/build" build-aux/app.ductus.Ductus.yml

origin=()
if [ -n "$key" ]; then
  origin=(--repo-url="${FLATPAK_REPO_URL:-https://wieluk.github.io/ductus/repo/}"
    --gpg-keys=build-aux/linux/ductus-repo.gpg --gpg-sign="$key")
fi
mkdir -p dist
flatpak build-bundle --runtime-repo=https://dl.flathub.org/repo/flathub.flatpakrepo \
  ${origin[@]+"${origin[@]}"} "$build/repo" "dist/Ductus-$version-linux-x86_64.flatpak" app.ductus.Ductus

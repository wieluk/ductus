#!/usr/bin/env bash
# Adds a Flatpak bundle to the update repository (ostree, served by GitHub Pages).
#   publish-repo.sh BUNDLE REPO_DIR   with FLATPAK_GPG_KEY_ID imported
set -euo pipefail

bundle=$1
repo=$2
root=$(cd "$(dirname "$0")/../.." && pwd)
key=${FLATPAK_GPG_KEY_ID:?the repository is signed; set FLATPAK_GPG_KEY_ID}
repo_url=${FLATPAK_REPO_URL:-https://wieluk.github.io/ductus/repo/}

if [ ! -d "$repo/objects" ]; then
  mkdir -p "$repo"
  ostree init --mode=archive-z2 --repo="$repo"
fi
mkdir -p "$repo"/{extensions,objects,state,tmp/cache} "$repo"/refs/{heads,mirrors,remotes}
flatpak build-import-bundle "$repo" "$bundle"
# Keep a few versions so the repository doesn't grow forever.
flatpak build-update-repo --gpg-sign="$key" --title=Ductus --default-branch=master \
  --generate-static-deltas --prune --prune-depth=3 "$repo"

gpg_key=$(base64 -w0 "$root/build-aux/linux/ductus-repo.gpg")
cat > "$repo/ductus.flatpakrepo" <<REPO
[Flatpak Repo]
Title=Ductus
Url=$repo_url
Homepage=https://github.com/wieluk/ductus
Comment=Handwritten notebooks
Description=Releases of Ductus
GPGKey=$gpg_key
REPO
cat > "$repo/ductus.flatpakref" <<REF
[Flatpak Ref]
Name=app.ductus.Ductus
Branch=master
Title=Ductus
Url=$repo_url
SuggestRemoteName=ductus
RuntimeRepo=https://dl.flathub.org/repo/flathub.flatpakrepo
IsRuntime=false
GPGKey=$gpg_key
REF
echo "Published into $repo"

#!/usr/bin/env bash
# Imports FLATPAK_GPG_PRIVATE_KEY and exports its id as FLATPAK_GPG_KEY_ID for later CI steps.
set -euo pipefail

if [ -z "${FLATPAK_GPG_PRIVATE_KEY:-}" ]; then
  echo "::warning::FLATPAK_GPG_PRIVATE_KEY is not set: Flatpak installs won't update."
  exit 0
fi
printf '%s\n' "$FLATPAK_GPG_PRIVATE_KEY" | gpg --batch --import
key=$(gpg --with-colons --import-options show-only --import "$(dirname "$0")/ductus-repo.gpg" \
  | awk -F: '/^fpr:/ {print $10; exit}')
if ! gpg --list-secret-keys "$key" > /dev/null 2>&1; then
  echo "::error::FLATPAK_GPG_PRIVATE_KEY doesn't match build-aux/linux/ductus-repo.gpg."
  exit 1
fi
echo "FLATPAK_GPG_KEY_ID=$key" >> "$GITHUB_ENV"

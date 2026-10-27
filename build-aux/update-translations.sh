#!/usr/bin/env bash
# Brings crates/ductus-qt/i18n/ductus_*.ts up to date with the strings in the QML and C++ sources.
#   update-translations.sh [code...]   (new codes add a file, e.g. "it")
# Needs a Qt lupdate that reads QML (Qt's own builds do; the KDE SDK's doesn't): LUPDATE or PATH.
set -euo pipefail
shopt -s nullglob

cd "$(dirname "$0")/../crates/ductus-qt"
lupdate=${LUPDATE:-lupdate}
files=(i18n/ductus_*.ts)
for code in "$@"; do files+=("i18n/ductus_$code.ts"); done
"$lupdate" -locations none -no-obsolete qml/*.qml cpp/*.cpp cpp/*.h -ts "${files[@]}"

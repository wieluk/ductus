#!/usr/bin/env bash
# Runs tests/ui/*.uitest against the app on the offscreen platform; screenshots go to target/ui-shots.
#   ui-tests.sh [path/to/ductus] [script-glob]
set -euo pipefail

cd "$(dirname "$0")/.."
binary=${1:-target/debug/ductus}
if [ -z "${DUCTUS_PDFIUM:-}" ] && [ -d target/pdfium/lib ]; then
  export DUCTUS_PDFIUM=$PWD/target/pdfium/lib
fi
# Windows' offscreen platform finds no fonts unless pointed at them.
if [ -n "${WINDIR:-}" ]; then
  export QT_QPA_FONTDIR="$WINDIR\\Fonts"
fi
failed=0
for script in tests/ui/${2:-*}.uitest; do
  name=$(basename "$script" .uitest)
  shots=target/ui-shots/$name
  rm -rf "$shots"
  sandbox=$(mktemp -d)
  command -v cygpath > /dev/null && sandbox=$(cygpath -m "$sandbox")
  # `# env: NAME=VALUE` lines in a script set the app's environment.
  script_env=()
  while IFS= read -r line; do script_env+=("$line"); done < <(sed -n 's/^# env: //p' "$script")
  if env ${script_env[@]+"${script_env[@]}"} QT_QPA_PLATFORM=offscreen DUCTUS_UI_SCRIPT="$script" \
    DUCTUS_UI_SHOTS="$shots" DUCTUS_LIBRARY="$sandbox/Notes" DUCTUS_CONFIG_DIR="$sandbox/config" \
    DUCTUS_CACHE_DIR="$sandbox/cache" timeout 300 "$binary" 2> >(grep -v '^Fontconfig error' >&2); then
    echo "PASS $name"
  else
    echo "FAIL $name"
    failed=1
  fi
  rm -rf "$sandbox"
done
exit "$failed"

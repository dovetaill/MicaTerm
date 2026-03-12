#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT_PATH="$ROOT_DIR/build-win-x64-repro.sh"

if [[ ! -f "$SCRIPT_PATH" ]]; then
  echo "missing build script: $SCRIPT_PATH" >&2
  exit 1
fi

bash -n "$SCRIPT_PATH"

grep -F 'TARGET="${TARGET:-x86_64-pc-windows-gnu}"' "$SCRIPT_PATH" >/dev/null
grep -F 'APP_NAME="${APP_NAME:-windows-theme-repro}"' "$SCRIPT_PATH" >/dev/null
grep -F 'BIN_NAME="${BIN_NAME:-windows_theme_repro}"' "$SCRIPT_PATH" >/dev/null

HELP_OUTPUT="$("$SCRIPT_PATH" --help)"

grep -F "x86_64-pc-windows-gnu" <<<"$HELP_OUTPUT" >/dev/null
grep -F ".zip" <<<"$HELP_OUTPUT" >/dev/null

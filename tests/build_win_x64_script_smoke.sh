#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT_PATH="$ROOT_DIR/build-win-x64.sh"

if [[ ! -f "$SCRIPT_PATH" ]]; then
  echo "missing build script: $SCRIPT_PATH" >&2
  exit 1
fi

bash -n "$SCRIPT_PATH"

HELP_OUTPUT="$("$SCRIPT_PATH" --help)"

grep -F "x86_64-pc-windows-gnu" <<<"$HELP_OUTPUT" >/dev/null
grep -F "x86_64-pc-windows-msvc" <<<"$HELP_OUTPUT" >/dev/null
grep -F "dist/" <<<"$HELP_OUTPUT" >/dev/null

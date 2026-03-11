#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT_PATH="$ROOT_DIR/build-desktop.sh"

if [[ ! -f "$SCRIPT_PATH" ]]; then
  echo "missing build script: $SCRIPT_PATH" >&2
  exit 1
fi

bash -n "$SCRIPT_PATH"

HELP_OUTPUT="$("$SCRIPT_PATH" --help)"

grep -F "x86_64-unknown-linux-gnu" <<<"$HELP_OUTPUT" >/dev/null
grep -F "aarch64-unknown-linux-gnu" <<<"$HELP_OUTPUT" >/dev/null
grep -F "x86_64-apple-darwin" <<<"$HELP_OUTPUT" >/dev/null
grep -F "aarch64-apple-darwin" <<<"$HELP_OUTPUT" >/dev/null
grep -F "aarch64-pc-windows-msvc" <<<"$HELP_OUTPUT" >/dev/null
grep -F "dist/<app>-<target>-<profile>.tar.gz" <<<"$HELP_OUTPUT" >/dev/null
grep -F "dist/<app>-<target>-<profile>.zip" <<<"$HELP_OUTPUT" >/dev/null

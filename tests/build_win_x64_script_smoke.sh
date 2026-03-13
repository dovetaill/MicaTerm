#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT_PATH="$ROOT_DIR/build-win-x64.sh"
LEGACY_MSVC_WRAPPER="$ROOT_DIR/build-win-x64-femtovg-wgpu.sh"
LEGACY_GNU_WRAPPER="$ROOT_DIR/build-win-x64-gnu-femtovg-wgpu.sh"

if [[ ! -f "$SCRIPT_PATH" ]]; then
  echo "missing build script: $SCRIPT_PATH" >&2
  exit 1
fi

bash -n "$SCRIPT_PATH"

HELP_OUTPUT="$("$SCRIPT_PATH" --help)"

grep -F "./build-win-x64.sh" <<<"$HELP_OUTPUT" >/dev/null
grep -F "x86_64-pc-windows-gnu" <<<"$HELP_OUTPUT" >/dev/null
grep -F "x86_64-pc-windows-msvc" <<<"$HELP_OUTPUT" >/dev/null
grep -F "TARGET=x86_64-pc-windows-msvc ./build-win-x64.sh" <<<"$HELP_OUTPUT" >/dev/null
grep -F ".zip" <<<"$HELP_OUTPUT" >/dev/null

grep -F 'Windows wrapper target:' "$SCRIPT_PATH" >/dev/null

if grep -F 'femtovg-wgpu-experimental' "$SCRIPT_PATH" >/dev/null; then
  echo "build-win-x64.sh should now target the default mainline route only" >&2
  exit 1
fi

if [[ -e "$LEGACY_MSVC_WRAPPER" ]]; then
  echo "legacy windows msvc wrapper should have been removed" >&2
  exit 1
fi

if [[ -e "$LEGACY_GNU_WRAPPER" ]]; then
  echo "legacy windows gnu wrapper should have been removed" >&2
  exit 1
fi

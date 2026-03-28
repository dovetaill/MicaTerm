#!/usr/bin/env bash
# Confirms the Windows x64 wrapper now targets the mainline Skia packaging route.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT_PATH="$ROOT_DIR/build-win-x64.sh"
SOFTWARE_WRAPPER="$ROOT_DIR/build-win-x64-software.sh"

if [[ ! -f "$SCRIPT_PATH" ]]; then
  echo "missing build script: $SCRIPT_PATH" >&2
  exit 1
fi

bash -n "$SCRIPT_PATH"

HELP_OUTPUT="$("$SCRIPT_PATH" --help)"

grep -F "./build-win-x64.sh" <<<"$HELP_OUTPUT" >/dev/null
grep -F "Windows Skia wrapper." <<<"$HELP_OUTPUT" >/dev/null
grep -F "x86_64-pc-windows-gnu" <<<"$HELP_OUTPUT" >/dev/null
grep -F "nasm" <<<"$HELP_OUTPUT" >/dev/null
grep -F "x86_64-pc-windows-msvc" <<<"$HELP_OUTPUT" >/dev/null
grep -F "TARGET=x86_64-pc-windows-msvc ./build-win-x64.sh" <<<"$HELP_OUTPUT" >/dev/null
grep -F "winit-skia-software" <<<"$HELP_OUTPUT" >/dev/null
grep -F ".zip" <<<"$HELP_OUTPUT" >/dev/null

grep -F 'Windows wrapper target:' "$SCRIPT_PATH" >/dev/null
grep -F 'export CARGO_NO_DEFAULT_FEATURES=1' "$SCRIPT_PATH" >/dev/null
grep -F 'export CARGO_FEATURES="slint-renderer-skia"' "$SCRIPT_PATH" >/dev/null
grep -F 'export MICA_TERM_BUILD_FLAVOR="windows-mainline"' "$SCRIPT_PATH" >/dev/null
grep -F 'export MICA_TERM_PACKAGE_RENDERER="skia-software"' "$SCRIPT_PATH" >/dev/null
grep -F 'export PACKAGE_FLAVOR_SUFFIX="-skia"' "$SCRIPT_PATH" >/dev/null

if grep -F 'femtovg-wgpu-experimental' "$SCRIPT_PATH" >/dev/null; then
  echo "build-win-x64.sh should now target the default mainline route only" >&2
  exit 1
fi

[[ -f "$SOFTWARE_WRAPPER" ]]

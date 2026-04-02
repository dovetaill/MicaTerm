#!/usr/bin/env bash
# Confirms the Windows x64 software compatibility wrapper exists beside the Skia mainline wrapper.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT_PATH="$ROOT_DIR/build-win-x64-software.sh"

if [[ ! -f "$SCRIPT_PATH" ]]; then
  echo "missing build script: $SCRIPT_PATH" >&2
  exit 1
fi

bash -n "$SCRIPT_PATH"

HELP_OUTPUT="$("$SCRIPT_PATH" --help)"

grep -F "./build-win-x64-software.sh" <<<"$HELP_OUTPUT" >/dev/null
grep -F "Windows software compatibility wrapper." <<<"$HELP_OUTPUT" >/dev/null
grep -F "winit-software" <<<"$HELP_OUTPUT" >/dev/null
grep -F "packaged terminal renderer: bitmap" <<<"$HELP_OUTPUT" >/dev/null
grep -F "x86_64-pc-windows-gnu" <<<"$HELP_OUTPUT" >/dev/null
grep -F "TARGET=x86_64-pc-windows-msvc ./build-win-x64-software.sh" <<<"$HELP_OUTPUT" >/dev/null
grep -F ".zip" <<<"$HELP_OUTPUT" >/dev/null

grep -F 'export CARGO_NO_DEFAULT_FEATURES=1' "$SCRIPT_PATH" >/dev/null
grep -F 'export CARGO_FEATURES="slint-renderer-software"' "$SCRIPT_PATH" >/dev/null
grep -F 'export MICA_TERM_BUILD_FLAVOR="windows-software-compat"' "$SCRIPT_PATH" >/dev/null
grep -F 'export MICA_TERM_PACKAGE_RENDERER="software"' "$SCRIPT_PATH" >/dev/null
grep -F 'export MICA_TERM_PACKAGE_TERMINAL_RENDERER="bitmap"' "$SCRIPT_PATH" >/dev/null
grep -F 'export MICA_TERM_PACKAGE_PORTABLE=1' "$SCRIPT_PATH" >/dev/null
grep -F 'export PACKAGE_FLAVOR_SUFFIX="-software"' "$SCRIPT_PATH" >/dev/null

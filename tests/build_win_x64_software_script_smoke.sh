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
grep -F "Windows software wrapper." <<<"$HELP_OUTPUT" >/dev/null
grep -F "Transitional Linux-host Windows native-only terminal surface path." <<<"$HELP_OUTPUT" >/dev/null
grep -F "winit-software" <<<"$HELP_OUTPUT" >/dev/null
grep -F "packaged terminal renderer: native" <<<"$HELP_OUTPUT" >/dev/null
grep -F "x86_64-pc-windows-gnu" <<<"$HELP_OUTPUT" >/dev/null
grep -F "TARGET=x86_64-pc-windows-msvc ./build-win-x64-software.sh" <<<"$HELP_OUTPUT" >/dev/null
grep -F ".zip" <<<"$HELP_OUTPUT" >/dev/null

grep -F 'export CARGO_NO_DEFAULT_FEATURES=1' "$SCRIPT_PATH" >/dev/null
grep -F 'export CARGO_FEATURES="slint-renderer-software,terminal-native-renderer"' "$SCRIPT_PATH" >/dev/null
grep -F 'export MICA_TERM_BUILD_FLAVOR="windows-software-compat"' "$SCRIPT_PATH" >/dev/null
grep -F 'export MICA_TERM_PACKAGE_RENDERER="software"' "$SCRIPT_PATH" >/dev/null
grep -F 'export MICA_TERM_PACKAGE_TERMINAL_RENDERER="native"' "$SCRIPT_PATH" >/dev/null
grep -F 'export MICA_TERM_PACKAGE_PORTABLE=1' "$SCRIPT_PATH" >/dev/null
grep -F 'export PACKAGE_FLAVOR_SUFFIX="-software"' "$SCRIPT_PATH" >/dev/null
grep -F 'Transitional Linux-host Windows native-only terminal surface path.' "$SCRIPT_PATH" >/dev/null

if grep -F 'compatibility wrapper' "$SCRIPT_PATH" >/dev/null; then
  echo "build-win-x64-software.sh should stop presenting itself as a long-term compatibility wrapper" >&2
  exit 1
fi

if grep -F 'bitmap fallback-only' "$SCRIPT_PATH" >/dev/null; then
  echo "build-win-x64-software.sh should stop documenting bitmap fallback semantics in its user-facing contract" >&2
  exit 1
fi

if grep -F 'packaged terminal renderer: bitmap' "$SCRIPT_PATH" >/dev/null; then
  echo "build-win-x64-software.sh should stop advertising bitmap terminal packaging metadata" >&2
  exit 1
fi

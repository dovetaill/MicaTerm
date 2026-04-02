#!/usr/bin/env bash
# Confirms the Windows x64 software compatibility wrapper ships a native-first terminal path beside the Skia mainline wrapper.

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
grep -F "Native-first Windows software compatibility wrapper." <<<"$HELP_OUTPUT" >/dev/null
grep -F "winit-software" <<<"$HELP_OUTPUT" >/dev/null
grep -F "packaged terminal renderer: native" <<<"$HELP_OUTPUT" >/dev/null
grep -F "packaged native present path: rendering-notifier" <<<"$HELP_OUTPUT" >/dev/null
grep -F "x86_64-pc-windows-gnu" <<<"$HELP_OUTPUT" >/dev/null
grep -F "TARGET=x86_64-pc-windows-msvc ./build-win-x64-software.sh" <<<"$HELP_OUTPUT" >/dev/null
grep -F ".zip" <<<"$HELP_OUTPUT" >/dev/null

grep -F 'export CARGO_NO_DEFAULT_FEATURES=1' "$SCRIPT_PATH" >/dev/null
grep -F 'export CARGO_FEATURES="slint-renderer-software,terminal-native-renderer"' "$SCRIPT_PATH" >/dev/null
grep -F 'export MICA_TERM_BUILD_FLAVOR="windows-software-compat"' "$SCRIPT_PATH" >/dev/null
grep -F 'export MICA_TERM_PACKAGE_RENDERER="software"' "$SCRIPT_PATH" >/dev/null
grep -F 'export MICA_TERM_PACKAGE_TERMINAL_RENDERER="native"' "$SCRIPT_PATH" >/dev/null
grep -F 'export MICA_TERM_PACKAGE_NATIVE_PRESENT_PATH="rendering-notifier"' "$SCRIPT_PATH" >/dev/null
grep -F 'export MICA_TERM_PACKAGE_PORTABLE=1' "$SCRIPT_PATH" >/dev/null
grep -F 'export PACKAGE_FLAVOR_SUFFIX="-software"' "$SCRIPT_PATH" >/dev/null
grep -F 'Native-first Windows software compatibility wrapper.' "$SCRIPT_PATH" >/dev/null

if grep -F 'packaged terminal renderer: bitmap' "$SCRIPT_PATH" >/dev/null; then
  echo "build-win-x64-software.sh should stop advertising bitmap packaging metadata for the software wrapper" >&2
  exit 1
fi

if grep -F 'Stable bitmap fallback for Linux-host Windows software builds.' "$SCRIPT_PATH" >/dev/null; then
  echo "build-win-x64-software.sh should stop documenting the software package as a bitmap fallback wrapper" >&2
  exit 1
fi

if ! grep -F 'rendering-notifier' "$SCRIPT_PATH" >/dev/null; then
  echo "build-win-x64-software.sh must publish native present-path metadata for the software package" >&2
  exit 1
fi

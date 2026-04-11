#!/usr/bin/env bash
# Confirms the Windows x64 Skia wrapper advertises the supported MSVC-first contract.

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
grep -F "Windows Skia GPU wrapper." <<<"$HELP_OUTPUT" >/dev/null
grep -F "Direct3D-first native terminal renderer package with retained-native presenter default." <<<"$HELP_OUTPUT" >/dev/null
grep -F "x86_64-pc-windows-msvc" <<<"$HELP_OUTPUT" >/dev/null
grep -F "Linux + cargo-xwin + clang" <<<"$HELP_OUTPUT" >/dev/null
grep -F "winit-skia" <<<"$HELP_OUTPUT" >/dev/null
grep -F "packaged terminal subsystem: retained-native-surface" <<<"$HELP_OUTPUT" >/dev/null
grep -F "packaged native present path: event-loop" <<<"$HELP_OUTPUT" >/dev/null
grep -F "runtime fallback chain: winit-skia+d3d -> winit-skia-software -> winit-software" <<<"$HELP_OUTPUT" >/dev/null
grep -F "./build-win-x64-software.sh" <<<"$HELP_OUTPUT" >/dev/null
grep -F ".zip" <<<"$HELP_OUTPUT" >/dev/null

grep -F 'Windows wrapper target:' "$SCRIPT_PATH" >/dev/null
grep -F 'TARGET="${TARGET:-x86_64-pc-windows-msvc}"' "$SCRIPT_PATH" >/dev/null
grep -F 'export CARGO_NO_DEFAULT_FEATURES=1' "$SCRIPT_PATH" >/dev/null
grep -F 'export CARGO_FEATURES="slint-renderer-skia,terminal-native-renderer"' "$SCRIPT_PATH" >/dev/null
grep -F 'export MICA_TERM_BUILD_FLAVOR="windows-mainline"' "$SCRIPT_PATH" >/dev/null
grep -F 'export MICA_TERM_PACKAGE_RENDERER="skia"' "$SCRIPT_PATH" >/dev/null
grep -F 'packaged terminal renderer: native' <<<"$HELP_OUTPUT" >/dev/null
grep -F 'export MICA_TERM_PACKAGE_TERMINAL_RENDERER="native"' "$SCRIPT_PATH" >/dev/null
grep -F 'export MICA_TERM_PACKAGE_TERMINAL_SUBSYSTEM="retained-native-surface"' "$SCRIPT_PATH" >/dev/null
grep -F 'export MICA_TERM_PACKAGE_NATIVE_PRESENT_PATH="event-loop"' "$SCRIPT_PATH" >/dev/null
grep -F 'export MICA_TERM_PACKAGE_PORTABLE=1' "$SCRIPT_PATH" >/dev/null
grep -F 'export PACKAGE_FLAVOR_SUFFIX="-skia"' "$SCRIPT_PATH" >/dev/null
grep -F 'rust-skia does not ship Windows GNU Skia binaries' "$SCRIPT_PATH" >/dev/null

if ! grep -F 'terminal-native-renderer' "$SCRIPT_PATH" >/dev/null; then
  echo "build-win-x64.sh must compile terminal-native-renderer for the Windows mainline package" >&2
  exit 1
fi

if grep -F 'femtovg-wgpu-experimental' "$SCRIPT_PATH" >/dev/null; then
  echo "build-win-x64.sh should now target the default mainline route only" >&2
  exit 1
fi

[[ -f "$SOFTWARE_WRAPPER" ]]

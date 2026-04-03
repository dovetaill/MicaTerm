#!/usr/bin/env bash
# Thin wrapper around the Windows x64 Skia GPU mainline package used by CI and manual packaging.
# Preferred native-only terminal surface path for Windows mainline shipping with Direct3D-first selection.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

usage() {
  cat <<'EOF'
Usage: ./build-win-x64.sh [--help]

Windows Skia GPU wrapper.
Direct3D-first native-only terminal surface path.

Default target:
  x86_64-pc-windows-msvc
  supported hosts: Windows MSVC shell/Git Bash or Linux + cargo-xwin + clang
  packaged renderer: winit-skia
  packaged terminal renderer: native
  packaged native present path: rendering-notifier
  runtime fallback chain: winit-skia+d3d -> winit-skia-software -> winit-software

Linux-host Windows GNU package:
  ./build-win-x64-software.sh

Outputs:
  dist/mica-term-x86_64-pc-windows-msvc-release-skia.zip
EOF
}

if [[ "${1:-}" == "--help" ]]; then
  usage
  exit 0
fi

TARGET="${TARGET:-x86_64-pc-windows-msvc}"

if [[ "$TARGET" == "x86_64-pc-windows-gnu" ]]; then
  cat >&2 <<'EOF'
error: rust-skia does not ship Windows GNU Skia binaries for x86_64-pc-windows-gnu.
Use ./build-win-x64-software.sh for GNU/software compatibility packages, or run TARGET=x86_64-pc-windows-msvc ./build-win-x64.sh for the MSVC GPU package.
EOF
  exit 1
fi

echo "==> Windows wrapper target: $TARGET"
export TARGET
export CARGO_NO_DEFAULT_FEATURES=1
export CARGO_FEATURES="slint-renderer-skia,terminal-native-renderer"
export MICA_TERM_BUILD_FLAVOR="windows-mainline"
export MICA_TERM_PACKAGE_RENDERER="skia"
export MICA_TERM_PACKAGE_TERMINAL_RENDERER="native"
export MICA_TERM_PACKAGE_NATIVE_PRESENT_PATH="rendering-notifier"
export MICA_TERM_PACKAGE_PORTABLE=1
export PACKAGE_FLAVOR_SUFFIX="-skia"

exec "$ROOT_DIR/build-desktop.sh" "$@"

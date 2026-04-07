#!/usr/bin/env bash
# Thin wrapper around the Windows x64 Skia GPU mainline package used by CI and manual packaging.
# Packaged terminal subsystem: scene-image.
# Keep packaged Skia builds on the scene-owned image presenter until the retained
# same-HWND native surface path is verified in real packaged runs.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

usage() {
  cat <<'EOF'
Usage: ./build-win-x64.sh [--help]

Windows Skia GPU wrapper.
Direct3D-first native terminal renderer package with scene-image presenter default.

Default target:
  x86_64-pc-windows-msvc
  supported hosts: Windows MSVC shell/Git Bash or Linux + cargo-xwin + clang
  packaged renderer: winit-skia
  packaged terminal renderer: native
  packaged terminal subsystem: scene-image
  packaged native present path: event-loop
  expected primary text path: directwrite-d2d
  compatibility text fallback path: bitmap-mask-compat
  verification matrix: DPI 100% | 125% | 150%; font sizes 12px | 13px | 14px | 15px
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
export MICA_TERM_PACKAGE_TERMINAL_SUBSYSTEM="scene-image"
export MICA_TERM_PACKAGE_NATIVE_PRESENT_PATH="event-loop"
export MICA_TERM_EXPECTED_TEXT_RENDERER_PATH="directwrite-d2d"
export MICA_TERM_TEXT_RENDERER_FALLBACK_PATH="bitmap-mask-compat"
export MICA_TERM_VERIFICATION_DPI_SCALE_MATRIX="100,125,150"
export MICA_TERM_VERIFICATION_FONT_PX_MATRIX="12,13,14,15"
export MICA_TERM_PACKAGE_PORTABLE=1
export PACKAGE_FLAVOR_SUFFIX="-skia"

echo "==> Text renderer path: ${MICA_TERM_EXPECTED_TEXT_RENDERER_PATH} -> ${MICA_TERM_TEXT_RENDERER_FALLBACK_PATH}"
echo "==> Verification matrix: DPI ${MICA_TERM_VERIFICATION_DPI_SCALE_MATRIX}; font px ${MICA_TERM_VERIFICATION_FONT_PX_MATRIX}"

exec "$ROOT_DIR/build-desktop.sh" "$@"

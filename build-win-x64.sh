#!/usr/bin/env bash
# Thin wrapper around the Windows x64 Skia mainline package used by CI and manual packaging.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

usage() {
  cat <<'EOF'
Usage: ./build-win-x64.sh [--help]

Windows Skia wrapper.

Default target:
  x86_64-pc-windows-gnu
  requires Linux host tools: x86_64-w64-mingw32-gcc and nasm
  packaged renderer: winit-skia-software

Override example:
  TARGET=x86_64-pc-windows-msvc ./build-win-x64.sh

Outputs:
  dist/mica-term-x86_64-pc-windows-gnu-release-skia.zip
  dist/mica-term-x86_64-pc-windows-msvc-release-skia.zip
EOF
}

if [[ "${1:-}" == "--help" ]]; then
  usage
  exit 0
fi

TARGET="${TARGET:-x86_64-pc-windows-gnu}"
echo "==> Windows wrapper target: $TARGET"
export TARGET
export CARGO_NO_DEFAULT_FEATURES=1
export CARGO_FEATURES="slint-renderer-skia"
export MICA_TERM_BUILD_FLAVOR="windows-mainline"
export MICA_TERM_PACKAGE_RENDERER="skia-software"
export PACKAGE_FLAVOR_SUFFIX="-skia"

exec "$ROOT_DIR/build-desktop.sh" "$@"

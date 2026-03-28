#!/usr/bin/env bash
# Thin wrapper around the Windows x64 software compatibility package used by CI and manual packaging.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

usage() {
  cat <<'EOF'
Usage: ./build-win-x64-software.sh [--help]

Windows software compatibility wrapper.

Default target:
  x86_64-pc-windows-gnu
  requires Linux host tools: x86_64-w64-mingw32-gcc and nasm
  packaged renderer: winit-software

Override example:
  TARGET=x86_64-pc-windows-msvc ./build-win-x64-software.sh

Outputs:
  dist/mica-term-x86_64-pc-windows-gnu-release-software.zip
  dist/mica-term-x86_64-pc-windows-msvc-release-software.zip
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
export CARGO_FEATURES="slint-renderer-software"
export MICA_TERM_BUILD_FLAVOR="windows-software-compat"
export MICA_TERM_PACKAGE_RENDERER="software"
export PACKAGE_FLAVOR_SUFFIX="-software"

exec "$ROOT_DIR/build-desktop.sh" "$@"

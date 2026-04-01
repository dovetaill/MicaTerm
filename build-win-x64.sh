#!/usr/bin/env bash
# Thin wrapper around the Windows x64 Skia mainline package used by CI and manual packaging.
# Preferred native-first terminal renderer path for Windows mainline shipping.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

usage() {
  cat <<'EOF'
Usage: ./build-win-x64.sh [--help]

Windows Skia wrapper.
Native-first terminal renderer path.

Default target:
  x86_64-pc-windows-msvc
  requires a Windows MSVC shell or Git Bash environment
  packaged renderer: winit-skia-software
  packaged terminal renderer: native

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
Use ./build-win-x64-software.sh for Linux-host Windows packages, or run TARGET=x86_64-pc-windows-msvc ./build-win-x64.sh from a Windows MSVC shell.
EOF
  exit 1
fi

case "$(uname -s)" in
  MINGW*|MSYS*|CYGWIN*)
    ;;
  *)
    cat >&2 <<EOF
error: target '$TARGET' must be built from a Windows MSVC shell or Git Bash environment.
EOF
    exit 1
    ;;
esac

echo "==> Windows wrapper target: $TARGET"
export TARGET
export CARGO_NO_DEFAULT_FEATURES=1
export CARGO_FEATURES="slint-renderer-skia,terminal-native-renderer"
export MICA_TERM_BUILD_FLAVOR="windows-mainline"
export MICA_TERM_PACKAGE_RENDERER="skia-software"
export MICA_TERM_PACKAGE_TERMINAL_RENDERER="native"
export MICA_TERM_PACKAGE_PORTABLE=1
export PACKAGE_FLAVOR_SUFFIX="-skia"

exec "$ROOT_DIR/build-desktop.sh" "$@"

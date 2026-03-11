#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT_PATH="$ROOT_DIR/build-win-x64-skia.sh"
BUILD_SCRIPT_PATH="$ROOT_DIR/build-desktop.sh"
CARGO_TOML="$ROOT_DIR/Cargo.toml"
README_PATH="$ROOT_DIR/readme.md"

if [[ ! -f "$SCRIPT_PATH" ]]; then
  echo "missing build script: $SCRIPT_PATH" >&2
  exit 1
fi

bash -n "$SCRIPT_PATH"

grep -F 'windows-skia-experimental' "$CARGO_TOML" >/dev/null
grep -F 'slint/renderer-skia' "$CARGO_TOML" >/dev/null
grep -F 'CARGO_FEATURES' "$BUILD_SCRIPT_PATH" >/dev/null
grep -F 'CARGO_NO_DEFAULT_FEATURES' "$BUILD_SCRIPT_PATH" >/dev/null
grep -F 'build-win-x64-skia.sh' "$README_PATH" >/dev/null
grep -F 'windows-skia-experimental' "$SCRIPT_PATH" >/dev/null
grep -F 'build-desktop.sh' "$SCRIPT_PATH" >/dev/null
grep -F 'x86_64-pc-windows-msvc' "$SCRIPT_PATH" >/dev/null
grep -F 'Windows MSVC shell' "$README_PATH" >/dev/null

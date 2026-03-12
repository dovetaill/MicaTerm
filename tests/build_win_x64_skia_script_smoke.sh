#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT_PATH="$ROOT_DIR/build-win-x64-skia.sh"
CARGO_TOML="$ROOT_DIR/Cargo.toml"
README_PATH="$ROOT_DIR/readme.md"
VERIFICATION_PATH="$ROOT_DIR/verification.md"

if [[ -f "$SCRIPT_PATH" ]]; then
  echo "unexpected skia build script remains: $SCRIPT_PATH" >&2
  exit 1
fi

for target in "$CARGO_TOML" "$README_PATH" "$VERIFICATION_PATH"; do
  if rg -n 'windows-skia-experimental|slint/renderer-skia|build-win-x64-skia\.sh|winit-skia-software|Skia Experimental' "$target" >/dev/null; then
    echo "unexpected skia experimental reference remains in $target" >&2
    exit 1
  fi
done

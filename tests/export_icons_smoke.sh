#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT_PATH="$ROOT_DIR/scripts/export-icons.sh"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

[[ -f "$SCRIPT_PATH" ]] || {
  echo "missing export script: $SCRIPT_PATH" >&2
  exit 1
}

bash -n "$SCRIPT_PATH"
OUTPUT_DIR="$TMP_DIR/out" "$SCRIPT_PATH"

for size in 16 20 24 32 40 48 64 128 256; do
  [[ -f "$TMP_DIR/out/png/mica-term-$size.png" ]] || {
    echo "missing png size: $size" >&2
    exit 1
  }
done

[[ -f "$TMP_DIR/out/windows/mica-term.ico" ]] || {
  echo "missing ico output" >&2
  exit 1
}

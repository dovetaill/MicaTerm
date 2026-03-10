#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SOURCE_DIR="${SOURCE_DIR:-$ROOT_DIR/assets/icons}"
OUTPUT_DIR="${OUTPUT_DIR:-$ROOT_DIR/assets/icons}"

APP_SVG="$SOURCE_DIR/mica-term-app.svg"
TASKBAR_SVG="$SOURCE_DIR/mica-term-taskbar.svg"
PNG_DIR="$OUTPUT_DIR/png"
WINDOWS_DIR="$OUTPUT_DIR/windows"

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "missing required command: $1" >&2
    exit 1
  }
}

require_cmd rsvg-convert
require_cmd magick

[[ -f "$APP_SVG" ]] || {
  echo "missing app svg: $APP_SVG" >&2
  exit 1
}

[[ -f "$TASKBAR_SVG" ]] || {
  echo "missing taskbar svg: $TASKBAR_SVG" >&2
  exit 1
}

mkdir -p "$PNG_DIR" "$WINDOWS_DIR"

for size in 16 20 24 32 40 48 64 128 256; do
  input_svg="$APP_SVG"
  if [[ "$size" -le 32 ]]; then
    input_svg="$TASKBAR_SVG"
  fi

  rsvg-convert -w "$size" -h "$size" "$input_svg" -o "$PNG_DIR/mica-term-$size.png"
done

magick \
  "$PNG_DIR/mica-term-16.png" \
  "$PNG_DIR/mica-term-20.png" \
  "$PNG_DIR/mica-term-24.png" \
  "$PNG_DIR/mica-term-32.png" \
  "$PNG_DIR/mica-term-40.png" \
  "$PNG_DIR/mica-term-48.png" \
  "$PNG_DIR/mica-term-64.png" \
  "$PNG_DIR/mica-term-128.png" \
  "$PNG_DIR/mica-term-256.png" \
  "$WINDOWS_DIR/mica-term.ico"

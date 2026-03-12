#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_FILE="$ROOT_DIR/ui/app-window.slint"
WINDOWING_FILE="$ROOT_DIR/src/app/windowing.rs"
GRIPS_FILE="$ROOT_DIR/ui/components/window-resize-grips.slint"
BOOTSTRAP_FILE="$ROOT_DIR/src/app/bootstrap.rs"

grep -F 'min-width:' "$APP_FILE" >/dev/null
grep -F 'min-height:' "$APP_FILE" >/dev/null
grep -F 'min_window_width' "$WINDOWING_FILE" >/dev/null
grep -F 'min_window_height' "$WINDOWING_FILE" >/dev/null
grep -F 'resize-requested(string)' "$GRIPS_FILE" >/dev/null
grep -F 'drag-resize-requested(string)' "$APP_FILE" >/dev/null
grep -F 'drag_resize_window' "$WINDOWING_FILE" >/dev/null
grep -F 'on_drag_resize_requested' "$BOOTSTRAP_FILE" >/dev/null

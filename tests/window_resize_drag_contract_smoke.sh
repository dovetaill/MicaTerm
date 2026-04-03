#!/usr/bin/env bash
# Verifies resize-drag callbacks remain wired between Slint and Rust.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_FILE="$ROOT_DIR/ui/app-window.slint"
WINDOWING_FILE="$ROOT_DIR/src/app/windowing.rs"
GRIPS_FILE="$ROOT_DIR/ui/components/window-resize-grips.slint"
WINDOWING_BOOTSTRAP_FILE="$ROOT_DIR/src/app/bootstrap/windowing.rs"

grep -F 'min-width:' "$APP_FILE" >/dev/null
grep -F 'min-height:' "$APP_FILE" >/dev/null
grep -F 'min_window_width' "$WINDOWING_FILE" >/dev/null
grep -F 'min_window_height' "$WINDOWING_FILE" >/dev/null
grep -F 'resize-requested(string)' "$GRIPS_FILE" >/dev/null
grep -F 'drag-resize-requested(string)' "$APP_FILE" >/dev/null
grep -F 'drag_resize_window' "$WINDOWING_FILE" >/dev/null
grep -F 'on_drag_resize_requested' "$WINDOWING_BOOTSTRAP_FILE" >/dev/null
grep -F 'on_blocking_modal_drag_requested' "$WINDOWING_BOOTSTRAP_FILE" >/dev/null
grep -F 'on_blocking_modal_drag_moved' "$WINDOWING_BOOTSTRAP_FILE" >/dev/null
grep -F 'on_blocking_modal_drag_ended' "$WINDOWING_BOOTSTRAP_FILE" >/dev/null
grep -F 'on_blocking_modal_focus_restore_requested' "$WINDOWING_BOOTSTRAP_FILE" >/dev/null

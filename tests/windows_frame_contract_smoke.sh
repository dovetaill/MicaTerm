#!/usr/bin/env bash
# Guards Windows-specific frame interop hooks and exported maximize geometry.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FRAME_FILE="$ROOT_DIR/src/app/windows_frame.rs"
BOOTSTRAP_FILE="$ROOT_DIR/src/app/bootstrap.rs"
TITLEBAR="$ROOT_DIR/ui/shell/titlebar.slint"
APP_FILE="$ROOT_DIR/ui/app-window.slint"

grep -F 'WM_NCHITTEST' "$FRAME_FILE" >/dev/null
grep -F 'HTMAXBUTTON' "$FRAME_FILE" >/dev/null
grep -F 'SetWindowSubclass' "$FRAME_FILE" >/dev/null
grep -F 'DefSubclassProc' "$FRAME_FILE" >/dev/null
grep -F 'RemoveWindowSubclass' "$FRAME_FILE" >/dev/null
grep -F 'GetPropW' "$FRAME_FILE" >/dev/null
grep -F 'SetPropW' "$FRAME_FILE" >/dev/null
grep -F 'RemovePropW' "$FRAME_FILE" >/dev/null
grep -F 'WM_NCDESTROY' "$FRAME_FILE" >/dev/null
grep -F 'point_hits_outer_resize_band' "$FRAME_FILE" >/dev/null
grep -F 'reserved resize band' "$FRAME_FILE" >/dev/null
grep -F 'native_surface_text_antialias_mode' "$FRAME_FILE" >/dev/null
grep -F 'native_surface_render_target_alpha_mode' "$FRAME_FILE" >/dev/null
grep -F 'native_surface_font_chain' "$FRAME_FILE" >/dev/null
grep -F 'native_surface_glyph_bounds_trace' "$FRAME_FILE" >/dev/null
grep -F 'trace_workspace_native_terminal_diagnostics' "$BOOTSTRAP_FILE" >/dev/null
grep -F 'layout-maximize-button-x' "$TITLEBAR" >/dev/null
grep -F 'layout-titlebar-maximize-button-x' "$APP_FILE" >/dev/null

if grep -F 'GetWindowSubclass' "$FRAME_FILE" >/dev/null; then
    exit 1
fi

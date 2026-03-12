#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_FILE="$ROOT_DIR/ui/app-window.slint"
WINDOWING_FILE="$ROOT_DIR/src/app/windowing.rs"
BOOTSTRAP_FILE="$ROOT_DIR/src/app/bootstrap.rs"

grep -F 'resize-border-width: 6px;' "$APP_FILE" >/dev/null
grep -F 'layout-resize-border-width' "$APP_FILE" >/dev/null
grep -F 'supports_true_window_state_tracking: true' "$WINDOWING_FILE" >/dev/null
grep -F 'supports_native_frame_adapter: true' "$WINDOWING_FILE" >/dev/null
grep -F 'WindowRecoveryController' "$BOOTSTRAP_FILE" >/dev/null
grep -F 'query_true_window_placement(winit_window)' "$BOOTSTRAP_FILE" >/dev/null
grep -F 'state.set_window_placement(next);' "$BOOTSTRAP_FILE" >/dev/null
grep -F 'notify_windows_window_recovery_transition_with_snapshot' "$BOOTSTRAP_FILE" >/dev/null

#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
FILE="$ROOT_DIR/src/app/window_effects.rs"
BOOTSTRAP_FILE="$ROOT_DIR/src/app/bootstrap.rs"
APP_WINDOW_FILE="$ROOT_DIR/ui/app-window.slint"
MAIN_FILE="$ROOT_DIR/src/main.rs"

grep -F 'window.set_theme(Some(' "$FILE" >/dev/null
grep -F 'window.request_redraw();' "$FILE" >/dev/null
grep -F 'window_vibrancy::apply_tabbed' "$FILE" >/dev/null
grep -F '#[cfg(target_os = "windows")]' "$FILE" >/dev/null
grep -F 'NoopWindowEffects' "$FILE" >/dev/null
grep -F 'window.window().request_redraw();' "$BOOTSTRAP_FILE" >/dev/null
grep -F 'backdrop_error' "$FILE" >/dev/null
grep -F 'backdrop_error' "$BOOTSTRAP_FILE" >/dev/null
grep -F 'on_winit_window_event' "$BOOTSTRAP_FILE" >/dev/null
grep -F 'WindowEvent::Moved' "$BOOTSTRAP_FILE" >/dev/null
grep -F 'preferred-width: 1440px;' "$APP_WINDOW_FILE" >/dev/null

for unexpected in \
  'winit-skia-software' \
  'SLINT_BACKEND' \
  'windows-skia-experimental' \
  'WindowRecoveryController' \
  'WindowRecoveryAction' \
  'current_window_visibility_snapshot' \
  'set_render_revision' \
  'set_experimental_recovery_mask_visible' \
  'set_experimental_recovery_mask_dark' \
  'slint::Timer::single_shot' \
  'arm_experimental_window_recovery' \
  'maybe_apply_experimental_window_recovery' \
  'render-revision' \
  'experimental-recovery-mask'
do
  if grep -F "$unexpected" "$MAIN_FILE" >/dev/null \
    || grep -F "$unexpected" "$BOOTSTRAP_FILE" >/dev/null \
    || grep -F "$unexpected" "$APP_WINDOW_FILE" >/dev/null
  then
    echo "unexpected recovery or legacy renderer contract remains: $unexpected" >&2
    exit 1
  fi
done

if [[ -e "$ROOT_DIR/src/app/window_recovery.rs" ]]; then
  echo "unexpected recovery module remains: $ROOT_DIR/src/app/window_recovery.rs" >&2
  exit 1
fi

if grep -F 'RedrawWindow(' "$FILE" >/dev/null; then
  echo "unexpected legacy RedrawWindow path remains in $FILE" >&2
  exit 1
fi

if grep -F 'RDW_UPDATENOW' "$FILE" >/dev/null; then
  echo "unexpected legacy RDW_UPDATENOW path remains in $FILE" >&2
  exit 1
fi

for unexpected in \
  'theme toggle requested' \
  'syncing theme state to slint window' \
  'requested slint redraw after theme change' \
  'native window appearance sync finished' \
  'marked offscreen theme recovery state' \
  'queued offscreen theme recovery size nudge' \
  'restoring window size after offscreen theme recovery nudge' \
  'bumped slint render revision after offscreen theme recovery'
do
  if grep -F "$unexpected" "$BOOTSTRAP_FILE" >/dev/null; then
    echo "unexpected debug theme diagnostic remains in $BOOTSTRAP_FILE: $unexpected" >&2
    exit 1
  fi
done

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
  'windows-skia-experimental'
do
  if grep -F "$unexpected" "$MAIN_FILE" >/dev/null; then
    echo "unexpected experimental renderer contract remains in $MAIN_FILE: $unexpected" >&2
    exit 1
  fi
done

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

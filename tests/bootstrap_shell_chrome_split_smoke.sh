#!/usr/bin/env bash
# Guards assets/keychain, shell chrome, and windowing binder extraction from bootstrap root.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BOOTSTRAP_ROOT="$ROOT_DIR/src/app/bootstrap.rs"
ASSETS_KEYCHAIN_MODULE="$ROOT_DIR/src/app/bootstrap/assets_keychain.rs"
SHELL_CHROME_MODULE="$ROOT_DIR/src/app/bootstrap/shell_chrome.rs"
WINDOWING_MODULE="$ROOT_DIR/src/app/bootstrap/windowing.rs"

for file in \
  "$BOOTSTRAP_ROOT" \
  "$ASSETS_KEYCHAIN_MODULE" \
  "$SHELL_CHROME_MODULE" \
  "$WINDOWING_MODULE"
do
  [[ -f "$file" ]] || {
    echo "missing $file" >&2
    exit 1
  }
done

grep -F 'mod assets_keychain;' "$BOOTSTRAP_ROOT" >/dev/null
grep -F 'mod shell_chrome;' "$BOOTSTRAP_ROOT" >/dev/null
grep -F 'mod windowing;' "$BOOTSTRAP_ROOT" >/dev/null

grep -F 'fn sync_asset_modal_state(' "$ASSETS_KEYCHAIN_MODULE" >/dev/null
grep -F 'fn bind_assets_keychain_callbacks(' "$ASSETS_KEYCHAIN_MODULE" >/dev/null
grep -F 'window.on_asset_selected(' "$ASSETS_KEYCHAIN_MODULE" >/dev/null
grep -F 'window.on_toggle_assets_search_requested(' "$ASSETS_KEYCHAIN_MODULE" >/dev/null

grep -F 'fn sync_top_status_bar_state(' "$SHELL_CHROME_MODULE" >/dev/null
grep -F 'fn bind_shell_chrome_callbacks(' "$SHELL_CHROME_MODULE" >/dev/null
grep -F 'window.on_toggle_global_menu_requested(' "$SHELL_CHROME_MODULE" >/dev/null
grep -F 'window.on_open_settings_panel_requested(' "$SHELL_CHROME_MODULE" >/dev/null

grep -F 'fn bind_windows_window_state_tracking(' "$WINDOWING_MODULE" >/dev/null
grep -F 'fn bind_windowing_callbacks(' "$WINDOWING_MODULE" >/dev/null
grep -F 'window.on_drag_resize_requested(' "$WINDOWING_MODULE" >/dev/null

if grep -F 'fn sync_asset_modal_state(' "$BOOTSTRAP_ROOT" >/dev/null; then
  echo "sync_asset_modal_state must move out of src/app/bootstrap.rs" >&2
  exit 1
fi

if grep -F 'window.on_asset_selected(' "$BOOTSTRAP_ROOT" >/dev/null; then
  echo "asset tree callbacks must move out of src/app/bootstrap.rs" >&2
  exit 1
fi

if grep -F 'fn sync_top_status_bar_state(' "$BOOTSTRAP_ROOT" >/dev/null; then
  echo "sync_top_status_bar_state must move out of src/app/bootstrap.rs" >&2
  exit 1
fi

if grep -F 'window.on_toggle_global_menu_requested(' "$BOOTSTRAP_ROOT" >/dev/null; then
  echo "shell chrome callbacks must move out of src/app/bootstrap.rs" >&2
  exit 1
fi

if grep -F 'fn bind_windows_window_state_tracking(' "$BOOTSTRAP_ROOT" >/dev/null; then
  echo "bind_windows_window_state_tracking must move out of src/app/bootstrap.rs" >&2
  exit 1
fi

if grep -F 'window.on_drag_resize_requested(' "$BOOTSTRAP_ROOT" >/dev/null; then
  echo "window drag/resize callbacks must move out of src/app/bootstrap.rs" >&2
  exit 1
fi

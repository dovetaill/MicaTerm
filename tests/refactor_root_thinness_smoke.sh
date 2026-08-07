#!/usr/bin/env bash
# Guards the three root facade files from regressing into heavy domain impl owners.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

BOOTSTRAP_ROOT="$ROOT_DIR/src/app/bootstrap.rs"
BOOTSTRAP_ASSETS_KEYCHAIN="$ROOT_DIR/src/app/bootstrap/assets_keychain.rs"
VIEW_MODEL_ROOT="$ROOT_DIR/src/shell/view_model.rs"
VIEW_MODEL_ASSETS="$ROOT_DIR/src/shell/view_model/assets.rs"
RUNTIME_ROOT="$ROOT_DIR/src/app/ssh/runtime.rs"
RUNTIME_TRANSPORT="$ROOT_DIR/src/app/ssh/runtime/transport.rs"
RUNTIME_TERMINAL="$ROOT_DIR/src/app/ssh/runtime/terminal.rs"

for file in \
  "$BOOTSTRAP_ROOT" \
  "$BOOTSTRAP_ASSETS_KEYCHAIN" \
  "$VIEW_MODEL_ROOT" \
  "$VIEW_MODEL_ASSETS" \
  "$RUNTIME_ROOT" \
  "$RUNTIME_TRANSPORT" \
  "$RUNTIME_TERMINAL"
do
  [[ -f "$file" ]] || {
    echo "missing file: $file" >&2
    exit 1
  }
done

for moved_symbol in \
  'fn context_menu_roots_for(state: &ShellViewModel) -> Vec<ContextMenuActionNode>' \
  'fn context_menu_primary_items_for(state: &ShellViewModel) -> Vec<AssetsContextMenuItem>' \
  'fn context_menu_secondary_items_for(state: &ShellViewModel) -> Vec<AssetsContextMenuItem>' \
  'fn context_menu_tertiary_items_for(state: &ShellViewModel) -> Vec<AssetsContextMenuItem>' \
  'fn context_menu_hover_path_for(' \
  'fn context_menu_column_rects_for(state: &ShellViewModel) -> [Option<Rect>; 3]' \
  'pub(super) fn update_context_menu_placement(window: &AppWindow, state: &mut ShellViewModel)'
do
  grep -F "$moved_symbol" "$BOOTSTRAP_ASSETS_KEYCHAIN" >/dev/null || {
    echo "bootstrap context menu helper must live in assets_keychain.rs: $moved_symbol" >&2
    exit 1
  }
done

for stale_symbol in \
  'fn context_menu_roots_for(state: &ShellViewModel) -> Vec<ContextMenuActionNode>' \
  'fn context_menu_primary_items_for(state: &ShellViewModel) -> Vec<AssetsContextMenuItem>' \
  'fn context_menu_secondary_items_for(state: &ShellViewModel) -> Vec<AssetsContextMenuItem>' \
  'fn context_menu_tertiary_items_for(state: &ShellViewModel) -> Vec<AssetsContextMenuItem>' \
  'fn context_menu_hover_path_for(' \
  'fn context_menu_column_rects_for(state: &ShellViewModel) -> [Option<Rect>; 3]' \
  'fn update_context_menu_placement(window: &AppWindow, state: &mut ShellViewModel)'
do
  if grep -F "$stale_symbol" "$BOOTSTRAP_ROOT" >/dev/null; then
    echo "bootstrap root must stay thin and drop moved helper: $stale_symbol" >&2
    exit 1
  fi
done

for moved_symbol in \
  'pub fn open_rename_asset_modal(&mut self, asset_id: String)' \
  'pub fn open_sftp_rename_entry_modal(&mut self, entry_id: String)' \
  'pub fn update_rename_asset_modal_name(&mut self, value: String)' \
  'pub fn open_delete_asset_confirm(&mut self, asset_id: String)' \
  'pub fn open_sftp_delete_confirm(&mut self, entry_ids: Vec<String>)' \
  'pub fn confirm_delete_asset(&mut self) -> bool'
do
  grep -F "$moved_symbol" "$VIEW_MODEL_ASSETS" >/dev/null || {
    echo "view model asset modal flow must live in assets.rs: $moved_symbol" >&2
    exit 1
  }
done

for stale_symbol in \
  'pub fn open_rename_asset_modal(&mut self, asset_id: String)' \
  'pub fn open_sftp_rename_entry_modal(&mut self, entry_id: String)' \
  'pub fn update_rename_asset_modal_name(&mut self, value: String)' \
  'pub fn open_delete_asset_confirm(&mut self, asset_id: String)' \
  'pub fn open_sftp_delete_confirm(&mut self, entry_ids: Vec<String>)' \
  'pub fn confirm_delete_asset(&mut self) -> bool'
do
  if grep -F "$stale_symbol" "$VIEW_MODEL_ROOT" >/dev/null; then
    echo "view model root must stay thin and drop moved helper: $stale_symbol" >&2
    exit 1
  fi
done

grep -F 'pub(super) fn ssh_client_config() -> client::Config' "$RUNTIME_TRANSPORT" >/dev/null || {
  echo "runtime transport config must live in transport.rs" >&2
  exit 1
}

for moved_symbol in \
  'pub fn negotiated_terminal_environment() -> [(&'\''static str, &'\''static str); 3]' \
  'pub(super) async fn await_channel_success(' \
  'pub(super) async fn negotiate_terminal_environment('
do
  grep -F "$moved_symbol" "$RUNTIME_TERMINAL" >/dev/null || {
    echo "runtime terminal helper must live in terminal.rs: $moved_symbol" >&2
    exit 1
  }
done

for stale_symbol in \
  'fn ssh_client_config() -> client::Config' \
  'pub fn negotiated_terminal_environment() -> [(&'\''static str, &'\''static str); 3]' \
  'async fn await_channel_success(' \
  'async fn negotiate_terminal_environment('
do
  if grep -F "$stale_symbol" "$RUNTIME_ROOT" >/dev/null; then
    echo "runtime root must stay thin and drop moved helper: $stale_symbol" >&2
    exit 1
  fi
done

#!/usr/bin/env bash
# Guards heavy ShellViewModel flows being extracted into dedicated helper modules.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VIEW_MODEL_ROOT="$ROOT_DIR/src/shell/view_model.rs"
ASSET_MODAL_EXECUTOR="$ROOT_DIR/src/shell/view_model/asset_modal_executor.rs"
SSH_MODAL_MODULE="$ROOT_DIR/src/shell/view_model/ssh_modal.rs"
CONTEXT_MENU_DISPATCHER="$ROOT_DIR/src/shell/view_model/context_menu_dispatcher.rs"

[[ -f "$VIEW_MODEL_ROOT" ]] || {
  echo "missing src/shell/view_model.rs" >&2
  exit 1
}

for file in \
  "$ASSET_MODAL_EXECUTOR" \
  "$SSH_MODAL_MODULE" \
  "$CONTEXT_MENU_DISPATCHER"
do
  [[ -f "$file" ]] || {
    echo "missing $file" >&2
    exit 1
  }
done

grep -F 'mod asset_modal_executor;' "$VIEW_MODEL_ROOT" >/dev/null
grep -F 'mod ssh_modal;' "$VIEW_MODEL_ROOT" >/dev/null
grep -F 'mod context_menu_dispatcher;' "$VIEW_MODEL_ROOT" >/dev/null

grep -F 'pub fn can_confirm_asset_modal(&self) -> bool {' "$ASSET_MODAL_EXECUTOR" >/dev/null
grep -F 'pub fn confirm_asset_modal(&mut self) -> bool {' "$ASSET_MODAL_EXECUTOR" >/dev/null
grep -F 'fn build_saved_ssh_connection_spec(' "$ASSET_MODAL_EXECUTOR" >/dev/null
grep -F 'fn build_draft_proxy_spec(' "$ASSET_MODAL_EXECUTOR" >/dev/null

grep -F 'pub fn update_ssh_modal_field(&mut self, field: &str, value: String) {' "$SSH_MODAL_MODULE" >/dev/null
grep -F 'pub fn begin_ssh_modal_action(&mut self, action_id: &str) -> bool {' "$SSH_MODAL_MODULE" >/dev/null

grep -F 'pub fn handle_context_menu_leaf_action(&mut self, action_id: &str) {' "$CONTEXT_MENU_DISPATCHER" >/dev/null
grep -F 'fn handle_sftp_context_menu_leaf_action(&mut self, action_id: &str) {' "$CONTEXT_MENU_DISPATCHER" >/dev/null
grep -F 'fn context_menu_roots(&self) -> Vec<ContextMenuActionNode> {' "$CONTEXT_MENU_DISPATCHER" >/dev/null

if grep -F 'pub fn can_confirm_asset_modal(&self) -> bool {' "$VIEW_MODEL_ROOT" >/dev/null; then
  echo "can_confirm_asset_modal must move out of src/shell/view_model.rs" >&2
  exit 1
fi

if grep -F 'pub fn confirm_asset_modal(&mut self) -> bool {' "$VIEW_MODEL_ROOT" >/dev/null; then
  echo "confirm_asset_modal must move out of src/shell/view_model.rs" >&2
  exit 1
fi

if grep -F 'pub fn update_ssh_modal_field(&mut self, field: &str, value: String) {' "$VIEW_MODEL_ROOT" >/dev/null; then
  echo "SSH modal field update flow must move out of src/shell/view_model.rs" >&2
  exit 1
fi

if grep -F 'pub fn begin_ssh_modal_action(&mut self, action_id: &str) -> bool {' "$VIEW_MODEL_ROOT" >/dev/null; then
  echo "SSH modal action flow must move out of src/shell/view_model.rs" >&2
  exit 1
fi

if grep -F 'pub fn handle_context_menu_leaf_action(&mut self, action_id: &str) {' "$VIEW_MODEL_ROOT" >/dev/null; then
  echo "context menu leaf dispatch must move out of src/shell/view_model.rs" >&2
  exit 1
fi

if grep -F 'fn handle_sftp_context_menu_leaf_action(&mut self, action_id: &str) {' "$VIEW_MODEL_ROOT" >/dev/null; then
  echo "SFTP context menu helper must move out of src/shell/view_model.rs" >&2
  exit 1
fi

if grep -F 'fn context_menu_roots(&self) -> Vec<ContextMenuActionNode> {' "$VIEW_MODEL_ROOT" >/dev/null; then
  echo "context_menu_roots must move out of src/shell/view_model.rs" >&2
  exit 1
fi

if grep -F 'fn build_saved_ssh_connection_spec(' "$VIEW_MODEL_ROOT" >/dev/null; then
  echo "build_saved_ssh_connection_spec must move out of src/shell/view_model.rs" >&2
  exit 1
fi

if grep -F 'fn build_draft_proxy_spec(' "$VIEW_MODEL_ROOT" >/dev/null; then
  echo "build_draft_proxy_spec must move out of src/shell/view_model.rs" >&2
  exit 1
fi

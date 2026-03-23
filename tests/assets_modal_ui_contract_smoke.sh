#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_WINDOW="$ROOT_DIR/ui/app-window.slint"
FOLDER_MODAL="$ROOT_DIR/ui/components/assets-folder-create-modal.slint"
SSH_MODAL="$ROOT_DIR/ui/components/assets-ssh-connection-modal.slint"
RENAME_MODAL="$ROOT_DIR/ui/components/assets-rename-modal.slint"
DELETE_MODAL="$ROOT_DIR/ui/components/assets-delete-confirm-modal.slint"
BOOTSTRAP="$ROOT_DIR/src/app/bootstrap.rs"

grep -F 'in-out property <bool> asset-modal-open: false;' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <string> asset-modal-kind: "";' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <int> asset-modal-focus-sequence: 0;' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <bool> asset-rename-modal-open: false;' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <string> asset-rename-modal-name: "";' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <string> asset-rename-modal-validation-message: "";' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <bool> asset-rename-modal-can-confirm: false;' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <bool> asset-delete-confirm-modal-open: false;' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <string> asset-delete-confirm-target-label: "";' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <int> asset-delete-confirm-descendant-count: 0;' "$APP_WINDOW" >/dev/null
grep -F 'callback close-asset-modal-requested();' "$APP_WINDOW" >/dev/null
grep -F 'callback confirm-asset-modal-requested();' "$APP_WINDOW" >/dev/null
grep -F 'callback asset-rename-modal-name-changed(string);' "$APP_WINDOW" >/dev/null
grep -F 'callback confirm-asset-rename-requested();' "$APP_WINDOW" >/dev/null
grep -F 'callback confirm-delete-asset-requested();' "$APP_WINDOW" >/dev/null
grep -F 'export component AssetsFolderCreateModal inherits Rectangle {' "$FOLDER_MODAL" >/dev/null
grep -F 'export component AssetsSshConnectionModal inherits Rectangle {' "$SSH_MODAL" >/dev/null
grep -F 'in-out property <string> asset-modal-validation-message: "";' "$APP_WINDOW" >/dev/null
grep -F 'in property <string> validation-message: "";' "$FOLDER_MODAL" >/dev/null
grep -F 'in property <bool> can-confirm: false;' "$FOLDER_MODAL" >/dev/null
grep -F 'export component AssetsRenameModal inherits Rectangle {' "$RENAME_MODAL" >/dev/null
grep -F 'export component AssetsDeleteConfirmModal inherits Rectangle {' "$DELETE_MODAL" >/dev/null
grep -F 'in property <string> active-tab: "standard";' "$SSH_MODAL" >/dev/null
grep -F 'in property <string> validation-message: "";' "$SSH_MODAL" >/dev/null
grep -F 'in property <bool> can-confirm: false;' "$SSH_MODAL" >/dev/null
grep -F 'in property <string> auth-method:' "$SSH_MODAL" >/dev/null
grep -F 'in property <string> private-key-source:' "$SSH_MODAL" >/dev/null
grep -F 'in property <string> password:' "$SSH_MODAL" >/dev/null
grep -F 'in property <string> remark:' "$SSH_MODAL" >/dev/null
grep -F 'callback tab-selected(string);' "$SSH_MODAL" >/dev/null
grep -F 'callback draft-changed(string, string);' "$SSH_MODAL" >/dev/null
grep -F 'callback action-requested(string);' "$SSH_MODAL" >/dev/null
grep -F 'text: "Test Connection";' "$SSH_MODAL" >/dev/null
grep -F 'text: "Save and Connect";' "$SSH_MODAL" >/dev/null
grep -F 'in property <string> item-name;' "$RENAME_MODAL" >/dev/null
grep -F 'in property <string> validation-message;' "$RENAME_MODAL" >/dev/null
grep -F 'in property <bool> can-confirm;' "$RENAME_MODAL" >/dev/null
grep -F 'callback name-changed(string);' "$RENAME_MODAL" >/dev/null
grep -F 'callback confirm-requested();' "$RENAME_MODAL" >/dev/null
grep -F 'callback close-requested();' "$RENAME_MODAL" >/dev/null
grep -F 'in property <string> target-label;' "$DELETE_MODAL" >/dev/null
grep -F 'in property <int> descendant-count;' "$DELETE_MODAL" >/dev/null
grep -F 'callback confirm-requested();' "$DELETE_MODAL" >/dev/null
grep -F 'callback close-requested();' "$DELETE_MODAL" >/dev/null
! rg -n '[一-龥]' "$FOLDER_MODAL" >/dev/null
! rg -n '[一-龥]' "$SSH_MODAL" >/dev/null
! rg -n '[一-龥]' "$RENAME_MODAL" >/dev/null
! rg -n '[一-龥]' "$DELETE_MODAL" >/dev/null
grep -F 'text: "New Folder";' "$FOLDER_MODAL" >/dev/null
grep -F 'text: "New SSH Connection";' "$SSH_MODAL" >/dev/null
grep -F 'text: "Rename";' "$RENAME_MODAL" >/dev/null
grep -F 'text: "Delete Asset";' "$DELETE_MODAL" >/dev/null
grep -F 'in property <int> focus-sequence: 0;' "$FOLDER_MODAL" >/dev/null
grep -F 'in property <int> focus-sequence: 0;' "$SSH_MODAL" >/dev/null
rg -n "validation-message" "$FOLDER_MODAL" >/dev/null
rg -n "validation-message" "$SSH_MODAL" >/dev/null
rg -n "can-confirm" "$FOLDER_MODAL" >/dev/null
rg -n "can-confirm" "$SSH_MODAL" >/dev/null
grep -F 'public function focus-primary-field() {' "$SSH_MODAL" >/dev/null
grep -F 'window.set_asset_modal_focus_sequence(window.get_asset_modal_focus_sequence() + 1);' "$BOOTSTRAP" >/dev/null

#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_WINDOW="$ROOT_DIR/ui/app-window.slint"
FOLDER_MODAL="$ROOT_DIR/ui/components/assets-folder-create-modal.slint"
SSH_MODAL="$ROOT_DIR/ui/components/assets-ssh-connection-modal.slint"
RENAME_MODAL="$ROOT_DIR/ui/components/assets-rename-modal.slint"
DELETE_MODAL="$ROOT_DIR/ui/components/assets-delete-confirm-modal.slint"
MODAL_SHELL="$ROOT_DIR/ui/components/blocking-modal-shell.slint"
BOOTSTRAP="$ROOT_DIR/src/app/bootstrap.rs"
TOKENS="$ROOT_DIR/ui/theme/tokens.slint"

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
grep -F 'callback drag-requested(length, length);' "$MODAL_SHELL" >/dev/null
! grep -F 'in property <string> dialog-title: "";' "$MODAL_SHELL" >/dev/null
! grep -F 'callback close-requested();' "$MODAL_SHELL" >/dev/null
! grep -F 'header := Rectangle {' "$MODAL_SHELL" >/dev/null
! grep -F 'close-button := Rectangle {' "$MODAL_SHELL" >/dev/null
grep -F 'clicked => { }' "$APP_WINDOW" >/dev/null
grep -F 'callback close-asset-modal-requested();' "$APP_WINDOW" >/dev/null
grep -F 'callback confirm-asset-modal-requested();' "$APP_WINDOW" >/dev/null
grep -F 'callback asset-rename-modal-name-changed(string);' "$APP_WINDOW" >/dev/null
grep -F 'callback confirm-asset-rename-requested();' "$APP_WINDOW" >/dev/null
grep -F 'callback confirm-delete-asset-requested();' "$APP_WINDOW" >/dev/null
grep -F 'host-titlebar-height: titlebar.height;' "$APP_WINDOW" >/dev/null
grep -F 'modal-height: 230px;' "$APP_WINDOW" >/dev/null
grep -F 'modal-height: 560px;' "$APP_WINDOW" >/dev/null
grep -F 'modal-height: 268px;' "$APP_WINDOW" >/dev/null
grep -F 'modal-height: 332px;' "$APP_WINDOW" >/dev/null
grep -F 'asset-folder-modal-overlay := AssetsFolderCreateModal {' "$APP_WINDOW" >/dev/null
grep -F 'asset-ssh-modal-overlay := AssetsSshConnectionModal {' "$APP_WINDOW" >/dev/null
grep -F 'asset-rename-modal-overlay := AssetsRenameModal {' "$APP_WINDOW" >/dev/null
grep -F 'asset-delete-confirm-modal-overlay := AssetsDeleteConfirmModal {' "$APP_WINDOW" >/dev/null
grep -F 'ssh-host-key-modal-overlay := SshHostKeyConfirmModal {' "$APP_WINDOW" >/dev/null
grep -F -A4 'asset-folder-modal-overlay := AssetsFolderCreateModal {' "$APP_WINDOW" | grep -F 'width: asset-folder-modal-shell.content-width;' >/dev/null
grep -F -A5 'asset-folder-modal-overlay := AssetsFolderCreateModal {' "$APP_WINDOW" | grep -F 'height: asset-folder-modal-shell.content-height;' >/dev/null
grep -F -A4 'asset-ssh-modal-overlay := AssetsSshConnectionModal {' "$APP_WINDOW" | grep -F 'width: asset-ssh-modal-shell.content-width;' >/dev/null
grep -F -A5 'asset-ssh-modal-overlay := AssetsSshConnectionModal {' "$APP_WINDOW" | grep -F 'height: asset-ssh-modal-shell.content-height;' >/dev/null
grep -F -A4 'asset-rename-modal-overlay := AssetsRenameModal {' "$APP_WINDOW" | grep -F 'width: asset-rename-modal-shell.content-width;' >/dev/null
grep -F -A5 'asset-rename-modal-overlay := AssetsRenameModal {' "$APP_WINDOW" | grep -F 'height: asset-rename-modal-shell.content-height;' >/dev/null
grep -F -A4 'asset-delete-confirm-modal-overlay := AssetsDeleteConfirmModal {' "$APP_WINDOW" | grep -F 'width: asset-delete-confirm-modal-shell.content-width;' >/dev/null
grep -F -A5 'asset-delete-confirm-modal-overlay := AssetsDeleteConfirmModal {' "$APP_WINDOW" | grep -F 'height: asset-delete-confirm-modal-shell.content-height;' >/dev/null
grep -F -A4 'ssh-host-key-modal-overlay := SshHostKeyConfirmModal {' "$APP_WINDOW" | grep -F 'width: ssh-host-key-modal-shell.content-width;' >/dev/null
grep -F -A5 'ssh-host-key-modal-overlay := SshHostKeyConfirmModal {' "$APP_WINDOW" | grep -F 'height: ssh-host-key-modal-shell.content-height;' >/dev/null
grep -F 'x: 0px;' "$APP_WINDOW" >/dev/null
grep -F 'y: 0px;' "$APP_WINDOW" >/dev/null
grep -F 'export component AssetsFolderCreateModal inherits Rectangle {' "$FOLDER_MODAL" >/dev/null
grep -F 'export component AssetsSshConnectionModal inherits Rectangle {' "$SSH_MODAL" >/dev/null
grep -F 'in property <string> dialog-title: "New Folder";' "$FOLDER_MODAL" >/dev/null
grep -F 'header := Rectangle {' "$FOLDER_MODAL" >/dev/null
grep -F 'x: 0px;' "$FOLDER_MODAL" >/dev/null
grep -F 'y: 0px;' "$FOLDER_MODAL" >/dev/null
grep -F 'footer := Rectangle {' "$FOLDER_MODAL" >/dev/null
grep -F 'drag-touch := TouchArea {' "$FOLDER_MODAL" >/dev/null
grep -F 'close-button := Rectangle {' "$FOLDER_MODAL" >/dev/null
grep -F 'in property <string> dialog-title: "Rename Asset";' "$RENAME_MODAL" >/dev/null
grep -F 'footer := Rectangle {' "$RENAME_MODAL" >/dev/null
grep -F 'drag-touch := TouchArea {' "$RENAME_MODAL" >/dev/null
grep -F 'close-button := Rectangle {' "$RENAME_MODAL" >/dev/null
grep -F 'in property <string> dialog-title: "Delete Asset";' "$DELETE_MODAL" >/dev/null
grep -F 'footer := Rectangle {' "$DELETE_MODAL" >/dev/null
grep -F 'drag-touch := TouchArea {' "$DELETE_MODAL" >/dev/null
grep -F 'close-button := Rectangle {' "$DELETE_MODAL" >/dev/null
grep -F 'in property <string> dialog-title: "Verify Host Key";' "$ROOT_DIR/ui/components/ssh-host-key-confirm-modal.slint" >/dev/null
grep -F 'footer := Rectangle {' "$ROOT_DIR/ui/components/ssh-host-key-confirm-modal.slint" >/dev/null
grep -F 'drag-touch := TouchArea {' "$ROOT_DIR/ui/components/ssh-host-key-confirm-modal.slint" >/dev/null
grep -F 'close-button := Rectangle {' "$ROOT_DIR/ui/components/ssh-host-key-confirm-modal.slint" >/dev/null
grep -F 'in-out property <string> asset-modal-validation-message: "";' "$APP_WINDOW" >/dev/null
grep -F 'in property <string> validation-message: "";' "$FOLDER_MODAL" >/dev/null
grep -F 'in property <bool> can-confirm: false;' "$FOLDER_MODAL" >/dev/null
grep -F 'export component AssetsRenameModal inherits Rectangle {' "$RENAME_MODAL" >/dev/null
grep -F 'export component AssetsDeleteConfirmModal inherits Rectangle {' "$DELETE_MODAL" >/dev/null
! grep -F 'in property <string> active-tab:' "$SSH_MODAL" >/dev/null
grep -F 'private property <string> active-tab: "standard";' "$SSH_MODAL" >/dev/null
grep -F 'in property <string> dialog-title: "New SSH Connection";' "$SSH_MODAL" >/dev/null
grep -F 'header := Rectangle {' "$SSH_MODAL" >/dev/null
grep -F 'tabs-host := Rectangle {' "$SSH_MODAL" >/dev/null
grep -F 'footer := Rectangle {' "$SSH_MODAL" >/dev/null
grep -F 'drag-touch := TouchArea {' "$SSH_MODAL" >/dev/null
grep -F 'close-button := Rectangle {' "$SSH_MODAL" >/dev/null
grep -F 'in property <string> validation-message: "";' "$SSH_MODAL" >/dev/null
grep -F 'in property <bool> can-confirm: false;' "$SSH_MODAL" >/dev/null
grep -F 'in property <string> auth-method:' "$SSH_MODAL" >/dev/null
grep -F 'in property <string> private-key-source:' "$SSH_MODAL" >/dev/null
grep -F 'in property <string> password:' "$SSH_MODAL" >/dev/null
grep -F 'in property <string> remark:' "$SSH_MODAL" >/dev/null
! grep -F 'callback tab-selected(string);' "$SSH_MODAL" >/dev/null
grep -F 'callback draft-changed(string, string);' "$SSH_MODAL" >/dev/null
grep -F 'callback action-requested(string);' "$SSH_MODAL" >/dev/null
grep -F 'min-width: 0px;' "$SSH_MODAL" >/dev/null
grep -F '"Standard"' "$SSH_MODAL" >/dev/null
grep -F '"Proxy"' "$SSH_MODAL" >/dev/null
grep -F '"Environment"' "$SSH_MODAL" >/dev/null
grep -F '"Advanced"' "$SSH_MODAL" >/dev/null
grep -F 'import { ScrollView } from "std-widgets.slint";' "$SSH_MODAL" >/dev/null
grep -F 'label: "Connect";' "$SSH_MODAL" >/dev/null
grep -F 'label: "Save and Connect";' "$SSH_MODAL" >/dev/null
grep -F 'label: "Test Connection";' "$SSH_MODAL" >/dev/null
grep -F 'label: "Save";' "$SSH_MODAL" >/dev/null
grep -F 'InputType.password' "$SSH_MODAL" >/dev/null
grep -F 'password_visibility' "$SSH_MODAL" >/dev/null
grep -F 'busy' "$SSH_MODAL" >/dev/null
grep -F 'hover' "$TOKENS" >/dev/null
grep -F 'pressed' "$TOKENS" >/dev/null
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
grep -F 'dialog-title: root.asset-ssh-modal-dialog-title;' "$APP_WINDOW" >/dev/null
grep -F 'in property <int> focus-sequence: 0;' "$FOLDER_MODAL" >/dev/null
grep -F 'in property <int> focus-sequence: 0;' "$SSH_MODAL" >/dev/null
rg -n "validation-message" "$FOLDER_MODAL" >/dev/null
rg -n "validation-message" "$SSH_MODAL" >/dev/null
rg -n "can-confirm" "$FOLDER_MODAL" >/dev/null
rg -n "can-confirm" "$SSH_MODAL" >/dev/null
grep -F 'public function focus-primary-field() {' "$SSH_MODAL" >/dev/null
grep -F 'window.set_asset_modal_focus_sequence(window.get_asset_modal_focus_sequence() + 1);' "$BOOTSTRAP" >/dev/null

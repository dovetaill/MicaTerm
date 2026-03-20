#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_WINDOW="$ROOT_DIR/ui/app-window.slint"
FOLDER_MODAL="$ROOT_DIR/ui/components/assets-folder-create-modal.slint"
SSH_MODAL="$ROOT_DIR/ui/components/assets-ssh-connection-modal.slint"
BOOTSTRAP="$ROOT_DIR/src/app/bootstrap.rs"

grep -F 'in-out property <bool> asset-modal-open: false;' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <string> asset-modal-kind: "";' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <int> asset-modal-focus-sequence: 0;' "$APP_WINDOW" >/dev/null
grep -F 'callback close-asset-modal-requested();' "$APP_WINDOW" >/dev/null
grep -F 'callback confirm-asset-modal-requested();' "$APP_WINDOW" >/dev/null
grep -F 'export component AssetsFolderCreateModal inherits Rectangle {' "$FOLDER_MODAL" >/dev/null
grep -F 'export component AssetsSshConnectionModal inherits Rectangle {' "$SSH_MODAL" >/dev/null
grep -F 'in property <string> active-tab: "standard";' "$SSH_MODAL" >/dev/null
grep -F 'callback tab-selected(string);' "$SSH_MODAL" >/dev/null
grep -F 'callback draft-changed(string, string);' "$SSH_MODAL" >/dev/null
! rg -n '[一-龥]' "$FOLDER_MODAL" >/dev/null
! rg -n '[一-龥]' "$SSH_MODAL" >/dev/null
grep -F 'text: "New Folder";' "$FOLDER_MODAL" >/dev/null
grep -F 'text: "New SSH Connection";' "$SSH_MODAL" >/dev/null
grep -F 'in property <int> focus-sequence: 0;' "$FOLDER_MODAL" >/dev/null
grep -F 'in property <int> focus-sequence: 0;' "$SSH_MODAL" >/dev/null
grep -F 'public function focus-primary-field() {' "$SSH_MODAL" >/dev/null
grep -F 'window.set_asset_modal_focus_sequence(window.get_asset_modal_focus_sequence() + 1);' "$BOOTSTRAP" >/dev/null

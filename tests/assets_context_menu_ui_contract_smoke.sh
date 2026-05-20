#!/usr/bin/env bash
# Guards the merged assets context-menu and inline-rename UI contract.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROW="$ROOT_DIR/ui/components/asset-node-row.slint"
MENU_ROW="$ROOT_DIR/ui/components/assets-context-menu-row.slint"
MENU_COLUMN="$ROOT_DIR/ui/components/assets-context-menu-column.slint"
MENU_OVERLAY="$ROOT_DIR/ui/components/assets-context-menu-overlay.slint"
ASSETS="$ROOT_DIR/ui/shell/assets-sidebar.slint"
APP_WINDOW="$ROOT_DIR/ui/app-window.slint"
TOKENS="$ROOT_DIR/ui/theme/tokens.slint"
RENAME_MODAL="$ROOT_DIR/ui/components/assets-rename-modal.slint"
DELETE_MODAL="$ROOT_DIR/ui/components/assets-delete-confirm-modal.slint"
ASSETS_BOOTSTRAP="$ROOT_DIR/src/app/bootstrap/assets_keychain.rs"
CONTEXT_MENU_RS="$ROOT_DIR/src/shell/context_menu.rs"

grep -F 'export component AssetNodeRow inherits Rectangle' "$ROW" >/dev/null
grep -F 'in property <int> depth: 0;' "$ROW" >/dev/null
grep -F 'in property <bool> has-children: false;' "$ROW" >/dev/null
grep -F 'in property <bool> focused: false;' "$ROW" >/dev/null
grep -F 'callback selected-requested(string);' "$ROW" >/dev/null
grep -F 'callback toggle-expanded-requested(string);' "$ROW" >/dev/null
grep -F 'callback context-menu-requested(string, string, length, length);' "$ROW" >/dev/null
grep -F 'private property <image> snippet-package-icon:' "$ROW" >/dev/null
grep -F 'private property <image> snippet-icon:' "$ROW" >/dev/null
grep -F 'root.item-kind == "snippet-package"' "$ROW" >/dev/null
grep -F 'root.item-kind == "snippet"' "$ROW" >/dev/null
! grep -F 'rename-input := TextInput {' "$ROW" >/dev/null
grep -F 'pointer-event(event) => {' "$ROW" >/dev/null
grep -F 'event.button == PointerEventButton.right' "$ROW" >/dev/null

grep -F 'depth: int,' "$ASSETS" >/dev/null
grep -F 'has_children: bool,' "$ASSETS" >/dev/null
grep -F 'focused: bool,' "$ASSETS" >/dev/null
grep -F 'disclosure_state: string,' "$ASSETS" >/dev/null
grep -F 'callback asset-context-menu-requested(string, string, length, length);' "$ASSETS" >/dev/null
! grep -F 'callback asset-rename-text-changed(string, string);' "$ASSETS" >/dev/null
! grep -F 'callback asset-rename-commit-requested(string, string);' "$ASSETS" >/dev/null
! grep -F 'callback asset-rename-cancel-requested(string);' "$ASSETS" >/dev/null
grep -F 'empty-state-context-touch := TouchArea {' "$ASSETS" >/dev/null
grep -F 'list-blank-fill-context-touch := TouchArea {' "$ASSETS" >/dev/null
grep -F 'selected-requested(item-id) => {' "$ASSETS" >/dev/null
grep -F 'toggle-expanded-requested(item-id) => {' "$ASSETS" >/dev/null

grep -F 'callback asset-context-menu-requested(string, string, length, length);' "$APP_WINDOW" >/dev/null
grep -F 'callback asset-rename-modal-name-changed(string);' "$APP_WINDOW" >/dev/null
grep -F 'callback confirm-asset-rename-requested();' "$APP_WINDOW" >/dev/null
grep -F 'callback confirm-delete-asset-requested();' "$APP_WINDOW" >/dev/null
! grep -F 'callback asset-rename-text-changed(string, string);' "$APP_WINDOW" >/dev/null
! grep -F 'callback asset-rename-commit-requested(string, string);' "$APP_WINDOW" >/dev/null
! grep -F 'callback asset-rename-cancel-requested(string);' "$APP_WINDOW" >/dev/null
! grep -F 'callback dismiss-active-asset-rename-requested();' "$APP_WINDOW" >/dev/null
grep -F 'callback assets-context-menu-pointer-moved(length, length);' "$APP_WINDOW" >/dev/null
grep -F 'assets-context-menu-overlay := AssetsContextMenuOverlay {' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <length> assets-context-menu-column-width:' "$APP_WINDOW" >/dev/null
grep -F 'assets-context-menu-dismiss-layer := TouchArea {' "$APP_WINDOW" >/dev/null
grep -F 'StatusPill {' "$APP_WINDOW" >/dev/null
grep -F 'root.context-menu-feedback-text' "$APP_WINDOW" >/dev/null
grep -F 'export component AssetsRenameModal inherits Rectangle {' "$RENAME_MODAL" >/dev/null
grep -F 'export component AssetsDeleteConfirmModal inherits Rectangle {' "$DELETE_MODAL" >/dev/null

grep -F 'export component AssetsContextMenuRow inherits Rectangle' "$MENU_ROW" >/dev/null
grep -F 'in property <image> icon-source;' "$MENU_ROW" >/dev/null
grep -F 'icon-slot := Rectangle {' "$MENU_ROW" >/dev/null
grep -F 'export component AssetsContextMenuColumn inherits Rectangle' "$MENU_COLUMN" >/dev/null
grep -F 'private property <image> clipboard-icon:' "$MENU_COLUMN" >/dev/null
grep -F 'private property <image> document-code-icon:' "$MENU_COLUMN" >/dev/null
grep -F 'private property <image> play-icon:' "$MENU_COLUMN" >/dev/null
grep -F 'in property <length> column-width:' "$MENU_COLUMN" >/dev/null
grep -F 'width: root.column-width;' "$MENU_COLUMN" >/dev/null
grep -F 'item.icon_id == "clipboard"' "$MENU_COLUMN" >/dev/null
grep -F 'item.icon_id == "document-code"' "$MENU_COLUMN" >/dev/null
grep -F 'item.icon_id == "play"' "$MENU_COLUMN" >/dev/null
grep -F 'export component AssetsContextMenuOverlay inherits Rectangle' "$MENU_OVERLAY" >/dev/null
grep -F 'in property <length> column-width:' "$MENU_OVERLAY" >/dev/null
grep -F 'out property <brush> explorer-row-hover-surface:' "$TOKENS" >/dev/null
grep -F 'out property <brush> explorer-row-selected-surface:' "$TOKENS" >/dev/null
grep -F 'out property <brush> menu-row-hover-surface:' "$TOKENS" >/dev/null
grep -F 'out property <brush> menu-row-open-surface:' "$TOKENS" >/dev/null
grep -F 'ThemeTokens.menu-row-hover-surface' "$MENU_ROW" >/dev/null
grep -F 'hover-open-delay := Timer {' "$MENU_OVERLAY" >/dev/null
grep -F 'corridor-close-delay := Timer {' "$MENU_OVERLAY" >/dev/null
grep -F 'callback row-hovered(int, int);' "$MENU_OVERLAY" >/dev/null
grep -F 'callback pointer-moved(length, length);' "$MENU_OVERLAY" >/dev/null
grep -F 'key-pressed(event) => {' "$MENU_OVERLAY" >/dev/null
grep -F 'event.text == Key.Escape' "$MENU_OVERLAY" >/dev/null
grep -F 'window.on_assets_context_menu_pointer_moved(move |pointer_x, pointer_y| {' "$ASSETS_BOOTSTRAP" >/dev/null
! grep -F 'ThemeTokens.control-hover-surface' "$MENU_ROW" >/dev/null
! grep -F '"proxy-chrome-via-server"' "$CONTEXT_MENU_RS" >/dev/null
! grep -F '"upload-ssh-public-key"' "$CONTEXT_MENU_RS" >/dev/null

#!/usr/bin/env bash
# Guards the minimal assets context menu row bridge contract.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROW="$ROOT_DIR/ui/components/asset-node-row.slint"
MENU_ROW="$ROOT_DIR/ui/components/assets-context-menu-row.slint"
MENU_COLUMN="$ROOT_DIR/ui/components/assets-context-menu-column.slint"
MENU_OVERLAY="$ROOT_DIR/ui/components/assets-context-menu-overlay.slint"
ASSETS="$ROOT_DIR/ui/shell/assets-sidebar.slint"
APP_WINDOW="$ROOT_DIR/ui/app-window.slint"

grep -F 'export component AssetNodeRow inherits Rectangle' "$ROW" >/dev/null
grep -F 'pointer-event(event) => {' "$ROW" >/dev/null
grep -F 'event.button == PointerEventButton.right' "$ROW" >/dev/null
grep -F 'callback asset-context-menu-requested(string, string, length, length);' "$ASSETS" >/dev/null
grep -F 'in property <[ConsoleAssetItem]> console-asset-items' "$ASSETS" >/dev/null
grep -F 'callback asset-context-menu-requested(string, string, length, length);' "$APP_WINDOW" >/dev/null
grep -F 'export component AssetsContextMenuRow inherits Rectangle' "$MENU_ROW" >/dev/null
grep -F 'export component AssetsContextMenuColumn inherits Rectangle' "$MENU_COLUMN" >/dev/null
grep -F 'export component AssetsContextMenuOverlay inherits Rectangle' "$MENU_OVERLAY" >/dev/null
grep -F 'text: "No assets yet";' "$ASSETS" >/dev/null
grep -F 'text: "Right-click or use Create to add a folder or SSH connection."; ' "$ASSETS" >/dev/null
grep -F 'empty-state-context-touch := TouchArea {' "$ASSETS" >/dev/null
grep -F 'list-blank-fill-context-touch := TouchArea {' "$ASSETS" >/dev/null
grep -F 'renaming: bool,' "$ASSETS" >/dev/null
grep -F 'rename_text: string,' "$ASSETS" >/dev/null
grep -F 'callback asset-rename-text-changed(string, string);' "$APP_WINDOW" >/dev/null
grep -F 'callback asset-rename-commit-requested(string, string);' "$APP_WINDOW" >/dev/null
grep -F 'callback asset-rename-cancel-requested(string);' "$APP_WINDOW" >/dev/null
grep -F 'rename-input := TextInput {' "$ROW" >/dev/null
! grep -F 'changed has-focus => {' "$ROW" >/dev/null
grep -F 'callback dismiss-active-asset-rename-requested();' "$APP_WINDOW" >/dev/null
grep -F 'in property <image> icon-source;' "$MENU_ROW" >/dev/null
grep -F 'icon-slot := Rectangle {' "$MENU_ROW" >/dev/null
! grep -F 'Text { text: "操作";' "$MENU_COLUMN" >/dev/null
! grep -F 'in property <string> title: "操作";' "$MENU_COLUMN" >/dev/null
! grep -F 'height: 320px;' "$MENU_OVERLAY" >/dev/null
! grep -F 'height: parent.height;' "$MENU_COLUMN" >/dev/null
grep -F 'hover-open-delay := Timer {' "$MENU_OVERLAY" >/dev/null
grep -F 'corridor-close-delay := Timer {' "$MENU_OVERLAY" >/dev/null
grep -F 'callback row-hovered(int, int);' "$MENU_OVERLAY" >/dev/null
grep -F 'callback pointer-moved(length, length);' "$MENU_OVERLAY" >/dev/null
grep -F 'key-pressed(event) => {' "$MENU_OVERLAY" >/dev/null
grep -F 'event.text == Key.Escape' "$MENU_OVERLAY" >/dev/null
grep -F 'assets-context-menu-overlay := AssetsContextMenuOverlay {' "$APP_WINDOW" >/dev/null
grep -F 'enabled: root.assets-context-menu-open;' "$APP_WINDOW" >/dev/null
grep -F 'StatusPill {' "$APP_WINDOW" >/dev/null
grep -F 'root.context-menu-feedback-text' "$APP_WINDOW" >/dev/null

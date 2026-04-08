#!/usr/bin/env bash
# Guards the merged assets sidebar toolbar and create-popover contract.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ASSETS="$ROOT_DIR/ui/shell/assets-sidebar.slint"
SIDEBAR="$ROOT_DIR/ui/shell/sidebar.slint"
APP_WINDOW="$ROOT_DIR/ui/app-window.slint"
BUTTON="$ROOT_DIR/ui/components/sidebar-toolbar-icon-button.slint"
ROW="$ROOT_DIR/ui/components/assets-toolbar-menu-row.slint"
MENU="$ROOT_DIR/ui/components/assets-create-menu.slint"
SEARCH="$ROOT_DIR/ui/components/assets-search-popover.slint"
VIEW_MODEL="$ROOT_DIR/src/shell/view_model.rs"
VIEW_MODEL_ASSETS="$ROOT_DIR/src/shell/view_model/assets.rs"

grep -F 'export component SidebarToolbarIconButton' "$BUTTON" >/dev/null
grep -F 'in property <string> tooltip-text;' "$BUTTON" >/dev/null
grep -F 'in property <string> tooltip-source-id;' "$BUTTON" >/dev/null
grep -F 'callback tooltip-open-requested(string, string, length, length, length);' "$BUTTON" >/dev/null
grep -F 'callback tooltip-close-requested(string);' "$BUTTON" >/dev/null
grep -F 'ThemeTokens.control-hover-surface' "$BUTTON" >/dev/null
grep -F 'ThemeTokens.control-active-surface' "$BUTTON" >/dev/null

grep -F 'export component AssetsToolbarMenuRow inherits Rectangle' "$ROW" >/dev/null
grep -F 'in property <image> icon-source;' "$ROW" >/dev/null
grep -F 'icon-slot := Rectangle {' "$ROW" >/dev/null
grep -F 'label-text := Text {' "$ROW" >/dev/null
grep -F 'x: 38px;' "$ROW" >/dev/null
grep -F 'width: parent.width - 50px;' "$ROW" >/dev/null

grep -F 'export component AssetsCreateMenu inherits Rectangle' "$MENU" >/dev/null
! grep -F 'export component AssetsCreateMenu inherits PopupWindow' "$MENU" >/dev/null
grep -F 'label: "New Folder"' "$MENU" >/dev/null
grep -F 'label: "New SSH Connection"' "$MENU" >/dev/null
grep -F 'public function focus-menu()' "$MENU" >/dev/null
grep -F 'new-folder-icon: @image-url("../../assets/icons/fluent/folder-20-regular.svg")' "$MENU" >/dev/null
grep -F 'new-ssh-connection-icon: @image-url("../../assets/icons/fluent/window-console-20-regular.svg")' "$MENU" >/dev/null

grep -F 'text: "Assets";' "$ASSETS" >/dev/null
grep -F 'color: ThemeTokens.text-secondary;' "$ASSETS" >/dev/null
grep -F 'callback toggle-assets-create-menu-requested();' "$ASSETS" >/dev/null
grep -F 'callback close-assets-create-menu-requested();' "$ASSETS" >/dev/null
grep -F 'in property <bool> asset-create-menu-open: false;' "$ASSETS" >/dev/null
grep -F 'in property <bool> asset-uses-create-popover: false;' "$ASSETS" >/dev/null
grep -F 'in property <bool> asset-tree-controls-enabled: true;' "$ASSETS" >/dev/null
grep -F 'out property <length> create-menu-anchor-x: create-button.absolute-position.x;' "$ASSETS" >/dev/null
grep -F 'out property <length> create-menu-anchor-y: create-button.absolute-position.y;' "$ASSETS" >/dev/null
grep -F 'out property <length> create-menu-anchor-width: create-button.width;' "$ASSETS" >/dev/null
grep -F 'out property <length> create-menu-anchor-height: create-button.height;' "$ASSETS" >/dev/null
grep -F 'toolbar-content := HorizontalLayout {' "$ASSETS" >/dev/null
grep -F 'search-button := SidebarToolbarIconButton' "$ASSETS" >/dev/null
grep -F 'tree-expansion-button := SidebarToolbarIconButton' "$ASSETS" >/dev/null
grep -F 'view-mode-button := SidebarToolbarIconButton' "$ASSETS" >/dev/null
grep -F 'create-button := SidebarToolbarIconButton {' "$ASSETS" >/dev/null
grep -F 'icon-source: root.create-add-icon;' "$ASSETS" >/dev/null
grep -F 'active: root.asset-create-menu-open;' "$ASSETS" >/dev/null
grep -F 'tooltip-text: root.asset-primary-create-tooltip;' "$ASSETS" >/dev/null
grep -F 'tooltip-text: root.asset-search-tooltip;' "$ASSETS" >/dev/null
grep -F 'tooltip-text: root.asset-view-mode-tooltip;' "$ASSETS" >/dev/null
grep -F 'tooltip-text: root.asset-tree-expansion-tooltip;' "$ASSETS" >/dev/null
grep -F 'enabled: root.asset-show-tree-controls && root.asset-tree-controls-enabled;' "$ASSETS" >/dev/null
! grep -F 'visible: root.asset-show-tree-controls && root.asset-view-mode == "tree";' "$ASSETS" >/dev/null
grep -F 'if root.asset-uses-create-popover {' "$ASSETS" >/dev/null
grep -F 'root.toggle-assets-create-menu-requested();' "$ASSETS" >/dev/null
grep -F 'root.assets-create-action-selected(root.asset-primary-create-action-id);' "$ASSETS" >/dev/null
grep -F 'width: expanded ? 320px : 0px;' "$ASSETS" >/dev/null
grep -F 'height: root.asset-search-expanded ? 44px : 0px;' "$ASSETS" >/dev/null
! grep -F 'create-menu := AssetsCreateMenu {' "$ASSETS" >/dev/null

grep -F 'in property <bool> asset-create-menu-open: false;' "$SIDEBAR" >/dev/null
grep -F 'in property <bool> asset-tree-controls-enabled: true;' "$SIDEBAR" >/dev/null
grep -F 'callback toggle-assets-create-menu-requested();' "$SIDEBAR" >/dev/null
grep -F 'callback close-assets-create-menu-requested();' "$SIDEBAR" >/dev/null
grep -F 'out property <length> create-menu-anchor-x: assets-sidebar.create-menu-anchor-x;' "$SIDEBAR" >/dev/null
grep -F 'out property <length> create-menu-anchor-y: assets-sidebar.create-menu-anchor-y;' "$SIDEBAR" >/dev/null
grep -F 'out property <length> create-menu-anchor-width: assets-sidebar.create-menu-anchor-width;' "$SIDEBAR" >/dev/null
grep -F 'out property <length> create-menu-anchor-height: assets-sidebar.create-menu-anchor-height;' "$SIDEBAR" >/dev/null
grep -F 'width: 44px + (root.show-assets-sidebar ? 320px : 0px);' "$SIDEBAR" >/dev/null
grep -F 'toggle-assets-create-menu-requested => {' "$SIDEBAR" >/dev/null
grep -F 'close-assets-create-menu-requested => {' "$SIDEBAR" >/dev/null
grep -F 'root.schedule-tooltip(source-id, text, anchor-x, anchor-y, anchor-width);' "$SIDEBAR" >/dev/null
grep -F 'root.queue-tooltip-close(source-id);' "$SIDEBAR" >/dev/null

grep -F 'import { AssetsCreateMenu } from "components/assets-create-menu.slint";' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <bool> asset-create-menu-open: false;' "$APP_WINDOW" >/dev/null
grep -F 'callback toggle-assets-create-menu-requested();' "$APP_WINDOW" >/dev/null
grep -F 'callback close-assets-create-menu-requested();' "$APP_WINDOW" >/dev/null
grep -F 'out property <length> layout-assets-create-menu-anchor-x: sidebar.create-menu-anchor-x;' "$APP_WINDOW" >/dev/null
grep -F 'out property <length> layout-assets-create-menu-anchor-y: sidebar.create-menu-anchor-y;' "$APP_WINDOW" >/dev/null
grep -F 'out property <length> layout-assets-create-menu-anchor-width: sidebar.create-menu-anchor-width;' "$APP_WINDOW" >/dev/null
grep -F 'out property <length> layout-assets-create-menu-anchor-height: sidebar.create-menu-anchor-height;' "$APP_WINDOW" >/dev/null
grep -F 'changed asset-create-menu-open => {' "$APP_WINDOW" >/dev/null
grep -F 'assets-create-menu-overlay.focus-menu();' "$APP_WINDOW" >/dev/null
grep -F 'assets-create-menu-dismiss-layer := TouchArea {' "$APP_WINDOW" >/dev/null
grep -F 'enabled: root.asset-create-menu-open;' "$APP_WINDOW" >/dev/null
grep -F 'assets-create-menu-overlay := AssetsCreateMenu {' "$APP_WINDOW" >/dev/null
grep -F 'visible: root.asset-create-menu-open;' "$APP_WINDOW" >/dev/null
grep -F 'x: sidebar.create-menu-anchor-x + sidebar.create-menu-anchor-width - self.width;' "$APP_WINDOW" >/dev/null
grep -F 'y: sidebar.create-menu-anchor-y + sidebar.create-menu-anchor-height + 6px;' "$APP_WINDOW" >/dev/null
! grep -F 'assets-create-menu-overlay.show();' "$APP_WINDOW" >/dev/null
! grep -F 'assets-create-menu-overlay.close();' "$APP_WINDOW" >/dev/null

grep -F 'public function focus-input()' "$SEARCH" >/dev/null
grep -F 'callback collapse-requested();' "$SEARCH" >/dev/null
grep -F 'callback close-requested();' "$SEARCH" >/dev/null
grep -F 'pub asset_create_menu_open: bool,' "$VIEW_MODEL" >/dev/null
grep -F 'pub fn toggle_asset_create_menu(&mut self) {' "$VIEW_MODEL_ASSETS" >/dev/null
grep -F 'pub fn close_asset_create_menu(&mut self) {' "$VIEW_MODEL_ASSETS" >/dev/null

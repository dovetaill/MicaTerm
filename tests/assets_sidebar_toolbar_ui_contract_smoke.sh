#!/usr/bin/env bash
# Guards the assets sidebar UI contract that other layout tests depend on.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ASSETS="$ROOT_DIR/ui/shell/assets-sidebar.slint"
SIDEBAR="$ROOT_DIR/ui/shell/sidebar.slint"
APP_WINDOW="$ROOT_DIR/ui/app-window.slint"
BUTTON="$ROOT_DIR/ui/components/sidebar-toolbar-icon-button.slint"
MENU="$ROOT_DIR/ui/components/assets-create-menu.slint"
SEARCH="$ROOT_DIR/ui/components/assets-search-popover.slint"
ROW="$ROOT_DIR/ui/components/assets-toolbar-menu-row.slint"

grep -F 'export component SidebarToolbarIconButton' "$BUTTON" >/dev/null
grep -F 'export component AssetsToolbarMenuRow inherits Rectangle' "$ROW" >/dev/null
grep -F 'icon-slot := Rectangle' "$ROW" >/dev/null
! grep -F 'HorizontalLayout {' "$ROW" >/dev/null
grep -F 'x: 12px;' "$ROW" >/dev/null
grep -F 'y: (parent.height - self.height) / 2;' "$ROW" >/dev/null
grep -F 'label-text := Text {' "$ROW" >/dev/null
grep -F 'x: 38px;' "$ROW" >/dev/null
grep -F 'width: parent.width - 50px;' "$ROW" >/dev/null
grep -F 'height: parent.height;' "$ROW" >/dev/null
grep -F 'vertical-alignment: center;' "$ROW" >/dev/null
grep -F 'export component AssetsCreateMenu inherits Rectangle' "$MENU" >/dev/null
! grep -F 'export component AssetsCreateMenu inherits PopupWindow' "$MENU" >/dev/null
grep -F 'label: "New Folder"' "$MENU" >/dev/null
grep -F 'label: "New SSH Connection"' "$MENU" >/dev/null
! grep -F 'close-policy:' "$MENU" >/dev/null
grep -F 'public function focus-menu()' "$MENU" >/dev/null
grep -F 'Text { text: "Assets";' "$ASSETS" >/dev/null
grep -F 'toolbar-content := HorizontalLayout {' "$ASSETS" >/dev/null
grep -F 'search-button := SidebarToolbarIconButton' "$ASSETS" >/dev/null
grep -F 'tree-expansion-button := SidebarToolbarIconButton' "$ASSETS" >/dev/null
grep -F 'view-mode-button := SidebarToolbarIconButton' "$ASSETS" >/dev/null
grep -F 'out property <length> search-row-height: search-row-host.height;' "$ASSETS" >/dev/null
grep -F 'search-row-host := Rectangle {' "$ASSETS" >/dev/null
grep -F 'inline-search := AssetsSearchPopover {' "$ASSETS" >/dev/null
grep -F 'header-search-dismiss-touch := TouchArea {' "$ASSETS" >/dev/null
grep -F 'panel-search-dismiss-touch := TouchArea {' "$ASSETS" >/dev/null
grep -F 'enabled: root.asset-search-expanded && root.assets-search-query == "";' "$ASSETS" >/dev/null
grep -F 'search-icon: @image-url("../../assets/icons/fluent/search-20-regular.svg")' "$ASSETS" >/dev/null
grep -F 'tree-expand-icon: @image-url("../../assets/icons/fluent/arrow-expand-all-20-regular.svg")' "$ASSETS" >/dev/null
grep -F 'tree-collapse-icon: @image-url("../../assets/icons/fluent/arrow-collapse-all-20-regular.svg")' "$ASSETS" >/dev/null
grep -F 'tree-view-icon: @image-url("../../assets/icons/fluent/branch-20-regular.svg")' "$ASSETS" >/dev/null
grep -F 'list-view-icon: @image-url("../../assets/icons/fluent/list-20-regular.svg")' "$ASSETS" >/dev/null
grep -F 'icon-source: root.search-icon;' "$ASSETS" >/dev/null
grep -F 'icon-source: root.asset-tree-fully-expanded ? root.tree-collapse-icon : root.tree-expand-icon;' "$ASSETS" >/dev/null
grep -F 'icon-source: root.asset-view-mode == "flat" ? root.list-view-icon : root.tree-view-icon;' "$ASSETS" >/dev/null
grep -F 'create-button' "$ASSETS" >/dev/null
! grep -F 'callback focus-assets-search-requested();' "$ASSETS" >/dev/null
! grep -F 'out property <length> search-anchor-x:' "$ASSETS" >/dev/null
! grep -F 'out property <length> search-anchor-y:' "$ASSETS" >/dev/null
! grep -F 'out property <length> search-anchor-width:' "$ASSETS" >/dev/null
! grep -F 'out property <length> search-anchor-height:' "$ASSETS" >/dev/null
! grep -F 'if root.asset-search-expanded : Rectangle' "$ASSETS" >/dev/null
grep -F 'asset-view-mode == "tree"' "$ASSETS" >/dev/null
grep -F 'asset-view-mode == "flat"' "$ASSETS" >/dev/null
grep -F 'collapse-requested => {' "$ASSETS" >/dev/null
grep -F 'root.collapse-assets-search-requested();' "$ASSETS" >/dev/null
grep -F 'in property <string> assets-search-query' "$SIDEBAR" >/dev/null
! grep -F 'callback focus-assets-search-requested();' "$SIDEBAR" >/dev/null
grep -F 'callback assets-search-query-changed(string);' "$APP_WINDOW" >/dev/null
grep -F 'out property <length> search-row-height: assets-sidebar.search-row-height;' "$SIDEBAR" >/dev/null
! grep -F 'focus-assets-search-requested => {' "$APP_WINDOW" >/dev/null
! grep -F 'assets-search-overlay.focus-input();' "$APP_WINDOW" >/dev/null
grep -F 'callback toggle-assets-view-mode-requested();' "$APP_WINDOW" >/dev/null
grep -F 'callback toggle-assets-tree-expansion-requested();' "$APP_WINDOW" >/dev/null
grep -F 'callback toggle-assets-create-menu-requested();' "$APP_WINDOW" >/dev/null
grep -F 'callback close-assets-create-menu-requested();' "$APP_WINDOW" >/dev/null
grep -F 'callback assets-create-action-selected(string);' "$APP_WINDOW" >/dev/null
grep -F 'overlay-dismiss-layer := TouchArea {' "$APP_WINDOW" >/dev/null
grep -F 'workspace-search-dismiss-layer := TouchArea {' "$APP_WINDOW" >/dev/null
grep -F 'enabled: root.asset-create-menu-open;' "$APP_WINDOW" >/dev/null
grep -F 'enabled: root.asset-search-expanded && root.assets-search-query == "";' "$APP_WINDOW" >/dev/null
grep -F 'root.collapse-assets-search-requested();' "$APP_WINDOW" >/dev/null
grep -F 'root.close-assets-create-menu-requested();' "$APP_WINDOW" >/dev/null
grep -F 'public function focus-input()' "$SEARCH" >/dev/null
grep -F 'callback collapse-requested();' "$SEARCH" >/dev/null
grep -F 'callback close-requested();' "$SEARCH" >/dev/null
grep -F 'height: 32px;' "$SEARCH" >/dev/null
grep -F 'glow-frame := Rectangle {' "$SEARCH" >/dev/null
grep -F 'border-color: ThemeTokens.accent;' "$SEARCH" >/dev/null
grep -F 'opacity:' "$SEARCH" >/dev/null
grep -F 'field-frame := Rectangle {' "$SEARCH" >/dev/null
grep -F 'border-radius: 0px;' "$SEARCH" >/dev/null
grep -F 'search-input := TextInput {' "$SEARCH" >/dev/null
grep -F 'y: 5px;' "$SEARCH" >/dev/null
grep -F 'height: 22px;' "$SEARCH" >/dev/null
grep -F 'font-size: 13px;' "$SEARCH" >/dev/null
grep -F 'color: ThemeTokens.text-primary;' "$SEARCH" >/dev/null
grep -F 'selection-background-color: ThemeTokens.accent;' "$SEARCH" >/dev/null
grep -F 'selection-foreground-color: ThemeTokens.text-primary;' "$SEARCH" >/dev/null
grep -F 'changed has-focus => {' "$SEARCH" >/dev/null
grep -F 'root.collapse-requested();' "$SEARCH" >/dev/null
grep -F 'key-pressed(event) => {' "$SEARCH" >/dev/null
grep -F 'event.text == Key.Escape' "$SEARCH" >/dev/null
! grep -F '#101418' "$SEARCH" >/dev/null
! grep -F '#f5f7fb' "$SEARCH" >/dev/null
grep -F 'callback close-assets-search-requested();' "$APP_WINDOW" >/dev/null
! grep -F 'assets-search-overlay := AssetsSearchPopover {' "$APP_WINDOW" >/dev/null
! grep -F 'layout-assets-search-anchor-x' "$APP_WINDOW" >/dev/null
! grep -F 'layout-assets-search-anchor-y' "$APP_WINDOW" >/dev/null
! grep -F 'layout-assets-search-anchor-width' "$APP_WINDOW" >/dev/null
! grep -F 'layout-assets-search-anchor-height' "$APP_WINDOW" >/dev/null
grep -F 'out property <length> layout-assets-search-row-height: sidebar.search-row-height;' "$APP_WINDOW" >/dev/null
grep -F 'out property <length> create-menu-anchor-x' "$ASSETS" >/dev/null
grep -F 'out property <length> create-menu-anchor-y' "$ASSETS" >/dev/null
grep -F 'out property <length> create-menu-anchor-width' "$ASSETS" >/dev/null
grep -F 'out property <length> create-menu-anchor-height' "$ASSETS" >/dev/null
grep -F 'height: root.asset-search-expanded ? 44px : 0px;' "$ASSETS" >/dev/null
grep -F 'out property <length> create-menu-anchor-x' "$SIDEBAR" >/dev/null
grep -F 'out property <length> create-menu-anchor-y' "$SIDEBAR" >/dev/null
grep -F 'out property <length> create-menu-anchor-width' "$SIDEBAR" >/dev/null
grep -F 'out property <length> create-menu-anchor-height' "$SIDEBAR" >/dev/null
grep -F 'assets-create-menu-overlay := AssetsCreateMenu {' "$APP_WINDOW" >/dev/null
grep -F 'visible: root.asset-create-menu-open;' "$APP_WINDOW" >/dev/null
grep -F 'x: sidebar.create-menu-anchor-x + sidebar.create-menu-anchor-width - self.width;' "$APP_WINDOW" >/dev/null
grep -F 'y: sidebar.create-menu-anchor-y + sidebar.create-menu-anchor-height + 6px;' "$APP_WINDOW" >/dev/null
! grep -F 'assets-create-menu-overlay.show();' "$APP_WINDOW" >/dev/null
! grep -F 'assets-create-menu-overlay.close();' "$APP_WINDOW" >/dev/null
! grep -F 'create-menu := AssetsCreateMenu {' "$ASSETS" >/dev/null
grep -F 'create-add-icon: @image-url("../../assets/icons/fluent/add-20-regular.svg")' "$ASSETS" >/dev/null
grep -F 'create-button := SidebarToolbarIconButton {' "$ASSETS" >/dev/null
grep -F 'icon-source: root.create-add-icon;' "$ASSETS" >/dev/null
grep -F 'active: root.asset-create-menu-open;' "$ASSETS" >/dev/null
! grep -F 'text: "Create";' "$ASSETS" >/dev/null
grep -F 'width: 216px;' "$MENU" >/dev/null
grep -F 'in property <image> icon-source;' "$ROW" >/dev/null
grep -F 'new-folder-icon: @image-url("../../assets/icons/fluent/folder-20-regular.svg")' "$MENU" >/dev/null
grep -F 'new-ssh-connection-icon: @image-url("../../assets/icons/fluent/window-console-20-regular.svg")' "$MENU" >/dev/null
grep -F 'icon-source: root.new-folder-icon;' "$MENU" >/dev/null
grep -F 'icon-source: root.new-ssh-connection-icon;' "$MENU" >/dev/null

grep -F 'ThemeTokens.control-hover-surface' "$BUTTON" >/dev/null
grep -F 'ThemeTokens.control-active-surface' "$BUTTON" >/dev/null
grep -F 'ThemeTokens.control-hover-surface' "$ROW" >/dev/null
grep -F 'ThemeTokens.control-active-surface' "$ROW" >/dev/null
grep -F 'background: ThemeTokens.inspector-surface;' "$MENU" >/dev/null
grep -F 'background: ThemeTokens.inspector-surface;' "$SEARCH" >/dev/null
grep -F 'border-color: ThemeTokens.divider-strong;' "$MENU" >/dev/null
grep -F 'border-color: ThemeTokens.divider-subtle;' "$SEARCH" >/dev/null

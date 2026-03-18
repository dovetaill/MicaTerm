#!/usr/bin/env bash
# Guards the assets sidebar toolbar contract after panel-aware descriptor refactor.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ASSETS="$ROOT_DIR/ui/shell/assets-sidebar.slint"
SIDEBAR="$ROOT_DIR/ui/shell/sidebar.slint"
APP_WINDOW="$ROOT_DIR/ui/app-window.slint"
BUTTON="$ROOT_DIR/ui/components/sidebar-toolbar-icon-button.slint"
SEARCH="$ROOT_DIR/ui/components/assets-search-popover.slint"
VIEW_MODEL="$ROOT_DIR/src/shell/view_model.rs"

grep -F 'export component SidebarToolbarIconButton' "$BUTTON" >/dev/null
grep -F 'in property <string> tooltip-text;' "$BUTTON" >/dev/null
grep -F 'in property <string> tooltip-source-id;' "$BUTTON" >/dev/null
grep -F 'callback tooltip-open-requested(string, string, length, length, length);' "$BUTTON" >/dev/null
grep -F 'callback tooltip-close-requested(string);' "$BUTTON" >/dev/null
grep -F 'ThemeTokens.control-hover-surface' "$BUTTON" >/dev/null
grep -F 'ThemeTokens.control-active-surface' "$BUTTON" >/dev/null

grep -F 'toolbar-content := HorizontalLayout {' "$ASSETS" >/dev/null
grep -F 'search-button := SidebarToolbarIconButton' "$ASSETS" >/dev/null
grep -F 'tree-expansion-button := SidebarToolbarIconButton' "$ASSETS" >/dev/null
grep -F 'view-mode-button := SidebarToolbarIconButton' "$ASSETS" >/dev/null
grep -F 'create-button := SidebarToolbarIconButton {' "$ASSETS" >/dev/null
grep -F 'create-add-icon: @image-url("../../assets/icons/fluent/add-20-regular.svg")' "$ASSETS" >/dev/null
grep -F 'root.assets-create-action-selected(root.asset-primary-create-action-id);' "$ASSETS" >/dev/null
grep -F 'tooltip-text: root.asset-primary-create-tooltip;' "$ASSETS" >/dev/null
grep -F 'tooltip-text: root.asset-search-tooltip;' "$ASSETS" >/dev/null
grep -F 'tooltip-text: root.asset-view-mode-tooltip;' "$ASSETS" >/dev/null
grep -F 'tooltip-text: root.asset-tree-expansion-tooltip;' "$ASSETS" >/dev/null
! grep -F 'Console Tree —' "$ASSETS" >/dev/null
! grep -F 'Hosts, recent sessions, favorites' "$ASSETS" >/dev/null
! grep -F 'header-search-dismiss-touch := TouchArea {' "$ASSETS" >/dev/null
! grep -F 'panel-search-dismiss-touch := TouchArea {' "$ASSETS" >/dev/null
! grep -F 'callback toggle-assets-create-menu-requested();' "$ASSETS" >/dev/null
! grep -F 'callback close-assets-create-menu-requested();' "$ASSETS" >/dev/null
! grep -F 'asset-create-menu-open' "$ASSETS" >/dev/null
! grep -F 'create-menu-anchor-x' "$ASSETS" >/dev/null

grep -F 'tooltip-open-requested(source-id, text, anchor-x, anchor-y, anchor-width) => {' "$SIDEBAR" >/dev/null
grep -F 'tooltip-close-requested(source-id) => {' "$SIDEBAR" >/dev/null
grep -F 'root.schedule-tooltip(source-id, text, anchor-x, anchor-y, anchor-width);' "$SIDEBAR" >/dev/null
grep -F 'root.queue-tooltip-close(source-id);' "$SIDEBAR" >/dev/null
! grep -F 'toggle-assets-create-menu-requested' "$SIDEBAR" >/dev/null
! grep -F 'close-assets-create-menu-requested' "$SIDEBAR" >/dev/null
! grep -F 'asset-create-menu-open' "$SIDEBAR" >/dev/null

grep -F 'callback assets-create-action-selected(string);' "$APP_WINDOW" >/dev/null
grep -F 'sidebar-tooltip-overlay := TitlebarTooltip' "$APP_WINDOW" >/dev/null
grep -F 'shell-body-empty-search-dismiss-layer := TouchArea {' "$APP_WINDOW" >/dev/null
! grep -F 'AssetsCreateMenu' "$APP_WINDOW" >/dev/null
! grep -F 'toggle-assets-create-menu-requested' "$APP_WINDOW" >/dev/null
! grep -F 'close-assets-create-menu-requested' "$APP_WINDOW" >/dev/null
! grep -F 'overlay-dismiss-layer := TouchArea {' "$APP_WINDOW" >/dev/null
! grep -F 'layout-assets-create-menu-anchor-x' "$APP_WINDOW" >/dev/null

! grep -F 'asset_create_menu_open' "$VIEW_MODEL" >/dev/null

grep -F 'public function focus-input()' "$SEARCH" >/dev/null
grep -F 'callback collapse-requested();' "$SEARCH" >/dev/null
grep -F 'callback close-requested();' "$SEARCH" >/dev/null

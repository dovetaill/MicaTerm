#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SIDEBAR="$ROOT_DIR/ui/shell/sidebar.slint"
ASSETS="$ROOT_DIR/ui/shell/assets-sidebar.slint"
APP_WINDOW="$ROOT_DIR/ui/app-window.slint"
BUTTON="$ROOT_DIR/ui/components/sidebar-nav-button.slint"

[[ -f "$BUTTON" ]] || {
  echo "missing ui/components/sidebar-nav-button.slint" >&2
  exit 1
}

[[ -f "$ASSETS" ]] || {
  echo "missing ui/shell/assets-sidebar.slint" >&2
  exit 1
}

grep -F 'export struct SidebarNavItem' "$SIDEBAR" >/dev/null
grep -F 'in property <[SidebarNavItem]> items;' "$SIDEBAR" >/dev/null
grep -F 'in property <bool> show-assets-sidebar: true;' "$SIDEBAR" >/dev/null
grep -F 'callback toggle-assets-sidebar-requested();' "$APP_WINDOW" >/dev/null
grep -F 'callback sidebar-destination-selected(string);' "$APP_WINDOW" >/dev/null
grep -F 'Sidebar {' "$APP_WINDOW" >/dev/null
grep -F 'show-assets-sidebar: root.show-assets-sidebar;' "$APP_WINDOW" >/dev/null
grep -F 'active-sidebar-destination: root.active-sidebar-destination;' "$APP_WINDOW" >/dev/null
grep -F 'Folder Open' "$ASSETS" >/dev/null || true
grep -F 'Window Console' "$ASSETS" >/dev/null
grep -F 'Snippets' "$ASSETS" >/dev/null
grep -F 'Keychain' "$ASSETS" >/dev/null
grep -F 'folder-open-20-regular.svg' "$BUTTON" >/dev/null
grep -F 'window-console-20-regular.svg' "$BUTTON" >/dev/null
grep -F 'document-code-16-regular.svg' "$BUTTON" >/dev/null
grep -F 'key-multiple-20-regular.svg' "$BUTTON" >/dev/null

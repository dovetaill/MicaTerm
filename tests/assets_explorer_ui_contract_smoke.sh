#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROW="$ROOT_DIR/ui/components/asset-node-row.slint"
ASSETS="$ROOT_DIR/ui/shell/assets-sidebar.slint"
APP_WINDOW="$ROOT_DIR/ui/app-window.slint"
TABBAR="$ROOT_DIR/ui/shell/tabbar.slint"

grep -F 'import { ListView } from "std-widgets.slint";' "$ASSETS" >/dev/null
grep -F 'for item in root.console-asset-items : AssetNodeRow' "$ASSETS" >/dev/null
grep -F 'in property <int> depth: 0;' "$ROW" >/dev/null
grep -F 'in property <bool> has-children: false;' "$ROW" >/dev/null
grep -F 'in property <string> path-hint: "";' "$ROW" >/dev/null
grep -F 'in property <string> disclosure-state:' "$ROW" >/dev/null
grep -F 'private property <image> chevron-right-icon:' "$ROW" >/dev/null
grep -F 'private property <image> chevron-down-icon:' "$ROW" >/dev/null
grep -F 'private property <length> row-height: 28px;' "$ROW" >/dev/null
! grep -F 'root.path-hint == "" ? 36px : 48px' "$ROW" >/dev/null
grep -F 'ThemeTokens.explorer-row-hover-surface' "$ROW" >/dev/null
grep -F 'callback toggle-expanded-requested(string);' "$ROW" >/dev/null
! grep -F 'in property <bool> show-disclosure: false;' "$ROW" >/dev/null
! grep -F 'rename-input := TextInput {' "$ROW" >/dev/null
grep -F 'callback asset-selected(string);' "$ASSETS" >/dev/null
grep -F 'items: root.workspace-tab-items;' "$APP_WINDOW" >/dev/null
grep -F 'for item in root.items : ActiveTab {' "$TABBAR" >/dev/null
grep -F 'x: 12px;' "$ASSETS" >/dev/null
grep -F 'width: parent.width - 24px;' "$ASSETS" >/dev/null
grep -F 'disclosure_state: string,' "$ASSETS" >/dev/null
! grep -F 'show_disclosure: bool,' "$ASSETS" >/dev/null
! grep -F 'renaming: bool,' "$ASSETS" >/dev/null
! grep -F 'rename_text: string,' "$ASSETS" >/dev/null
! grep -F 'ThemeTokens.control-hover-surface' "$ROW" >/dev/null

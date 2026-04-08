#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TOKENS="$ROOT_DIR/ui/theme/tokens.slint"
TITLEBAR="$ROOT_DIR/ui/shell/titlebar.slint"
SIDEBAR="$ROOT_DIR/ui/shell/sidebar.slint"
ASSETS="$ROOT_DIR/ui/shell/assets-sidebar.slint"
APP_WINDOW="$ROOT_DIR/ui/app-window.slint"
RIGHT_PANEL="$ROOT_DIR/ui/shell/right-panel.slint"
TABBAR="$ROOT_DIR/ui/shell/tabbar.slint"
ACTIVE_TAB="$ROOT_DIR/ui/components/active-tab.slint"
SEARCH="$ROOT_DIR/ui/components/assets-search-popover.slint"
STATUS_PILL="$ROOT_DIR/ui/components/status-pill.slint"
WORKSPACE="$ROOT_DIR/ui/shell/workspace-pane.slint"

for token in \
  'out property <brush> window-surface:' \
  'out property <brush> titlebar-surface:' \
  'out property <brush> activity-surface:' \
  'out property <brush> assets-surface:' \
  'out property <brush> tabbar-surface:' \
  'out property <brush> tab-active-surface:' \
  'out property <brush> tab-inactive-surface:' \
  'out property <brush> tab-active-indicator:' \
  'out property <brush> workspace-surface:' \
  'out property <brush> inspector-surface:' \
  'out property <brush> panel-surface:' \
  'out property <brush> input-surface:' \
  'out property <brush> input-border:' \
  'out property <brush> input-focus-ring:' \
  'out property <brush> text-secondary:' \
  'out property <brush> text-muted:' \
  'out property <brush> divider-subtle:' \
  'out property <brush> divider-strong:' \
  'out property <brush> control-hover-surface:' \
  'out property <brush> control-active-surface:' \
  'out property <brush> status-pill-surface:' \
  'out property <brush> status-pill-border:'
do
  grep -F "$token" "$TOKENS" >/dev/null
done

grep -F 'background: ThemeTokens.titlebar-surface;' "$TITLEBAR" >/dev/null
grep -F 'background: ThemeTokens.activity-surface;' "$SIDEBAR" >/dev/null
grep -F 'background: ThemeTokens.assets-surface;' "$ASSETS" >/dev/null
grep -F 'background: ThemeTokens.workspace-surface;' "$WORKSPACE" >/dev/null
grep -F 'background: ThemeTokens.inspector-surface;' "$RIGHT_PANEL" >/dev/null
grep -F 'background: ThemeTokens.tabbar-surface;' "$TABBAR" >/dev/null
grep -F 'ThemeTokens.tab-active-surface' "$ACTIVE_TAB" >/dev/null
grep -F 'ThemeTokens.tab-inactive-surface' "$ACTIVE_TAB" >/dev/null
grep -F 'ThemeTokens.tab-active-indicator' "$ACTIVE_TAB" >/dev/null
grep -F 'ThemeTokens.input-surface' "$SEARCH" >/dev/null
grep -F 'ThemeTokens.input-border' "$SEARCH" >/dev/null
grep -F 'ThemeTokens.input-focus-ring' "$SEARCH" >/dev/null
grep -F 'ThemeTokens.status-pill-surface' "$STATUS_PILL" >/dev/null
grep -F 'ThemeTokens.status-pill-border' "$STATUS_PILL" >/dev/null
grep -F 'border-color: ThemeTokens.divider-subtle;' "$APP_WINDOW" >/dev/null
grep -F 'background: ThemeTokens.divider-strong;' "$RIGHT_PANEL" >/dev/null

if rg -n 'ThemeTokens\.(shell-surface|shell-stroke|command-tint|panel-tint|terminal-surface)' "$ROOT_DIR/ui" >/dev/null; then
  echo "obsolete generic surface token reference remains under ui/" >&2
  exit 1
fi

if rg -n 'out property <brush> (shell-surface|shell-stroke|command-tint|panel-tint|terminal-surface)' "$TOKENS" >/dev/null; then
  echo "obsolete generic surface token alias remains in ui/theme/tokens.slint" >&2
  exit 1
fi

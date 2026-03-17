#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TOKENS="$ROOT_DIR/ui/theme/tokens.slint"
TITLEBAR="$ROOT_DIR/ui/shell/titlebar.slint"
SIDEBAR="$ROOT_DIR/ui/shell/sidebar.slint"
ASSETS="$ROOT_DIR/ui/shell/assets-sidebar.slint"
APP_WINDOW="$ROOT_DIR/ui/app-window.slint"
RIGHT_PANEL="$ROOT_DIR/ui/shell/right-panel.slint"

for token in \
  'out property <brush> window-surface:' \
  'out property <brush> titlebar-surface:' \
  'out property <brush> activity-surface:' \
  'out property <brush> assets-surface:' \
  'out property <brush> workspace-surface:' \
  'out property <brush> inspector-surface:' \
  'out property <brush> divider-subtle:' \
  'out property <brush> divider-strong:' \
  'out property <brush> control-hover-surface:' \
  'out property <brush> control-active-surface:'
do
  grep -F "$token" "$TOKENS" >/dev/null
done

grep -F 'background: ThemeTokens.titlebar-surface;' "$TITLEBAR" >/dev/null
grep -F 'background: ThemeTokens.activity-surface;' "$SIDEBAR" >/dev/null
grep -F 'background: ThemeTokens.assets-surface;' "$ASSETS" >/dev/null
grep -F 'background: ThemeTokens.workspace-surface;' "$APP_WINDOW" >/dev/null
grep -F 'background: ThemeTokens.inspector-surface;' "$RIGHT_PANEL" >/dev/null
grep -F 'border-color: ThemeTokens.divider-subtle;' "$APP_WINDOW" >/dev/null
grep -F 'background: ThemeTokens.divider-strong;' "$RIGHT_PANEL" >/dev/null

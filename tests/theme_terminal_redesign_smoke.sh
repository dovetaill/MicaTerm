#!/usr/bin/env bash
set -euo pipefail

TOKENS=ui/theme/tokens.slint
ACTIVE_TAB=ui/components/active-tab.slint
ASSET_ROW=ui/components/asset-node-row.slint

grep -F 'out property <brush> titlebar-background:' "$TOKENS"
grep -F 'out property <brush> terminal-surface-background:' "$TOKENS"
grep -F 'out property <brush> sidebar-item-selected-border:' "$TOKENS"
grep -F 'ThemeTokens.tab-active-line' "$ACTIVE_TAB"
grep -F 'ThemeTokens.sidebar-item-selected-background' "$ASSET_ROW"

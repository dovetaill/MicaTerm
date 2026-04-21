#!/usr/bin/env bash
set -euo pipefail

TOKENS=ui/theme/tokens.slint
ACTIVE_TAB=ui/components/active-tab.slint
ASSET_ROW=ui/components/asset-node-row.slint
SIDEBAR_BUTTON=ui/components/sidebar-nav-button.slint
SETTINGS=ui/components/settings-modal.slint

grep -F 'in property <string> theme-variant: "premium-default";' "$TOKENS"
grep -F 'out property <brush> titlebar-background:' "$TOKENS"
grep -F 'out property <brush> tab-active-text:' "$TOKENS"
grep -F 'out property <brush> sidebar-item-selected-indicator:' "$TOKENS"
grep -F 'out property <brush> terminal-surface-background:' "$TOKENS"
grep -F 'out property <brush> sidebar-item-selected-border:' "$TOKENS"
grep -F 'ThemeTokens.tab-active-text' "$ACTIVE_TAB"
grep -F 'ThemeTokens.tab-inactive-text' "$ACTIVE_TAB"
grep -F 'ThemeTokens.sidebar-item-selected-indicator' "$SIDEBAR_BUTTON"
grep -F 'ThemeTokens.sidebar-item-selected-background' "$ASSET_ROW"
grep -F 'ThemeTokens.sidebar-text-active' "$ASSET_ROW"
grep -F 'theme-variant' "$SETTINGS"
grep -F 'terminal-input-highlighting-enabled' "$SETTINGS"
grep -F 'terminal-output-rule-profile' "$SETTINGS"
grep -F 'terminal-command-decorations-enabled' "$SETTINGS"
grep -F 'terminal-overview-markers-enabled' "$SETTINGS"
grep -F 'terminal-search-match-highlight' "$SETTINGS"
if grep -F 'terminal-accent-color' "$SETTINGS" >/dev/null; then
  echo "settings modal should not expose raw per-color tuning" >&2
  exit 1
fi

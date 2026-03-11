#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BUTTON="$ROOT_DIR/ui/components/sidebar-nav-button.slint"
SIDEBAR="$ROOT_DIR/ui/shell/sidebar.slint"
APP_WINDOW="$ROOT_DIR/ui/app-window.slint"
TOOLTIP="$ROOT_DIR/ui/components/titlebar-tooltip.slint"

grep -F 'callback tooltip-open-requested(string, string, length, length, length);' "$BUTTON" >/dev/null
grep -F 'callback tooltip-close-requested(string);' "$BUTTON" >/dev/null
grep -F 'tooltip-text: root.show-assets-sidebar ? "Collapse sidebar" : "Expand sidebar";' "$SIDEBAR" >/dev/null
grep -F 'tooltip-text: item.label;' "$SIDEBAR" >/dev/null
grep -F 'out property <string> tooltip-text' "$SIDEBAR" >/dev/null
grep -F 'sidebar-tooltip-overlay := TitlebarTooltip' "$APP_WINDOW" >/dev/null
grep -F 'place-right: true;' "$APP_WINDOW" >/dev/null
grep -F 'anchor-width: sidebar.tooltip-anchor-width;' "$APP_WINDOW" >/dev/null
grep -F 'host-height: root.height;' "$APP_WINDOW" >/dev/null
grep -F 'in property <bool> place-right: false;' "$TOOLTIP" >/dev/null
grep -F 'in property <length> anchor-width: 0px;' "$TOOLTIP" >/dev/null
grep -F 'in property <length> host-height: 0px;' "$TOOLTIP" >/dev/null

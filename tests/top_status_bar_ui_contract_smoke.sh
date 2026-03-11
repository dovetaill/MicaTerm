#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TITLEBAR="$ROOT_DIR/ui/shell/titlebar.slint"
TOOLTIP="$ROOT_DIR/ui/components/titlebar-tooltip.slint"

[[ -f "$TOOLTIP" ]] || {
  echo "missing ui/components/titlebar-tooltip.slint" >&2
  exit 1
}

grep -F 'menu-button := TitlebarIconButton' "$TITLEBAR" >/dev/null
grep -F 'panel-toggle-button := TitlebarIconButton' "$TITLEBAR" >/dev/null
grep -F 'tooltip-popup := TitlebarTooltip' "$TITLEBAR" >/dev/null
grep -F 'tooltip-delay := Timer' "$TITLEBAR" >/dev/null
grep -F 'tooltip-text: "Open menu"' "$TITLEBAR" >/dev/null
grep -F 'tooltip-text: "Toggle right panel"' "$TITLEBAR" >/dev/null
grep -F 'tooltip-text: "Minimize window"' "$TITLEBAR" >/dev/null
grep -F 'tooltip-text: "Close window"' "$TITLEBAR" >/dev/null
grep -F 'menu-button.absolute-position.x' "$TITLEBAR" >/dev/null
grep -F 'menu-button.absolute-position.y' "$TITLEBAR" >/dev/null
grep -F 'menu-icon-regular: @image-url("../../assets/icons/fluent/menu-20-regular.svg")' "$TITLEBAR" >/dev/null
grep -F 'panel-right-icon-regular: @image-url("../../assets/icons/fluent/panel-right-20-regular.svg")' "$TITLEBAR" >/dev/null
grep -F 'icon-source: root.minimize-icon' "$TITLEBAR" >/dev/null
grep -F 'icon-source: root.is-window-maximized ? root.restore-icon : root.maximize-icon' "$TITLEBAR" >/dev/null
grep -F 'icon-source: root.close-icon' "$TITLEBAR" >/dev/null
! grep -F 'actions-zone.absolute-position.x' "$TITLEBAR" >/dev/null
! grep -F 'label: "S"' "$TITLEBAR" >/dev/null
! grep -F 'label: "M"' "$TITLEBAR" >/dev/null
! grep -F 'label: root.show-right-panel ? "R-" : "R+"' "$TITLEBAR" >/dev/null
! grep -F 'label: "-"' "$TITLEBAR" >/dev/null
! grep -F 'label: "X"' "$TITLEBAR" >/dev/null

#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TITLEBAR="$ROOT_DIR/ui/shell/titlebar.slint"
TOOLTIP="$ROOT_DIR/ui/components/titlebar-tooltip.slint"
APP_WINDOW="$ROOT_DIR/ui/app-window.slint"

[[ -f "$TOOLTIP" ]] || {
  echo "missing ui/components/titlebar-tooltip.slint" >&2
  exit 1
}

grep -F 'nav-button := TitlebarIconButton' "$TITLEBAR" >/dev/null
grep -F 'theme-button := TitlebarIconButton' "$TITLEBAR" >/dev/null
grep -F 'panel-toggle-button := TitlebarIconButton' "$TITLEBAR" >/dev/null
grep -F 'pin-button := TitlebarIconButton' "$TITLEBAR" >/dev/null
grep -F 'brand-logotype := Image' "$TITLEBAR" >/dev/null
grep -F 'out property <string> tooltip-text <=> root.tooltip-text-value;' "$TITLEBAR" >/dev/null
grep -F 'out property <bool> tooltip-visible <=> root.tooltip-visible-value;' "$TITLEBAR" >/dev/null
grep -F 'out property <length> tooltip-anchor-x <=> root.tooltip-anchor-x-value;' "$TITLEBAR" >/dev/null
grep -F 'out property <length> tooltip-anchor-y <=> root.tooltip-anchor-y-value;' "$TITLEBAR" >/dev/null
grep -F 'tooltip-source-id-value' "$TITLEBAR" >/dev/null
grep -F 'tooltip-delay := Timer' "$TITLEBAR" >/dev/null
grep -F 'tooltip-close-delay := Timer' "$TITLEBAR" >/dev/null
grep -F 'tooltip-delay.start();' "$TITLEBAR" >/dev/null
grep -F 'tooltip-close-delay.start();' "$TITLEBAR" >/dev/null
grep -F 'tooltip-close-source-id-value' "$TITLEBAR" >/dev/null
grep -F 'function queue-tooltip-close(source-id: string)' "$TITLEBAR" >/dev/null
grep -F 'root.tooltip-source-id-value == source-id && root.tooltip-text-value == text' "$TITLEBAR" >/dev/null
grep -F 'tooltip-text: "Open menu"' "$TITLEBAR" >/dev/null
grep -F '"Switch to dark mode"' "$TITLEBAR" >/dev/null
grep -F '"Switch to light mode"' "$TITLEBAR" >/dev/null
grep -F 'tooltip-text: "Toggle right panel"' "$TITLEBAR" >/dev/null
grep -F '"Pin window on top"' "$TITLEBAR" >/dev/null
grep -F '"Unpin window from top"' "$TITLEBAR" >/dev/null
grep -F 'tooltip-text: "Minimize window"' "$TITLEBAR" >/dev/null
grep -F 'tooltip-text: "Close window"' "$TITLEBAR" >/dev/null
grep -F 'pointer-event(event)' "$TITLEBAR" >/dev/null
grep -F 'PointerEventKind.down' "$TITLEBAR" >/dev/null
grep -F 'nav-button.absolute-position.x' "$TITLEBAR" >/dev/null
grep -F 'nav-button.absolute-position.y' "$TITLEBAR" >/dev/null
grep -F 'titlebar := Titlebar {' "$APP_WINDOW" >/dev/null
grep -F 'tooltip-overlay := TitlebarTooltip {' "$APP_WINDOW" >/dev/null
grep -F 'text: titlebar.tooltip-text;' "$APP_WINDOW" >/dev/null
grep -F 'anchor-x: titlebar.tooltip-anchor-x;' "$APP_WINDOW" >/dev/null
grep -F 'anchor-y: titlebar.tooltip-anchor-y;' "$APP_WINDOW" >/dev/null
grep -F 'tooltip-visible: titlebar.tooltip-visible;' "$APP_WINDOW" >/dev/null
grep -F 'host-width: root.width;' "$APP_WINDOW" >/dev/null
grep -F 'navigation-24-regular.svg' "$TITLEBAR" >/dev/null
grep -F 'mica-term-header-logotype.svg' "$TITLEBAR" >/dev/null
grep -F 'dark-theme-20-regular.svg' "$TITLEBAR" >/dev/null
grep -F 'weather-sunny-20-regular.svg' "$TITLEBAR" >/dev/null
grep -F 'panel-right-icon-regular: @image-url("../../assets/icons/fluent/panel-right-20-regular.svg")' "$TITLEBAR" >/dev/null
grep -F 'pin-20-regular.svg' "$TITLEBAR" >/dev/null
grep -F 'pin-off-20-regular.svg' "$TITLEBAR" >/dev/null
grep -F 'divider-line := Rectangle' "$TITLEBAR" >/dev/null
grep -F 'icon-source: root.minimize-icon' "$TITLEBAR" >/dev/null
grep -F 'icon-source: root.is-window-maximized ? root.restore-icon : root.maximize-icon' "$TITLEBAR" >/dev/null
grep -F 'icon-source: root.close-icon' "$TITLEBAR" >/dev/null
grep -F 'in property <string> tooltip-source-id;' "$ROOT_DIR/ui/components/titlebar-icon-button.slint" >/dev/null
grep -F 'in property <string> tooltip-source-id;' "$ROOT_DIR/ui/components/window-control-button.slint" >/dev/null
grep -F 'callback tooltip-open-requested(string, string, length, length);' "$ROOT_DIR/ui/components/titlebar-icon-button.slint" >/dev/null
grep -F 'callback tooltip-close-requested(string);' "$ROOT_DIR/ui/components/window-control-button.slint" >/dev/null
grep -F 'visible: root.tooltip-visible;' "$TOOLTIP" >/dev/null
grep -F 'TITLEBAR_TOOLTIP_DELAY_MS' "$ROOT_DIR/src/shell/metrics.rs" >/dev/null
grep -F 'TITLEBAR_TOOLTIP_CLOSE_DEBOUNCE_MS' "$ROOT_DIR/src/shell/metrics.rs" >/dev/null
[[ ! -f "$ROOT_DIR/src/app/tooltip_debug_log.rs" ]]
! grep -F 'inherits PopupWindow' "$TOOLTIP" >/dev/null
! grep -F 'tooltip-overlay := TitlebarTooltip' "$TITLEBAR" >/dev/null
! grep -F 'tooltip-delay.restart();' "$TITLEBAR" >/dev/null
! grep -F 'tooltip-close-delay.restart();' "$TITLEBAR" >/dev/null
! grep -F 'tooltip-debug-event-requested' "$TITLEBAR" >/dev/null
! grep -F 'tooltip-debug-event-requested' "$APP_WINDOW" >/dev/null
! grep -F 'bind_top_status_bar_with_store_and_log_dir' "$ROOT_DIR/src/app/bootstrap.rs" >/dev/null
! grep -F 'bind_top_status_bar_with_store_and_effects_and_log_dir' "$ROOT_DIR/src/app/bootstrap.rs" >/dev/null
! grep -F 'titlebar-tooltip.log' "$ROOT_DIR/src/app/bootstrap.rs" >/dev/null
! grep -F 'actions-zone.absolute-position.x' "$TITLEBAR" >/dev/null
! grep -F 'text: "Workspace"' "$TITLEBAR" >/dev/null
! grep -F 'text: "SSH"' "$TITLEBAR" >/dev/null
! grep -F 'label: "S"' "$TITLEBAR" >/dev/null
! grep -F 'label: "M"' "$TITLEBAR" >/dev/null
! grep -F 'label: root.show-right-panel ? "R-" : "R+"' "$TITLEBAR" >/dev/null
! grep -F 'label: "-"' "$TITLEBAR" >/dev/null
! grep -F 'label: "X"' "$TITLEBAR" >/dev/null

if grep -F 'moved => {' "$TITLEBAR" >/dev/null; then
    exit 1
fi

#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_WINDOW="$ROOT_DIR/ui/app-window.slint"
SIDEBAR="$ROOT_DIR/ui/shell/sidebar.slint"
RIGHT_PANEL="$ROOT_DIR/ui/shell/right-panel.slint"
WELCOME="$ROOT_DIR/ui/welcome/welcome-view.slint"

grep -F 'shell-frame := Rectangle' "$APP_WINDOW" >/dev/null
grep -F 'body-host := Rectangle' "$APP_WINDOW" >/dev/null
grep -F 'vertical-stretch: 1;' "$APP_WINDOW" >/dev/null
grep -F 'shell-body := HorizontalLayout' "$APP_WINDOW" >/dev/null
grep -F 'show-assets-sidebar: root.effective-show-assets-sidebar;' "$APP_WINDOW" >/dev/null
grep -F 'expanded: root.effective-show-right-panel;' "$APP_WINDOW" >/dev/null
grep -F 'visible: root.expanded;' "$RIGHT_PANEL" >/dev/null
grep -F 'VerticalLayout {' "$WELCOME" >/dev/null
grep -F 'activity-bar := Rectangle' "$SIDEBAR" >/dev/null

BODY_HOST_BLOCK="$(sed -n '/body-host := Rectangle {/,/shell-body := HorizontalLayout {/p' "$APP_WINDOW")"
grep -F 'y: titlebar.height;' <<<"$BODY_HOST_BLOCK" >/dev/null
grep -F 'height: max(0px, parent.height - titlebar.height);' <<<"$BODY_HOST_BLOCK" >/dev/null

if grep -F 'height: root.shell-body-height-cache;' <<<"$BODY_HOST_BLOCK" >/dev/null; then
    echo "body-host should not clamp shell height with a fixed cache" >&2
    exit 1
fi

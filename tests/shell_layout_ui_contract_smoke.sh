#!/usr/bin/env bash
# Verifies the exported Slint layout hooks used by Rust shell layout logic.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_WINDOW="$ROOT_DIR/ui/app-window.slint"
SIDEBAR="$ROOT_DIR/ui/shell/sidebar.slint"
RIGHT_PANEL="$ROOT_DIR/ui/shell/right-panel.slint"
WORKSPACE_PANE="$ROOT_DIR/ui/shell/workspace-pane.slint"
WELCOME="$ROOT_DIR/ui/welcome/welcome-view.slint"

grep -F 'shell-frame := Rectangle' "$APP_WINDOW" >/dev/null
grep -F 'chrome-host := Rectangle' "$APP_WINDOW" >/dev/null
grep -F 'body-host := Rectangle' "$APP_WINDOW" >/dev/null
grep -F 'border-radius: 0px;' "$APP_WINDOW" >/dev/null
grep -F 'clip: true;' "$APP_WINDOW" >/dev/null
grep -F 'vertical-stretch: 1;' "$APP_WINDOW" >/dev/null
grep -F 'shell-body := Rectangle' "$APP_WINDOW" >/dev/null
grep -F 'WorkspacePane' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <length> assets-sidebar-expanded-width: 320px;' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <length> right-panel-expanded-width: 392px;' "$APP_WINDOW" >/dev/null
grep -F 'callback assets-sidebar-edge-toggle-requested();' "$APP_WINDOW" >/dev/null
grep -F 'callback assets-sidebar-edge-drag-start-requested(length);' "$APP_WINDOW" >/dev/null
grep -F 'callback assets-sidebar-edge-drag-move-requested(length);' "$APP_WINDOW" >/dev/null
grep -F 'callback assets-sidebar-edge-drag-end-requested(length);' "$APP_WINDOW" >/dev/null
grep -F 'callback right-panel-edge-toggle-requested();' "$APP_WINDOW" >/dev/null
grep -F 'callback right-panel-edge-drag-start-requested(length);' "$APP_WINDOW" >/dev/null
grep -F 'callback right-panel-edge-drag-move-requested(length);' "$APP_WINDOW" >/dev/null
grep -F 'callback right-panel-edge-drag-end-requested(length);' "$APP_WINDOW" >/dev/null
grep -F 'show-assets-sidebar: root.effective-show-assets-sidebar;' "$APP_WINDOW" >/dev/null
grep -F 'assets-sidebar-expanded-width: root.assets-sidebar-expanded-width;' "$APP_WINDOW" >/dev/null
grep -F 'expanded: root.effective-show-right-panel;' "$APP_WINDOW" >/dev/null
grep -F 'expanded-width: root.right-panel-expanded-width;' "$APP_WINDOW" >/dev/null
grep -F 'horizontal-stretch: 1;' "$WORKSPACE_PANE" >/dev/null
grep -F 'min-width: 0px;' "$WORKSPACE_PANE" >/dev/null
grep -F 'width: 100%;' "$WORKSPACE_PANE" >/dev/null
grep -F 'in property <length> expanded-width: 392px;' "$RIGHT_PANEL" >/dev/null
grep -F 'callback right-panel-edge-toggle-requested();' "$RIGHT_PANEL" >/dev/null
grep -F 'callback right-panel-edge-drag-start-requested(length);' "$RIGHT_PANEL" >/dev/null
grep -F 'callback right-panel-edge-drag-move-requested(length);' "$RIGHT_PANEL" >/dev/null
grep -F 'callback right-panel-edge-drag-end-requested(length);' "$RIGHT_PANEL" >/dev/null
grep -F 'left-divider := Rectangle {' "$RIGHT_PANEL" >/dev/null
grep -F 'visible: root.expanded;' "$RIGHT_PANEL" >/dev/null
grep -F 'border-radius: 0px;' "$RIGHT_PANEL" >/dev/null
grep -F 'border-width: 0px;' "$RIGHT_PANEL" >/dev/null
! grep -F 'width: root.expanded ? 392px : 0px;' "$RIGHT_PANEL" >/dev/null
grep -F 'VerticalLayout {' "$WELCOME" >/dev/null
grep -F 'QuickLaunchDetailPane' "$WELCOME" >/dev/null
grep -F 'activity-bar := Rectangle' "$SIDEBAR" >/dev/null
grep -F 'in property <length> assets-sidebar-expanded-width: 320px;' "$SIDEBAR" >/dev/null
grep -F 'callback assets-sidebar-edge-toggle-requested();' "$SIDEBAR" >/dev/null
grep -F 'callback assets-sidebar-edge-drag-start-requested(length);' "$SIDEBAR" >/dev/null
grep -F 'callback assets-sidebar-edge-drag-move-requested(length);' "$SIDEBAR" >/dev/null
grep -F 'callback assets-sidebar-edge-drag-end-requested(length);' "$SIDEBAR" >/dev/null
! grep -F 'width: 44px + (root.show-assets-sidebar ? 320px : 0px);' "$SIDEBAR" >/dev/null

! grep -F 'border-radius: 14px;' "$RIGHT_PANEL" >/dev/null

BODY_HOST_BLOCK="$(sed -n '/body-host := Rectangle {/,/shell-body := Rectangle {/p' "$APP_WINDOW")"
grep -F 'y: titlebar.height;' <<<"$BODY_HOST_BLOCK" >/dev/null
grep -F 'height: max(0px, parent.height - titlebar.height);' <<<"$BODY_HOST_BLOCK" >/dev/null

SHELL_BODY_BLOCK="$(sed -n '/shell-body := Rectangle {/,/expanded: root.effective-show-right-panel;/p' "$APP_WINDOW")"
grep -F 'x: sidebar.width;' <<<"$SHELL_BODY_BLOCK" >/dev/null
grep -F 'width: max(0px, parent.width - sidebar.width - right-panel.width);' <<<"$SHELL_BODY_BLOCK" >/dev/null
grep -F 'x: parent.width - self.width;' <<<"$SHELL_BODY_BLOCK" >/dev/null

if grep -F 'height: root.shell-body-height-cache;' <<<"$BODY_HOST_BLOCK" >/dev/null; then
    echo "body-host should not clamp shell height with a fixed cache" >&2
    exit 1
fi

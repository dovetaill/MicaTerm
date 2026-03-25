#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_WINDOW="$ROOT_DIR/ui/app-window.slint"
HOST_KEY_MODAL="$ROOT_DIR/ui/components/ssh-host-key-confirm-modal.slint"
ACTIVE_TAB="$ROOT_DIR/ui/components/active-tab.slint"
TABBAR="$ROOT_DIR/ui/shell/tabbar.slint"
WORKSPACE_HOST="$ROOT_DIR/ui/shell/terminal-session-host.slint"

grep -F 'export component SshHostKeyConfirmModal inherits Rectangle {' "$HOST_KEY_MODAL" >/dev/null
grep -F 'callback accept-requested();' "$HOST_KEY_MODAL" >/dev/null
grep -F 'callback reject-requested();' "$HOST_KEY_MODAL" >/dev/null
grep -F 'in-out property <bool> ssh-host-key-modal-open: false;' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <[WorkspaceTabItem]> workspace-tab-items: [];' "$APP_WINDOW" >/dev/null
grep -F 'callback workspace-tab-selected(string);' "$APP_WINDOW" >/dev/null
grep -F 'callback workspace-tab-close-requested(string);' "$APP_WINDOW" >/dev/null
grep -F 'callback workspace-session-text-input(string);' "$APP_WINDOW" >/dev/null
grep -F 'callback workspace-session-key-input(string, bool, bool, bool);' "$APP_WINDOW" >/dev/null
grep -F 'callback workspace-session-resize-requested(int, int);' "$APP_WINDOW" >/dev/null
! grep -F 'width: 216px;' "$TABBAR" >/dev/null
! grep -F 'horizontal-stretch: 1;' "$TABBAR" >/dev/null
grep -F 'min-width: 0px;' "$ACTIVE_TAB" >/dev/null
grep -F 'callback close-requested();' "$ACTIVE_TAB" >/dev/null
grep -F 'overflow: elide;' "$ACTIVE_TAB" >/dev/null
! grep -F 'text: root.subtitle;' "$ACTIVE_TAB" >/dev/null
grep -F 'content-hit-target := TouchArea {' "$ACTIVE_TAB" >/dev/null
grep -F 'close-hit-target := TouchArea {' "$ACTIVE_TAB" >/dev/null
grep -F 'connecting' "$TABBAR" >/dev/null
grep -F 'error' "$TABBAR" >/dev/null
grep -F 'export component TerminalSessionHost inherits Rectangle {' "$WORKSPACE_HOST" >/dev/null
grep -F 'callback text-input(string);' "$WORKSPACE_HOST" >/dev/null
grep -F 'callback key-input(string, bool, bool, bool);' "$WORKSPACE_HOST" >/dev/null
grep -F 'callback surface-resize-requested(int, int);' "$WORKSPACE_HOST" >/dev/null
grep -F 'in property <string> session-error-detail: "";' "$WORKSPACE_HOST" >/dev/null
! grep -F 'Interactive terminal ready.' "$WORKSPACE_HOST" >/dev/null
! grep -F 'Terminal Session' "$WORKSPACE_HOST" >/dev/null
! grep -F 'if root.session-subtitle != ""' "$WORKSPACE_HOST" >/dev/null
! grep -F 'Review credentials, host key, or network reachability.' "$WORKSPACE_HOST" >/dev/null
! grep -F 'Remote shell is ready but has not produced output yet.' "$WORKSPACE_HOST" >/dev/null

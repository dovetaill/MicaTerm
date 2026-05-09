#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_WINDOW="$ROOT_DIR/ui/app-window.slint"
HOST_KEY_MODAL="$ROOT_DIR/ui/components/ssh-host-key-confirm-modal.slint"
ACTIVE_TAB="$ROOT_DIR/ui/components/active-tab.slint"
TABBAR="$ROOT_DIR/ui/shell/tabbar.slint"
WORKSPACE_PANE="$ROOT_DIR/ui/shell/workspace-pane.slint"
WORKSPACE_HOST="$ROOT_DIR/ui/shell/terminal-session-host.slint"

grep -F 'export component SshHostKeyConfirmModal inherits Rectangle {' "$HOST_KEY_MODAL" >/dev/null
grep -F 'callback accept-requested();' "$HOST_KEY_MODAL" >/dev/null
grep -F 'callback reject-requested();' "$HOST_KEY_MODAL" >/dev/null
grep -F 'in-out property <bool> ssh-host-key-modal-open: false;' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <[WorkspaceTabItem]> workspace-tab-items: [];' "$APP_WINDOW" >/dev/null
grep -F 'callback workspace-tab-selected(string);' "$APP_WINDOW" >/dev/null
grep -F 'callback workspace-tab-close-requested(string);' "$APP_WINDOW" >/dev/null
grep -F 'callback workspace-new-tab-requested();' "$APP_WINDOW" >/dev/null
grep -F 'callback workspace-session-text-input(string);' "$APP_WINDOW" >/dev/null
grep -F 'callback workspace-session-key-input(string, bool, bool, bool);' "$APP_WINDOW" >/dev/null
grep -F 'callback workspace-session-resize-requested(int, int);' "$APP_WINDOW" >/dev/null
! grep -F 'width: 216px;' "$TABBAR" >/dev/null
grep -F 'for item[index] in root.items : ActiveTab {' "$TABBAR" >/dev/null
grep -F '@image-url("../../assets/icons/fluent/add-20-regular.svg")' "$TABBAR" >/dev/null
grep -F 'callback new-tab-requested();' "$TABBAR" >/dev/null
grep -F 'new-tab-requested => {' "$WORKSPACE_PANE" >/dev/null
grep -F 'horizontal-stretch: 0;' "$TABBAR" >/dev/null
grep -F 'trailing-spacer := Rectangle {' "$TABBAR" >/dev/null
grep -F 'background: ThemeTokens.titlebar-background;' "$TABBAR" >/dev/null
grep -F 'if root.workspace-tab-items.length > 0 : tab-strip := TabBar {' "$WORKSPACE_PANE" >/dev/null
grep -F 'min-width: 0px;' "$ACTIVE_TAB" >/dev/null
grep -F 'callback close-requested();' "$ACTIVE_TAB" >/dev/null
grep -F 'overflow: elide;' "$ACTIVE_TAB" >/dev/null
! grep -F 'text: root.subtitle;' "$ACTIVE_TAB" >/dev/null
grep -F '@image-url("../../assets/icons/fluent/dismiss-20-regular.svg")' "$ACTIVE_TAB" >/dev/null
grep -F 'root.state == "launcher" ? ThemeTokens.divider-subtle' "$ACTIVE_TAB" >/dev/null
! grep -F 'text: "×";' "$ACTIVE_TAB" >/dev/null
! grep -F 'background: root.active ? ThemeTokens.accent : transparent;' "$ACTIVE_TAB" >/dev/null
grep -F 'content-hit-target := TouchArea {' "$ACTIVE_TAB" >/dev/null
grep -F 'close-hit-target := TouchArea {' "$ACTIVE_TAB" >/dev/null
grep -F 'close-visible' "$ACTIVE_TAB" >/dev/null
if grep -F 'width: root.close-visible ? close-button.x : parent.width;' "$ACTIVE_TAB" >/dev/null; then
    exit 1
fi
grep -F 'connecting' "$TABBAR" >/dev/null
grep -F 'error' "$TABBAR" >/dev/null
grep -F 'export component TerminalSessionHost inherits Rectangle {' "$WORKSPACE_HOST" >/dev/null
grep -F 'callback text-input(string);' "$WORKSPACE_HOST" >/dev/null
grep -F 'callback key-input(string, bool, bool, bool);' "$WORKSPACE_HOST" >/dev/null
grep -F 'callback surface-resize-requested(int, int);' "$WORKSPACE_HOST" >/dev/null
grep -F 'in property <string> session-error-detail: "";' "$WORKSPACE_HOST" >/dev/null
grep -F 'workspace-session-connection-steps' "$APP_WINDOW" >/dev/null
grep -F 'workspace-session-connection-diagnostics' "$APP_WINDOW" >/dev/null
grep -F 'workspace-session-connection-current-detail' "$APP_WINDOW" >/dev/null
grep -F 'workspace-session-connection-steps' "$WORKSPACE_PANE" >/dev/null
grep -F 'workspace-session-connection-diagnostics' "$WORKSPACE_PANE" >/dev/null
grep -F 'if root.mode == "connection-progress"' "$WORKSPACE_HOST" >/dev/null
grep -F 'summary-header := Rectangle {' "$WORKSPACE_HOST" >/dev/null
grep -F 'workflow-rail := Rectangle {' "$WORKSPACE_HOST" >/dev/null
grep -F 'current-task-panel := Rectangle {' "$WORKSPACE_HOST" >/dev/null
grep -F 'diagnostics-section := Rectangle {' "$WORKSPACE_HOST" >/dev/null
grep -F 'Trust key' "$WORKSPACE_HOST" >/dev/null
grep -F 'Diagnostics' "$WORKSPACE_HOST" >/dev/null
grep -F 'Copy details' "$WORKSPACE_HOST" >/dev/null
grep -F 'trust-host-key' "$WORKSPACE_HOST" >/dev/null
grep -F 'reject-host-key' "$WORKSPACE_HOST" >/dev/null
! grep -F 'Trust and Continue' "$WORKSPACE_HOST" >/dev/null
! grep -F 'Reject' "$WORKSPACE_HOST" >/dev/null
! grep -F 'Show Diagnostics' "$WORKSPACE_HOST" >/dev/null
! grep -F 'Hide Diagnostics' "$WORKSPACE_HOST" >/dev/null
! grep -F 'Copy Diagnostics' "$WORKSPACE_HOST" >/dev/null
! grep -F 'header-card := Rectangle {' "$WORKSPACE_HOST" >/dev/null
! grep -F 'timeline-card := Rectangle {' "$WORKSPACE_HOST" >/dev/null
! grep -F 'current-detail-card := Rectangle {' "$WORKSPACE_HOST" >/dev/null
! grep -F 'host-key-card := Rectangle {' "$WORKSPACE_HOST" >/dev/null
! grep -F 'diagnostics-card := Rectangle {' "$WORKSPACE_HOST" >/dev/null
grep -F 'Cancel' "$WORKSPACE_HOST" >/dev/null
grep -F 'Retry' "$WORKSPACE_HOST" >/dev/null
! grep -F 'Interactive terminal ready.' "$WORKSPACE_HOST" >/dev/null
! grep -F 'Terminal Session' "$WORKSPACE_HOST" >/dev/null
! grep -F 'if root.session-subtitle != ""' "$WORKSPACE_HOST" >/dev/null
! grep -F 'Review credentials, host key, or network reachability.' "$WORKSPACE_HOST" >/dev/null
! grep -F 'Remote shell is ready but has not produced output yet.' "$WORKSPACE_HOST" >/dev/null

#!/usr/bin/env bash
# Verifies the welcome quick launch dashboard contract exported by the Slint shell.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WELCOME="$ROOT_DIR/ui/welcome/welcome-view.slint"
APP_WINDOW="$ROOT_DIR/ui/app-window.slint"
WORKSPACE_PANE="$ROOT_DIR/ui/shell/workspace-pane.slint"
SESSION_HOST="$ROOT_DIR/ui/shell/terminal-session-host.slint"
MODAL="$ROOT_DIR/ui/components/open-saved-ssh-modal.slint"

grep -F 'text: "New Tab"' "$WELCOME" >/dev/null
grep -F 'QuickLaunchSection' "$WELCOME" >/dev/null
grep -F 'Open Saved SSH' "$WELCOME" >/dev/null
grep -F 'callback open-saved-ssh-requested();' "$WELCOME" >/dev/null
! grep -F 'favorite-items' "$WELCOME" >/dev/null
! grep -F 'group-items' "$WELCOME" >/dev/null
! grep -F 'visible-group-items' "$WELCOME" >/dev/null
! grep -F 'selected-detail' "$WELCOME" >/dev/null
! grep -F 'search-query' "$WELCOME" >/dev/null
! grep -F 'callback asset-selected' "$WELCOME" >/dev/null
! grep -F 'callback search-changed' "$WELCOME" >/dev/null
! grep -F 'callback connect-in-new-tab-requested' "$WELCOME" >/dev/null
! grep -F 'callback toggle-favorite-requested' "$WELCOME" >/dev/null
! grep -F 'callback reveal-in-assets-requested' "$WELCOME" >/dev/null
! grep -F 'QuickLaunchDetailPane' "$WELCOME" >/dev/null
! grep -F 'search-shell := Rectangle {' "$WELCOME" >/dev/null
! grep -F 'Open Saved SSH Connections' "$WELCOME" >/dev/null
! grep -F 'Environment' "$WELCOME" >/dev/null
! grep -F 'Status' "$WELCOME" >/dev/null
! grep -F 'Favorite' "$WELCOME" >/dev/null
grep -F 'time_label: string' "$ROOT_DIR/ui/welcome/quick-launch-types.slint" >/dev/null
grep -F 'state_label: string' "$ROOT_DIR/ui/welcome/quick-launch-types.slint" >/dev/null
grep -F 'text: root.item.state_label != "" ? root.item.state_label : root.item.time_label' "$ROOT_DIR/ui/welcome/quick-launch-card.slint" >/dev/null
grep -F 'assets/icons/new-tab/open-saved-ssh.svg' "$WELCOME" >/dev/null
grep -F 'assets/icons/new-tab/server-stack.svg' "$ROOT_DIR/ui/welcome/quick-launch-card.slint" >/dev/null
grep -F 'callback welcome-quick-launch-connect-requested(string);' "$APP_WINDOW" >/dev/null
! grep -F 'welcome-quick-launch-favorite-items' "$APP_WINDOW" >/dev/null
! grep -F 'welcome-quick-launch-group-items' "$APP_WINDOW" >/dev/null
! grep -F 'welcome-quick-launch-visible-group-items' "$APP_WINDOW" >/dev/null
! grep -F 'welcome-quick-launch-selected-detail' "$APP_WINDOW" >/dev/null
! grep -F 'welcome-quick-launch-search-query' "$APP_WINDOW" >/dev/null
! grep -F 'callback welcome-quick-launch-toggle-favorite-requested' "$APP_WINDOW" >/dev/null
! grep -F 'callback welcome-quick-launch-reveal-in-assets-requested' "$APP_WINDOW" >/dev/null
! grep -F 'callback welcome-quick-launch-search-changed' "$APP_WINDOW" >/dev/null
grep -F 'callback welcome-open-saved-ssh-requested();' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <bool> open-saved-ssh-modal-open: false;' "$APP_WINDOW" >/dev/null
grep -F 'callback open-saved-ssh-modal-close-requested();' "$APP_WINDOW" >/dev/null
grep -F 'callback open-saved-ssh-modal-query-changed(string);' "$APP_WINDOW" >/dev/null
grep -F 'callback open-saved-ssh-modal-asset-activated(string);' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <bool> open-saved-ssh-modal-can-open-selection: false;' "$APP_WINDOW" >/dev/null
grep -F 'callback open-saved-ssh-modal-activate-selection-requested();' "$APP_WINDOW" >/dev/null
grep -F 'callback welcome-open-saved-ssh-requested();' "$WORKSPACE_PANE" >/dev/null
grep -F 'callback welcome-open-saved-ssh-requested();' "$SESSION_HOST" >/dev/null
! grep -F 'welcome-quick-launch-favorite-items' "$WORKSPACE_PANE" >/dev/null
! grep -F 'welcome-quick-launch-group-items' "$WORKSPACE_PANE" >/dev/null
! grep -F 'welcome-quick-launch-selected-detail' "$WORKSPACE_PANE" >/dev/null
! grep -F 'welcome-quick-launch-search-query' "$WORKSPACE_PANE" >/dev/null
! grep -F 'welcome-quick-launch-favorite-items' "$SESSION_HOST" >/dev/null
! grep -F 'welcome-quick-launch-group-items' "$SESSION_HOST" >/dev/null
! grep -F 'welcome-quick-launch-selected-detail' "$SESSION_HOST" >/dev/null
! grep -F 'welcome-quick-launch-search-query' "$SESSION_HOST" >/dev/null
grep -F 'export component OpenSavedSshModal inherits Rectangle {' "$MODAL" >/dev/null
grep -F 'component SavedSshPickerRow inherits Rectangle {' "$MODAL" >/dev/null
grep -F 'callback activate-selection-requested();' "$MODAL" >/dev/null

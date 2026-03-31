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
grep -F 'Open Saved SSH Connections' "$WELCOME" >/dev/null
grep -F 'callback open-saved-ssh-requested();' "$WELCOME" >/dev/null
! grep -F 'QuickLaunchDetailPane' "$WELCOME" >/dev/null
! grep -F 'search-shell := Rectangle {' "$WELCOME" >/dev/null
grep -F 'callback welcome-quick-launch-connect-requested(string);' "$APP_WINDOW" >/dev/null
grep -F 'callback welcome-open-saved-ssh-requested();' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <bool> open-saved-ssh-modal-open: false;' "$APP_WINDOW" >/dev/null
grep -F 'callback open-saved-ssh-modal-close-requested();' "$APP_WINDOW" >/dev/null
grep -F 'callback open-saved-ssh-modal-query-changed(string);' "$APP_WINDOW" >/dev/null
grep -F 'callback open-saved-ssh-modal-asset-activated(string);' "$APP_WINDOW" >/dev/null
grep -F 'callback welcome-open-saved-ssh-requested();' "$WORKSPACE_PANE" >/dev/null
grep -F 'callback welcome-open-saved-ssh-requested();' "$SESSION_HOST" >/dev/null
grep -F 'export component OpenSavedSshModal inherits Rectangle {' "$MODAL" >/dev/null
grep -F 'AssetNodeRow' "$MODAL" >/dev/null

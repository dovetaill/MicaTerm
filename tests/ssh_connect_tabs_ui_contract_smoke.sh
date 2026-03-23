#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_WINDOW="$ROOT_DIR/ui/app-window.slint"
HOST_KEY_MODAL="$ROOT_DIR/ui/components/ssh-host-key-confirm-modal.slint"
WORKSPACE_HOST="$ROOT_DIR/ui/shell/terminal-session-host.slint"

grep -F 'export component SshHostKeyConfirmModal inherits Rectangle {' "$HOST_KEY_MODAL" >/dev/null
grep -F 'callback accept-requested();' "$HOST_KEY_MODAL" >/dev/null
grep -F 'callback reject-requested();' "$HOST_KEY_MODAL" >/dev/null
grep -F 'in-out property <bool> ssh-host-key-modal-open: false;' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <[WorkspaceTabItem]> workspace-tab-items: [];' "$APP_WINDOW" >/dev/null
grep -F 'callback workspace-tab-selected(string);' "$APP_WINDOW" >/dev/null
grep -F 'callback workspace-tab-close-requested(string);' "$APP_WINDOW" >/dev/null
grep -F 'export component TerminalSessionHost inherits Rectangle {' "$WORKSPACE_HOST" >/dev/null

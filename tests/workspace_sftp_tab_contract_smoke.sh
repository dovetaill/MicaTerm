#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SFTP_HOST="$ROOT_DIR/ui/shell/sftp-workspace-host.slint"
WORKSPACE_PANE="$ROOT_DIR/ui/shell/workspace-pane.slint"
APP_WINDOW="$ROOT_DIR/ui/app-window.slint"

require_contains() {
  local needle="$1"
  local file="$2"
  grep -F "$needle" "$file" >/dev/null || {
    echo "missing contract in $(basename "$file"): $needle" >&2
    exit 1
  }
}

require_absent() {
  local needle="$1"
  local file="$2"
  if grep -F "$needle" "$file" >/dev/null; then
    echo "unexpected legacy contract in $(basename "$file"): $needle" >&2
    exit 1
  fi
}

[[ -f "$SFTP_HOST" ]] || {
  echo "missing ui/shell/sftp-workspace-host.slint" >&2
  exit 1
}

[[ -f "$WORKSPACE_PANE" ]] || {
  echo "missing ui/shell/workspace-pane.slint" >&2
  exit 1
}

[[ -f "$APP_WINDOW" ]] || {
  echo "missing ui/app-window.slint" >&2
  exit 1
}

require_contains 'workspace-header :=' "$SFTP_HOST"
require_contains 'workspace-toolbar :=' "$SFTP_HOST"
require_contains 'workspace-breadcrumb-shell :=' "$SFTP_HOST"
require_contains 'workspace-file-table :=' "$SFTP_HOST"
require_contains 'workspace-statusbar :=' "$SFTP_HOST"
require_contains 'function workspace-width-tier() -> string {' "$SFTP_HOST"
require_contains 'text: "Permissions"' "$SFTP_HOST"
require_contains 'text: "Owner"' "$SFTP_HOST"
require_contains 'text: "Group"' "$SFTP_HOST"
require_contains 'workspace-sftp-tooltip-overlay := TitlebarTooltip {' "$APP_WINDOW"
require_contains 'in-out property <bool> workspace-sftp-tooltip-visible: false;' "$APP_WINDOW"
require_contains 'in-out property <string> workspace-sftp-tooltip-text: "";' "$APP_WINDOW"
require_contains 'in-out property <string> workspace-sftp-toolbar-disabled-reason: "";' "$APP_WINDOW"
require_contains 'tooltip-open-requested' "$SFTP_HOST"
require_contains 'tooltip-close-requested' "$SFTP_HOST"
require_contains 'function effective-tooltip-text() -> string {' "$SFTP_HOST"
require_contains 'disabled-tooltip-text' "$SFTP_HOST"
require_contains 'function workspace-toolbar-actions-compact() -> bool {' "$SFTP_HOST"
require_contains 'transfer-center-button := WorkspaceActionButton {' "$SFTP_HOST"
require_contains 'label: root.workspace-toolbar-actions-compact() ? "" : "Upload";' "$SFTP_HOST"
require_contains 'label: root.workspace-toolbar-actions-compact() ? "" : "New Folder";' "$SFTP_HOST"
require_contains 'label: root.workspace-toolbar-actions-compact() ? "" : "Transfer Center";' "$SFTP_HOST"
require_contains 'shell-sidebar-item-selected: root.shell-sidebar-item-selected;' "$WORKSPACE_PANE"
require_contains 'shell-sidebar-item-selected-border: root.shell-sidebar-item-selected-border;' "$WORKSPACE_PANE"
require_contains 'callback workspace-sftp-tooltip-open-requested(string, string, length, length, length);' "$WORKSPACE_PANE"
require_contains 'callback workspace-sftp-tooltip-close-requested(string);' "$WORKSPACE_PANE"
require_contains 'in-out property <string> right-panel-display-policy: "visible";' "$APP_WINDOW"
require_contains 'in-out property <bool> right-panel-can-revive: true;' "$APP_WINDOW"
require_contains 'if !root.effective-show-right-panel && root.right-panel-can-revive : right-panel-revive-strip := Rectangle {' "$APP_WINDOW"
require_contains 'policy-hidden-sftp-workspace' "$APP_WINDOW"
require_absent 'New Fold...' "$SFTP_HOST"
require_absent 'New Fol...' "$SFTP_HOST"
require_absent 'root.workspace-sftp-path-submitted(root.workspace-sftp-path);' "$SFTP_HOST"
require_absent 'if !root.effective-show-right-panel : right-panel-revive-strip := Rectangle {' "$APP_WINDOW"

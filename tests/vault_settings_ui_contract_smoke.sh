#!/usr/bin/env bash
# Verifies the formal Sync titlebar entry and non-vault Settings contract.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_WINDOW="$ROOT_DIR/ui/app-window.slint"
TITLEBAR_MENU="$ROOT_DIR/ui/components/titlebar-menu.slint"
TITLEBAR="$ROOT_DIR/ui/shell/titlebar.slint"
RIGHT_PANEL="$ROOT_DIR/ui/shell/right-panel.slint"
VIEW_MODEL="$ROOT_DIR/src/shell/view_model.rs"

ensure_absent() {
  local pattern="$1"
  local file="$2"

  if grep -F "$pattern" "$file" >/dev/null; then
    echo "unexpected pattern in $file: $pattern" >&2
    exit 1
  fi
}

[[ -f "$TITLEBAR" ]] || {
  echo "missing ui/shell/titlebar.slint" >&2
  exit 1
}

grep -F 'callback open-settings-panel-requested();' "$APP_WINDOW" >/dev/null
grep -F 'callback open-sync-modal-requested();' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <bool> sync-modal-open: false;' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <string> right-panel-view: "sftp";' "$APP_WINDOW" >/dev/null
grep -F 'open-settings-panel-requested => {' "$APP_WINDOW" >/dev/null
grep -F 'root.open-settings-panel-requested();' "$APP_WINDOW" >/dev/null
grep -F 'open-sync-modal-requested => {' "$APP_WINDOW" >/dev/null
grep -F 'root.open-sync-modal-requested();' "$APP_WINDOW" >/dev/null

grep -F 'sync-button := TitlebarIconButton' "$TITLEBAR" >/dev/null
grep -F 'root.open-sync-modal-requested();' "$TITLEBAR" >/dev/null

grep -F 'label: "Settings";' "$TITLEBAR_MENU" >/dev/null
grep -F 'root.settings-selected();' "$TITLEBAR_MENU" >/dev/null
ensure_absent 'label: "Preferences"' "$TITLEBAR_MENU"
ensure_absent 'label: "Appearance";' "$TITLEBAR_MENU"
ensure_absent 'text: "Sync & Vault"' "$RIGHT_PANEL"
ensure_absent 'panel-view == "vault"' "$RIGHT_PANEL"
ensure_absent 'VaultProviderCard' "$RIGHT_PANEL"
ensure_absent 'self.right_panel_view = RightPanelView::Vault;' "$VIEW_MODEL"

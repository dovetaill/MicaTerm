#!/usr/bin/env bash
# Verifies the Sync & Vault right-panel UI contract.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_WINDOW="$ROOT_DIR/ui/app-window.slint"
TITLEBAR_MENU="$ROOT_DIR/ui/components/titlebar-menu.slint"
RIGHT_PANEL="$ROOT_DIR/ui/shell/right-panel.slint"
PROVIDER_CARD="$ROOT_DIR/ui/components/vault-provider-card.slint"

[[ -f "$PROVIDER_CARD" ]] || {
  echo "missing ui/components/vault-provider-card.slint" >&2
  exit 1
}

grep -F 'callback open-settings-panel-requested();' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <string> right-panel-view: "appearance";' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <string> vault-panel-title: "Sync & Vault";' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <string> vault-lock-state-label: "Locked";' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <string> vault-sync-now-label: "Sync now";' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <string> vault-export-bootstrap-label: "Export bootstrap";' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <string> vault-import-bootstrap-label: "Import bootstrap";' "$APP_WINDOW" >/dev/null
grep -F 'open-settings-panel-requested => {' "$APP_WINDOW" >/dev/null
grep -F 'root.open-settings-panel-requested();' "$APP_WINDOW" >/dev/null

grep -F 'label: "Settings";' "$TITLEBAR_MENU" >/dev/null
grep -F 'root.settings-selected();' "$TITLEBAR_MENU" >/dev/null

grep -F 'in property <string> panel-view: "appearance";' "$RIGHT_PANEL" >/dev/null
grep -F 'text: "Sync & Vault"' "$RIGHT_PANEL" >/dev/null
grep -F 'text: root.vault-lock-state-label;' "$RIGHT_PANEL" >/dev/null
grep -F 'text: root.vault-primary-action-label;' "$RIGHT_PANEL" >/dev/null
grep -F 'text: root.vault-sync-now-label;' "$RIGHT_PANEL" >/dev/null
grep -F 'text: root.vault-export-bootstrap-label;' "$RIGHT_PANEL" >/dev/null
grep -F 'text: root.vault-import-bootstrap-label;' "$RIGHT_PANEL" >/dev/null
grep -F 'text: "Primary"' "$PROVIDER_CARD" >/dev/null
grep -F 'text: "Mirror"' "$PROVIDER_CARD" >/dev/null

! grep -F 'label: "Preferences"' "$TITLEBAR_MENU" >/dev/null

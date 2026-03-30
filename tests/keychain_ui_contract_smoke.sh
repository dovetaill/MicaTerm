#!/usr/bin/env bash
# Guards the keychain panel and modal UI contract introduced in Task 6.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SIDEBAR="$ROOT_DIR/ui/shell/assets-sidebar.slint"
IDENTITY_MODAL="$ROOT_DIR/ui/components/assets-keychain-identity-modal.slint"
SSH_KEY_MODAL="$ROOT_DIR/ui/components/assets-keychain-ssh-key-modal.slint"
SSH_MODAL="$ROOT_DIR/ui/components/assets-ssh-connection-modal.slint"
CREATE_MENU="$ROOT_DIR/ui/components/assets-create-menu.slint"

[[ -f "$SIDEBAR" ]] || {
  echo "missing ui/shell/assets-sidebar.slint" >&2
  exit 1
}

[[ -f "$IDENTITY_MODAL" ]] || {
  echo "missing ui/components/assets-keychain-identity-modal.slint" >&2
  exit 1
}

[[ -f "$SSH_KEY_MODAL" ]] || {
  echo "missing ui/components/assets-keychain-ssh-key-modal.slint" >&2
  exit 1
}

grep -F 'keychain-asset-items' "$SIDEBAR" >/dev/null
grep -F 'Identity' "$IDENTITY_MODAL" >/dev/null
grep -F 'SSH Key' "$IDENTITY_MODAL" >/dev/null
grep -F 'Generate Key Pair' "$SSH_KEY_MODAL" >/dev/null
grep -F 'Copy Public Key' "$SSH_KEY_MODAL" >/dev/null
grep -F 'Manual' "$SSH_MODAL" >/dev/null
grep -F 'Keychain Identity' "$SSH_MODAL" >/dev/null
grep -F 'Authentication Summary' "$SSH_MODAL" >/dev/null
if grep -F 'Use Existing Keychain Identity' "$SSH_MODAL" >/dev/null; then
  echo "ssh modal must no longer expose the temporary keychain identity button" >&2
  exit 1
fi
grep -F 'New Identity' "$CREATE_MENU" >/dev/null
grep -F 'New SSH Key' "$CREATE_MENU" >/dev/null

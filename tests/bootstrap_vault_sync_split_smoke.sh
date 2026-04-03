#!/usr/bin/env bash
# Guards vault sync helpers moving behind the dedicated bootstrap module.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BOOTSTRAP_ROOT="$ROOT_DIR/src/app/bootstrap.rs"
VAULT_SYNC_MODULE="$ROOT_DIR/src/app/bootstrap/vault_sync.rs"

[[ -f "$BOOTSTRAP_ROOT" ]] || {
  echo "missing src/app/bootstrap.rs" >&2
  exit 1
}

[[ -f "$VAULT_SYNC_MODULE" ]] || {
  echo "missing src/app/bootstrap/vault_sync.rs" >&2
  exit 1
}

grep -F 'mod vault_sync;' "$BOOTSTRAP_ROOT" >/dev/null
grep -F 'fn update_sync_modal_for_local_state(' "$VAULT_SYNC_MODULE" >/dev/null
grep -F 'fn sync_local_vault(' "$VAULT_SYNC_MODULE" >/dev/null

if grep -F 'fn update_sync_modal_for_local_state(' "$BOOTSTRAP_ROOT" >/dev/null; then
  echo "update_sync_modal_for_local_state must move out of src/app/bootstrap.rs" >&2
  exit 1
fi

if grep -F 'fn sync_local_vault(' "$BOOTSTRAP_ROOT" >/dev/null; then
  echo "sync_local_vault must move out of src/app/bootstrap.rs" >&2
  exit 1
fi

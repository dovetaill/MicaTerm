#!/usr/bin/env bash
# Guards the bootstrap facade staying at the root file while internal domain modules emerge.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BOOTSTRAP_ROOT="$ROOT_DIR/src/app/bootstrap.rs"

[[ -f "$BOOTSTRAP_ROOT" ]] || {
  echo "missing src/app/bootstrap.rs" >&2
  exit 1
}

[[ ! -f "$ROOT_DIR/src/app/bootstrap/mod.rs" ]] || {
  echo "src/app/bootstrap.rs must remain the stable root module" >&2
  exit 1
}

for module in vault_sync workspace_terminal sftp assets_keychain shell_chrome windowing; do
  [[ -f "$ROOT_DIR/src/app/bootstrap/${module}.rs" ]] || {
    echo "missing src/app/bootstrap/${module}.rs" >&2
    exit 1
  }
  grep -F "mod ${module};" "$BOOTSTRAP_ROOT" >/dev/null
done

grep -F 'fn bind_top_status_bar_with_store_and_profile_and_effects_and_session_bridge(' \
  "$BOOTSTRAP_ROOT" >/dev/null

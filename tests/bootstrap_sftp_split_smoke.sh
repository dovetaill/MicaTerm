#!/usr/bin/env bash
# Guards SFTP bootstrap helpers moving behind the dedicated module.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BOOTSTRAP_ROOT="$ROOT_DIR/src/app/bootstrap.rs"
SFTP_MODULE="$ROOT_DIR/src/app/bootstrap/sftp.rs"

[[ -f "$BOOTSTRAP_ROOT" ]] || {
  echo "missing src/app/bootstrap.rs" >&2
  exit 1
}

[[ -f "$SFTP_MODULE" ]] || {
  echo "missing src/app/bootstrap/sftp.rs" >&2
  exit 1
}

grep -F 'mod sftp;' "$BOOTSTRAP_ROOT" >/dev/null
grep -F 'fn execute_sftp_browser_request(' "$SFTP_MODULE" >/dev/null
grep -F 'fn bind_sftp_callbacks(' "$SFTP_MODULE" >/dev/null
grep -F 'window.on_sftp_panel_item_activated(' "$SFTP_MODULE" >/dev/null

if grep -F 'fn execute_sftp_browser_request(' "$BOOTSTRAP_ROOT" >/dev/null; then
  echo "execute_sftp_browser_request must move out of src/app/bootstrap.rs" >&2
  exit 1
fi

if grep -F 'window.on_sftp_panel_item_activated(' "$BOOTSTRAP_ROOT" >/dev/null; then
  echo "SFTP panel callbacks must move out of src/app/bootstrap.rs" >&2
  exit 1
fi

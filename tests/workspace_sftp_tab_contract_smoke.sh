#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORKSPACE_TABS="$ROOT_DIR/src/shell/tabs.rs"
SFTP_MOD="$ROOT_DIR/src/app/sftp/mod.rs"
BROWSER_SESSION="$ROOT_DIR/src/app/sftp/browser_session.rs"

[[ -f "$WORKSPACE_TABS" ]] || {
  echo "missing src/shell/tabs.rs" >&2
  exit 1
}

[[ -f "$SFTP_MOD" ]] || {
  echo "missing src/app/sftp/mod.rs" >&2
  exit 1
}

[[ -f "$BROWSER_SESSION" ]] || {
  echo "missing src/app/sftp/browser_session.rs" >&2
  exit 1
}

grep -F 'pub type WorkspaceTabId = String;' "$WORKSPACE_TABS" >/dev/null
grep -F 'Sftp,' "$WORKSPACE_TABS" >/dev/null
grep -F 'pub tab_id: WorkspaceTabId,' "$WORKSPACE_TABS" >/dev/null
grep -F 'pub fn sftp(' "$WORKSPACE_TABS" >/dev/null
grep -F 'pub mod browser_session;' "$SFTP_MOD" >/dev/null
grep -F 'pub use browser_session::{FileBrowserSession, FileBrowserSessionId, HostProfileRef};' "$SFTP_MOD" >/dev/null

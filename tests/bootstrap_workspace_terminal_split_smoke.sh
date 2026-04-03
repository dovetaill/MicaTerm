#!/usr/bin/env bash
# Guards workspace terminal projection and forwarding helpers moving behind the dedicated module.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BOOTSTRAP_ROOT="$ROOT_DIR/src/app/bootstrap.rs"
WORKSPACE_TERMINAL_MODULE="$ROOT_DIR/src/app/bootstrap/workspace_terminal.rs"

[[ -f "$BOOTSTRAP_ROOT" ]] || {
  echo "missing src/app/bootstrap.rs" >&2
  exit 1
}

[[ -f "$WORKSPACE_TERMINAL_MODULE" ]] || {
  echo "missing src/app/bootstrap/workspace_terminal.rs" >&2
  exit 1
}

grep -F 'mod workspace_terminal;' "$BOOTSTRAP_ROOT" >/dev/null
grep -F 'fn sync_workspace_projection_from_manager(' "$WORKSPACE_TERMINAL_MODULE" >/dev/null
grep -F 'fn forward_active_workspace_text_input(' "$WORKSPACE_TERMINAL_MODULE" >/dev/null
grep -F 'fn forward_active_workspace_scroll(' "$WORKSPACE_TERMINAL_MODULE" >/dev/null

if grep -F 'fn sync_workspace_projection_from_manager(' "$BOOTSTRAP_ROOT" >/dev/null; then
  echo "sync_workspace_projection_from_manager must move out of src/app/bootstrap.rs" >&2
  exit 1
fi

if grep -F 'fn forward_active_workspace_text_input(' "$BOOTSTRAP_ROOT" >/dev/null; then
  echo "forward_active_workspace_text_input must move out of src/app/bootstrap.rs" >&2
  exit 1
fi

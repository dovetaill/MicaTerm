#!/usr/bin/env bash
# Guards the SSH runtime facade staying at the root file while internal runtime modules emerge.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RUNTIME_ROOT="$ROOT_DIR/src/app/ssh/runtime.rs"

[[ -f "$RUNTIME_ROOT" ]] || {
  echo "missing src/app/ssh/runtime.rs" >&2
  exit 1
}

[[ ! -f "$ROOT_DIR/src/app/ssh/runtime/mod.rs" ]] || {
  echo "src/app/ssh/runtime.rs must remain the stable root module" >&2
  exit 1
}

for module in contracts transport auth pump terminal sftp_backend; do
  [[ -f "$ROOT_DIR/src/app/ssh/runtime/${module}.rs" ]] || {
    echo "missing src/app/ssh/runtime/${module}.rs" >&2
    exit 1
  }
  grep -F "mod ${module};" "$RUNTIME_ROOT" >/dev/null
done

grep -F 'pub struct SshSessionRuntime {' "$RUNTIME_ROOT" >/dev/null

CONTRACTS_MODULE="$ROOT_DIR/src/app/ssh/runtime/contracts.rs"
TERMINAL_MODULE="$ROOT_DIR/src/app/ssh/runtime/terminal.rs"

for symbol in \
  'pub struct TerminalSurfaceState {' \
  'pub struct TerminalSurfaceSignature {' \
  'pub struct TerminalRowState {' \
  'pub struct TerminalCellState {' \
  'pub enum TerminalCursorShape {' \
  'pub struct TerminalCursorState {' \
  'pub enum TerminalMouseEventKind {' \
  'pub enum TerminalMouseButton {' \
  'pub struct TerminalMouseInput {' \
  'pub enum TerminalKeyKind {' \
  'pub struct TerminalKeyEvent {'
do
  grep -F "$symbol" "$CONTRACTS_MODULE" >/dev/null || {
    echo "missing runtime contract in contracts.rs: $symbol" >&2
    exit 1
  }
done

grep -F 'impl TerminalKeyEvent {' "$CONTRACTS_MODULE" >/dev/null
grep -F 'pub struct TerminalSession {' "$TERMINAL_MODULE" >/dev/null
grep -F 'pub fn encode_named_key_input(' "$TERMINAL_MODULE" >/dev/null

grep -F 'pub use contracts::{' "$RUNTIME_ROOT" >/dev/null || {
  echo "runtime.rs must re-export moved terminal contracts via contracts.rs" >&2
  exit 1
}

grep -F 'pub use terminal::{' "$RUNTIME_ROOT" >/dev/null || {
  echo "runtime.rs must re-export terminal engine symbols via terminal.rs" >&2
  exit 1
}

for export_symbol in 'encode_named_key_input' 'TerminalSession'; do
  grep -F "$export_symbol" "$RUNTIME_ROOT" >/dev/null || {
    echo "runtime.rs must keep terminal facade export: $export_symbol" >&2
    exit 1
  }
done

grep -F 'extract_current_working_directory_from_osc7' "$RUNTIME_ROOT" >/dev/null || {
  echo "runtime.rs must keep working-directory extractor exported from terminal.rs" >&2
    exit 1
}

for symbol in \
  'pub struct TerminalSurfaceState {' \
  'pub struct TerminalSurfaceSignature {' \
  'pub struct TerminalRowState {' \
  'pub struct TerminalCellState {' \
  'pub enum TerminalCursorShape {' \
  'pub struct TerminalCursorState {' \
  'pub enum TerminalMouseEventKind {' \
  'pub enum TerminalMouseButton {' \
  'pub struct TerminalMouseInput {' \
  'pub enum TerminalKeyKind {' \
  'pub struct TerminalKeyEvent {' \
  'pub struct TerminalSession {' \
  'pub fn encode_named_key_input('
do
  if grep -F "$symbol" "$RUNTIME_ROOT" >/dev/null; then
    echo "runtime.rs still owns moved terminal symbol: $symbol" >&2
    exit 1
  fi
done

#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MAIN_FILE="$ROOT_DIR/src/main.rs"

grep -F 'BackendSelector::new' "$MAIN_FILE" >/dev/null
grep -F 'backend_name("winit".into())' "$MAIN_FILE" >/dev/null
grep -F 'renderer_name("femtovg-wgpu".into())' "$MAIN_FILE" >/dev/null
grep -F 'require_wgpu_28' "$MAIN_FILE" >/dev/null
grep -F 'femtovg-wgpu-experimental' "$MAIN_FILE" >/dev/null

if rg -n 'SLINT_BACKEND|set_var\("SLINT_BACKEND"' "$MAIN_FILE" >/dev/null; then
  echo "unexpected SLINT_BACKEND override path remains in $MAIN_FILE" >&2
  exit 1
fi

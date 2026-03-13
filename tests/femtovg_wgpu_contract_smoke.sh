#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MAIN_FILE="$ROOT_DIR/src/main.rs"
CARGO_TOML="$ROOT_DIR/Cargo.toml"
VENDORED_WGPU_FILE="$ROOT_DIR/vendor/i-slint-renderer-femtovg/wgpu.rs"

grep -F 'BackendSelector::new' "$MAIN_FILE" >/dev/null
grep -F 'backend_name("winit".into())' "$MAIN_FILE" >/dev/null
grep -F 'renderer_name("femtovg-wgpu".into())' "$MAIN_FILE" >/dev/null
grep -F 'require_wgpu_28' "$MAIN_FILE" >/dev/null
grep -F 'Backends::DX12' "$MAIN_FILE" >/dev/null
grep -F 'with_winit_window_attributes_hook' "$MAIN_FILE" >/dev/null
grep -F 'with_transparent(false)' "$MAIN_FILE" >/dev/null
grep -F 'AppRuntimeProfile::mainline()' "$MAIN_FILE" >/dev/null
grep -F 'i-slint-renderer-femtovg = { path = "vendor/i-slint-renderer-femtovg" }' "$CARGO_TOML" >/dev/null
grep -F 'tracing::info!' "$VENDORED_WGPU_FILE" >/dev/null
grep -F 'wgpu adapter initialized for femtovg renderer' "$VENDORED_WGPU_FILE" >/dev/null
grep -F 'surface_format = ?surface_config.format' "$VENDORED_WGPU_FILE" >/dev/null
grep -F 'present_mode = ?surface_config.present_mode' "$VENDORED_WGPU_FILE" >/dev/null
grep -F 'alpha_mode = ?surface_config.alpha_mode' "$VENDORED_WGPU_FILE" >/dev/null

if rg -n 'SLINT_BACKEND|set_var\("SLINT_BACKEND"|femtovg-wgpu-experimental|formal\(' "$MAIN_FILE" >/dev/null; then
  echo "unexpected SLINT_BACKEND override path remains in $MAIN_FILE" >&2
  exit 1
fi

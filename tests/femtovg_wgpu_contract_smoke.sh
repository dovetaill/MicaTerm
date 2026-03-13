#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MAIN_FILE="$ROOT_DIR/src/main.rs"
CARGO_TOML="$ROOT_DIR/Cargo.toml"
BOOTSTRAP_FILE="$ROOT_DIR/src/app/bootstrap.rs"
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
grep -F 'surface_config.format = swapchain_format;' "$VENDORED_WGPU_FILE" >/dev/null
grep -F 'surface_config.present_mode =' "$VENDORED_WGPU_FILE" >/dev/null
grep -F 'select_preferred_present_mode(&swapchain_capabilities.present_modes);' "$VENDORED_WGPU_FILE" >/dev/null
grep -F 'surface_config.alpha_mode =' "$VENDORED_WGPU_FILE" >/dev/null
grep -F 'select_preferred_alpha_mode(&swapchain_capabilities.alpha_modes);' "$VENDORED_WGPU_FILE" >/dev/null

if rg -n 'SLINT_BACKEND|set_var\("SLINT_BACKEND"|femtovg-wgpu-experimental|formal\(' "$MAIN_FILE" >/dev/null; then
  echo "unexpected SLINT_BACKEND override path remains in $MAIN_FILE" >&2
  exit 1
fi

for file in "$MAIN_FILE" "$BOOTSTRAP_FILE" "$VENDORED_WGPU_FILE"; do
  if rg -n \
    'MICA_TRACE_RENDER_PIPELINE|RenderTraceGeometry|RenderTraceSnapshot|install_render_pipeline_tracing|render_pipeline_trace_enabled|observed winit window event|observed slint rendering lifecycle|femtovg renderer received requested graphics api|wgpu adapter initialized for femtovg renderer|wgpu surface capabilities resolved for femtovg renderer|wgpu default surface configuration resolved for femtovg renderer|wgpu surface configured for femtovg renderer|configuring wgpu backend preferences for femtovg renderer|wgpu_surface_reports_opaque_alpha_only|requested window minimize' \
    "$file" >/dev/null
  then
    echo "unexpected temporary femtovg-wgpu debug trace remains in $file" >&2
    exit 1
  fi
done

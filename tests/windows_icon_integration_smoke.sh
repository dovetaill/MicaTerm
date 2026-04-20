#!/usr/bin/env bash
# Confirms Windows icon resource, runtime icon, and diagnostics integration remain configured.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

grep -F 'winresource' "$ROOT_DIR/Cargo.toml" >/dev/null
grep -F 'assets/icons/windows/mica-term.ico' "$ROOT_DIR/build.rs" >/dev/null
grep -F 'icon: @image-url("../assets/icons/mica-term-app.svg")' "$ROOT_DIR/ui/app-window.slint" >/dev/null
grep -F 'pub(crate) mod windows_icon;' "$ROOT_DIR/src/app/mod.rs" >/dev/null
grep -F 'windows_icon::log_window_icon_state(&window, "after_window_new")' "$ROOT_DIR/src/app/bootstrap.rs" >/dev/null
grep -F 'windows_icon::log_window_icon_state(&window, "before_window_run")' "$ROOT_DIR/src/app/bootstrap.rs" >/dev/null
grep -F 'fn icon_is_effectively_empty(' "$ROOT_DIR/vendor/i-slint-backend-winit/winitwindowadapter.rs" >/dev/null
grep -F 'preserving existing native window/taskbar icons' "$ROOT_DIR/vendor/i-slint-backend-winit/winitwindowadapter.rs" >/dev/null

[[ -f "$ROOT_DIR/assets/icons/windows/mica-term.ico" ]] || {
  echo "missing committed windows icon" >&2
  exit 1
}

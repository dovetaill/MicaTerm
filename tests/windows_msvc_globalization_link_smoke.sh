#!/usr/bin/env bash
# Confirms our vendored Windows crates avoid Win32_Globalization imports that
# pull icu.dll into the MSVC link alongside Skia's bundled ICU.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

grep -F 'dirs-sys = { path = "vendor/dirs-sys" }' "$ROOT_DIR/Cargo.toml" >/dev/null
grep -F 'muda = { path = "vendor/muda" }' "$ROOT_DIR/Cargo.toml" >/dev/null
grep -F 'winit = { path = "vendor/winit" }' "$ROOT_DIR/Cargo.toml" >/dev/null

if rg -n 'Win32_Globalization' "$ROOT_DIR/vendor/dirs-sys/Cargo.toml" "$ROOT_DIR/vendor/muda/Cargo.toml" "$ROOT_DIR/vendor/winit/Cargo.toml" >/dev/null; then
  echo "vendored Windows crates should not request Win32_Globalization anymore" >&2
  exit 1
fi

grep -F 'fn wide_ptr_len' "$ROOT_DIR/vendor/dirs-sys/src/lib.rs" >/dev/null
grep -F 'fn wide_ptr_len' "$ROOT_DIR/vendor/muda/src/platform_impl/windows/util.rs" >/dev/null
grep -F 'type HIMC = isize;' "$ROOT_DIR/vendor/winit/src/platform_impl/windows/ime.rs" >/dev/null

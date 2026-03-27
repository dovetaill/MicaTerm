#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
CARGO_TOML="$ROOT_DIR/Cargo.toml"
BACKEND_LIB="$ROOT_DIR/vendor/i-slint-backend-winit/lib.rs"
BACKEND_EVENT_LOOP="$ROOT_DIR/vendor/i-slint-backend-winit/event_loop.rs"
BACKEND_WINDOW_ADAPTER="$ROOT_DIR/vendor/i-slint-backend-winit/winitwindowadapter.rs"
BACKEND_SW="$ROOT_DIR/vendor/i-slint-backend-winit/renderer/sw.rs"

grep -F '[patch.crates-io]' "$CARGO_TOML" >/dev/null
grep -F 'i-slint-backend-winit = { path = "vendor/i-slint-backend-winit" }' "$CARGO_TOML" >/dev/null
grep -F 'mod partial_visibility;' "$BACKEND_LIB" >/dev/null
grep -F 'WindowEvent::Moved(_)' "$BACKEND_EVENT_LOOP" >/dev/null
grep -F 'handle_partial_visibility_change' "$BACKEND_WINDOW_ADAPTER" >/dev/null
grep -F 'present_existing_buffer' "$BACKEND_SW" >/dev/null

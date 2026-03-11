#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
FILE="$ROOT_DIR/src/app/window_effects.rs"

grep -F 'window.set_theme(Some(' "$FILE" >/dev/null
grep -F 'window.request_redraw();' "$FILE" >/dev/null
grep -F 'window_vibrancy::apply_tabbed' "$FILE" >/dev/null
grep -F '#[cfg(target_os = "windows")]' "$FILE" >/dev/null
grep -F 'NoopWindowEffects' "$FILE" >/dev/null

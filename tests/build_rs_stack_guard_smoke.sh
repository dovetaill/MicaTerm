#!/usr/bin/env bash
# Guards the build script against Windows stack overflows by requiring the Slint compile to run on a larger stack.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT_PATH="$ROOT_DIR/build.rs"

[[ -f "$SCRIPT_PATH" ]]

grep -F 'thread::Builder::new()' "$SCRIPT_PATH" >/dev/null
grep -F '.stack_size(' "$SCRIPT_PATH" >/dev/null
grep -F 'compile_with_config("ui/app-window.slint"' "$SCRIPT_PATH" >/dev/null
grep -F 'failed to compile Slint UI' "$SCRIPT_PATH" >/dev/null

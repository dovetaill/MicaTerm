#!/usr/bin/env bash
# Guards frameless window chrome invariants across Rust and Slint.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"

for unexpected in \
  'WindowChromeMode' \
  'chrome_mode(' \
  'uses_flat_window_chrome' \
  'set_use_flat_window_chrome' \
  'get_use_flat_window_chrome' \
  'use-flat-window-chrome'
do
  if rg -n --fixed-strings "$unexpected" \
    "$ROOT_DIR/src" "$ROOT_DIR/ui" \
    -g '*.rs' -g '*.slint'
  then
    echo "unexpected rounded/flat chrome symbol remains: $unexpected" >&2
    exit 1
  fi
done

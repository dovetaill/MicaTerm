#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

check_file() {
  local path="$1"
  local view_box="$2"

  [[ -f "$path" ]] || {
    echo "missing icon asset: $path" >&2
    exit 1
  }

  grep -F "$view_box" "$path" >/dev/null
}

check_file "$ROOT_DIR/assets/icons/mica-term-logo.svg" 'viewBox="0 0 720 256"'
check_file "$ROOT_DIR/assets/icons/mica-term-app.svg" 'viewBox="0 0 256 256"'
check_file "$ROOT_DIR/assets/icons/mica-term-taskbar.svg" 'viewBox="0 0 256 256"'
check_file "$ROOT_DIR/assets/icons/mica-term-mark.svg" 'viewBox="0 0 256 256"'
check_file "$ROOT_DIR/assets/icons/mica-term-header-logotype.svg" 'viewBox='

grep -F '#4ea1ff' "$ROOT_DIR/assets/icons/mica-term-logo.svg" >/dev/null
grep -F 'id="m-frame"' "$ROOT_DIR/assets/icons/mica-term-app.svg" >/dev/null
grep -F 'id="taskbar-m-frame"' "$ROOT_DIR/assets/icons/mica-term-taskbar.svg" >/dev/null
grep -F 'fill="currentColor"' "$ROOT_DIR/assets/icons/mica-term-mark.svg" >/dev/null
grep -F 'currentColor' "$ROOT_DIR/assets/icons/mica-term-header-logotype.svg" >/dev/null

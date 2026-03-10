#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

grep -F 'winresource' "$ROOT_DIR/Cargo.toml" >/dev/null
grep -F 'assets/icons/windows/mica-term.ico' "$ROOT_DIR/build.rs" >/dev/null
grep -F 'assets/icons/windows/mica-term.ico' "$ROOT_DIR/build-win-x64.sh" >/dev/null
grep -F 'scripts/export-icons.sh' "$ROOT_DIR/readme.md" >/dev/null

[[ -f "$ROOT_DIR/assets/icons/windows/mica-term.ico" ]] || {
  echo "missing committed windows icon" >&2
  exit 1
}

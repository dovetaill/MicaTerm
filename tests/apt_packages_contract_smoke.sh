#!/usr/bin/env bash
# Verifies the documented APT inventory stays aligned with the installer script.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
INVENTORY_PATH="$ROOT_DIR/apt-packages.md"
SCRIPT_PATH="$ROOT_DIR/install-apt-packages.sh"

[[ -f "$INVENTORY_PATH" ]] || {
  echo "missing inventory file: $INVENTORY_PATH" >&2
  exit 1
}

[[ -f "$SCRIPT_PATH" ]] || {
  echo "missing installer script: $SCRIPT_PATH" >&2
  exit 1
}

bash -n "$SCRIPT_PATH"

HELP_OUTPUT="$("$SCRIPT_PATH" --help)"
grep -F 'gcc-mingw-w64-x86-64-posix' <<<"$HELP_OUTPUT" >/dev/null
grep -F 'nasm' <<<"$HELP_OUTPUT" >/dev/null
grep -F 'llvm-19' <<<"$HELP_OUTPUT" >/dev/null
grep -F 'clang-19' <<<"$HELP_OUTPUT" >/dev/null
grep -F 'zip' <<<"$HELP_OUTPUT" >/dev/null
grep -F 'libwayland-dev' <<<"$HELP_OUTPUT" >/dev/null
grep -F 'pkg-config' <<<"$HELP_OUTPUT" >/dev/null

RUN_OUTPUT="$(printf 'n\n' | "$SCRIPT_PATH")"
grep -F 'Packages that will be installed:' <<<"$RUN_OUTPUT" >/dev/null
grep -F 'Installation cancelled.' <<<"$RUN_OUTPUT" >/dev/null

grep -F '## APT Packages Installed During This Windows Build Work' "$INVENTORY_PATH" >/dev/null
grep -F '## Current APT Prerequisites For The Build Chain' "$INVENTORY_PATH" >/dev/null
grep -F '## Current Cargo-Managed Project Dependencies' "$INVENTORY_PATH" >/dev/null
grep -F '`gcc-mingw-w64-x86-64-posix`' "$INVENTORY_PATH" >/dev/null
grep -F '`nasm`' "$INVENTORY_PATH" >/dev/null
grep -F '`llvm-19`' "$INVENTORY_PATH" >/dev/null
grep -F '`clang-19`' "$INVENTORY_PATH" >/dev/null
grep -F '`zip`' "$INVENTORY_PATH" >/dev/null
grep -F '`libwayland-dev`' "$INVENTORY_PATH" >/dev/null
grep -F '`pkg-config`' "$INVENTORY_PATH" >/dev/null
grep -F '`slint`' "$INVENTORY_PATH" >/dev/null
if grep -F '`i-slint-renderer-femtovg`' "$INVENTORY_PATH" >/dev/null; then
  echo "apt inventory should no longer describe vendored femtovg renderer patches as current dependencies" >&2
  exit 1
fi

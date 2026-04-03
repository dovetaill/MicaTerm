#!/usr/bin/env bash
# Confirms the Linux-host Windows MSVC bootstrap helper documents the required toolchain steps.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT_PATH="$ROOT_DIR/scripts/bootstrap-win-msvc-build.sh"

if [[ ! -f "$SCRIPT_PATH" ]]; then
  echo "missing bootstrap script: $SCRIPT_PATH" >&2
  exit 1
fi

bash -n "$SCRIPT_PATH"

HELP_OUTPUT="$("$SCRIPT_PATH" --help)"

grep -F "./scripts/bootstrap-win-msvc-build.sh" <<<"$HELP_OUTPUT" >/dev/null
grep -F "cargo install cargo-xwin" <<<"$HELP_OUTPUT" >/dev/null
grep -F "rustup target add x86_64-pc-windows-msvc" <<<"$HELP_OUTPUT" >/dev/null
grep -F "clang-19" <<<"$HELP_OUTPUT" >/dev/null
grep -F "./install-apt-packages.sh" <<<"$HELP_OUTPUT" >/dev/null
grep -F "./build-win-x64.sh" <<<"$HELP_OUTPUT" >/dev/null

grep -F 'cargo xwin --version' "$SCRIPT_PATH" >/dev/null
grep -F 'rustup target list --installed' "$SCRIPT_PATH" >/dev/null
grep -F 'clang-19' "$SCRIPT_PATH" >/dev/null
grep -F 'clang' "$SCRIPT_PATH" >/dev/null

#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT_PATH="$ROOT_DIR/build-release.sh"

[[ -f "$SCRIPT_PATH" ]]
bash -n "$SCRIPT_PATH"

HELP_OUTPUT="$("$SCRIPT_PATH" --help)"
grep -F 'fail-fast' <<<"$HELP_OUTPUT" >/dev/null
grep -F 'best-effort' <<<"$HELP_OUTPUT" >/dev/null
grep -F 'x86_64-unknown-linux-gnu' <<<"$HELP_OUTPUT" >/dev/null
grep -F 'x86_64-pc-windows-gnu' <<<"$HELP_OUTPUT" >/dev/null

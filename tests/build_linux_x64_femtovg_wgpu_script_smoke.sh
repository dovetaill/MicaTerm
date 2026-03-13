#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT_PATH="$ROOT_DIR/build-linux-x64-femtovg-wgpu.sh"

[[ -f "$SCRIPT_PATH" ]] || {
  echo "missing build script: $SCRIPT_PATH" >&2
  exit 1
}

bash -n "$SCRIPT_PATH"
HELP_OUTPUT="$("$SCRIPT_PATH" --help)"

grep -F 'x86_64-unknown-linux-gnu' <<<"$HELP_OUTPUT" >/dev/null
grep -F 'mica-term-femtovg-wgpu-experimental' <<<"$HELP_OUTPUT" >/dev/null
grep -F -- '--no-default-features' <<<"$HELP_OUTPUT" >/dev/null
grep -F '.tar.gz' <<<"$HELP_OUTPUT" >/dev/null
grep -F 'femtovg-wgpu-experimental' "$SCRIPT_PATH" >/dev/null

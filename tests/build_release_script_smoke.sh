#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT_PATH="$ROOT_DIR/build-release.sh"

[[ -f "$SCRIPT_PATH" ]]
bash -n "$SCRIPT_PATH"

HELP_OUTPUT="$("$SCRIPT_PATH" --help)"
grep -F 'Mainline GPU release aggregator' <<<"$HELP_OUTPUT" >/dev/null
grep -F 'fail-fast' <<<"$HELP_OUTPUT" >/dev/null
grep -F 'best-effort' <<<"$HELP_OUTPUT" >/dev/null
grep -F 'x86_64-unknown-linux-gnu' <<<"$HELP_OUTPUT" >/dev/null
grep -F 'x86_64-pc-windows-gnu' <<<"$HELP_OUTPUT" >/dev/null

if rg -n 'Formal|femtovg-wgpu-experimental|--no-default-features|CARGO_FEATURES=' "$SCRIPT_PATH" >/dev/null; then
  echo "mainline release script must not expose formal or experimental split semantics" >&2
  exit 1
fi

#!/usr/bin/env bash
# Validates the release aggregator wrapper contract for Linux software + Windows GNU software packaging.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT_PATH="$ROOT_DIR/build-release.sh"

[[ -f "$SCRIPT_PATH" ]]
bash -n "$SCRIPT_PATH"

HELP_OUTPUT="$("$SCRIPT_PATH" --help)"
grep -F 'Mainline Linux software + Windows GNU software release aggregator' <<<"$HELP_OUTPUT" >/dev/null
grep -F 'fail-fast' <<<"$HELP_OUTPUT" >/dev/null
grep -F 'best-effort' <<<"$HELP_OUTPUT" >/dev/null
grep -F 'x86_64-unknown-linux-gnu' <<<"$HELP_OUTPUT" >/dev/null
grep -F 'x86_64-pc-windows-gnu' <<<"$HELP_OUTPUT" >/dev/null
grep -F 'Windows GNU packages default to winit-software.' <<<"$HELP_OUTPUT" >/dev/null

grep -F 'build-win-x64-software.sh' "$SCRIPT_PATH" >/dev/null

LEGACY_EXPERIMENTAL='windows-skia'
LEGACY_EXPERIMENTAL_SUFFIX='-experimental'

if rg -n "Formal|femtovg-wgpu-experimental|${LEGACY_EXPERIMENTAL}${LEGACY_EXPERIMENTAL_SUFFIX}" "$SCRIPT_PATH" >/dev/null; then
  echo "mainline release script must not expose old experimental split semantics" >&2
  exit 1
fi

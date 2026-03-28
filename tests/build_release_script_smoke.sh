#!/usr/bin/env bash
# Validates the release aggregator wrapper contract for Linux software + Windows Skia mainline.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT_PATH="$ROOT_DIR/build-release.sh"

[[ -f "$SCRIPT_PATH" ]]
bash -n "$SCRIPT_PATH"

HELP_OUTPUT="$("$SCRIPT_PATH" --help)"
grep -F 'Mainline Linux software + Windows Skia release aggregator' <<<"$HELP_OUTPUT" >/dev/null
grep -F 'fail-fast' <<<"$HELP_OUTPUT" >/dev/null
grep -F 'best-effort' <<<"$HELP_OUTPUT" >/dev/null
grep -F 'x86_64-unknown-linux-gnu' <<<"$HELP_OUTPUT" >/dev/null
grep -F 'x86_64-pc-windows-gnu' <<<"$HELP_OUTPUT" >/dev/null
grep -F 'Windows x64 packages default to winit-skia-software.' <<<"$HELP_OUTPUT" >/dev/null

grep -F 'MICA_TERM_PACKAGE_RENDERER="skia-software"' "$SCRIPT_PATH" >/dev/null
grep -F 'MICA_TERM_BUILD_FLAVOR="windows-mainline"' "$SCRIPT_PATH" >/dev/null
grep -F 'CARGO_FEATURES="slint-renderer-skia"' "$SCRIPT_PATH" >/dev/null

LEGACY_EXPERIMENTAL='windows-skia'
LEGACY_EXPERIMENTAL_SUFFIX='-experimental'

if rg -n "Formal|femtovg-wgpu-experimental|${LEGACY_EXPERIMENTAL}${LEGACY_EXPERIMENTAL_SUFFIX}" "$SCRIPT_PATH" >/dev/null; then
  echo "mainline release script must not expose old experimental split semantics" >&2
  exit 1
fi

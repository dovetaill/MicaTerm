#!/usr/bin/env bash
# Confirms the Skia wrapper rejects the unsupported Windows GNU target before cargo starts building.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT_PATH="$ROOT_DIR/build-win-x64.sh"

[[ -f "$SCRIPT_PATH" ]]

set +e
OUTPUT="$(TARGET=x86_64-pc-windows-gnu "$SCRIPT_PATH" 2>&1)"
STATUS=$?
set -e

if [[ "$STATUS" -eq 0 ]]; then
  echo "expected GNU Skia wrapper run to fail" >&2
  exit 1
fi

grep -F 'rust-skia does not ship Windows GNU Skia binaries for x86_64-pc-windows-gnu' <<<"$OUTPUT" >/dev/null
grep -F './build-win-x64-software.sh' <<<"$OUTPUT" >/dev/null

if grep -F '==> Building mica-term' <<<"$OUTPUT" >/dev/null; then
  echo "GNU Skia guard should fail before cargo build starts" >&2
  exit 1
fi

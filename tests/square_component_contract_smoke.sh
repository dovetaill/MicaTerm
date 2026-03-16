#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"

if rg -n 'border-radius:' "$ROOT_DIR/ui" | rg -v 'border-radius:\s*0px;'; then
  echo "unexpected rounded border-radius remains under ui/" >&2
  exit 1
fi

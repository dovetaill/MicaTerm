#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

usage() {
  cat <<'EOF'
Usage: ./build-win-x64.sh [--help]

Single Windows x64 wrapper.

Default target:
  x86_64-pc-windows-gnu

Override example:
  TARGET=x86_64-pc-windows-msvc ./build-win-x64.sh

Outputs:
  dist/mica-term-x86_64-pc-windows-gnu-release.zip
  dist/mica-term-x86_64-pc-windows-msvc-release.zip
EOF
}

if [[ "${1:-}" == "--help" ]]; then
  usage
  exit 0
fi

TARGET="${TARGET:-x86_64-pc-windows-gnu}"
echo "==> Windows wrapper target: $TARGET"
export TARGET

exec "$ROOT_DIR/build-desktop.sh" "$@"

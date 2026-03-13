#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

usage() {
  cat <<'EOF'
Usage: ./build-linux-x64-femtovg-wgpu.sh [--help]

Mainline target:
  x86_64-unknown-linux-gnu

Output:
  dist/mica-term-x86_64-unknown-linux-gnu-release.tar.gz
EOF
}

if [[ "${1:-}" == "--help" ]]; then
  usage
  exit 0
fi

export TARGET="${TARGET:-x86_64-unknown-linux-gnu}"

exec "$ROOT_DIR/build-desktop.sh" "$@"

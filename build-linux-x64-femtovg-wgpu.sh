#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

usage() {
  cat <<'EOF'
Usage: ./build-linux-x64-femtovg-wgpu.sh [--help]

Experimental target:
  x86_64-unknown-linux-gnu

Cargo shape:
  --no-default-features
  --features femtovg-wgpu-experimental

Output:
  dist/mica-term-femtovg-wgpu-experimental-x86_64-unknown-linux-gnu-release.tar.gz
EOF
}

if [[ "${1:-}" == "--help" ]]; then
  usage
  exit 0
fi

export TARGET="${TARGET:-x86_64-unknown-linux-gnu}"
export APP_NAME="${APP_NAME:-mica-term-femtovg-wgpu-experimental}"
export BIN_NAME="${BIN_NAME:-mica-term}"
export CARGO_NO_DEFAULT_FEATURES="${CARGO_NO_DEFAULT_FEATURES:-1}"
export CARGO_FEATURES="${CARGO_FEATURES:-femtovg-wgpu-experimental}"

exec "$ROOT_DIR/build-desktop.sh" "$@"

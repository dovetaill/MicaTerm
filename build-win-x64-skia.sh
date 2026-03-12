#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export TARGET="${TARGET:-x86_64-pc-windows-msvc}"
export CARGO_NO_DEFAULT_FEATURES="${CARGO_NO_DEFAULT_FEATURES:-1}"
export CARGO_FEATURES="${CARGO_FEATURES:-windows-skia-experimental}"
exec "$ROOT_DIR/build-desktop.sh" "$@"

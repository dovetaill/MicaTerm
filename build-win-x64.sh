#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export TARGET="${TARGET:-x86_64-pc-windows-gnu}"
exec "$ROOT_DIR/build-desktop.sh" "$@"

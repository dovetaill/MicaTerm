#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export TARGET="${TARGET:-x86_64-pc-windows-gnu}"
export APP_NAME="${APP_NAME:-windows-theme-repro}"
export BIN_NAME="${BIN_NAME:-windows_theme_repro}"
exec "$ROOT_DIR/build-desktop.sh" "$@"

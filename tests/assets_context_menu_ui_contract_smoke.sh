#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ASSETS="$ROOT_DIR/ui/shell/assets-sidebar.slint"
APP_WINDOW="$ROOT_DIR/ui/app-window.slint"

grep -F 'callback asset-context-menu-requested(string, string, length, length);' "$ASSETS" >/dev/null
grep -F 'callback assets-context-menu-action-invoked(string);' "$APP_WINDOW" >/dev/null

#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROW="$ROOT_DIR/ui/components/asset-node-row.slint"
ASSETS="$ROOT_DIR/ui/shell/assets-sidebar.slint"

grep -F 'in property <int> depth;' "$ROW" >/dev/null
grep -F 'in property <bool> has-children;' "$ROW" >/dev/null
grep -F 'callback toggle-expanded-requested(string);' "$ROW" >/dev/null
grep -F 'callback asset-selected(string);' "$ASSETS" >/dev/null

#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROW="$ROOT_DIR/ui/components/asset-node-row.slint"
ASSETS="$ROOT_DIR/ui/shell/assets-sidebar.slint"

grep -F 'import { ListView } from "std-widgets.slint";' "$ASSETS" >/dev/null
grep -F 'for item in root.console-asset-items : AssetNodeRow' "$ASSETS" >/dev/null
grep -F 'in property <int> depth: 0;' "$ROW" >/dev/null
grep -F 'in property <bool> has-children: false;' "$ROW" >/dev/null
grep -F 'in property <string> path-hint: "";' "$ROW" >/dev/null
grep -F 'private property <image> chevron-icon:' "$ROW" >/dev/null
grep -F 'callback toggle-expanded-requested(string);' "$ROW" >/dev/null
grep -F 'callback asset-selected(string);' "$ASSETS" >/dev/null

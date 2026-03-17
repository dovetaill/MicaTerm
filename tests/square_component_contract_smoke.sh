#!/usr/bin/env bash
# Preserves the intentionally square shell component styling contract.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"

grep -F 'border-radius: 8px;' "$ROOT_DIR/ui/components/titlebar-tooltip.slint" >/dev/null
grep -F 'border-radius: 8px;' "$ROOT_DIR/ui/components/titlebar-icon-button.slint" >/dev/null
grep -F 'border-radius: 6px;' "$ROOT_DIR/ui/components/sidebar-toolbar-icon-button.slint" >/dev/null
grep -F 'border-radius: 8px;' "$ROOT_DIR/ui/components/sidebar-nav-button.slint" >/dev/null

if rg -n 'border-radius:' "$ROOT_DIR/ui" | rg -v \
  'ui/components/titlebar-tooltip\.slint:.*border-radius:\s*8px;|ui/components/titlebar-icon-button\.slint:.*border-radius:\s*8px;|ui/components/sidebar-toolbar-icon-button\.slint:.*border-radius:\s*6px;|ui/components/sidebar-nav-button\.slint:.*border-radius:\s*8px;|border-radius:\s*0px;'
then
  echo "unexpected rounded border-radius remains under ui/" >&2
  exit 1
fi

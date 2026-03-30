#!/usr/bin/env bash
# Verifies the welcome quick launch dashboard contract exported by the Slint shell.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WELCOME="$ROOT_DIR/ui/welcome/welcome-view.slint"
APP_WINDOW="$ROOT_DIR/ui/app-window.slint"
WORKSPACE_PANE="$ROOT_DIR/ui/shell/workspace-pane.slint"
SESSION_HOST="$ROOT_DIR/ui/shell/terminal-session-host.slint"

grep -F 'text: "Quick Start"' "$WELCOME" >/dev/null
grep -F 'QuickLaunchSection' "$WELCOME" >/dev/null
grep -F 'QuickLaunchDetailPane' "$WELCOME" >/dev/null
grep -F 'callback welcome-quick-launch-connect-requested(string);' "$APP_WINDOW" >/dev/null
grep -F 'callback welcome-quick-launch-connect-in-new-tab-requested(string);' "$APP_WINDOW" >/dev/null
grep -F 'callback welcome-quick-launch-toggle-favorite-requested(string);' "$APP_WINDOW" >/dev/null
grep -F 'callback welcome-quick-launch-reveal-in-assets-requested(string);' "$APP_WINDOW" >/dev/null
grep -F 'callback welcome-quick-launch-search-changed(string);' "$WORKSPACE_PANE" >/dev/null
grep -F 'callback welcome-quick-launch-asset-selected(string);' "$SESSION_HOST" >/dev/null

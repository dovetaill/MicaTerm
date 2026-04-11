#!/usr/bin/env bash
set -euo pipefail

worktree_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$worktree_root"

legacy_variant='Scene''Image'
legacy_label='scene''-''image'

if grep -F "$legacy_variant" src/app/runtime_profile.rs >/dev/null; then
    echo "runtime_profile.rs still contains the retired Windows variant marker" >&2
    exit 1
fi
if grep -F "$legacy_label" src/app/runtime_profile.rs >/dev/null; then
    echo "runtime_profile.rs still contains the retired Windows label" >&2
    exit 1
fi

grep -F 'visible: root.session-render-mode == "bitmap";' ui/shell/terminal-session-host.slint >/dev/null
grep -F 'if root.session-render-mode == "bitmap" && root.session-cursor-visible && root.cursor-blink-visible : cursor-overlay := Rectangle {' ui/shell/terminal-session-host.slint >/dev/null
grep -F 'if root.mode == "terminal" && root.session-render-mode == "bitmap" && root.session-cursor-visible && root.session-cursor-blinking {' ui/shell/terminal-session-host.slint >/dev/null

grep -F 'fn clear_workspace_session_cursor_overlay(window: &AppWindow) {' src/app/bootstrap.rs >/dev/null
grep -F 'clear_workspace_session_cursor_overlay(window);' src/app/bootstrap.rs >/dev/null
! grep -F 'if let Some(cursor) = native_cursor {' src/app/bootstrap.rs >/dev/null

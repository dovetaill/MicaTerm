#!/usr/bin/env bash
set -euo pipefail

worktree_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$worktree_root"

composition_block="$(sed -n '/pub fn terminal_composition_mode(self) -> TerminalCompositionMode {/,/pub fn prefers_native_terminal_renderer(self) -> bool {/p' src/app/runtime_profile.rs)"
grep -F 'pub enum TerminalSubsystemMode' src/app/runtime_profile.rs >/dev/null
grep -F 'std::env::var("MICA_TERM_TERMINAL_SUBSYSTEM")' src/app/runtime_profile.rs >/dev/null
grep -F 'option_env!("MICA_TERM_PACKAGE_TERMINAL_SUBSYSTEM")' src/app/runtime_profile.rs >/dev/null
grep -F 'TerminalSubsystemMode::SceneImage => TerminalCompositionMode::SceneImage' <<<"$composition_block" >/dev/null
grep -F 'TerminalSubsystemMode::RetainedNativeSurface => {' <<<"$composition_block" >/dev/null
grep -F 'TerminalCompositionMode::PostRenderNativeSurface' <<<"$composition_block" >/dev/null
! grep -F 'AppBuildFlavor::WindowsMainline if self.prefers_direct3d() => {' <<<"$composition_block" >/dev/null
grep -F 'export MICA_TERM_PACKAGE_TERMINAL_SUBSYSTEM="retained-native-surface"' build-win-x64.sh >/dev/null

grep -F 'visible: root.session-render-mode == "bitmap";' ui/shell/terminal-session-host.slint >/dev/null
grep -F 'if root.session-render-mode == "bitmap" && root.session-cursor-visible && root.cursor-blink-visible : cursor-overlay := Rectangle {' ui/shell/terminal-session-host.slint >/dev/null
grep -F 'if root.mode == "terminal" && root.session-render-mode == "bitmap" && root.session-cursor-visible && root.session-cursor-blinking {' ui/shell/terminal-session-host.slint >/dev/null

grep -F 'fn clear_workspace_session_cursor_overlay(window: &AppWindow) {' src/app/bootstrap.rs >/dev/null
grep -F 'clear_workspace_session_cursor_overlay(window);' src/app/bootstrap.rs >/dev/null
! grep -F 'if let Some(cursor) = native_cursor {' src/app/bootstrap.rs >/dev/null

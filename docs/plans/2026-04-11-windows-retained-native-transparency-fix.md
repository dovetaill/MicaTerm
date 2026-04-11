# Windows Retained-Native Terminal Transparency Fix

## Summary

This note records the root cause and final fix for the Windows retained-native terminal issue that still showed a white board / see-through host surface / incorrect screenshots after flicker had already been removed.

The final root cause was not the terminal glyph draw path itself. The retained child HWND was alive, drawing, and reporting valid glyph diagnostics. The remaining problem came from the host window composition chain on Windows:

- the retained-native terminal was presented through a child HWND
- the Slint/winit host window still defaulted to a transparent shell window
- the application still applied the `MicaAlt` backdrop to that host window
- Windows then composed the child surface under a transparent + backdrop-enabled host, which produced the remaining translucent/white-board/screenshot artifacts

## Evidence That Changed the Direction

The decisive evidence came from runtime diagnostics and logs:

- the retained child surface stayed `surface_visible=true`
- the native render target stayed `render_target_ready=true`
- DirectWrite glyph bounds and frame tokens continued updating normally
- this made a terminal-local alpha or glyph positioning bug much less likely than a host-window composition problem

That evidence moved the investigation away from text rendering details and toward the Windows shell host and Slint/winit window attributes.

## What Changed

### 1. Keep native child HWND repaint recovery deterministic

Files:

- `src/app/terminal_renderer/native_surface.rs`
- `src/app/terminal_renderer/platform/windows.rs`
- `src/app/terminal_renderer/platform/windows_child_host.rs`
- `src/app/windows_frame.rs`

Changes:

- promoted replay damage to a full repaint when a host redraw replay is pending
- promoted dirty-without-explicit-damage frames to full repaint
- added explicit fallback-paint ownership for the Windows child HWND host
- restricted fallback GDI paints to bootstrap / recovery phases instead of letting fallback paints continue over active Direct2D content
- kept the child host fallback background synchronized with the terminal default background
- disabled fallback paint again after a successful native present
- preserved parent clipping for child surfaces

These changes removed the earlier flicker and stopped stale or partially transparent child-host areas from lingering during redraw recovery.

### 2. Force an opaque Windows host window for retained-native startup

Files:

- `src/app/bootstrap.rs`
- `vendor/i-slint-backend-winit/winitwindowadapter.rs`

Changes:

- added a scoped bootstrap override via `MICA_TERM_FORCE_OPAQUE_HOST_WINDOW`
- applied the override only while creating the main Slint window for `TerminalSubsystemMode::RetainedNativeSurface`
- taught the vendored Slint winit backend to honor that override by switching `with_transparent(false)` on Windows during window creation and suspended-window recreation

This ensures the retained-native child HWND is no longer hosted under a transparent shell window.

### 3. Disable backdrop composition for retained-native workspace windows

Files:

- `src/app/bootstrap/shell_chrome.rs`
- `tests/top_status_bar_smoke.rs`

Changes:

- built the native window appearance request from the active runtime profile
- when the workspace runs in `RetainedNativeSurface` mode, forced `BackdropPreference::None`
- kept the normal dark/light theme synchronization behavior intact

This removes the remaining `MicaAlt` backdrop from the retained-native host window so the child HWND is not composed through a translucent backdrop path.

### 4. Preserve vendored Slint 1.15.1 compatibility while fixing the host path

Files:

- `vendor/i-slint-backend-winit/lib.rs`
- `vendor/i-slint-backend-winit/renderer/skia.rs`
- `vendor/i-slint-renderer-skia/lib.rs`
- `vendor/i-slint-renderer-skia/d3d_surface.rs`
- `tests/windows_native_terminal_host_window_contract_spec.rs`

Changes:

- restored the local `WinitWindowMemoryPurge` hook that had been lost during an earlier vendor sync attempt
- restored the Skia purge plumbing used by the app's terminal-memory diagnostics contracts
- added source-level contract coverage for the new opaque-host override path

This kept the local vendored Slint baseline buildable and testable while the Windows host fix was applied.

## Was Slint Upstream Changed?

### Official upstream

No official upstream Slint repository was modified by this fix.

### User fork

The user fork branch was updated separately:

- fork: `dovetaill/slint`
- branch: `fix/windows-software-partial-visibility`
- that branch was merged with the latest fork `master`
- resulting branch head: `32adb14a5`

### What mica-term actually uses

`mica-term` did **not** switch wholesale to the merged fork branch.

Reason:

- the project currently depends on the Slint `1.15.1` API surface
- the merged fork branch now includes newer upstream code on the `1.16` line
- directly replacing the vendored backend with the merged fork caused API mismatches and build failures

So the final fix here was:

- keep the local `1.15.1` vendored baseline
- apply the minimum local vendored Windows host override in `vendor/i-slint-backend-winit`
- keep the rest of the retained-native fix inside `mica-term`

## Verification

The fix was validated with:

- `cargo test --test slint_backend_purge_contract_spec -- --nocapture`
- `cargo test --test windows_native_terminal_host_window_contract_spec -- --nocapture`
- `cargo test --test top_status_bar_smoke retained_native_terminal_bind_disables_backdrop_composition -- --nocapture`
- `cargo test --test top_status_bar_smoke bootstrap_syncs_native_window_effects_on_bind_and_theme_toggle -- --nocapture`
- `bash tests/slint_backend_patch_contract_smoke.sh`
- `cargo check`
- `./build-win-x64.sh`

The packaged Windows build was then re-tested by the user and confirmed fixed.

## Final Cause Statement

The final Windows transparency issue was caused by the retained-native child HWND being composed under a transparent + backdrop-enabled Slint/winit host window, not by the terminal text renderer itself.

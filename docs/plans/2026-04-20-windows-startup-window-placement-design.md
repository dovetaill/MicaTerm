# Windows Startup Window Placement Design

**Date:** 2026-04-20

## Goal

Make the Windows desktop window open in a predictable, visible, and comfortable place.
The app should stop appearing partly below the screen or arbitrarily near the top-left corner,
and the default window should feel slightly larger on first launch.

## Current State

- Startup currently applies only a default size in `src/app/bootstrap.rs`.
- The main window does not set an explicit startup position.
- Top-level window bounds are not persisted in `UiPreferences`.
- Windows-specific frame code already knows how to query monitor work area and window placement,
  but that data is used for custom frame behavior, not startup restoration.

## Industry Practice

The mature Windows pattern is not "always center the app".
It is:

1. First launch: create a reasonable default size and place the window in a visible work area,
   commonly centered.
2. Later launches: restore the user's last normal window bounds.
3. If the saved bounds are no longer valid because monitors, DPI, or taskbar layout changed,
   clamp or re-center the window into the nearest visible work area.

This matches Microsoft's Win32 and WPF guidance and also aligns with higher-level desktop
framework behavior:

- Win32 `CW_USEDEFAULT` gives a system default, not a polished centered experience.
- Microsoft provides a sample for saving and restoring window placement with multi-monitor awareness.
- `SetWindowPlacement` documents that placement should be treated carefully with work-area
  coordinates and that Windows will correct completely off-screen placement.
- WPF and Electron both expose explicit centering options, which are typically used for first
  show or child dialogs rather than forced every-launch placement.

References:
- https://learn.microsoft.com/en-us/windows/win32/winmsg/window-features
- https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-windowplacement
- https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwindowplacement
- https://learn.microsoft.com/en-us/samples/microsoft/wpf-samples/save-window-placement-state-sample/
- https://learn.microsoft.com/en-us/dotnet/api/system.windows.window.windowstartuplocation?view=windowsdesktop-10.0
- https://www.electronjs.org/docs/latest/api/browser-window

## Decision

Use this Windows startup policy:

- If no saved main-window bounds exist, open the window centered in the active monitor's work area.
- Persist the last restored window bounds and reuse them on the next launch.
- When restoring saved bounds, ensure the rectangle still intersects a visible monitor work area.
- If saved bounds are invalid or mostly off-screen, fall back to centered placement in the nearest
  available work area.
- Increase the default window size modestly so first launch feels less cramped.

## Scope

### In scope

- Windows startup placement for the main application window.
- Persisting the main window's last restored bounds in UI preferences.
- Restoring the saved bounds when valid.
- Re-centering or clamping when saved bounds are outside the visible work area.
- Slightly increasing the first-launch default size.

### Out of scope

- Changing Linux or macOS startup behavior.
- Persisting modal positions.
- Full workspace/session restoration unrelated to the main window frame.
- Reworking maximize/snap behavior beyond preserving sane restored bounds.

## Proposed Architecture

### 1. Persisted window bounds in UI preferences

Add a small persisted structure in `src/app/ui_preferences.rs` for the main window's restored bounds.
This keeps the feature next to the existing theme and window-level preferences instead of creating a
new config file.

### 2. Windows startup geometry resolver

Add a Windows-focused helper that can:

- inspect the current monitor/work area,
- compute centered first-launch bounds,
- validate saved bounds against available monitors,
- clamp or fall back when the saved rectangle is not usable.

The logic should be mostly pure and testable with synthetic monitor rectangles.

### 3. Startup application in bootstrap

During startup in `src/app/bootstrap.rs`:

- load UI preferences,
- choose the initial size and optional position,
- apply the resolved bounds before `window.run()`.

This should happen before the window is shown so the user does not see the app jump.

### 4. Persist bounds on move/resize changes

Extend the existing winit window event tracking in `src/app/bootstrap/windowing.rs` so that when the
window is moved or resized in restored mode, the new bounds are saved back to `UiPreferences`.
Avoid overwriting restored bounds while maximized.

## Behavior Details

### First launch

- Use a larger default size than today.
- Center the window inside the current monitor's work area, not the full monitor rect.
- This avoids taskbar overlap and feels more intentional on Windows.

### Subsequent launches

- If saved restored bounds are still reasonable, reopen there.
- If monitor topology changed, move the window back into a visible work area.
- If the saved bounds cannot be trusted, use the same centered-first-launch fallback.

### Size recommendation

Current default is `1440x900`.
Recommended default is approximately `1600x960`, still bounded by the monitor work area so the
window does not exceed the visible desktop on smaller displays.

## Error Handling

- If preferences cannot be loaded, fall back to the centered default behavior.
- If Windows monitor/work-area queries fail, fall back to the larger default size without a custom
  position instead of blocking startup.
- If preference saving fails, log the error and keep the session usable.

## Testing Strategy

- Add unit tests for persisted bounds serialization defaults.
- Add unit tests for centering and off-screen recovery with synthetic monitor/work-area rectangles.
- Add unit tests for "keep restored bounds when valid" and "fallback to centered bounds when
  invalid" decisions.
- Run targeted Rust tests and at least one full `cargo test` pass relevant to the touched modules.

## Why This Approach

This approach matches common Windows desktop behavior without being overly custom.
It gives users a polished first launch, respects user-adjusted window placement later, and handles
multi-monitor/taskbar changes gracefully.

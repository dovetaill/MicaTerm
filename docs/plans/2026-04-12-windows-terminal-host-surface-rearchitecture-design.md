# Windows Terminal Host-Surface Rearchitecture Design

**Status:** Proposed

**Goal**

Retire the current Windows `child HWND` interactive terminal-body architecture and replace it with a same-host-window, composition-backed native surface path that keeps the terminal pane visually native while restoring reliable pane fill, right-click behavior, selection/copy integration, minimize/restore stability, and overlay correctness.

## Decision Summary

The recommended repair is a **host-surface rearchitecture**, not another incremental patch to the current child-window path:

- stop treating the main Windows terminal body as a separate interactive `child HWND`,
- keep the top-level Slint/winit host window as the owner of input, focus, hit-testing, context-menu state, and pane layout,
- change the native renderer into a **surface producer** that renders into a host-owned composition surface,
- wire that surface into the same host-window composition/presentation path instead of creating, hiding, and destroying a separate child window for ordinary workspace interaction.

This should be implemented as a **same-host-window composition surface** on Windows, with DirectComposition-backed ownership as the primary direction. It should **not** be implemented by returning to same-`HWND` `GetDC` / `BindDC` overlay drawing, and it should **not** keep the child-`HWND` path as the main production architecture.

## Problem Summary

The current retained-native Windows path is no longer failing in one isolated way. It now has a cluster of failures that all trace back to the same architectural mismatch: the terminal body is being hosted as an input-transparent, disabled `child HWND` that is created and destroyed according to host-shell state.

Current local evidence:

- `src/app/terminal_renderer/platform/windows_child_host.rs` creates a `WS_CHILD | WS_DISABLED | WS_VISIBLE` child window and forwards `WM_RBUTTON*` / `WM_CONTEXTMENU` back to the host while also returning `HTTRANSPARENT` and `MA_NOACTIVATE`.
- `src/app/terminal_renderer/platform/windows.rs` treats that child window as the terminal presentation target and tears it down when the effective rect is not visible.
- `src/app/bootstrap.rs` currently makes `workspace_native_terminal_rect(...)` return `NativeTerminalSurfaceRect::default()` when modal state is active **or** `workspace_session_context_menu_open` is true.
- `src/app/bootstrap.rs` then pushes that zero rect into the native surface path, which causes `src/app/terminal_renderer/platform/windows.rs` to destroy the child surface window.

That means opening a context menu or hitting a stale visibility/layout transition can physically remove the visible terminal surface. The result matches the reported behavior:

- the pane does not always fill the host region,
- right-click can reveal menu UI while the terminal body vanishes underneath,
- minimize/restore and visibility transitions can recreate the surface with stale geometry,
- selection/copy integration becomes fragile because the body surface is not the same visual/input owner as the shell.

This is not just a typography issue. Typography can be tuned later, but the first problem is that the **presentation and interaction ownership model is wrong for a primary terminal body**.

## Requirements

- Keep Windows terminal text on a native rendering path.
- Do not rely on a separate interactive `child HWND` for the main workspace terminal body.
- Do not revert to same-`HWND` host-window `GetDC` / `BindDC` overlay drawing.
- Keep monospaced grid semantics, cursor placement, selection geometry, hit-testing, and scrollback contracts stable.
- Preserve host-shell ownership of focus, context-menu state, modal layering, selection affordances, and pane layout.
- Ensure terminal panes continue to fill their host rect through right-click, overlay, tab-switch, hidden, and minimize/restore transitions.
- Prefer a design aligned with mainstream Windows terminal/composition patterns.

## External Evidence

### Mainstream Windows terminal architecture

- Windows Terminal's main product architecture uses a host control and composition/swapchain-based rendering path attached inside the app's visual tree, not a transparent disabled child window as the main interactive terminal body.
- Windows Terminal's WPF `HwndHost` path exists, but that is an embedding/interop path rather than the core application model.
- Microsoft guidance for DirectX/XAML interop and `SwapChainPanel` centers on putting accelerated content into the host composition tree rather than juggling a hidden input-transparent child `HWND` above or below other UI.

Representative references:

- `Building Windows Terminal with WinUI`: https://devblogs.microsoft.com/commandline/building-windows-terminal-with-winui/
- `TermControl.xaml`: https://github.com/microsoft/terminal/blob/master/src/cascadia/TerminalControl/TermControl.xaml
- `ControlCore.h`: https://github.com/microsoft/terminal/blob/master/src/cascadia/TerminalControl/ControlCore.h
- `TerminalContainer.cs` (WPF embedding path): https://github.com/microsoft/terminal/blob/master/src/cascadia/WpfTerminalControl/TerminalContainer.cs
- `DirectX and XAML interop`: https://learn.microsoft.com/en-us/windows/uwp/gaming/directx-and-xaml-interop
- `SwapChainPanel`: https://learn.microsoft.com/en-us/uwp/api/windows.ui.xaml.controls.swapchainpanel?view=winrt-22621

### Win32 message semantics

- `WM_CONTEXTMENU` and mouse activation behavior are much easier to keep correct when the visible body is not a disabled child window that forwards input back to its parent.
- A transparent/non-activating child may be acceptable for narrow embed scenarios, but it is a poor fit for a primary, always-interactive terminal body that must coexist with selection, menu state, overlays, and pane resizing.

Representative references:

- `WM_CONTEXTMENU`: https://learn.microsoft.com/en-us/windows/win32/menurc/wm-contextmenu
- `WM_MOUSEACTIVATE`: https://learn.microsoft.com/en-us/windows/win32/inputdev/wm-mouseactivate

## Why The Current Path Feels Wrong

The current child-window model makes the terminal body look like a separate surface that the shell temporarily mounts and unmounts. That introduces two structural problems:

1. **Visibility is coupled to shell state too aggressively.**  
   The terminal rect is collapsed to zero when the workspace context menu opens, so the renderer destroys the actual visible surface at the moment the user expects a simple overlay/menu action.

2. **Interaction ownership is split.**  
   The terminal pixels live in one window while focus, context menu, hit-testing, selection state, and modal behavior live in another. That is the opposite of how mainstream terminal apps structure their primary body surface.

Because of that split, the current path is prone to:

- stale geometry,
- overlay z-order confusion,
- right-click/copy/selection drift,
- minimize/restore recreation bugs,
- "pane not fully filled" artifacts,
- diagnostics that describe `surface_hwnd` lifecycle rather than a stable host-owned surface contract.

## Considered Approaches

### Option 1: Keep the current child `HWND` path and patch around it

Examples:

- stop zeroing rects on context menu open,
- add more forwarding for right-click and focus,
- special-case minimize/restore,
- keep tightening geometry sync and z-order behavior.

Pros:

- smallest code delta on paper,
- preserves the current `ID2D1HwndRenderTarget` path.

Cons:

- keeps the wrong ownership model,
- preserves split input/presentation responsibility,
- leaves the main terminal body dependent on child-window lifecycle quirks,
- likely leads to more special cases every time menu, modal, visibility, or selection behavior changes.

Recommendation: **reject** as the main path.

### Option 2: Revert to same-host-window `GetDC` / `BindDC` overlay drawing

Pros:

- removes the child-window lifecycle complexity.

Cons:

- returns to the already-rejected same-`HWND` presentation path that conflicted with the flip-model host renderer,
- risks blank or unreliable presentation again,
- does not align with the earlier retained-native recovery findings.

Recommendation: **reject**.

### Option 3: Move the terminal body to a same-host-window composition surface

Examples:

- host-owned DirectComposition surface,
- host-owned swapchain/composition bridge,
- native surface attached to the same host-window composition tree with no interactive child window.

Pros:

- aligns much better with mainstream Windows terminal architecture,
- keeps input, context menu, selection, and layout owned by the host shell,
- avoids destroying the visible body surface because a context menu or overlay opens,
- keeps a true native rendering path without relying on same-`HWND` DC rebinding.

Cons:

- larger change than another child-window patch,
- requires a new Windows composition-surface seam.

Recommendation: **choose this option**.

### Option 4: Rewrite the terminal into the host renderer's own draw stack

Pros:

- fully unified architecture in the long run.

Cons:

- too large for the current repair,
- mixes a reliability fix with a much broader renderer rewrite.

Recommendation: keep as a possible future direction, but **not** for this fix.

## Recommendation

Use **Option 3: same-host-window composition surface** as the new Windows terminal-body architecture.

The key design rule is:

> The Windows native terminal renderer should become a host-owned surface producer, not a separate interactive window.

That means:

- the host Slint/winit window remains the owner of input and pane interaction,
- the Windows native backend owns a composition-backed surface and native text rendering resources,
- bootstrap/layout code provides stable pane geometry without using context-menu-open state as a surface-destruction trigger,
- diagnostics describe host-surface readiness instead of child-window lifecycle.

## Chosen Architecture

### 1. Replace `surface_hwnd` ownership with host-surface ownership

The backend currently revolves around a `surface_hwnd`. That should be replaced by a host-surface state object that represents:

- the host `HWND`,
- the composition surface/swapchain handle,
- renderer/device generation,
- current pane rect in physical pixels,
- visibility/attachment state,
- fallback reason if native presentation cannot be established.

The surface itself should be host-owned and composition-backed. It should not be a separately interactive window.

### 2. Keep input, hit-testing, context menu, and selection in the host shell

The host shell already owns:

- pane layout,
- focus policy,
- selection state,
- context menu state,
- modal layering,
- right-click and copy/paste affordances.

That ownership should become explicit rather than incidental. The native renderer should not forward `WM_RBUTTON*` or `WM_CONTEXTMENU` from a separate child window because the main terminal body should no longer be a separate child window at all.

### 3. Stop using context-menu-open as a native-surface teardown condition

`workspace_native_terminal_rect(...)` currently returns a zero rect when `workspace_session_context_menu_open` is true. That behavior is incompatible with a stable terminal body.

The new contract should be:

- modal states that truly block or replace the pane may still suspend or hide the native body,
- opening a workspace context menu should **not** collapse the native body rect,
- overlays should layer over a still-present terminal body instead of destroying and recreating it.

### 4. Keep retained-native frame production, but retarget presentation

The existing retained-native pieces are still useful:

- `NativeTerminalFrame`,
- frame token accounting,
- damage tracking,
- glyph shaping/output diagnostics,
- present scheduling.

Those should stay where practical. The change is not "throw away retained-native"; the change is "present retained-native output through the correct host-surface architecture."

### 5. Use controlled fallback when host-surface initialization fails

If the composition surface cannot be initialized, fallback should be explicit and diagnosable. The preferred order is:

- native host-surface path,
- controlled bitmap presenter fallback if needed.

The repair should **not** silently resurrect the child-`HWND` path as a fallback hack.

### 6. Update diagnostics to describe host-surface state

Current logs emphasize child-window creation/destruction:

- `created retained-native child HWND host`
- `tearing down retained-native child HWND because the surface is not visible`
- `destroying retained-native child HWND host`

The new diagnostics should instead report:

- host-surface backend selected,
- composition-surface creation success/failure,
- current pane rect,
- attached/visible status,
- native text renderer path,
- fallback reason,
- whether overlays/context menus are layered over the body without collapsing it.

## Files Expected To Change

Primary source files:

- `src/app/terminal_renderer/platform/windows.rs`
- `src/app/terminal_renderer/platform/backend.rs`
- `src/app/terminal_renderer/native_surface.rs`
- `src/app/terminal_renderer/present_driver.rs`
- `src/app/terminal_presenter.rs`
- `src/app/bootstrap.rs`
- `src/app/terminal_renderer/diagnostics.rs`
- `src/app/windows_frame.rs`
- `ui/shell/terminal-session-host.slint`
- `ui/shell/workspace-pane.slint`
- `ui/app-window.slint`

Likely new file:

- `src/app/terminal_renderer/platform/windows_composition_surface.rs`

Likely retired from the main path:

- `src/app/terminal_renderer/platform/windows_child_host.rs`

Tests that must be rewritten because they currently encode child-window assumptions:

- `tests/native_terminal_surface_contract_spec.rs`
- `tests/windows_terminal_diagnostics_spec.rs`
- `tests/windows_native_text_renderer_contract_spec.rs`
- `tests/windows_native_terminal_host_window_contract_spec.rs`
- `tests/bootstrap_profile_smoke.rs`
- `tests/bootstrap_smoke.rs`

## Non-Goals

- No immediate typography retuning in this design pass.
- No change to the terminal font family chain requirement.
- No return to same-`HWND` overlay drawing.
- No attempt to keep `child HWND` as the preferred production path.
- No full renderer rewrite into a brand-new GPU-only text stack as part of this repair.

## Risks And Mitigations

- **Risk:** composition-surface integration is more work than another child-window patch.  
  **Mitigation:** keep the retained-native frame/damage pipeline, and only replace the presentation boundary.

- **Risk:** host-shell overlays still need careful ordering over the native body.  
  **Mitigation:** make overlay behavior part of the host-window contract and keep the body surface alive while menus/selection UI are shown.

- **Risk:** existing tests and diagnostics hard-code `child HWND` assumptions.  
  **Mitigation:** rewrite source-contract tests first so implementation work is driven by the new architecture rather than the old one.

- **Risk:** fallback behavior becomes ambiguous during migration.  
  **Mitigation:** explicitly document that controlled bitmap fallback is acceptable, but child-window resurrection is not.

## Success Criteria

After this rearchitecture lands, the Windows terminal body should meet all of the following:

- the main workspace terminal body is no longer implemented as an interactive `child HWND`,
- opening the workspace context menu no longer collapses or destroys the visible terminal body,
- the pane reliably fills its host rect across layout, tab, hide/show, and minimize/restore transitions,
- right-click, selection, copy/paste, and overlay behavior remain host-owned and reliable,
- diagnostics describe a host-surface/composition-surface path rather than child-window lifecycle churn,
- the native renderer stays on a real native text path without reintroducing same-`HWND` DC overlay problems.

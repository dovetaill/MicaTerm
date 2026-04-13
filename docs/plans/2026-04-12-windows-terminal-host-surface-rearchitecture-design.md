# Windows Terminal Host-Surface Rearchitecture Design

**Status:** Revised after runtime investigation

**Goal**

Retire the current Windows `child HWND` terminal-body architecture and restore a reliable, host-owned terminal body on Windows without sacrificing native text rendering. The immediate repair must keep pane fill, right-click behavior, selection/copy integration, minimize/restore stability, and overlay correctness intact. A true same-host-window DirectComposition body surface is deferred until the host renderer can participate in the same composition tree.

## Decision Summary

The architectural conclusion changed after runtime verification.

The repair is still a **host-owned rearchitecture**, not another incremental patch to the current child-window path:

- stop treating the main Windows terminal body as a separate interactive `child HWND`,
- keep the top-level Slint/winit host window as the owner of input, focus, hit-testing, context-menu state, overlays, and pane layout,
- keep Windows text rendering native,
- make the native renderer a **host-owned offscreen producer** whose visible output is composed by the host scene,
- defer true same-`HWND` DirectComposition body presentation until the host renderer itself can target a composition visual instead of presenting directly to the host `HWND`.

This means the immediate production fix should **not** attempt to place a new DirectComposition visual over the current same-`HWND` Slint/Skia flip-model host renderer. That combination breaks the design goal that host-shell overlays remain authoritative.

## Runtime Addendum: Why Plan A Is Blocked Today

Local runtime and source evidence established that the current placeholder `windows_composition_surface` seam is not a real composition-backed body surface:

- `src/app/terminal_renderer/platform/windows_composition_surface.rs` still uses `CreateHwndRenderTarget` / `ID2D1HwndRenderTarget` rather than a composition swapchain.
- `vendor/i-slint-renderer-skia/d3d_surface.rs` presents the host renderer with `IDXGIFactory2::CreateSwapChainForHwnd(...)` and `DXGI_SWAP_EFFECT_FLIP_DISCARD`.
- `vendor/i-slint-backend-winit/winitwindowadapter.rs` does call the native after-draw hook, so the blank-body regression is not explained by missing notifier timing anymore.
- Windows retesting showed the retained-native path was attached and the DirectWrite text path initialized successfully, yet the terminal body was still blank.

That pushed the investigation from "timing bug" to "presentation-boundary incompatibility".

The next prototype question was whether to replace the placeholder seam with a true DirectComposition visual + `CreateSwapChainForComposition(...)` body surface on the same host `HWND`.

That path is blocked under the current host renderer because:

- the host renderer still presents directly to the top-level `HWND`, and
- DirectComposition content targeted at that same `HWND` is composited above the window's direct present content.

Under the current UI structure, pane-local overlays such as the terminal context menu, selection affordances, IME previews, and other Slint-owned shell visuals must remain above the terminal body. A same-`HWND` DirectComposition terminal visual would invert that ownership and cause a new class of z-order regressions.

So the conclusion is not "DirectComposition is bad"; the conclusion is:

> A same-`HWND` DirectComposition terminal body is not viable until the host renderer also participates in the same composition tree.

## Problem Summary

The current retained-native Windows path no longer fails in one isolated way. It has multiple symptoms that all trace back to the same ownership mismatch: the terminal body is being treated as a separately mounted presentation target instead of as host-owned content.

Current local evidence:

- `src/app/terminal_renderer/platform/windows_child_host.rs` creates a `WS_CHILD | WS_DISABLED | WS_VISIBLE` child window and forwards `WM_RBUTTON*` / `WM_CONTEXTMENU` back to the host while also returning `HTTRANSPARENT` and `MA_NOACTIVATE`.
- `src/app/terminal_renderer/platform/windows.rs` treats that window or window-like target as the visible body surface and tears it down on visibility changes.
- `src/app/bootstrap.rs` has been using shell state transitions such as workspace context-menu open to collapse the native body rect to zero.
- The placeholder host-surface seam still renders through an `HWND` render target, which is structurally incompatible with the current flip-model host renderer.

This produces exactly the class of failures reported during manual testing:

- pane fill can be incomplete or blank,
- right-click can reveal menu UI while the terminal body disappears or is occluded,
- minimize/restore and visibility transitions can recreate stale geometry,
- selection/copy integration becomes fragile because the visible body and the interaction owner are split.

This is not merely a typography problem. Typography can be tuned later; the first-order problem is that the **visible terminal body is owned by the wrong presentation boundary**.

## Requirements

- Keep Windows terminal text on a native rendering path.
- Do not rely on a separately interactive `child HWND` for the main workspace terminal body.
- Do not return to same-`HWND` `GetDC` / `BindDC` overlay drawing.
- Preserve host ownership of focus, context menus, selection state, overlays, and pane layout.
- Ensure terminal panes continue to fill their host rect through right-click, overlay, tab-switch, hidden, and minimize/restore transitions.
- Keep monospaced grid semantics, cursor placement, selection geometry, hit-testing, and scrollback contracts stable.
- Prefer a repair that is executable on the current Slint/winit host renderer rather than one that depends on unimplemented upstream composition support.

## External Evidence

### Mainstream Windows terminal architecture

- Windows Terminal's main product architecture uses host-owned controls and composition-aware rendering paths rather than a transparent disabled child window as the primary terminal body.
- Microsoft guidance for DirectX/XAML interop centers on attaching accelerated content to the host composition tree, not on juggling a separately interactive child `HWND` for the primary surface.

Representative references:

- `Building Windows Terminal with WinUI`: https://devblogs.microsoft.com/commandline/building-windows-terminal-with-winui/
- `TermControl.xaml`: https://github.com/microsoft/terminal/blob/master/src/cascadia/TerminalControl/TermControl.xaml
- `ControlCore.h`: https://github.com/microsoft/terminal/blob/master/src/cascadia/TerminalControl/ControlCore.h
- `DirectX and XAML interop`: https://learn.microsoft.com/en-us/windows/uwp/gaming/directx-and-xaml-interop
- `SwapChainPanel`: https://learn.microsoft.com/en-us/uwp/api/windows.ui.xaml.controls.swapchainpanel?view=winrt-22621

### DirectComposition / DXGI guidance relevant to the blocker

- `IDXGIFactory2::CreateSwapChainForComposition` is the right primitive when content is going into a composition visual rather than directly to an `HWND`.
- `IDCompositionDevice::CreateTargetForHwnd` creates a composition target for a window. That only solves the layering problem cleanly when the content that must interoperate with it is also part of the same composition tree.

Representative references:

- `IDXGIFactory2::CreateSwapChainForComposition`: https://learn.microsoft.com/en-us/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgifactory2-createswapchainforcomposition
- `IDCompositionDevice::CreateTargetForHwnd`: https://learn.microsoft.com/en-us/windows/win32/api/dcomp/nf-dcomp-idcompositiondevice-createtargetforhwnd
- `For best performance, use DXGI flip model`: https://learn.microsoft.com/en-us/windows/win32/direct3ddxgi/for-best-performance--use-dxgi-flip-model

## Considered Approaches

### Option 1: Keep the current child `HWND` path and patch around it

Examples:

- stop zeroing rects on context menu open,
- add more forwarding for right-click and focus,
- special-case minimize/restore,
- keep tightening geometry sync and z-order behavior.

Pros:

- smallest code delta on paper,
- preserves today's `HWND` render target path.

Cons:

- keeps the wrong ownership model,
- preserves split input/presentation responsibility,
- keeps the body dependent on child-window lifecycle quirks,
- likely creates more special cases every time menu, modal, visibility, or selection behavior changes.

Recommendation: **reject**.

### Option 2: Revert to same-host-window `GetDC` / `BindDC` overlay drawing

Pros:

- removes child-window lifecycle complexity.

Cons:

- returns to a previously rejected same-`HWND` presentation path,
- risks blank or unreliable presentation again,
- does not align with the retained-native recovery findings.

Recommendation: **reject**.

### Option 3: Add a same-host-window DirectComposition body surface on top of the current host renderer

Examples:

- host-owned DirectComposition visual,
- `CreateSwapChainForComposition(...)`,
- DirectComposition target rooted at the current Slint/winit host `HWND`.

Pros:

- looks architecturally attractive on paper,
- removes child-window lifecycle ownership,
- keeps the terminal body on a real native swapchain path.

Cons:

- blocked by the current host renderer, which still presents directly to the same `HWND`,
- would put the terminal body above host-drawn pane-local overlays unless the host renderer also moves into the same composition tree,
- does not satisfy the current requirement that host-shell overlays remain authoritative.

Recommendation: **reject for the current repair**. Revisit only after host-renderer composition support exists.

### Option 4: Move visible terminal presentation back under the host scene while keeping native text rendering

Examples:

- keep the host window as the sole visible presenter,
- render Windows terminal text through native DirectWrite/Direct2D offscreen production,
- feed that output into the existing host-owned retained frame / bitmap presentation path.

Pros:

- executable on the current Slint/winit renderer,
- preserves host ownership of overlays, context menus, selection affordances, and pane layout,
- removes both child-window lifecycle bugs and same-`HWND` DComp z-order conflicts,
- keeps a native text pipeline while making the final visible presentation host-owned.

Cons:

- does not deliver the originally desired true DirectComposition terminal body,
- may require more careful offscreen invalidation and upload discipline,
- can be somewhat less elegant than a future fully unified composition tree.

Recommendation: **choose this option for the immediate repair**.

### Option 5: Replatform the entire Windows host renderer onto a composition-tree presentation model

Pros:

- restores the possibility of a real same-`HWND` DirectComposition terminal body,
- aligns best with a longer-term unified Windows presentation architecture.

Cons:

- larger and riskier than the current repair,
- likely requires vendored Slint/winit/Skia integration work,
- too large to mix into the immediate terminal regression repair.

Recommendation: **future direction, not current repair**.

## Recommendation

Use **Option 4: host-owned visible presentation with native offscreen text production** for the immediate fix.

The key design rule is:

> The Windows native terminal renderer should remain a native text producer, but the host scene should remain the sole visible presenter until the host renderer itself can join a composition tree.

That means:

- the host Slint/winit window remains the owner of visible composition, input, and pane interaction,
- the Windows backend keeps native text rendering and retained-native frame production,
- the visible body is composed by the host scene rather than by an `HWND`-bound or DirectComposition-overlaid surface,
- true DirectComposition body presentation is deferred, not faked.

## Chosen Architecture

### 1. Keep the host window as the sole visible presenter

The terminal body should no longer be visually owned by a child window, an `HWND` render target, or a same-`HWND` DirectComposition visual layered outside the host scene. The host renderer stays responsible for what the user actually sees.

### 2. Keep Windows native rendering, but make it an offscreen producer

The retained-native backend should continue to own:

- `NativeTerminalFrame`,
- frame token accounting,
- damage tracking,
- glyph shaping/output diagnostics,
- native DirectWrite text rendering,
- any offscreen D2D/DWrite bitmap preparation needed for visible presentation.

The change is the presentation boundary: the backend produces host-owned content instead of directly presenting into an `HWND`.

#### Immediate bridge detail: use a WIC-backed offscreen bitmap, not a fake composition seam

Runtime inspection showed an additional implementation gap in the first revision of this design: simply removing the visible `HWND` render target from `src/app/terminal_renderer/platform/windows.rs` is not enough, because the current host UI only displays `session-surface-image` in the scene-owned image path while `PresentedTerminalFrame::Native` still flows into `NativeTerminalSurface`.

So the immediate host-owned bridge needs one more explicit rule:

- Windows native text should rasterize into an offscreen WIC bitmap through `ID2D1Factory::CreateWicBitmapRenderTarget(...)`,
- the backend should read out BGRA pixels from the `IWICBitmap` via `Lock` / `CopyPixels`,
- that BGRA payload should then be published back into the host-owned `session-surface-image` contract so the Slint scene remains the sole visible presenter,
- the old `windows_composition_surface` placeholder must not remain in the production path pretending to be a composition-backed visible surface when it still binds to `CreateHwndRenderTarget(...)`.

This keeps the text renderer native while making the visible body explicitly scene-owned on the current Slint/winit renderer.

### 3. Preserve host ownership of overlays and interaction state

The host shell already owns:

- pane layout,
- focus policy,
- selection state,
- context-menu state,
- modal layering,
- right-click and copy/paste affordances,
- scrollbars and pane-local overlay visuals.

That ownership must remain explicit and authoritative.

### 4. Stop using context-menu-open as a native-surface teardown condition

The native terminal rect must no longer collapse to zero just because the workspace context menu opens. Menus and overlays should layer over a still-present terminal body.

### 5. Reframe diagnostics around host-owned presentation

Diagnostics should report:

- host `HWND`,
- whether the host-owned terminal body is attached and visible,
- native text renderer path,
- fallback reason,
- current rect,
- draw counters and frame tokens,
- any transitional host-surface state only as an implementation detail, not as the core ownership model.

### 6. Defer true DirectComposition until the host renderer can join the same composition tree

A future Windows-specific follow-up may still implement a real composition swapchain terminal body, but only after the host renderer can target a visual tree-compatible presentation path. That work is intentionally out of scope for this repair.

## Files Expected To Change

Primary source files:

- `src/app/terminal_renderer/platform/windows.rs`
- `src/app/terminal_renderer/platform/backend.rs`
- `src/app/terminal_renderer/native_surface.rs`
- `src/app/terminal_renderer/present_driver.rs`
- `src/app/terminal_presenter.rs`
- `src/app/bootstrap.rs`
- `src/app/terminal_renderer/diagnostics.rs`
- `ui/shell/terminal-session-host.slint`
- `Cargo.toml`

The WIC-based host bridge also makes `src/app/terminal_renderer/platform/windows_composition_surface.rs` a retirement candidate for the main path rather than the centerpiece of the fix.
- `src/app/windows_frame.rs`
- `ui/shell/terminal-session-host.slint`
- `ui/shell/workspace-pane.slint`
- `ui/app-window.slint`

Transitional files that may be rewritten or retired from the main path:

- `src/app/terminal_renderer/platform/windows_composition_surface.rs`
- `src/app/terminal_renderer/platform/windows_child_host.rs`

Tests that must reflect the new ownership model:

- `tests/native_terminal_surface_contract_spec.rs`
- `tests/windows_terminal_diagnostics_spec.rs`
- `tests/windows_native_text_renderer_contract_spec.rs`
- `tests/windows_native_terminal_host_window_contract_spec.rs`
- `tests/bootstrap_profile_smoke.rs`
- `tests/bootstrap_smoke.rs`

## Non-Goals

- No return to same-`HWND` DC overlay drawing.
- No attempt to keep `child HWND` as the preferred production path.
- No fake "DirectComposition success" that still binds the terminal body to `CreateHwndRenderTarget`.
- No full vendored Slint renderer rewrite as part of this repair.
- No immediate typography retuning in this design pass.

## Risks And Mitigations

- **Risk:** offscreen native-text presentation is less direct than a swapchain-backed body surface.  
  **Mitigation:** keep the retained-native frame/damage pipeline and only replace the visible presentation boundary.

- **Risk:** placeholder host-surface code could confuse future maintenance if left half-wired.  
  **Mitigation:** either retire it from the main path or clearly mark it as deferred/future-only infrastructure.

- **Risk:** tests and diagnostics still encode `child HWND` or fake host-surface assumptions.  
  **Mitigation:** rewrite source-contract tests first so implementation is driven by the corrected ownership model.

- **Risk:** a future true DirectComposition path is forgotten.  
  **Mitigation:** document it explicitly as a deferred architecture track that depends on host-renderer composition support.

## Success Criteria

After this rearchitecture lands, the Windows terminal body should meet all of the following:

- the main workspace terminal body is no longer implemented as an interactive `child HWND`,
- the main path no longer relies on `CreateHwndRenderTarget` / `ID2D1HwndRenderTarget` ownership for visible body presentation,
- opening the workspace context menu no longer collapses or destroys the visible terminal body,
- the pane reliably fills its host rect across layout, tab, hide/show, and minimize/restore transitions,
- right-click, selection, copy/paste, scrollbar, and overlay behavior remain host-owned and reliable,
- diagnostics describe host-owned presentation plus native text state rather than child-window lifecycle churn,
- the native renderer stays on a real native text path without reintroducing same-`HWND` DC overlay problems.

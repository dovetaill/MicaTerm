# Retained-Native Child HWND Design

**Status:** Approved

**Goal**

Replace the blank Windows retained-native terminal path with a true retained-native presentation boundary that does not depend on drawing into the Slint/Skia host `HWND`.

## Problem Summary

The current Windows retained-native backend binds a Direct2D DC render target to the top-level host window and paints terminal content into a terminal sub-rect of that same `HWND`.

- The same-HWND drawing path is implemented in `src/app/terminal_renderer/platform/windows.rs` via `GetDC(HWND(host_hwnd as _))` and `render_target.BindDC(...)`.
- The host Slint/Skia Windows renderer presents through a DXGI flip-model swapchain in `/root/.cargo/registry/src/mirrors.aliyun.com-0671735e7cc7f5e7/i-slint-renderer-skia-1.15.1/d3d_surface.rs` using `DXGI_SWAP_EFFECT_FLIP_DISCARD`.
- Runtime diagnostics already prove the terminal renderer is producing visible content candidates: background runs, glyph counts, cursor visibility, and replay counts all advance while the pane remains blank.

This means the bug is not text shaping, baseline alignment, padding, or redraw scheduling anymore. The bug is that the terminal pixels are being generated on a presentation path that Windows does not reliably show when the host `HWND` is already owned by a flip-model swapchain.

## Requirements

- Keep retained-native as a true native presenter on Windows.
- Do not change terminal typography metrics, font size, line height, baseline policy, or padding.
- Preserve the current retained frame, damage tracking, and diagnostics model where possible.
- Avoid broad renderer rewrites in the first fix.
- Make the presentation architecture consistent with mature Windows guidance.

## External Evidence

### Microsoft guidance

Microsoft's DXGI flip-model documentation says flip model does not support layering multiple APIs into the same `HWND` on a present-by-present basis, and GDI updates on the same `HWND` are not a supported composition strategy for a flip-model host window.

References:
- `DXGI flip model`: https://learn.microsoft.com/en-us/windows/win32/direct3ddxgi/dxgi-flip-model
- `For best performance, use DXGI flip model`: https://learn.microsoft.com/en-us/windows/win32/direct3ddxgi/for-best-performance--use-dxgi-flip-model
- `DXGI_SWAP_EFFECT`: https://learn.microsoft.com/en-us/windows/win32/api/dxgi/ne-dxgi-dxgi_swap_effect
- `CreateSwapChainForComposition`: https://learn.microsoft.com/en-us/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgifactory2-createswapchainforcomposition
- `DCompositionCreateSurfaceHandle`: https://learn.microsoft.com/en-us/windows/win32/api/dcomp/nf-dcomp-dcompositioncreatesurfacehandle

### Mature terminal implementations

- Windows Terminal uses a dedicated swapchain/composition-hosted terminal surface and attaches it into XAML via `SwapChainPanel`, rather than painting terminal content into the same host window DC.
- WezTerm and Alacritty both use unified GPU-owned window surfaces instead of same-HWND GDI/DC overlay drawing.

Representative references:
- Windows Terminal `TermControl.xaml`: https://github.com/microsoft/terminal/blob/main/src/cascadia/TerminalControl/TermControl.xaml
- Windows Terminal `TermControl.cpp`: https://github.com/microsoft/terminal/blob/main/src/cascadia/TerminalControl/TermControl.cpp
- Windows Terminal `AtlasEngine.r.cpp`: https://github.com/microsoft/terminal/blob/main/src/renderer/atlas/AtlasEngine.r.cpp
- WezTerm frontend docs: https://github.com/wezterm/wezterm/blob/main/docs/config/lua/config/front_end.md

## Considered Approaches

### Option 1: DirectComposition visual plus dedicated composition swapchain

Render the terminal into a dedicated composition-backed DXGI surface and attach it into the host visual tree.

Pros:
- Closest to Windows Terminal's production architecture.
- Most modern Windows composition model.
- Clean presentation boundary.

Cons:
- Highest integration risk with the current `Slint + winit + Skia` stack.
- Requires substantial host-side compositor integration work before the first visible fix lands.

### Option 2: Dedicated child `HWND` retained-native host

Create a real child window for the terminal pane, and make the retained-native renderer own that child window's presentation target.

Pros:
- True presentation isolation from the host flip-model swapchain.
- Mature Win32 embedding pattern.
- Fits the current retained-native D2D/DWrite pipeline with much less churn than a full composition-surface integration.
- Keeps the change scoped to the Windows retained-native backend and pane layout bridge.

Cons:
- Requires explicit focus, IME, geometry, visibility, and z-order handling.
- Overlay interactions above the terminal pane must be considered deliberately.

### Option 3: Full unified GPU renderer

Remove the native presenter boundary and render retained-native content directly inside the host GPU renderer.

Pros:
- Matches the architecture used by WezTerm and Alacritty.
- Long-term clean renderer story.

Cons:
- This is a renderer rewrite, not a retained-native fix.
- Too large and risky for the immediate bug.

## Recommendation

Use **Option 2: dedicated child `HWND` retained-native host**.

This is the best fit for `mica-term` right now because it fixes the real presentation bug without rewriting the whole renderer stack. It also preserves retained-native rendering as a native Windows path while replacing the unsupported same-HWND DC overlay assumption with an isolated presentation boundary.

## Chosen Architecture

### 1. Add a Windows child host abstraction

Introduce a Windows-only child host module responsible for:
- creating a child `HWND` under the Slint host window,
- synchronizing its pixel rect with the terminal pane rect,
- showing, hiding, and destroying the child window,
- exposing the child `HWND` as the retained-native presentation target.

This abstraction must be narrow enough that the rendering backend can later swap from `ID2D1HwndRenderTarget` to a child-owned DXGI swapchain without changing the higher-level retained-native surface contract.

### 2. Retarget retained-native rendering to the child host

The retained-native backend must stop calling `GetDC` on the host `HWND`. Instead, it should render into a target owned by the child window.

For the first implementation, the backend should use a child-window-owned Direct2D `ID2D1HwndRenderTarget` plus existing DirectWrite and bitmap fallback logic. That keeps the text pipeline stable while solving the presentation-boundary bug.

Key consequence:
- terminal draw coordinates stay local to the child surface,
- diagnostics can still project glyph positions back into parent-window coordinates for logging and hit analysis.

### 3. Keep retained frame and damage logic

`src/app/terminal_renderer/native_surface.rs` should keep the retained frame, frame token, dirty flag, and damage tracking model. The fix is not to throw away the retained surface scheduler; it is to change what surface is being presented.

The present driver must stop assuming that host redraw replay is the core visibility mechanism. Host redraws can still be used as a backstop for layout invalidation, but the child host should own the actual visible terminal surface lifecycle.

### 4. Synchronize child host geometry from pane layout

When the pane rect changes, the Windows backend must update the child window's physical pixel bounds to match the pane. Existing scale-factor bridging from `src/app/bootstrap.rs` remains important; the child host should receive already scaled physical coordinates.

### 5. Focus, IME, and input ownership

The child host must behave like the terminal surface, not like an unrelated floating window.

Design rules:
- clicking the terminal pane focuses the child host,
- pane hide/show and tab switches hide/show the child window,
- IME and text cursor ownership follow the child host,
- cursor shape should come from the terminal path rather than the parent shell pretending to own text input.

### 6. Diagnostics and rollout

Diagnostics should report both the host `HWND` and child `HWND` relationship, child-host readiness, and native target status. This allows runtime logs to distinguish:
- child host not created,
- child host created but target not ready,
- target ready and drawing, and
- target drawing but clipped or hidden.

## Files Expected To Change

Primary implementation files:
- `src/app/terminal_renderer/platform/windows.rs`
- `src/app/terminal_renderer/native_surface.rs`
- `src/app/bootstrap.rs`
- `src/app/windows_frame.rs`

Likely new file:
- `src/app/terminal_renderer/platform/windows_child_host.rs`

Tests to update:
- `tests/native_terminal_surface_contract_spec.rs`
- `tests/windows_native_text_renderer_contract_spec.rs`
- `tests/windows_terminal_diagnostics_spec.rs`

## Non-Goals

- No terminal metrics retuning.
- No switch back to scene-image or bitmap presenter as the final fix.
- No full migration to a composition swapchain in this first repair.
- No broad refactor of non-Windows presenters.

## Risks And Mitigations

- **Risk:** child window overlays fight with shell UI.  
  **Mitigation:** keep the child host clipped strictly to the pane rect and document overlay limitations in diagnostics and follow-up work.

- **Risk:** focus and IME regressions.  
  **Mitigation:** add explicit diagnostics and narrow tests around focus handoff and cursor visibility.

- **Risk:** future swapchain migration becomes harder if the child host abstraction leaks D2D details.  
  **Mitigation:** keep the host abstraction surface-oriented and renderer-agnostic.

## Success Criteria

- Retained-native Windows panes visibly render text, background, and cursor in packaged and local runs.
- The implementation no longer depends on same-HWND host-window `GetDC/BindDC` presentation.
- Logs can distinguish child-host lifecycle and render-target state.
- Existing retained-native damage and frame-token behavior still works.
- No typography metric regressions are introduced.

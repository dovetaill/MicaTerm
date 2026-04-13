# Windows Terminal Host-Surface Rearchitecture Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Remove the Windows child-`HWND` terminal-body main path and restore a reliable host-owned terminal body while keeping native Windows text rendering. The immediate implementation must not depend on a same-`HWND` DirectComposition body surface, because that is blocked under the current Slint/winit `CreateSwapChainForHwnd(...)` host renderer.

**Architecture:** First rewrite the source contracts so the codebase rejects both child-`HWND` ownership and the current fake host-surface placeholder (`CreateHwndRenderTarget` / `ID2D1HwndRenderTarget`). Then retarget the Windows retained-native backend so it remains a native text / retained-frame producer but stops directly owning visible `HWND` presentation. The immediate bridge must use an offscreen WIC bitmap + host-owned `session-surface-image` handoff, because the current Slint host has no other production-visible sink for `PresentedTerminalFrame::Native`. Next stabilize bootstrap geometry and host overlays, reframe diagnostics around host-owned presentation, retire the child-host main path, and run the full regression suite.

**Tech Stack:** Rust, Win32 API, DirectWrite, Direct2D, host-owned retained frame presentation, Slint/winit, Cargo integration tests, shell smoke checks.

**Important runtime constraint:** Do **not** attempt to revive "Plan A" by layering a new DirectComposition visual over the current same-`HWND` Slint renderer. That premise has been invalidated by runtime investigation and is no longer executable under the current host renderer.

---

### Task 1: Rewrite the contracts to reject child-HWND ownership and fake host-surface presentation

**Files:**
- Modify: `tests/native_terminal_surface_contract_spec.rs`
- Modify: `tests/windows_native_text_renderer_contract_spec.rs`
- Reference: `docs/plans/2026-04-12-windows-terminal-host-surface-rearchitecture-design.md`

**Step 1: Write the failing test**

Replace the old child-host assumptions and reject the placeholder `HWND` render-target seam.

```rust
assert!(!windows_backend_source.contains("WindowsChildSurfaceHost"));
assert!(!windows_backend_source.contains("created retained-native child HWND host"));
assert!(!windows_backend_source.contains("CreateHwndRenderTarget"));
assert!(!windows_backend_source.contains("ID2D1HwndRenderTarget"));
assert!(windows_backend_source.contains("text_renderer_path"));
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test native_terminal_surface_contract_spec --test windows_native_text_renderer_contract_spec -q`
Expected: FAIL because the codebase still contains the placeholder `HWND` render-target seam in the Windows main path.

**Step 3: Keep the contract narrow and architectural**

The rewritten tests should assert only the intended ownership facts:

- no child-`HWND` main-body ownership,
- no fake host-surface presentation through `CreateHwndRenderTarget`,
- native Windows text diagnostics remain part of the contract,
- visible presentation is being pulled back under host ownership.

**Step 4: Re-run after later tasks**

Run: `cargo test --test native_terminal_surface_contract_spec --test windows_native_text_renderer_contract_spec -q`
Expected: PASS once Tasks 2-3 remove direct `HWND` presentation from the main path.

**Step 5: Commit**

```bash
git add tests/native_terminal_surface_contract_spec.rs tests/windows_native_text_renderer_contract_spec.rs
git commit -m "test: encode windows host-owned terminal contract"
```

### Task 2: Retarget the Windows backend away from direct HWND presentation

**Files:**
- Modify: `src/app/terminal_renderer/platform/windows.rs`
- Modify: `src/app/terminal_renderer/native_surface.rs`
- Modify: `src/app/terminal_renderer/present_driver.rs`
- Modify: `src/app/terminal_presenter.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `ui/shell/terminal-session-host.slint`
- Modify: `Cargo.toml`
- Modify or retire from main path: `src/app/terminal_renderer/platform/windows_composition_surface.rs`
- Test: `tests/native_terminal_surface_contract_spec.rs`
- Test: `tests/windows_native_text_renderer_contract_spec.rs`

**Step 1: Write the failing test**

Require the backend state to stop treating an `HWND` render target as the visible owner of terminal pixels.

```rust
assert!(!windows_backend_source.contains("pub surface_hwnd: Option<isize>"));
assert!(!windows_backend_source.contains("CreateHwndRenderTarget"));
assert!(native_surface_source.contains("update_terminal_rect(&self, rect: NativeTerminalSurfaceRect)"));
assert!(!present_driver_source.contains("child HWND"));
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test native_terminal_surface_contract_spec --test windows_native_text_renderer_contract_spec -q`
Expected: FAIL because the Windows backend still creates and drives an `HWND` render target in the main path.

**Step 3: Write minimal implementation**

Refactor the Windows backend so it remains a retained-native / native-text producer but no longer directly presents into an `HWND`-bound render target.

```rust
pub struct WindowsNativeSurfaceState {
    pub host_hwnd: Option<isize>,
    pub rect: NativeTerminalSurfaceRect,
    pub attached: bool,
    pub retained_frame: Option<RetainedNativeTerminalSurfaceFrame>,
    pub last_prepared_frame_token: u64,
    pub last_presented_frame_token: u64,
    // native text / offscreen production state lives here
}
```

Implementation requirements:

- remove direct visible presentation ownership from `CreateHwndRenderTarget` / `ID2D1HwndRenderTarget`,
- replace the visible `HWND` target with an offscreen WIC bitmap bridge that can hand BGRA pixels back to the host-owned `session-surface-image` path,
- keep retained-native frame tokens, damage tracking, and draw scheduling intact,
- keep Windows native text rendering intact,
- if `windows_composition_surface.rs` remains, keep it disconnected from the main production path until a future host-composition effort exists,
- treat host redraw as synchronization/supporting behavior rather than as a second visible owner.

**Step 4: Run test to verify it passes**

Run: `cargo test --test native_terminal_surface_contract_spec --test windows_native_text_renderer_contract_spec -q`
Expected: PASS once the backend no longer describes direct `HWND` presentation ownership.

**Step 5: Commit**

```bash
git add src/app/terminal_renderer/platform/windows.rs src/app/terminal_renderer/native_surface.rs src/app/terminal_renderer/present_driver.rs src/app/terminal_presenter.rs src/app/terminal_renderer/platform/windows_composition_surface.rs tests/native_terminal_surface_contract_spec.rs tests/windows_native_text_renderer_contract_spec.rs
git commit -m "refactor: retarget windows terminal body to host-owned presentation"
```

### Task 3: Preserve native Windows text rendering while moving visible output under host ownership

**Files:**
- Modify: `src/app/terminal_renderer/platform/windows.rs`
- Modify: `src/app/terminal_renderer/native_surface.rs`
- Modify: `src/app/terminal_presenter.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `ui/shell/terminal-session-host.slint`
- Modify: `Cargo.toml`
- Test: `tests/windows_native_text_renderer_contract_spec.rs`
- Test: `tests/terminal_renderer_dwrite_spec.rs`

**Step 1: Write the failing test**

Keep the DirectWrite primary text path but require it to survive the presentation-boundary rewrite.

```rust
assert!(windows_backend_source.contains("fn ensure_directwrite_text_renderer(&mut self)"));
assert!(windows_backend_source.contains("fn draw_directwrite_text("));
assert!(windows_backend_source.contains("DrawGlyphRun("));
assert!(windows_backend_source.contains("text_renderer_path: Some("));
assert!(!windows_backend_source.contains("ID2D1HwndRenderTarget"));
```

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test --test windows_native_text_renderer_contract_spec --test terminal_renderer_dwrite_spec -q
```

Expected: FAIL until the native text path is kept alive without the old direct `HWND` render target.

**Step 3: Write minimal implementation**

Refactor the Windows text path so DirectWrite remains the primary text renderer even though visible presentation is host-owned.

Implementation requirements:

- preserve DirectWrite renderer state and monitor-aware rendering params,
- render native text into the offscreen WIC target through the Windows backend rather than into an `HWND` render target,
- publish the resulting host-owned image back through the existing workspace terminal image contract so native mode remains visibly filled,
- preserve glyph diagnostics and fallback tracing,
- keep the host-visible body synchronized through retained frame output rather than direct `HWND` present,
- do not quietly downgrade the primary path to a child-window or fake host-surface hack.

**Step 4: Run test to verify it passes**

Run:

```bash
cargo test --test windows_native_text_renderer_contract_spec --test terminal_renderer_dwrite_spec -q
```

Expected: PASS.

**Step 5: Commit**

```bash
git add src/app/terminal_renderer/platform/windows.rs src/app/terminal_renderer/native_surface.rs src/app/terminal_presenter.rs tests/windows_native_text_renderer_contract_spec.rs tests/terminal_renderer_dwrite_spec.rs
git commit -m "feat: keep windows native text path under host-owned presentation"
```

### Task 4: Remove context-menu-driven terminal rect collapse and stabilize pane geometry

**Files:**
- Modify: `src/app/bootstrap.rs`
- Modify: `ui/shell/terminal-session-host.slint`
- Modify: `ui/shell/workspace-pane.slint`
- Modify: `ui/app-window.slint`
- Test: `tests/bootstrap_profile_smoke.rs`
- Test: `tests/bootstrap_smoke.rs`

**Step 1: Write the failing test**

Assert that a workspace context menu no longer zeroes the terminal rect and that host geometry remains authoritative while overlays are open.

```rust
assert!(!bootstrap_source.contains("|| window.get_workspace_session_context_menu_open()"));
assert!(bootstrap_source.contains("workspace_blocks_native_terminal_surface(window)"));
assert!(bootstrap_source.contains("sync_workspace_native_terminal_surface_geometry"));
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test bootstrap_profile_smoke --test bootstrap_smoke -q`
Expected: FAIL because `workspace_native_terminal_rect(...)` still collapses when the workspace context menu is open.

**Step 3: Write minimal implementation**

Refactor the bootstrap geometry contract so the terminal body remains present while host-owned overlays are shown.

```rust
fn workspace_native_terminal_rect(window: &AppWindow) -> NativeTerminalSurfaceRect {
    if workspace_blocks_native_terminal_surface(window) {
        return NativeTerminalSurfaceRect::default();
    }
    current_workspace_rect(window)
}
```

Implementation requirements:

- remove context-menu-open from the teardown/collapse condition,
- keep true modal blockers as explicit suspend/hide conditions only where necessary,
- ensure layout export still produces a full pane rect during menu/selection/overlay state,
- keep physical-pixel scaling correct.

**Step 4: Run test to verify it passes**

Run: `cargo test --test bootstrap_profile_smoke --test bootstrap_smoke -q`
Expected: PASS.

**Step 5: Commit**

```bash
git add src/app/bootstrap.rs ui/shell/terminal-session-host.slint ui/shell/workspace-pane.slint ui/app-window.slint tests/bootstrap_profile_smoke.rs tests/bootstrap_smoke.rs
git commit -m "fix: keep windows terminal body stable during host overlays"
```

### Task 5: Reframe diagnostics and Windows helpers around host-owned presentation

**Files:**
- Modify: `src/app/terminal_renderer/diagnostics.rs`
- Modify: `src/app/windows_frame.rs`
- Modify: `src/app/terminal_renderer/platform/windows.rs`
- Test: `tests/windows_terminal_diagnostics_spec.rs`
- Test: `tests/windows_native_terminal_host_window_contract_spec.rs`

**Step 1: Write the failing test**

Require diagnostics to describe host-owned presentation plus native text state, not child-window or fake host-surface churn.

```rust
assert!(diagnostics_source.contains("text_renderer_path"));
assert!(diagnostics_source.contains("host_hwnd"));
assert!(!diagnostics_source.contains("pub surface_hwnd: Option<isize>"));
assert!(!windows_frame_source.contains("native_surface_surface_hwnd"));
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test windows_terminal_diagnostics_spec --test windows_native_terminal_host_window_contract_spec -q`
Expected: FAIL because diagnostics and helpers still reflect the transitional host-surface state model.

**Step 3: Write minimal implementation**

Update diagnostics and helper accessors so they report:

- host `HWND`,
- host-owned visibility/readiness state,
- native text renderer path,
- fallback reason,
- current rect and frame-token state,
- draw counters and glyph diagnostics.

Remove child-window lifecycle logging from the main success path.

**Step 4: Run test to verify it passes**

Run: `cargo test --test windows_terminal_diagnostics_spec --test windows_native_terminal_host_window_contract_spec -q`
Expected: PASS.

**Step 5: Commit**

```bash
git add src/app/terminal_renderer/diagnostics.rs src/app/windows_frame.rs src/app/terminal_renderer/platform/windows.rs tests/windows_terminal_diagnostics_spec.rs tests/windows_native_terminal_host_window_contract_spec.rs
git commit -m "refactor: report windows host-owned terminal diagnostics"
```

### Task 6: Retire the child-HWND module from the main workspace path and run the regression suite

**Files:**
- Modify: `src/app/terminal_renderer/platform/windows.rs`
- Modify: `src/app/terminal_renderer/platform/mod.rs`
- Delete or retire from main path: `src/app/terminal_renderer/platform/windows_child_host.rs`
- Modify: `src/app/terminal_renderer/platform/backend.rs`
- Reference: `docs/plans/2026-04-12-windows-terminal-host-surface-rearchitecture-design.md`
- Reference: `docs/plans/2026-04-12-windows-terminal-host-surface-rearchitecture-implementation-plan.md`

**Step 1: Write the failing test**

Make the main-path contract reject active use of the child-host module.

```rust
assert!(!windows_backend_source.contains("windows_child_host"));
assert!(!windows_backend_source.contains("ensure_child_surface_window"));
assert!(windows_backend_source.contains("bitmap") || windows_backend_source.contains("retained_frame"));
```

**Step 2: Run the focused source-contract suite**

Run:

```bash
cargo test --test native_terminal_surface_contract_spec \
           --test windows_terminal_diagnostics_spec \
           --test windows_native_text_renderer_contract_spec \
           --test windows_native_terminal_host_window_contract_spec \
           --test bootstrap_profile_smoke \
           --test bootstrap_smoke -q
```

Expected: PASS.

**Step 3: Run the broader native terminal contract suite**

Run:

```bash
cargo test --test terminal_renderer_dwrite_spec \
           --test terminal_runtime_perf_contract_spec \
           --test terminal_color_emoji_spec -q
```

Expected: PASS.

**Step 4: Run the shell smoke checks**

Run:

```bash
bash tests/build_win_x64_script_smoke.sh
bash tests/build_desktop_windows_msvc_tool_shims_smoke.sh
```

Expected: PASS.

**Step 5: Run a source grep audit**

Run:

```bash
rg -n "child HWND|child_hwnd|WindowsChildSurfaceHost|surface_hwnd|workspace_session_context_menu_open|CreateHwndRenderTarget|ID2D1HwndRenderTarget" src tests ui
```

Expected:

- no live main-path references to child-window ownership,
- no live main-path references to `CreateHwndRenderTarget` / `ID2D1HwndRenderTarget`,
- any remaining `workspace_session_context_menu_open` references should no longer be tied to native-body rect collapse,
- any remaining host-surface mentions should be clearly transitional or deferred only.

**Step 6: Perform Windows manual verification**

Verify on Windows:

- pane fills the terminal host area at startup and after tab switches,
- right-click shows the menu without hiding the terminal body,
- selected text can still be copied through the expected host interaction,
- the scrollbar and pane-local overlays remain visually above the terminal body,
- minimize/restore does not shrink the terminal body to a stale strip,
- native text renderer logs still report the intended native text path.

**Step 7: Commit**

```bash
git add docs/plans/2026-04-12-windows-terminal-host-surface-rearchitecture-design.md docs/plans/2026-04-12-windows-terminal-host-surface-rearchitecture-implementation-plan.md src/app/terminal_renderer/platform/windows.rs src/app/terminal_renderer/platform/mod.rs src/app/terminal_renderer/platform/backend.rs tests/native_terminal_surface_contract_spec.rs tests/windows_terminal_diagnostics_spec.rs tests/windows_native_text_renderer_contract_spec.rs tests/windows_native_terminal_host_window_contract_spec.rs tests/bootstrap_profile_smoke.rs tests/bootstrap_smoke.rs tests/terminal_renderer_dwrite_spec.rs
git rm src/app/terminal_renderer/platform/windows_child_host.rs
git commit -m "plan: switch windows terminal repair to host-owned presentation"
```

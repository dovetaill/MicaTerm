# Windows Terminal Host-Surface Rearchitecture Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace the current Windows child-`HWND` terminal-body architecture with a same-host-window, composition-backed native surface path that preserves native text rendering while fixing pane fill, right-click, selection, and minimize/restore reliability.

**Architecture:** First rewrite the source contracts so the codebase stops asserting child-window ownership for the main terminal body. Then introduce a host-owned Windows composition-surface seam, retarget retained-native presentation to that seam, remove context-menu-driven rect collapse from bootstrap, and update diagnostics/UI contracts so host input and overlays remain authoritative while the native renderer becomes a surface producer instead of a separate interactive window.

**Tech Stack:** Rust, Win32 API, Direct2D, DirectWrite, DirectComposition-oriented host surface integration, Slint/winit, Cargo integration tests, shell smoke checks.

---

### Task 1: Rewrite the architecture contracts away from child-HWND ownership

**Files:**
- Modify: `tests/native_terminal_surface_contract_spec.rs`
- Modify: `tests/windows_terminal_diagnostics_spec.rs`
- Modify: `tests/windows_native_text_renderer_contract_spec.rs`
- Modify: `tests/windows_native_terminal_host_window_contract_spec.rs`
- Modify: `tests/bootstrap_profile_smoke.rs`
- Modify: `tests/bootstrap_smoke.rs`
- Reference: `docs/plans/2026-04-12-windows-terminal-host-surface-rearchitecture-design.md`

**Step 1: Write the failing test**

Replace the child-host assertions with host-surface assertions and explicitly forbid the current child-window assumptions.

```rust
assert!(!windows_backend_source.contains("WindowsChildSurfaceHost"));
assert!(!windows_backend_source.contains("created retained-native child HWND host"));
assert!(!bootstrap_source.contains("window.get_workspace_session_context_menu_open()"));
assert!(windows_backend_source.contains("windows_composition_surface"));
assert!(diagnostics_source.contains("host_surface"));
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test native_terminal_surface_contract_spec --test windows_terminal_diagnostics_spec --test windows_native_text_renderer_contract_spec --test windows_native_terminal_host_window_contract_spec --test bootstrap_profile_smoke --test bootstrap_smoke -q`
Expected: FAIL because the codebase still encodes child-`HWND` lifecycle, context-menu-driven rect collapse, and `surface_hwnd`-centric diagnostics.

**Step 3: Keep the new contract narrow and architectural**

The rewritten tests should assert only the intended architectural facts:

- no child-`HWND` main-body ownership,
- no context-menu-open rect collapse,
- new host-surface/composition-surface seam is present,
- diagnostics no longer describe child-window lifecycle as the main state model.

**Step 4: Re-run after later tasks**

Run: `cargo test --test native_terminal_surface_contract_spec --test windows_terminal_diagnostics_spec --test windows_native_text_renderer_contract_spec --test windows_native_terminal_host_window_contract_spec --test bootstrap_profile_smoke --test bootstrap_smoke -q`
Expected: PASS once the new host-surface implementation is in place.

**Step 5: Commit**

```bash
git add tests/native_terminal_surface_contract_spec.rs tests/windows_terminal_diagnostics_spec.rs tests/windows_native_text_renderer_contract_spec.rs tests/windows_native_terminal_host_window_contract_spec.rs tests/bootstrap_profile_smoke.rs tests/bootstrap_smoke.rs
git commit -m "test: encode windows host-surface terminal contract"
```

### Task 2: Introduce a Windows host composition-surface seam

**Files:**
- Create: `src/app/terminal_renderer/platform/windows_composition_surface.rs`
- Modify: `src/app/terminal_renderer/platform/windows.rs`
- Modify: `src/app/terminal_renderer/platform/backend.rs`
- Test: `tests/native_terminal_surface_contract_spec.rs`
- Test: `tests/windows_native_text_renderer_contract_spec.rs`

**Step 1: Write the failing test**

Add source-contract checks for a dedicated host-surface abstraction.

```rust
assert!(windows_backend_source.contains("WindowsCompositionSurfaceHost"));
assert!(windows_backend_source.contains("ensure_host_surface"));
assert!(windows_backend_source.contains("sync_host_surface_rect"));
assert!(windows_backend_source.contains("destroy_host_surface"));
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test native_terminal_surface_contract_spec --test windows_native_text_renderer_contract_spec -q`
Expected: FAIL because no composition-surface host abstraction exists yet.

**Step 3: Write minimal implementation**

Create a focused Windows-only surface host that owns composition-surface lifecycle rather than a child window.

```rust
pub struct WindowsCompositionSurfaceHost {
    pub host_hwnd: isize,
    pub attached: bool,
    pub rect: NativeTerminalSurfaceRect,
    // composition surface / swapchain handles live here
}
```

Implementation requirements:

- create a host-surface module separate from `windows.rs`,
- model creation, resize, visibility, and destroy around a host-owned composition surface,
- avoid any `CreateWindowExW(... WS_CHILD ...)` ownership for the main terminal body,
- keep the abstraction narrow so the renderer can target it without leaking host-shell logic into the drawing code.

**Step 4: Run test to verify it passes**

Run: `cargo test --test native_terminal_surface_contract_spec --test windows_native_text_renderer_contract_spec -q`
Expected: PASS for the new seam assertions, with later tests still failing until the renderer is retargeted.

**Step 5: Commit**

```bash
git add src/app/terminal_renderer/platform/windows_composition_surface.rs src/app/terminal_renderer/platform/windows.rs src/app/terminal_renderer/platform/backend.rs tests/native_terminal_surface_contract_spec.rs tests/windows_native_text_renderer_contract_spec.rs
git commit -m "feat: add windows host composition surface seam"
```

### Task 3: Retarget the Windows native renderer from surface_hwnd to host-surface presentation

**Files:**
- Modify: `src/app/terminal_renderer/platform/windows.rs`
- Modify: `src/app/terminal_renderer/native_surface.rs`
- Modify: `src/app/terminal_renderer/present_driver.rs`
- Modify: `src/app/terminal_presenter.rs`
- Test: `tests/native_terminal_surface_contract_spec.rs`
- Test: `tests/windows_native_text_renderer_contract_spec.rs`

**Step 1: Write the failing test**

Require the backend state to revolve around host-surface ownership instead of `surface_hwnd`.

```rust
assert!(!windows_backend_source.contains("pub surface_hwnd: Option<isize>"));
assert!(windows_backend_source.contains("host_surface"));
assert!(present_driver_source.contains("update_terminal_rect(rect)"));
assert!(!present_driver_source.contains("child HWND"));
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test native_terminal_surface_contract_spec --test windows_native_text_renderer_contract_spec -q`
Expected: FAIL because the renderer still manages a child `HWND` and child-window-specific logging/state.

**Step 3: Write minimal implementation**

Refactor the Windows backend so the renderer draws into the new host-surface path while preserving retained-native frame production.

```rust
pub struct WindowsNativeBackendState {
    pub host_hwnd: Option<isize>,
    pub host_surface: Option<WindowsCompositionSurfaceHost>,
    pub rect: NativeTerminalSurfaceRect,
    pub attached: bool,
}
```

Implementation requirements:

- move lifecycle management away from `ensure_child_surface_window()` / `destroy_child_surface_window()`,
- keep retained-native frame tokens, damage tracking, and draw scheduling intact,
- keep text rendering native,
- treat host redraw as synchronization/supporting behavior rather than the place where terminal pixels are logically owned.

**Step 4: Run test to verify it passes**

Run: `cargo test --test native_terminal_surface_contract_spec --test windows_native_text_renderer_contract_spec -q`
Expected: PASS once the backend state and present path describe the host-surface model.

**Step 5: Commit**

```bash
git add src/app/terminal_renderer/platform/windows.rs src/app/terminal_renderer/native_surface.rs src/app/terminal_renderer/present_driver.rs src/app/terminal_presenter.rs tests/native_terminal_surface_contract_spec.rs tests/windows_native_text_renderer_contract_spec.rs
git commit -m "refactor: retarget windows native renderer to host surface"
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

Assert that a workspace context menu no longer zeroes the native terminal rect and that host geometry remains authoritative while overlays are open.

```rust
assert!(!bootstrap_source.contains("|| window.get_workspace_session_context_menu_open()"));
assert!(bootstrap_source.contains("workspace_blocks_native_terminal_surface(window)"));
assert!(bootstrap_source.contains("sync_workspace_native_terminal_surface_geometry"));
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test bootstrap_profile_smoke --test bootstrap_smoke -q`
Expected: FAIL because `workspace_native_terminal_rect(...)` still collapses to zero when the workspace context menu is open.

**Step 3: Write minimal implementation**

Refactor the bootstrap geometry contract so the terminal body remains present while overlays are shown.

```rust
fn workspace_native_terminal_rect(window: &AppWindow) -> NativeTerminalSurfaceRect {
    if workspace_blocks_native_terminal_surface(window) {
        return NativeTerminalSurfaceRect::default();
    }
    // context menu open must not collapse the native body rect
    current_workspace_rect(window)
}
```

Implementation requirements:

- remove context-menu-open from the teardown/collapse condition,
- keep true modal blockers as explicit suspend/hide conditions only where necessary,
- make sure layout export still produces a full pane rect during menu/selection/overlay state,
- keep physical-pixel scaling correct.

**Step 4: Run test to verify it passes**

Run: `cargo test --test bootstrap_profile_smoke --test bootstrap_smoke -q`
Expected: PASS with stable geometry semantics.

**Step 5: Commit**

```bash
git add src/app/bootstrap.rs ui/shell/terminal-session-host.slint ui/shell/workspace-pane.slint ui/app-window.slint tests/bootstrap_profile_smoke.rs tests/bootstrap_smoke.rs
git commit -m "fix: keep windows terminal body stable during context menus"
```

### Task 5: Reframe diagnostics and windows_frame helpers around host-surface state

**Files:**
- Modify: `src/app/terminal_renderer/diagnostics.rs`
- Modify: `src/app/windows_frame.rs`
- Modify: `src/app/terminal_renderer/platform/windows.rs`
- Test: `tests/windows_terminal_diagnostics_spec.rs`
- Test: `tests/windows_native_terminal_host_window_contract_spec.rs`

**Step 1: Write the failing test**

Require diagnostics to expose host-surface state instead of child-window churn.

```rust
assert!(diagnostics_source.contains("host_surface"));
assert!(!diagnostics_source.contains("surface_hwnd"));
assert!(!windows_frame_source.contains("native_surface_surface_hwnd"));
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test windows_terminal_diagnostics_spec --test windows_native_terminal_host_window_contract_spec -q`
Expected: FAIL because diagnostics and helpers still expose `surface_hwnd` as a primary concept.

**Step 3: Write minimal implementation**

Update diagnostics and frame helpers so they report:

- host `HWND`,
- host-surface readiness,
- current rect,
- attachment/visibility state,
- native text renderer path,
- fallback reason.

Remove child-window lifecycle logging from the primary success path and replace it with host-surface lifecycle logs.

**Step 4: Run test to verify it passes**

Run: `cargo test --test windows_terminal_diagnostics_spec --test windows_native_terminal_host_window_contract_spec -q`
Expected: PASS.

**Step 5: Commit**

```bash
git add src/app/terminal_renderer/diagnostics.rs src/app/windows_frame.rs src/app/terminal_renderer/platform/windows.rs tests/windows_terminal_diagnostics_spec.rs tests/windows_native_terminal_host_window_contract_spec.rs
git commit -m "refactor: report windows host-surface diagnostics"
```

### Task 6: Retire the child-HWND module from the main workspace path and wire a controlled fallback story

**Files:**
- Modify: `src/app/terminal_renderer/platform/windows.rs`
- Modify: `src/app/terminal_renderer/platform/mod.rs`
- Delete or retire from main path: `src/app/terminal_renderer/platform/windows_child_host.rs`
- Modify: `src/app/terminal_renderer/platform/backend.rs`
- Test: `tests/native_terminal_surface_contract_spec.rs`
- Test: `tests/windows_native_text_renderer_contract_spec.rs`

**Step 1: Write the failing test**

Make the main-path contract reject active use of the child-host module.

```rust
assert!(!windows_backend_source.contains("windows_child_host"));
assert!(!windows_backend_source.contains("ensure_child_surface_window"));
assert!(windows_backend_source.contains("bitmap"));
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test native_terminal_surface_contract_spec --test windows_native_text_renderer_contract_spec -q`
Expected: FAIL because the backend still imports and uses `windows_child_host`.

**Step 3: Write minimal implementation**

Remove the child-host module from the main workspace terminal route.

Implementation requirements:

- stop importing or using `windows_child_host` in the main Windows backend,
- either delete the file or leave it disconnected and marked as non-production historical code during the migration,
- keep controlled bitmap fallback as the only runtime fallback path if host-surface initialization fails,
- do not reintroduce child-`HWND` resurrection as a hidden compatibility branch.

**Step 4: Run test to verify it passes**

Run: `cargo test --test native_terminal_surface_contract_spec --test windows_native_text_renderer_contract_spec -q`
Expected: PASS.

**Step 5: Commit**

```bash
git add src/app/terminal_renderer/platform/windows.rs src/app/terminal_renderer/platform/mod.rs src/app/terminal_renderer/platform/backend.rs tests/native_terminal_surface_contract_spec.rs tests/windows_native_text_renderer_contract_spec.rs
git rm src/app/terminal_renderer/platform/windows_child_host.rs
git commit -m "refactor: retire child hwnd windows terminal path"
```

### Task 7: Run the Windows reliability regression suite and capture migration gaps

**Files:**
- Reference: `docs/plans/2026-04-12-windows-terminal-host-surface-rearchitecture-design.md`
- Reference: `docs/plans/2026-04-12-windows-terminal-host-surface-rearchitecture-implementation-plan.md`

**Step 1: Run the focused source-contract suite**

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

**Step 2: Run the broader native terminal contract suite**

Run:

```bash
cargo test --test terminal_renderer_dwrite_spec \
           --test terminal_runtime_perf_contract_spec \
           --test terminal_color_emoji_spec -q
```

Expected: PASS with no regressions that force a child-window rollback.

**Step 3: Run a source grep audit**

Run:

```bash
rg -n "child HWND|child_hwnd|WindowsChildSurfaceHost|surface_hwnd|workspace_session_context_menu_open" src tests ui
```

Expected:

- no live main-path references to child-window ownership,
- any remaining `workspace_session_context_menu_open` references should no longer be tied to native-surface rect collapse,
- any remaining `surface_hwnd` mentions should be historical or intentionally transitional only.

**Step 4: Perform Windows manual verification**

Verify on Windows:

- pane fills the terminal host area at startup and after tab switches,
- right-click shows the menu without hiding the terminal body,
- selected text can still be copied through the expected host interaction,
- minimize/restore does not shrink the terminal body to a stale strip,
- native text renderer logs still report the intended native text path.

**Step 5: Commit**

```bash
git add docs/plans/2026-04-12-windows-terminal-host-surface-rearchitecture-design.md docs/plans/2026-04-12-windows-terminal-host-surface-rearchitecture-implementation-plan.md
git commit -m "docs: record windows host-surface rearchitecture plan"
```

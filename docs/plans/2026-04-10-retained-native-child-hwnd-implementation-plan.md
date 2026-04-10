# Retained-Native Child HWND Host Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace the blank same-HWND retained-native Windows terminal path with a child-HWND hosted retained-native surface that owns its own native Direct2D/DirectWrite presentation boundary.

**Architecture:** The Windows backend creates a dedicated child `HWND` for the terminal pane, keeps it synchronized with the pane rect, and renders retained-native content into that child surface instead of binding a DC from the Skia host window. The first implementation uses a child-owned Direct2D `ID2D1HwndRenderTarget` while keeping the host abstraction ready for a later swapchain/composition upgrade.

**Tech Stack:** Rust, Win32 API, Direct2D, DirectWrite, Slint/winit, cargo tests

---

### Task 1: Reassert the backend contract around a child host window

**Files:**
- Modify: `tests/native_terminal_surface_contract_spec.rs`
- Modify: `tests/windows_terminal_diagnostics_spec.rs`
- Modify: `tests/windows_native_text_renderer_contract_spec.rs`
- Reference: `src/app/terminal_renderer/platform/windows.rs`

**Step 1: Write the failing test**

Update the contract assertions so they require a dedicated surface window and forbid host-window DC binding. Add assertions like:

```rust
assert!(windows_backend_source.contains("surface_hwnd: Option<isize>"));
assert!(windows_backend_source.contains("CreateWindowExW("));
assert!(windows_backend_source.contains("SetWindowPos("));
assert!(windows_backend_source.contains("DestroyWindow("));
assert!(!windows_backend_source.contains("GetDC(HWND(host_hwnd as _))"));
assert!(!windows_backend_source.contains("render_target.BindDC("));
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test native_terminal_surface_contract_spec windows_backend_source_exposes_same_hwnd_present_contract -- --nocapture`
Expected: FAIL because the backend still encodes same-HWND presentation.

**Step 3: Write minimal implementation**

Rename the relevant contract tests and update their expectation messages so they describe the child-HWND architecture instead of the host-HWND DC architecture. Keep the assertions narrowly focused on:
- child host presence,
- child window lifecycle helpers,
- removal of host-window `BindDC` usage,
- diagnostics projecting child-local glyphs back into host coordinates.

**Step 4: Run test to verify it passes**

Run: `cargo test --test native_terminal_surface_contract_spec -- --nocapture`
Expected: PASS for the updated contract file, with other retained-native implementation tests still failing until later tasks land.

**Step 5: Commit**

```bash
git add tests/native_terminal_surface_contract_spec.rs tests/windows_terminal_diagnostics_spec.rs tests/windows_native_text_renderer_contract_spec.rs
git commit -m "test: encode child-host retained-native contract"
```

### Task 2: Introduce the Windows child host lifecycle seam

**Files:**
- Create: `src/app/terminal_renderer/platform/windows_child_host.rs`
- Modify: `src/app/terminal_renderer/platform/windows.rs`
- Test: `tests/native_terminal_surface_contract_spec.rs`

**Step 1: Write the failing test**

Add source-contract assertions for the new helper module and lifecycle methods, for example:

```rust
assert!(windows_backend_source.contains("fn ensure_child_surface_window(&mut self)"));
assert!(windows_backend_source.contains("fn sync_child_surface_window_rect(&mut self)"));
assert!(windows_backend_source.contains("fn destroy_child_surface_window(&mut self)"));
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test native_terminal_surface_contract_spec windows_backend_source_exposes_host_window_present_contract -- --nocapture`
Expected: FAIL because those helpers do not exist yet.

**Step 3: Write minimal implementation**

Create a Windows-only child host module that owns:

```rust
pub struct WindowsChildSurfaceHost {
    parent_hwnd: isize,
    surface_hwnd: isize,
}
```

Implement focused helpers for:
- creating the child window with `CreateWindowExW(..., WS_CHILD | WS_VISIBLE, ...)`,
- moving it with `SetWindowPos(...)`,
- destroying it with `DestroyWindow(...)`,
- showing and hiding it during attach/detach.

In `windows.rs`, store `surface_hwnd: Option<isize>` and call the helper from attach, rect-sync, and detach.

**Step 4: Run test to verify it passes**

Run: `cargo test --test native_terminal_surface_contract_spec -- --nocapture`
Expected: PASS for the child-host lifecycle contract assertions.

**Step 5: Commit**

```bash
git add src/app/terminal_renderer/platform/windows_child_host.rs src/app/terminal_renderer/platform/windows.rs tests/native_terminal_surface_contract_spec.rs
git commit -m "feat: add retained-native child host window seam"
```

### Task 3: Replace host DC binding with a child-owned hwnd render target

**Files:**
- Modify: `src/app/terminal_renderer/platform/windows.rs`
- Modify: `tests/native_terminal_surface_contract_spec.rs`
- Modify: `tests/windows_native_text_renderer_contract_spec.rs`

**Step 1: Write the failing test**

Add or update assertions that require a child-owned Direct2D hwnd render target path and reject host-DC binding:

```rust
assert!(windows_backend_source.contains("CreateHwndRenderTarget") || windows_backend_source.contains("ID2D1HwndRenderTarget"));
assert!(!windows_backend_source.contains("GetDC(HWND(host_hwnd as _))"));
assert!(!windows_backend_source.contains("BindDC(hdc, &bind_rect)"));
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test windows_native_text_renderer_contract_spec -- --nocapture`
Expected: FAIL because the backend still creates a DC render target and binds it to the host window.

**Step 3: Write minimal implementation**

In `windows.rs`:
- replace `ID2D1DCRenderTarget` ownership with a child-window-owned hwnd render target,
- create or resize that target against the child `HWND`,
- make `begin_frame()` and `end_frame()` operate on the child-owned target,
- keep draw coordinates local to the child surface,
- preserve existing DirectWrite and bitmap fallback logic.

The state should look roughly like:

```rust
pub struct WindowsHwndRenderTargetState {
    pub hwnd: isize,
    pub generation: u64,
    pub render_target: ID2D1HwndRenderTarget,
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test --test windows_native_text_renderer_contract_spec --test native_terminal_surface_contract_spec -- --nocapture`
Expected: PASS for the target-ownership and no-`BindDC` assertions.

**Step 5: Commit**

```bash
git add src/app/terminal_renderer/platform/windows.rs tests/native_terminal_surface_contract_spec.rs tests/windows_native_text_renderer_contract_spec.rs
git commit -m "feat: render retained-native through child hwnd target"
```

### Task 4: Rewire retained-native scheduling around the child surface

**Files:**
- Modify: `src/app/terminal_renderer/native_surface.rs`
- Modify: `src/app/terminal_renderer/present_driver.rs`
- Modify: `tests/native_terminal_surface_contract_spec.rs`

**Step 1: Write the failing test**

Add assertions that retained-native presentation updates the child surface directly instead of relying on host redraw replay as the primary visibility mechanism.

Example contract snippet:

```rust
assert!(native_surface_source.contains("state.backend.present(damage);"));
assert!(native_surface_source.contains("state.backend.update_surface_rect(rect);"));
assert!(!native_surface_source.contains("host redraw can overpaint same-HWND native pixels"));
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test native_terminal_surface_contract_spec rendering_notifier_path_ -- --nocapture`
Expected: FAIL because the present-driver comments and assumptions still describe same-HWND overpaint behavior.

**Step 3: Write minimal implementation**

Update the retained-native scheduler so that:
- host redraw requests remain a synchronization hint only,
- the child surface owns visible present output,
- replay helpers stop encoding the assumption that the host redraw is repainting native pixels into the same `HWND`.

Keep the coalescing gates, dirty flags, and frame token behavior intact.

**Step 4: Run test to verify it passes**

Run: `cargo test --lib after_draw_replay_ -- --nocapture`
Expected: PASS, with logic still coalescing redraws while no longer describing same-HWND ownership.

**Step 5: Commit**

```bash
git add src/app/terminal_renderer/native_surface.rs src/app/terminal_renderer/present_driver.rs tests/native_terminal_surface_contract_spec.rs
git commit -m "refactor: decouple retained-native scheduling from host hwnd replay"
```

### Task 5: Bridge child-host geometry, visibility, and diagnostics

**Files:**
- Modify: `src/app/bootstrap.rs`
- Modify: `src/app/windows_frame.rs`
- Modify: `src/app/terminal_renderer/diagnostics.rs`
- Modify: `src/app/terminal_renderer/platform/windows.rs`
- Modify: `tests/windows_terminal_diagnostics_spec.rs`
- Modify: `tests/bootstrap_profile_smoke.rs`

**Step 1: Write the failing test**

Add diagnostics and bootstrap assertions that require child-host-aware geometry and visibility reporting.

Example snippets:

```rust
assert!(bootstrap_source.contains("window_scale_factor(window)"));
assert!(windows_backend_source.contains("surface_hwnd"));
assert!(diagnostics_source.contains("host_hwnd") && diagnostics_source.contains("surface_hwnd"));
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test windows_terminal_diagnostics_spec --test bootstrap_profile_smoke -- --nocapture`
Expected: FAIL because diagnostics do not yet expose the child host relationship.

**Step 3: Write minimal implementation**

- keep pane geometry in physical pixels,
- push rect changes into the child host on every layout update,
- expose both parent and child hwnd values in diagnostics,
- report child target readiness and visibility state,
- keep glyph-bound diagnostics projected into host-window coordinates for debugging.

**Step 4: Run test to verify it passes**

Run: `cargo test --test windows_terminal_diagnostics_spec --test bootstrap_profile_smoke -- --nocapture`
Expected: PASS.

**Step 5: Commit**

```bash
git add src/app/bootstrap.rs src/app/windows_frame.rs src/app/terminal_renderer/diagnostics.rs src/app/terminal_renderer/platform/windows.rs tests/windows_terminal_diagnostics_spec.rs tests/bootstrap_profile_smoke.rs
git commit -m "feat: report child-host retained-native geometry and diagnostics"
```

### Task 6: Run the retained-native regression suite and document residual gaps

**Files:**
- Modify: `readme.md`
- Reference: `docs/plans/2026-04-10-retained-native-child-hwnd-design.md`
- Reference: `docs/plans/2026-04-10-retained-native-child-hwnd-implementation-plan.md`

**Step 1: Write the failing test**

If README coverage is missing, add a source-contract or documentation smoke assertion that the retained-native bring-up notes mention the child-host path instead of the old same-HWND path.

```rust
assert!(readme_source.contains("child HWND") || readme_source.contains("child host"));
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test windows_native_text_renderer_contract_spec -- --nocapture`
Expected: FAIL if the docs still describe the old model.

**Step 3: Write minimal implementation**

Update `readme.md` so the retained-native Windows bring-up notes explain:
- retained-native uses a dedicated child host window,
- scene-image remains the packaged default until retained-native is explicitly selected,
- same-HWND DC overlay is no longer part of the design.

**Step 4: Run test to verify it passes**

Run:
- `cargo test --test native_terminal_surface_contract_spec --test windows_terminal_diagnostics_spec --test windows_native_text_renderer_contract_spec --test runtime_profile -- --nocapture`
- `cargo test --lib after_draw_replay_ -- --nocapture`

Expected: PASS.

**Step 5: Commit**

```bash
git add readme.md docs/plans/2026-04-10-retained-native-child-hwnd-design.md docs/plans/2026-04-10-retained-native-child-hwnd-implementation-plan.md
git commit -m "docs: record retained-native child host architecture"
```

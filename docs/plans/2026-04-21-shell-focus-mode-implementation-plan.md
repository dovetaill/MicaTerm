# Shell Focus Mode Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make edge-handle click collapse more discoverable and add a shell focus mode that hides the left and right sidebars together, then restores their previous requested visibility on exit.

**Architecture:** Reuse the existing requested-versus-effective shell visibility model and remembered panel widths. Add runtime focus-mode state to `ShellViewModel`, expose a titlebar button plus a local workspace shortcut, and route both through one Rust toggle helper so manual sidebar re-open requests automatically exit focus mode.

**Tech Stack:** Rust, Slint, `ShellViewModel`, titlebar shell chrome, generated-window smoke tests, shell UI contract shell scripts.

---

### Task 1: Lock focus-mode and edge-hint behavior with failing tests

**Files:**
- Modify: `tests/sidebar_navigation_spec.rs`
- Modify: `tests/sidebar_navigation_smoke.rs`
- Modify: `tests/bootstrap_smoke.rs`
- Modify: `tests/shell_layout_ui_contract_smoke.sh`
- Reference: `src/shell/view_model/assets.rs`
- Reference: `src/shell/view_model/projection.rs`
- Reference: `ui/shell/titlebar.slint`
- Reference: `ui/shell/sidebar.slint`
- Reference: `ui/shell/right-panel.slint`
- Reference: `ui/app-window.slint`
- Reference: `ui/shell/terminal-session-host.slint`

**Step 1: Write failing shell-state tests**
- Add a test that enables focus mode from a state where both side regions are requested open and asserts both become requested hidden while remembered widths stay unchanged.
- Add a test that exits focus mode and restores the pre-focus requested visibility for each side independently.
- Add a test that manually reopening either side while focus mode is active clears focus mode immediately.

**Step 2: Write failing interaction smoke tests**
- Extend `tests/sidebar_navigation_smoke.rs` with a generated-window test that clicks the focus-mode titlebar action, verifies both side regions hide, clicks again, and verifies the prior state is restored.
- Extend `tests/bootstrap_smoke.rs` with a local-keyboard smoke test for `Ctrl+Shift+M` and assert it stays local instead of forwarding a remote key chord.

**Step 3: Write failing UI-contract checks**
- Update `tests/shell_layout_ui_contract_smoke.sh` to assert:
  - `ui/app-window.slint` exports `workspace-focus-mode` and `toggle-workspace-focus-mode-requested()`,
  - `ui/shell/titlebar.slint` exposes a focus-mode button and callback,
  - edge-handle tooltip copy includes the click-to-collapse guidance for expanded and collapsed states.

**Step 4: Run tests to verify they fail**
Run: `cargo test --test sidebar_navigation_spec --test sidebar_navigation_smoke --test bootstrap_smoke -- --nocapture`
Expected: FAIL because focus-mode state, keyboard routing, and titlebar wiring do not exist yet.

**Step 5: Run source smoke checks to verify they fail**
Run: `bash tests/shell_layout_ui_contract_smoke.sh`
Expected: FAIL because the focus-mode contract and updated tooltip copy do not exist yet.

**Step 6: Commit the failing-test scaffold**
```bash
git add tests/sidebar_navigation_spec.rs tests/sidebar_navigation_smoke.rs tests/bootstrap_smoke.rs tests/shell_layout_ui_contract_smoke.sh
git commit -m "test: lock shell focus mode behavior"
```

### Task 2: Add runtime focus-mode state and restore helpers to the shell domain

**Files:**
- Modify: `src/shell/view_model.rs`
- Modify: `src/shell/view_model/assets.rs`
- Modify: `src/shell/view_model/projection.rs`
- Reference: `tests/sidebar_navigation_spec.rs`

**Step 1: Extend `ShellViewModel` with focus-mode state**
- Add fields for:
  - `workspace_focus_mode`,
  - saved pre-focus requested state for the assets sidebar,
  - saved pre-focus requested state for the right panel.
- Initialize all new fields in `Default`.

**Step 2: Add focus-mode helpers**
- Add a helper that enters focus mode by snapshotting the current requested visibility state, then hiding both side regions.
- Add a helper that exits focus mode by restoring the saved requested visibility state.
- Add a toggle helper that switches between those two paths.

**Step 3: Make manual reopen requests clear focus mode**
- Update the existing left and right visibility toggle / reopen helpers so that any explicit request to show a side region while focus mode is active exits focus mode first.
- Keep remembered widths untouched by these helpers.

**Step 4: Run focused shell-state tests**
Run: `cargo test --test sidebar_navigation_spec -- --nocapture`
Expected: PASS for the pure shell-state coverage while UI and keyboard-path tests still fail.

**Step 5: Commit the domain work**
```bash
git add src/shell/view_model.rs src/shell/view_model/assets.rs src/shell/view_model/projection.rs tests/sidebar_navigation_spec.rs
git commit -m "feat: add shell focus mode state"
```

### Task 3: Add titlebar focus-mode UI and clearer edge-handle hints in Slint

**Files:**
- Modify: `ui/app-window.slint`
- Modify: `ui/shell/titlebar.slint`
- Modify: `ui/shell/sidebar.slint`
- Modify: `ui/shell/right-panel.slint`
- Modify: `tests/shell_layout_ui_contract_smoke.sh`
- Reference: `docs/plans/2026-04-21-shell-focus-mode-design.md`

**Step 1: Add the app-window focus-mode contract**
- Add `workspace-focus-mode` as an app-window property.
- Add `toggle-workspace-focus-mode-requested()` as an app-window callback.
- Thread the focus-mode property and callback into the titlebar component.

**Step 2: Add a titlebar focus-mode button**
- Add a dedicated button in the titlebar utility cluster between the right-panel toggle and transfer-center button.
- Give it active styling tied to focus mode.
- Use tooltip copy:
  - inactive: `Enter focus mode`
  - active: `Exit focus mode`

**Step 3: Update edge-handle tooltip guidance**
- Adjust the expanded edge-handle tooltip copy to communicate click-to-collapse plus drag-to-resize.
- Adjust the collapsed revive-strip tooltip copy to communicate click-to-expand.
- Keep the handle visually quiet while slightly improving hover clarity.

**Step 4: Run the source smoke checks**
Run: `bash tests/shell_layout_ui_contract_smoke.sh`
Expected: PASS

**Step 5: Commit the Slint contract**
```bash
git add ui/app-window.slint ui/shell/titlebar.slint ui/shell/sidebar.slint ui/shell/right-panel.slint tests/shell_layout_ui_contract_smoke.sh
git commit -m "feat: add shell focus mode UI contract"
```

### Task 4: Wire focus mode through bootstrap and local workspace shortcuts

**Files:**
- Modify: `src/app/bootstrap.rs`
- Modify: `src/app/bootstrap/sftp.rs`
- Modify: `src/app/bootstrap/shell_chrome.rs`
- Modify: `src/shell/view_model/sftp.rs`
- Modify: `ui/shell/terminal-session-host.slint`
- Modify: `tests/sidebar_navigation_smoke.rs`
- Modify: `tests/bootstrap_smoke.rs`
- Reference: `src/shell/view_model/assets.rs`
- Reference: `src/shell/view_model/projection.rs`

**Step 1: Sync focus-mode state into the generated window**
- Publish the new focus-mode property from Rust into `AppWindow` during shell sync.
- Keep width syncing and effective visibility syncing unchanged aside from using the new focus-mode state.

**Step 2: Handle titlebar focus-mode requests**
- Bind `toggle-workspace-focus-mode-requested()` in bootstrap.
- Route it to the shared `ShellViewModel` focus-mode toggle helper.
- Re-run shell layout sync after every focus-mode transition.

**Step 3: Add the keyboard shortcut path**
- Extend `ui/shell/terminal-session-host.slint` so `Ctrl+Shift+M` maps to a new local action id.
- Handle that local action in bootstrap by routing it to the same focus-mode toggle helper.

**Step 4: Verify manual reopen exits focus mode**
- Ensure the existing sidebar and right-panel toggle handlers reuse the updated shell-domain helpers so any explicit reopen while focus mode is active clears focus mode.
- Ensure the explicit SFTP panel open path also exits focus mode cleanly before restoring the requested shell layout.

**Step 5: Run focused interaction tests**
Run: `cargo test --test sidebar_navigation_smoke --test bootstrap_smoke -- --nocapture`
Expected: PASS

**Step 6: Commit the wiring**
```bash
git add src/app/bootstrap.rs src/app/bootstrap/shell_chrome.rs ui/shell/terminal-session-host.slint tests/sidebar_navigation_smoke.rs tests/bootstrap_smoke.rs
git commit -m "feat: wire shell focus mode interactions"
```

### Task 5: Run full verification and refresh the docs

**Files:**
- Modify: `docs/plans/2026-04-21-shell-focus-mode-design.md`
- Modify: `docs/plans/2026-04-21-shell-focus-mode-implementation-plan.md`
- Reference: all files touched in Tasks 1-4

**Step 1: Run focused regression tests**
Run: `cargo test --test sidebar_navigation_spec --test sidebar_navigation_smoke --test bootstrap_smoke -- --nocapture`
Run: `bash tests/shell_layout_ui_contract_smoke.sh`
Expected: PASS

**Step 2: Run adjacent shell regressions**
Run: `cargo test --test shell_layout_policy -- --nocapture`
Run: `cargo test --test top_status_bar_smoke -- --nocapture`
Run: `cargo test --test titlebar_layout_spec -- --nocapture`
Run: `cargo test --test window_geometry_spec -- --nocapture`
Expected: PASS

**Step 3: Refresh docs if implementation details shifted**
- Update the design doc if the button placement, tooltip copy, or shortcut naming changed during implementation.
- Update this plan if execution uncovered missing verification or commit steps, such as extra shell wiring through the SFTP open path or titlebar metric budget coverage.

**Step 4: Commit the completed feature**
```bash
git add docs/plans/2026-04-21-shell-focus-mode-design.md docs/plans/2026-04-21-shell-focus-mode-implementation-plan.md ui/app-window.slint ui/shell/titlebar.slint ui/shell/sidebar.slint ui/shell/right-panel.slint ui/shell/terminal-session-host.slint src/shell/view_model.rs src/shell/view_model/assets.rs src/shell/view_model/projection.rs src/app/bootstrap.rs src/app/bootstrap/shell_chrome.rs tests/sidebar_navigation_spec.rs tests/sidebar_navigation_smoke.rs tests/bootstrap_smoke.rs tests/shell_layout_ui_contract_smoke.sh
git commit -m "feat: add shell focus mode"
```

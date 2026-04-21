# Sidebar Edge Handles Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add workspace-edge handles that let users click to collapse or reopen the left assets sidebar and right utility panel, drag to resize each side, and auto-collapse them when dragged below a small width threshold.

**Architecture:** Keep requested versus effective visibility owned by Rust, extend the shell view model with remembered widths for each side, make the layout resolver consume actual requested widths instead of fixed panel constants, and add Slint edge-handle callbacks in `ui/app-window.slint` so the workspace boundary becomes the primary interaction surface. Existing far-left and titlebar toggle buttons remain as secondary entry points that reuse the same state transitions.

**Tech Stack:** Rust, Slint, existing `ShellViewModel`, shell layout policy, generated-window smoke tests, shell UI contract shell scripts.

---

### Task 1: Lock the edge-handle behavior with failing tests

**Files:**
- Modify: `tests/sidebar_navigation_spec.rs`
- Modify: `tests/sidebar_navigation_smoke.rs`
- Modify: `tests/shell_layout_policy.rs`
- Modify: `tests/shell_layout_ui_contract_smoke.sh`
- Modify: `tests/assets_sidebar_toolbar_ui_contract_smoke.sh`
- Reference: `src/shell/view_model/assets.rs`
- Reference: `src/shell/layout.rs`
- Reference: `ui/app-window.slint`
- Reference: `ui/shell/sidebar.slint`
- Reference: `ui/shell/right-panel.slint`

**Step 1: Write failing view-model expectations**
- Add a test that stores a resized left sidebar width, collapses the sidebar, reopens it, and asserts the remembered width is preserved.
- Add a matching test for the right panel width restore path.
- Add a test that resizing either side below its collapse threshold flips the requested visibility off while preserving the last expanded width.

**Step 2: Write failing responsive-layout expectations**
- Extend `tests/shell_layout_policy.rs` so the resolver accepts non-default panel widths and still preserves the terminal-first hiding order.
- Add a case that proves the left sidebar is effectively hidden first, then the right panel, when large remembered widths exceed the window budget.

**Step 3: Write failing UI-contract checks**
- Update the shell smoke scripts to assert fixed width literals are gone from `ui/shell/sidebar.slint` and `ui/shell/right-panel.slint`.
- Add assertions that `ui/app-window.slint` exports edge-handle callbacks and width properties for both side regions.

**Step 4: Run tests to verify they fail**
Run: `cargo test --test sidebar_navigation_spec --test sidebar_navigation_smoke --test shell_layout_policy -- --nocapture`
Expected: FAIL because remembered widths, threshold collapse helpers, and edge-handle callbacks do not exist yet.

**Step 5: Run source smoke checks to verify they fail**
Run: `bash tests/shell_layout_ui_contract_smoke.sh`
Run: `bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh`
Expected: FAIL because the Slint files still hard-code expanded widths and have no edge-handle contract.

**Step 6: Commit the failing-test scaffold**
```bash
git add tests/sidebar_navigation_spec.rs tests/sidebar_navigation_smoke.rs tests/shell_layout_policy.rs tests/shell_layout_ui_contract_smoke.sh tests/assets_sidebar_toolbar_ui_contract_smoke.sh
git commit -m "test: lock sidebar edge handle behavior"
```

### Task 2: Add remembered widths and threshold helpers to the shell domain

**Files:**
- Modify: `src/shell/view_model.rs`
- Modify: `src/shell/view_model/assets.rs`
- Modify: `src/shell/view_model/projection.rs`
- Modify: `src/shell/layout.rs`
- Modify: `src/shell/metrics.rs`
- Modify: `src/app/bootstrap.rs`
- Reference: `tests/sidebar_navigation_spec.rs`
- Reference: `tests/shell_layout_policy.rs`

**Step 1: Add shell metrics for width ranges and collapse thresholds**
- Introduce shared constants for default, min, max, and collapse-threshold widths for both side regions.
- Keep the current visual defaults aligned with the existing layout so the feature starts from today's sizing.

**Step 2: Extend `ShellViewModel` with remembered widths**
- Add fields for the left expanded width and right expanded width.
- Initialize them from the shared default metrics in `Default`.

**Step 3: Add width update helpers**
- Add methods that clamp requested widths into the allowed range.
- Add methods that apply resize proposals, decide when a side should auto-collapse, and keep the previous expanded width available for reopen.
- Keep the existing toggle helpers working as simple visibility toggles that do not discard remembered widths.

**Step 4: Make layout resolution width-aware**
- Change `ShellLayoutInput` to carry requested left and right widths in addition to visibility flags.
- Update `resolve_shell_layout` so occupied width is computed from the actual requested widths instead of fixed panel constants.
- Preserve the existing order where the left sidebar collapses effectively before the right panel when space is tight.

**Step 5: Run the Rust tests**
Run: `cargo test --test sidebar_navigation_spec --test shell_layout_policy -- --nocapture`
Expected: PASS for the pure state and layout coverage, while UI-contract tests still fail.

**Step 6: Commit the domain work**
```bash
git add src/shell/view_model.rs src/shell/view_model/assets.rs src/shell/view_model/projection.rs src/shell/layout.rs src/shell/metrics.rs src/app/bootstrap.rs tests/sidebar_navigation_spec.rs tests/shell_layout_policy.rs
git commit -m "feat: remember shell panel widths"
```

### Task 3: Add edge handles and dynamic width bindings to the Slint shell

**Files:**
- Modify: `ui/app-window.slint`
- Modify: `ui/shell/assets-sidebar.slint`
- Modify: `ui/shell/sidebar.slint`
- Modify: `ui/shell/right-panel.slint`
- Reference: `tests/shell_layout_ui_contract_smoke.sh`
- Reference: `tests/assets_sidebar_toolbar_ui_contract_smoke.sh`

**Step 1: Replace hard-coded expanded widths with properties**
- Add left and right expanded-width properties to `AppWindow`.
- Thread those properties into `Sidebar`, `AssetsSidebar`, and `RightPanel`.
- Replace fixed `320px` and `392px` expanded width expressions with the injected property values.

**Step 2: Add edge-handle hit targets**
- Add a narrow handle strip on the workspace-facing edge of the left sidebar.
- Add a matching handle strip on the workspace-facing edge of the right panel.
- Add slim collapsed revive strips so the handles remain reachable when a panel is hidden.

**Step 3: Expose handle lifecycle callbacks**
- Add callbacks for left-toggle, left-drag-start, left-drag-move, left-drag-end.
- Add the symmetrical callback set for the right side.
- Keep callback naming consistent with the existing `*-requested` convention.

**Step 4: Keep the handle visually quiet**
- Use hover-only emphasis and resize cursors.
- Ensure the handle hit area stays narrow enough that it does not steal normal terminal interaction outside the boundary strip.

**Step 5: Run the source smoke checks**
Run: `bash tests/shell_layout_ui_contract_smoke.sh`
Run: `bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh`
Expected: PASS

**Step 6: Commit the Slint contract**
```bash
git add ui/app-window.slint ui/shell/assets-sidebar.slint ui/shell/sidebar.slint ui/shell/right-panel.slint tests/shell_layout_ui_contract_smoke.sh tests/assets_sidebar_toolbar_ui_contract_smoke.sh
git commit -m "feat: add shell edge handle UI contract"
```

### Task 4: Wire the edge-handle callbacks through bootstrap and sync widths into Slint

**Files:**
- Modify: `src/app/bootstrap.rs`
- Modify: `src/app/bootstrap/shell_chrome.rs`
- Modify: `src/app/bootstrap/assets_keychain.rs`
- Modify: `src/app/bootstrap/sftp.rs`
- Modify: `tests/sidebar_navigation_smoke.rs`
- Reference: `src/shell/view_model/assets.rs`
- Reference: `ui/app-window.slint`

**Step 1: Sync width state into the generated window**
- Push remembered left and right widths into the new `AppWindow` width properties during bootstrap sync.
- Keep effective visibility sync unchanged except for using the width-aware layout decision.

**Step 2: Handle left edge interactions**
- Route left-handle toggle requests to the existing assets sidebar toggle helper.
- Route left drag start/move/end callbacks through Rust so width changes and threshold collapse stay in one place.

**Step 3: Handle right edge interactions**
- Add matching bootstrap handlers for the right panel.
- Reuse the existing right-panel toggle path for click toggles and the new width helpers for drag interactions.

**Step 4: Preserve workspace geometry updates**
- Re-run shell layout sync after each toggle or resize interaction.
- Keep native terminal surface geometry updates and right-panel SFTP sync working after width changes.

**Step 5: Run interaction smoke coverage**
Run: `cargo test --test sidebar_navigation_smoke -- --nocapture`
Expected: PASS

**Step 6: Commit the bootstrap wiring**
```bash
git add src/app/bootstrap.rs src/app/bootstrap/shell_chrome.rs src/app/bootstrap/assets_keychain.rs src/app/bootstrap/sftp.rs tests/sidebar_navigation_smoke.rs
git commit -m "feat: wire sidebar edge handle interactions"
```

### Task 5: Run full verification and update the docs

**Files:**
- Modify: `docs/plans/2026-04-21-sidebar-edge-handles-design.md`
- Modify: `docs/plans/2026-04-21-sidebar-edge-handles-implementation-plan.md`
- Reference: all files touched in Tasks 1-4

**Step 1: Run focused regression tests**
Run: `cargo test --test sidebar_navigation_spec --test sidebar_navigation_smoke --test shell_layout_policy -- --nocapture`
Run: `bash tests/shell_layout_ui_contract_smoke.sh`
Run: `bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh`
Expected: PASS

**Step 2: Run adjacent shell regressions**
Run: `cargo test --test top_status_bar_smoke -- --nocapture`
Run: `cargo test --test bootstrap_smoke -- --nocapture`
Run: `cargo test --test window_geometry_spec -- --nocapture`
Expected: PASS

**Step 3: Refresh the docs if implementation details shifted**
- Update the design doc if any callback names, thresholds, or file ownership changed during implementation.
- Update this plan if execution uncovered missing verification or commit steps.

**Step 4: Commit the completed feature**
```bash
git add docs/plans/2026-04-21-sidebar-edge-handles-design.md docs/plans/2026-04-21-sidebar-edge-handles-implementation-plan.md ui/app-window.slint ui/shell/assets-sidebar.slint ui/shell/sidebar.slint ui/shell/right-panel.slint src/shell/view_model.rs src/shell/view_model/assets.rs src/shell/view_model/projection.rs src/shell/layout.rs src/shell/metrics.rs src/app/bootstrap.rs src/app/bootstrap/shell_chrome.rs src/app/bootstrap/assets_keychain.rs src/app/bootstrap/sftp.rs tests/sidebar_navigation_spec.rs tests/sidebar_navigation_smoke.rs tests/shell_layout_policy.rs tests/shell_layout_ui_contract_smoke.sh tests/assets_sidebar_toolbar_ui_contract_smoke.sh
git commit -m "feat: add workspace edge handles for shell panels"
```

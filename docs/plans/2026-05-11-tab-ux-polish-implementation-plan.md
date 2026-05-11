# Tab UX Polish Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Polish the tab UX by fixing context-menu overflow, making drag-reorder feedback clearly readable, and changing the titlebar summary to `IP · tab name` without altering session semantics.

**Architecture:** Keep all session-first tab behavior and Rust-side tab identity semantics intact. Limit Rust changes to summary-string shaping and UI contract updates, and keep drag improvements presentation-only inside Slint components.

**Tech Stack:** Rust, Slint, existing workspace tab view-model, existing titlebar tooltip and context-menu components.

---

### Task 1: Re-shape the titlebar active-session summary to `IP · tab name`

**Files:**
- Modify: `src/shell/view_model/workspace.rs`
- Modify: `src/shell/tabs.rs`
- Modify: `src/app/bootstrap/shell_chrome.rs`
- Modify: `ui/app-window.slint`
- Modify: `ui/shell/titlebar.slint`
- Test: `tests/workspace_tabs_spec.rs`

**Step 1: Inspect the current summary contract**
Run: `sed -n '1,120p' src/shell/view_model/workspace.rs && sed -n '420,455p' src/app/bootstrap/shell_chrome.rs && sed -n '250,330p' ui/shell/titlebar.slint`
Expected: the current contract projects `display_name`, `host`, `status`, and the titlebar renders name/detail as two separate text regions.

**Step 2: Add a dedicated primary summary string**
- Extend `ActiveWorkspaceTabSummary` to expose a primary summary string in the order `IP · tab name`.
- Reuse structured `host` and `display_name` instead of rebuilding from subtitle text.
- Keep tooltip text unchanged and fully structured.

**Step 3: Update the titlebar UI contract**
- Replace the current split emphasis so the titlebar has one clear primary summary lane.
- Keep host/status as low-priority supporting data only if there is a remaining secondary slot worth rendering.
- Preserve drag behavior on the summary lane.

**Step 4: Add a contract-level regression test**
- Add or update a test in `tests/workspace_tabs_spec.rs` that checks the titlebar contract still exposes the summary lane and that the new primary summary field is present.

**Step 5: Verify the contract wiring**
Run: `rg -n "active-session-summary|active-session-display-name|active-session-host|active-session-status-label" src ui tests/workspace_tabs_spec.rs`
Expected: matches show the new primary summary path and no broken titlebar contract wiring.

### Task 2: Fix tab context-menu width and make long commands fit cleanly

**Files:**
- Modify: `ui/components/workspace-tab-context-menu.slint`
- Modify: `ui/components/assets-context-menu-row.slint`
- Test: `tests/workspace_tabs_spec.rs`

**Step 1: Verify the current menu is hard-coded too narrow**
Run: `sed -n '1,220p' ui/components/workspace-tab-context-menu.slint && sed -n '1,120p' ui/components/assets-context-menu-row.slint`
Expected: the menu width is `236px` and the longest labels are `Close Tabs to the Right` / `Close Tabs to the Left`.

**Step 2: Widen the menu to a safe desktop width**
- Increase the menu width into the approved desktop-safe range.
- Keep anchor clamping intact so the menu still stays on-screen.

**Step 3: Shorten the two longest labels and add overflow safety**
- Rename:
  - `Close Tabs to the Right` -> `Close Right Tabs`
  - `Close Tabs to the Left` -> `Close Left Tabs`
- Add explicit single-line overflow behavior in the shared row text so future regressions degrade with ellipsis instead of visual spill.

**Step 4: Add a UI contract regression test**
- Add or update a test in `tests/workspace_tabs_spec.rs` to assert the new labels exist and the context menu remains wired through `ui/app-window.slint`.

**Step 5: Verify the menu strings and width change are discoverable**
Run: `rg -n "Close Right Tabs|Close Left Tabs|width: 2[89][0-9]px|workspace-tab-context-menu" ui tests/workspace_tabs_spec.rs`
Expected: the new labels and widened menu width are present.

### Task 3: Strengthen drag-reorder feedback with a placeholder gap and clearer dragged state

**Files:**
- Modify: `ui/components/active-tab.slint`
- Modify: `ui/shell/tabbar.slint`
- Test: `tests/workspace_tabs_spec.rs`
- Test: `tests/bootstrap_smoke.rs`

**Step 1: Confirm the current drag feedback is only lift plus a thin insertion line**
Run: `sed -n '1,240p' ui/components/active-tab.slint && sed -n '1,220p' ui/shell/tabbar.slint`
Expected: dragged tabs only shift slightly and the target preview is mostly a thin line.

**Step 2: Strengthen the dragged-tab visual state**
- Increase the dragged tab's lift and separation from the strip.
- Make the dragged tab visually read as "picked up" using stronger border/surface contrast and slightly lower opacity.
- Keep the style restrained and Fluent-like.

**Step 3: Add a visible placeholder gap at the target slot**
- Use the existing drag preview slot state to open a clearer tab-width placeholder region.
- Keep the accent insertion line inside that placeholder so the user sees both approximate slot and exact anchor.
- Do not change reorder semantics or Rust callbacks.

**Step 4: Keep click-vs-drag behavior unchanged**
- Ensure the stronger visuals do not alter the current drag threshold semantics.
- Preserve the rule that releasing after crossing drag threshold reorders instead of selecting.

**Step 5: Add regression coverage**
- Add or update a UI contract test in `tests/workspace_tabs_spec.rs` that checks the placeholder/insertion preview plumbing still exists.
- Add or keep a smoke-level test in `tests/bootstrap_smoke.rs` that verifies reorder still preserves active session and only changes UI order.

**Step 6: Verify the drag-preview contract**
Run: `rg -n "drag-preview-slot|show-leading-insertion-line|tab-reorder-requested|drag-active" ui tests/workspace_tabs_spec.rs tests/bootstrap_smoke.rs`
Expected: the stronger preview path remains driven by the existing stable tab-id reorder contract.

### Task 4: Run polish verification and prepare execution handoff

**Files:**
- Modify: any files above as needed

**Step 1: Run formatting**
Run: `cargo fmt --all --manifest-path Cargo.toml`
Expected: formatting completes cleanly.

**Step 2: Run compile verification**
Run: `cargo test -q --manifest-path Cargo.toml --no-run`
Expected: the workspace compiles.

**Step 3: Run focused tab contract tests**
Run: `cargo test -q --manifest-path Cargo.toml --test workspace_tabs_spec -- --nocapture`
Expected: tab contract coverage passes, including titlebar and context-menu assertions.

**Step 4: Run focused reorder smoke coverage**
Run: `cargo test -q --manifest-path Cargo.toml workspace_tab_reorder -- --nocapture`
Expected: reorder smoke coverage passes and active session remains stable.

**Step 5: Manual verification checklist**
- Right-click a tab and confirm the menu is wide enough to show all rows fully.
- Confirm `Close Right Tabs` and `Close Left Tabs` do not wrap or spill outside the menu.
- Drag a middle tab across several neighbors and confirm a visible placeholder slot opens where it will land.
- Confirm the dragged tab looks clearly lifted but still matches the Fluent/Mica visual language.
- Confirm the titlebar summary reads `IP · tab name` and still shows the full structured tooltip on hover.
- Confirm no drag/reorder action rebuilds a session or changes reconnect behavior.

**Step 6: Commit the implementation**
```bash
git add docs/plans/2026-05-11-tab-ux-polish-design.md \
        docs/plans/2026-05-11-tab-ux-polish-implementation-plan.md \
        ui/components/assets-context-menu-row.slint \
        ui/components/workspace-tab-context-menu.slint \
        ui/components/active-tab.slint \
        ui/shell/tabbar.slint \
        ui/shell/titlebar.slint \
        ui/app-window.slint \
        src/shell/tabs.rs \
        src/shell/view_model/workspace.rs \
        src/app/bootstrap/shell_chrome.rs \
        tests/workspace_tabs_spec.rs \
        tests/bootstrap_smoke.rs

git commit -m "fix: polish tab menu, drag feedback, and titlebar summary"
```

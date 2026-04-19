# Modal Refinement Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Repair the recent modal unification regressions by slimming shared chrome, restoring consistent form grid behavior, and cleaning up elevation across modal and transfer surfaces.

**Architecture:** Adjust the shared shell/tokens first so all dialogs inherit cleaner elevation and lighter footer/body framing, then refactor form dialogs to use a lighter content rhythm and explicit split-row sizing instead of nested cards and fixed-height wrappers. Verify with smoke tests plus render coverage focused on layout and elevation contracts.

**Tech Stack:** Slint UI components, Rust test harness, source-contract smoke tests, software-render render specs

---

### Task 1: Capture the regression contract in tests

**Files:**
- Modify: `tests/assets_modal_smoke.rs`
- Modify: `tests/assets_modal_render_spec.rs`

**Step 1: Write failing smoke assertions**
- Add a smoke test that asserts:
  - `ModalFooterBar` no longer defines the heavy `action-rail` wrapper.
  - Form modal headers no longer use the current slogan-style subtitle strings.
  - `AssetsFolderCreateModal` no longer wraps its single field in `DialogSectionCard`.

**Step 2: Run the targeted smoke test to verify it fails**

Run: `cargo test --test assets_modal_smoke modal_refinement_regression_contract -q`

Expected: FAIL because the current chrome still contains `action-rail`, the old subtitles are still present, and the folder modal still uses the card wrapper.

**Step 3: Add a failing render assertion**
- Add/adjust render coverage so the SSH and folder/snippet dialogs assert a lighter shell/footer relationship instead of the current heavy framed appearance.

**Step 4: Run the targeted render test to verify it fails**

Run: `cargo test --test assets_modal_render_spec <new_test_name> -q`

Expected: FAIL before implementation.

### Task 2: Slim the shared shell, body, footer, and elevation tokens

**Files:**
- Modify: `ui/components/blocking-modal-shell.slint`
- Modify: `ui/components/modal-chrome.slint`
- Modify: `ui/theme/tokens.slint`
- Modify: `ui/shell/transfer-center.slint`

**Step 1: Update tokens for cleaner elevation**
- Reduce modal/utility/conflict shadow opacity and tighten glow values.
- Slightly refine header/footer/body surface tokens for lighter light-theme contrast.
- Reduce oversized footer/header defaults where they currently add visual mass.

**Step 2: Refactor `BlockingModalShell`**
- Keep the same callbacks and focus/ESC behavior.
- Tighten far/near shadow offsets and halo bounds so the shell reads as one elevated layer.

**Step 3: Refactor `ModalBodyScrollArea` and `ModalFooterBar`**
- Remove the heavy footer action rail.
- Add a lighter unframed body mode appropriate for forms.
- Keep the divider, spacing, and slot behavior compatible with current modal callers.

**Step 4: Mirror the new elevation treatment in Transfer Center**
- Reduce stacked shadow appearance under the transfer utility surface.

**Step 5: Run the targeted smoke/render tests**

Run:
- `cargo test --test assets_modal_smoke modal_refinement_regression_contract -q`
- `cargo test --test assets_modal_render_spec <updated_test_name> -q`

Expected: Tests move closer to green, with any remaining failures isolated to form dialog content.

### Task 3: Rebuild the form modal rhythm around lighter sections and stable row sizing

**Files:**
- Modify: `ui/components/assets-ssh-connection-modal.slint`
- Modify: `ui/components/assets-folder-create-modal.slint`
- Modify: `ui/components/assets-snippet-modal.slint`
- Modify: `ui/components/assets-keychain-ssh-key-modal.slint`
- Modify: `ui/components/assets-keychain-identity-modal.slint`
- Modify: `ui/components/settings-modal.slint`
- Modify: `ui/components/sync-vault-modal.slint`

**Step 1: Shorten header copy**
- Replace slogan-like subtitles with short product copy or remove them where unnecessary.

**Step 2: Remove unnecessary card shells**
- For simple forms, place fields directly in body content with section headings.
- Keep `DialogSectionCard` only for emphasized summary/warning/detail blocks.

**Step 3: Normalize split-row layout**
- Replace fixed-height wrappers with auto-sizing row containers based on field heights.
- Use clear primary/secondary widths for split rows, especially the SSH `User / Port` pattern.

**Step 4: Clean up footer composition**
- Keep button alignment behavior while relying on the lighter shared footer.

**Step 5: Run focused modal suites**

Run:
- `cargo test --test assets_modal_smoke -q`
- `cargo test --test assets_modal_render_spec -q`

Expected: PASS.

### Task 4: Final verification

**Files:**
- No additional file changes unless needed for test fixes.

**Step 1: Run focused regression coverage**

Run:
- `cargo test --test assets_modal_smoke -q`
- `cargo test --test assets_modal_render_spec -q`
- `cargo test --test assets_context_menu_smoke -q`

**Step 2: If modal-specific failures remain, fix only those regressions and re-run the same commands**

**Step 3: Commit**

```bash
git add docs/plans/2026-04-19-modal-refinement-design.md \
  docs/plans/2026-04-19-modal-refinement.md \
  ui/components/blocking-modal-shell.slint \
  ui/components/modal-chrome.slint \
  ui/theme/tokens.slint \
  ui/shell/transfer-center.slint \
  ui/components/assets-ssh-connection-modal.slint \
  ui/components/assets-folder-create-modal.slint \
  ui/components/assets-snippet-modal.slint \
  ui/components/assets-keychain-ssh-key-modal.slint \
  ui/components/assets-keychain-identity-modal.slint \
  ui/components/settings-modal.slint \
  ui/components/sync-vault-modal.slint \
  tests/assets_modal_smoke.rs \
  tests/assets_modal_render_spec.rs

git commit -m "fix: refine modal shell layout and elevation"
```

# Remaining Modal Migration Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Migrate the remaining old blocking dialogs to the shared modal shell/chrome system and add light/dark render coverage for them.

**Architecture:** Reuse the shared modal infrastructure already established in the first refactor wave. Move the three lightweight confirm dialogs and the editor-style remote-file dialog onto the same token-backed header/body/footer system, then add render and smoke coverage to lock in structure, visibility, and theme parity.

**Tech Stack:** Rust, Slint, software renderer tests, existing modal tokens/chrome components.

---

### Task 1: Lock in the expected shared-chrome contract for the remaining dialogs

**Files:**
- Modify: `tests/assets_modal_smoke.rs`
- Test: `tests/assets_modal_smoke.rs`

**Step 1: Write the failing test**

Add assertions that `assets-rename-modal`, `assets-delete-confirm-modal`, `ssh-host-key-confirm-modal`, and `sftp-remote-file-modal` use `ModalHeaderBar` / `ModalFooterBar`, and that confirm-style dialogs use shared section / field / banner primitives where appropriate.

**Step 2: Run test to verify it fails**

Run: `cargo test --test assets_modal_smoke remaining_old_dialogs_adopt_shared_modal_chrome_contract -- --nocapture`
Expected: FAIL because the remaining dialogs still contain legacy bespoke header/footer shells.

**Step 3: Write minimal implementation**

Update the targeted modal files only as much as needed to satisfy the new shared-chrome contract.

**Step 4: Run test to verify it passes**

Run: `cargo test --test assets_modal_smoke remaining_old_dialogs_adopt_shared_modal_chrome_contract -- --nocapture`
Expected: PASS.

### Task 2: Migrate `assets-rename-modal` to the shared shell language

**Files:**
- Modify: `ui/components/assets-rename-modal.slint`
- Test: `tests/assets_modal_render_spec.rs`

**Step 1: Write the failing test**

Add a render test that proves the rename dialog has a visible shared footer action rail, a section-backed body surface, and a field surface that remains visible in light theme.

**Step 2: Run test to verify it fails**

Run: `cargo test --test assets_modal_render_spec rename_modal_renders_shared_section_field_and_footer_actions -- --nocapture`
Expected: FAIL on the legacy dialog.

**Step 3: Write minimal implementation**

Replace bespoke header/body/footer chrome with shared modal primitives while keeping the same callbacks and rename behavior.

**Step 4: Run test to verify it passes**

Run: `cargo test --test assets_modal_render_spec rename_modal_renders_shared_section_field_and_footer_actions -- --nocapture`
Expected: PASS.

### Task 3: Migrate `assets-delete-confirm-modal` to the shared confirm-dialog language

**Files:**
- Modify: `ui/components/assets-delete-confirm-modal.slint`
- Test: `tests/assets_modal_render_spec.rs`

**Step 1: Write the failing test**

Add a render test asserting the delete confirm dialog renders a structured message card and a visible destructive footer action cluster.

**Step 2: Run test to verify it fails**

Run: `cargo test --test assets_modal_render_spec delete_confirm_modal_renders_structured_warning_and_destructive_footer -- --nocapture`
Expected: FAIL on the legacy dialog.

**Step 3: Write minimal implementation**

Adopt shared header/footer primitives, migrate body messaging into shared section card styling, and map delete to the shared destructive button hierarchy.

**Step 4: Run test to verify it passes**

Run: `cargo test --test assets_modal_render_spec delete_confirm_modal_renders_structured_warning_and_destructive_footer -- --nocapture`
Expected: PASS.

### Task 4: Migrate `ssh-host-key-confirm-modal` to the shared confirm-dialog language

**Files:**
- Modify: `ui/components/ssh-host-key-confirm-modal.slint`
- Test: `tests/assets_modal_render_spec.rs`

**Step 1: Write the failing test**

Add a render test asserting the host-key confirm dialog renders a structured verification card, visible fingerprint content, and a clear accept/reject footer layout.

**Step 2: Run test to verify it fails**

Run: `cargo test --test assets_modal_render_spec ssh_host_key_modal_renders_verification_card_and_action_row -- --nocapture`
Expected: FAIL on the legacy dialog.

**Step 3: Write minimal implementation**

Move the dialog to shared header/footer/card primitives while preserving trust/reject semantics.

**Step 4: Run test to verify it passes**

Run: `cargo test --test assets_modal_render_spec ssh_host_key_modal_renders_verification_card_and_action_row -- --nocapture`
Expected: PASS.

### Task 5: Migrate `sftp-remote-file-modal` to the shared editor-dialog shell

**Files:**
- Modify: `ui/components/sftp-remote-file-modal.slint`
- Test: `tests/assets_modal_render_spec.rs`

**Step 1: Write the failing test**

Add a render test asserting the remote-file dialog renders a separated editor body surface, path/status context, and a shared footer action bar.

**Step 2: Run test to verify it fails**

Run: `cargo test --test assets_modal_render_spec sftp_remote_file_modal_renders_editor_surface_status_and_footer_actions -- --nocapture`
Expected: FAIL on the legacy dialog.

**Step 3: Write minimal implementation**

Adopt the shared shell/header/footer primitives while keeping the editor-centered body structure and existing callbacks.

**Step 4: Run test to verify it passes**

Run: `cargo test --test assets_modal_render_spec sftp_remote_file_modal_renders_editor_surface_status_and_footer_actions -- --nocapture`
Expected: PASS.

### Task 6: Add light/dark modal render coverage for the migrated dialogs

**Files:**
- Modify: `tests/assets_modal_render_spec.rs`
- Test: `tests/assets_modal_render_spec.rs`

**Step 1: Write the failing test**

Add light/dark render assertions that compare modal shell regions for the migrated dialogs and verify both themes preserve visible chrome separation.

**Step 2: Run test to verify it fails**

Run: `cargo test --test assets_modal_render_spec migrated_remaining_modals_preserve_distinct_light_and_dark_shells -- --nocapture`
Expected: FAIL until the render assertions and migrated shells align.

**Step 3: Write minimal implementation**

Adjust tokens/layout only where needed so the migrated dialogs show clear shell/body/footer separation in both themes.

**Step 4: Run test to verify it passes**

Run: `cargo test --test assets_modal_render_spec migrated_remaining_modals_preserve_distinct_light_and_dark_shells -- --nocapture`
Expected: PASS.

### Task 7: Run focused regression coverage for migrated modal behavior

**Files:**
- Test: `tests/assets_modal_smoke.rs`
- Test: `tests/assets_modal_render_spec.rs`

**Step 1: Run the modal smoke suite**

Run: `cargo test --test assets_modal_smoke -q`
Expected: PASS.

**Step 2: Run the modal render suite**

Run: `cargo test --test assets_modal_render_spec -q`
Expected: PASS.

**Step 3: Run any targeted remote-file modal coverage if present**

Run: `cargo test --test assets_modal_smoke sftp_remote -- --nocapture`
Expected: PASS or no matching failures.

**Step 4: Commit**

```bash
git add docs/plans/2026-04-19-remaining-modal-migration-design.md \
  docs/plans/2026-04-19-remaining-modal-migration.md \
  ui/components/assets-rename-modal.slint \
  ui/components/assets-delete-confirm-modal.slint \
  ui/components/ssh-host-key-confirm-modal.slint \
  ui/components/sftp-remote-file-modal.slint \
  tests/assets_modal_smoke.rs \
  tests/assets_modal_render_spec.rs

git commit -m "feat: migrate remaining dialogs to shared modal chrome"
```

# Modal Scroll Wheel-Only Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Disable left-button drag scrolling inside shared modal bodies so modal content scrolls only by mouse wheel or scrollbar dragging.

**Architecture:** Keep the behavior change scoped to the shared `ModalBodyScrollArea` in `ui/components/modal-chrome.slint`. Update the existing modal smoke and shell contract tests first so they describe the intended input policy, then make the minimal shared `ScrollView` change to satisfy those tests.

**Tech Stack:** Slint UI components, Rust smoke tests, shell contract smoke tests.

---

### Task 1: Lock the new shared modal scroll policy in tests

**Files:**
- Modify: `tests/assets_modal_smoke.rs`
- Modify: `tests/assets_modal_ui_contract_smoke.sh`

**Step 1: Write the failing Rust assertion**

Change the shared modal chrome expectation so the smoke test asserts `mouse-drag-pan-enabled: false;` instead of `true;`.

**Step 2: Run the focused Rust test to verify it fails**

Run: `cargo test --test assets_modal_smoke sync_modal_header_body_and_footer_are_explicitly_anchored_and_scrollable -- --exact`
Expected: FAIL because the shared modal chrome still enables drag panning.

**Step 3: Write the failing shell contract assertion**

Change the shell smoke check so it greps for `mouse-drag-pan-enabled: false;` in `ui/components/modal-chrome.slint`.

**Step 4: Run the shell contract to verify it fails**

Run: `bash tests/assets_modal_ui_contract_smoke.sh`
Expected: FAIL because the shared modal chrome still enables drag panning.

**Step 5: Commit the contract updates if working in commit-sized slices**

```bash
git add tests/assets_modal_smoke.rs tests/assets_modal_ui_contract_smoke.sh
git commit -m "test: lock shared modal wheel-only scrolling"
```

### Task 2: Remove shared modal drag-to-scroll

**Files:**
- Modify: `ui/components/modal-chrome.slint`
- Review: `ui/components/assets-ssh-connection-modal.slint`
- Review: `ui/components/assets-keychain-identity-modal.slint`
- Review: `ui/components/assets-keychain-ssh-key-modal.slint`
- Review: `ui/components/assets-snippet-modal.slint`
- Review: `ui/components/sync-vault-modal.slint`

**Step 1: Make the minimal shared Slint change**

Set `mouse-drag-pan-enabled: false;` in the shared `ScrollView` inside `ModalBodyScrollArea`.

**Step 2: Keep surrounding behavior unchanged**

Do not change:
- `horizontal-scrollbar-policy: always-off;`
- body sizing math
- header drag callbacks
- any modal-specific form layout

**Step 3: Run the focused Rust test again**

Run: `cargo test --test assets_modal_smoke sync_modal_header_body_and_footer_are_explicitly_anchored_and_scrollable -- --exact`
Expected: PASS.

**Step 4: Run the shell contract again**

Run: `bash tests/assets_modal_ui_contract_smoke.sh`
Expected: PASS.

**Step 5: Commit the UI change if requested**

```bash
git add ui/components/modal-chrome.slint tests/assets_modal_smoke.rs tests/assets_modal_ui_contract_smoke.sh
git commit -m "fix: disable shared modal drag scrolling"
```

### Task 3: Verify the shared modal contract end-to-end

**Files:**
- Review: `ui/components/modal-chrome.slint`
- Review: `tests/assets_modal_smoke.rs`
- Review: `tests/assets_modal_ui_contract_smoke.sh`

**Step 1: Run the focused Rust modal smoke suite**

Run: `cargo test --test assets_modal_smoke -q`
Expected: PASS.

**Step 2: Run the shell modal contract suite**

Run: `bash tests/assets_modal_ui_contract_smoke.sh`
Expected: PASS.

**Step 3: Review the final diff**

Run: `git diff -- ui/components/modal-chrome.slint tests/assets_modal_smoke.rs tests/assets_modal_ui_contract_smoke.sh`
Confirm the diff only changes the shared drag-pan flag and its test expectations.

**Step 4: Stop before broader edits**

No modal-specific follow-up should be needed unless verification shows another code path re-enables drag scrolling.

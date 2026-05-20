# Dead Actions Cleanup Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Remove the three most misleading dead UI entry points without expanding into broader SFTP or key-management redesign.

**Architecture:** Delete the dead actions at the source so the view-model no longer exposes them. Update tests first so the shipped contract becomes absence instead of placeholder feedback.

**Tech Stack:** Rust, Slint, cargo test, shell smoke scripts

---

### Task 1: Lock removal contract for dead SSH menu actions and identity modal button

**Files:**
- Modify: `tests/assets_context_menu_spec.rs`
- Modify: `tests/assets_context_menu_smoke.rs`
- Modify: `tests/assets_modal_smoke.rs`
- Modify: `tests/assets_context_menu_ui_contract_smoke.sh`

**Step 1: Write the failing test**
- Assert SSH context menu no longer contains `proxy-chrome-via-server`
- Assert SSH context menu no longer contains `upload-ssh-public-key`
- Assert identity modal no longer renders `Create New Key`
- Run focused tests first and watch them fail before implementation

**Step 2: Run test to verify it fails**

Run:
- `cargo test -q --manifest-path Cargo.toml --test assets_context_menu_spec --test assets_context_menu_smoke --test assets_modal_smoke -- --nocapture`
- `bash tests/assets_context_menu_ui_contract_smoke.sh`

Expected: FAIL because current UI still exposes the dead actions.

**Step 3: Write minimal implementation**

**Files:**
- Modify: `src/shell/context_menu.rs`
- Modify: `ui/components/assets-keychain-identity-modal.slint`
- Modify: `src/app/bootstrap/assets_keychain.rs` only if any dead handler path needs cleanup

Implementation notes:
- Delete the two SSH planned actions from `resolve_ssh_connection_actions(...)`
- Remove the `Create New Key` button from the identity modal
- Do not change SFTP planned actions
- Do not introduce replacement flows in this task

**Step 4: Run test to verify it passes**

Run:
- `cargo test -q --manifest-path Cargo.toml --test assets_context_menu_spec --test assets_context_menu_smoke --test assets_modal_smoke -- --nocapture`
- `bash tests/assets_context_menu_ui_contract_smoke.sh`
- `cargo test -q --manifest-path Cargo.toml --no-run`

Expected: PASS

**Step 5: Commit**

```bash
git add docs/plans/2026-05-20-dead-actions-cleanup-design.md \
        docs/plans/2026-05-20-dead-actions-cleanup-implementation-plan.md \
        src/shell/context_menu.rs \
        ui/components/assets-keychain-identity-modal.slint \
        tests/assets_context_menu_spec.rs \
        tests/assets_context_menu_smoke.rs \
        tests/assets_modal_smoke.rs \
        tests/assets_context_menu_ui_contract_smoke.sh
git commit -m "fix: remove dead ssh asset actions"
```

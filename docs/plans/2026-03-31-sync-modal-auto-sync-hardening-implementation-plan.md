# Sync Modal And Auto Sync Hardening Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make sync settings fully reachable in constrained windows, centralize auto-sync scheduling for all vault mutations, and expose clear titlebar sync feedback without risking empty-local overwrites.

**Architecture:** Keep the existing `AppWindow -> bootstrap -> ShellViewModel -> vault runtime` pipeline, but add a dedicated sync scheduler inside bootstrap, feed its state into both the modal and the titlebar, and make sync modal sizing responsive at the shell level. The scheduler owns dirty tracking, debounce, periodic retries, and push safety checks so mutation paths only need to report successful writes.

**Tech Stack:** Rust, Slint, Tokio timers, cargo test, shell smoke tests

---

### Task 1: Lock down the responsive sync modal contract

**Files:**
- Modify: `ui/app-window.slint`
- Modify: `ui/components/sync-vault-modal.slint`
- Modify: `ui/theme/tokens.slint`
- Test: `tests/assets_modal_smoke.rs`
- Test: `tests/assets_modal_render_spec.rs`
- Test: `tests/assets_modal_ui_contract_smoke.sh`

**Step 1: Write the failing tests**

Add assertions for:

- the sync modal shell no longer hardcodes `modal-height: 620px`
- the sync modal footer remains pinned and visibly distinct in render output
- error content stays in the body region instead of compressing the footer
- the footer/action bar color contrast is stronger than the body surface

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test assets_modal_smoke --test assets_modal_render_spec -q
bash tests/assets_modal_ui_contract_smoke.sh
```

Expected:

- FAIL because the sync shell still uses a fixed height
- FAIL because current footer contrast and long-error layout do not satisfy the new assertions

**Step 3: Write minimal implementation**

- clamp the sync modal shell height to the available viewport
- tighten sync modal spacing so the footer remains visible on shorter windows
- give header/body/footer stronger surface separation and button contrast
- keep header drag behavior intact

**Step 4: Run tests to verify they pass**

Run the same commands from Step 2.

**Step 5: Commit**

```bash
git add ui/app-window.slint ui/components/sync-vault-modal.slint ui/theme/tokens.slint tests/assets_modal_smoke.rs tests/assets_modal_render_spec.rs tests/assets_modal_ui_contract_smoke.sh
git commit -m "fix: harden sync modal layout and contrast"
```

### Task 2: Add scheduler state to the shell and titlebar feedback path

**Files:**
- Modify: `src/shell/view_model.rs`
- Modify: `ui/shell/titlebar.slint`
- Modify: `ui/app-window.slint`
- Modify: `src/app/bootstrap.rs`
- Test: `tests/top_status_bar_smoke.rs`
- Test: `tests/top_status_bar_ui_contract_smoke.sh`

**Step 1: Write the failing tests**

Add coverage for:

- titlebar exposes a sync status state that can render idle/syncing/success/error
- sync-now action still exists and is separate from opening settings
- bootstrap threads sync state updates into the window surface

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test top_status_bar_smoke -q
bash tests/top_status_bar_ui_contract_smoke.sh
```

Expected:

- FAIL because there is no titlebar sync feedback state yet

**Step 3: Write minimal implementation**

- add sync-status properties to the relevant view model/window surface
- update the titlebar to render a visible in-flight/success/error treatment
- preserve the existing manual sync-now action path

**Step 4: Run tests to verify they pass**

Run the same commands from Step 2.

**Step 5: Commit**

```bash
git add src/shell/view_model.rs ui/shell/titlebar.slint ui/app-window.slint src/app/bootstrap.rs tests/top_status_bar_smoke.rs tests/top_status_bar_ui_contract_smoke.sh
git commit -m "feat: add titlebar sync feedback states"
```

### Task 3: Introduce a centralized vault sync scheduler

**Files:**
- Modify: `src/app/bootstrap.rs`
- Test: `tests/bootstrap_smoke.rs`
- Test: `tests/sync_vault_modal_smoke.rs`

**Step 1: Write the failing tests**

Add coverage for:

- successful vault mutations mark sync dirty and schedule one debounced sync
- repeated mutations during the debounce window coalesce into one sync request
- unlocking or reopening settings does not trigger a push by itself
- periodic sync ticks skip push when there is no local dirty state

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test bootstrap_smoke --test sync_vault_modal_smoke -q
```

Expected:

- FAIL because the current implementation still syncs from scattered call sites and has no shared debounce/periodic scheduler

**Step 3: Write minimal implementation**

- add scheduler state with dirty/in-flight/last-result/debounce/periodic fields
- move sync triggering behind helpers such as `mark_vault_sync_dirty(...)` and `request_vault_sync(...)`
- start a `2 minute` repeated timer only when sync is configured and the vault is unlocked
- ensure automatic push requires both `dirty == true` and a present local state

**Step 4: Run tests to verify they pass**

Run the same command from Step 2.

**Step 5: Commit**

```bash
git add src/app/bootstrap.rs tests/bootstrap_smoke.rs tests/sync_vault_modal_smoke.rs
git commit -m "feat: centralize vault auto sync scheduling"
```

### Task 4: Route all vault mutation paths through the scheduler

**Files:**
- Modify: `src/app/bootstrap.rs`
- Test: `tests/bootstrap_smoke.rs`

**Step 1: Write the failing tests**

Add or extend coverage for:

- SSH asset save triggers dirty scheduling
- snippet save triggers dirty scheduling
- folder create/rename/delete triggers dirty scheduling
- key or identity mutations trigger dirty scheduling

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test bootstrap_smoke -q
```

Expected:

- FAIL because some mutation paths still bypass the scheduler helper

**Step 3: Write minimal implementation**

- replace direct `sync_local_vault_if_auto_enabled(...)` calls with the scheduler helper
- hook the remaining mutation entrypoints after successful persistence only
- keep failed writes from marking dirty

**Step 4: Run tests to verify they pass**

Run the same command from Step 2.

**Step 5: Commit**

```bash
git add src/app/bootstrap.rs tests/bootstrap_smoke.rs
git commit -m "feat: route vault mutations through sync scheduler"
```

### Task 5: Verify end-to-end sync behavior and finish the branch

**Files:**
- Modify: `src/app/bootstrap.rs`
- Modify: `tests/bootstrap_smoke.rs`
- Modify: `tests/sync_vault_modal_smoke.rs`
- Modify: `tests/top_status_bar_smoke.rs`

**Step 1: Write the failing tests**

Add final regression coverage for:

- titlebar enters syncing then success/error after manual sync
- local data loss with an existing remote revision does not auto-push an empty local snapshot
- periodic sync can recover state without treating unlock as a mutation

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test bootstrap_smoke --test sync_vault_modal_smoke --test top_status_bar_smoke -q
```

Expected:

- FAIL until the final feedback and safety edges are wired through the scheduler

**Step 3: Write minimal implementation**

- finalize scheduler result publication to the titlebar and modal
- keep remote-first recovery logic for empty-local cases
- ensure manual sync reuses the same status update path as auto-sync

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test -q
```

Expected:

- PASS, or a documented pre-existing unrelated failure only

**Step 5: Commit**

```bash
git add src/app/bootstrap.rs tests/bootstrap_smoke.rs tests/sync_vault_modal_smoke.rs tests/top_status_bar_smoke.rs
git commit -m "fix: finalize sync safety and feedback flow"
```

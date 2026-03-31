# Sync Entry And Auto Sync Implementation Plan

日期: 2026-03-31
执行者: Codex
状态: 已实现并合并到 `master`

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Rework sync so the titlebar button performs immediate sync, configuration lives under the app menu, and auto-sync becomes the default interaction model.

**Architecture:** Keep the existing `AppWindow -> bootstrap -> ShellViewModel -> vault` pipeline, but split “sync action” from “sync settings”. Reuse the blocking modal shell for settings, move error rendering into the body, and let bootstrap decide whether the titlebar action can sync immediately or must route into settings/unlock. Add a small persisted sync-settings state surface for one primary Gitee target plus one optional mirror and auto-sync.

**Tech Stack:** Rust, Slint, Tokio, reqwest, cargo test, shell smoke scripts

---

### Task 1: Freeze the new titlebar and menu contract

**Files:**
- Modify: `ui/shell/titlebar.slint`
- Modify: `ui/components/titlebar-menu.slint`
- Modify: `ui/app-window.slint`
- Modify: `src/app/bootstrap.rs`
- Test: `tests/vault_settings_smoke.rs`
- Test: `tests/top_status_bar_smoke.rs`
- Test: `tests/top_status_bar_ui_contract_smoke.sh`

**Step 1: Write the failing tests**

Add coverage for:

- titlebar sync button now invokes immediate sync rather than opening the sync modal
- app menu exposes `Sync Settings`
- clicking the titlebar sync button only opens settings when prerequisites are missing

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test vault_settings_smoke --test top_status_bar_smoke -q
bash tests/top_status_bar_ui_contract_smoke.sh
```

Expected:

- FAIL because titlebar still invokes `open-sync-modal-requested`
- FAIL because menu does not expose `Sync Settings`

**Step 3: Write minimal implementation**

- Rename the titlebar callback path to `sync-now-requested`
- Add `sync-settings-selected` to the titlebar menu
- Update bootstrap wiring so titlebar sync routes through a dedicated sync-now handler

**Step 4: Run tests to verify they pass**

Run the same commands from Step 2.

**Step 5: Commit**

```bash
git add ui/shell/titlebar.slint ui/components/titlebar-menu.slint ui/app-window.slint src/app/bootstrap.rs tests/vault_settings_smoke.rs tests/top_status_bar_smoke.rs tests/top_status_bar_ui_contract_smoke.sh
git commit -m "feat: split sync action from sync settings"
```

### Task 2: Replace the current sync modal with a real sync settings modal

**Files:**
- Modify: `ui/components/sync-vault-modal.slint`
- Modify: `ui/app-window.slint`
- Modify: `src/shell/view_model.rs`
- Modify: `src/app/bootstrap.rs`
- Test: `tests/sync_vault_modal_smoke.rs`
- Test: `tests/assets_modal_render_spec.rs`
- Test: `tests/assets_modal_ui_contract_smoke.sh`

**Step 1: Write the failing tests**

Add coverage for:

- modal title becomes `Sync Settings`
- error banner renders inside the body instead of the footer
- long error text does not starve the footer action zone
- modal still exposes drag callbacks and close behavior

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test sync_vault_modal_smoke --test assets_modal_render_spec -q
bash tests/assets_modal_ui_contract_smoke.sh
```

Expected:

- FAIL because the modal still uses the old sync headline and footer error layout

**Step 3: Write minimal implementation**

- Move the error banner into the body content stack
- Increase contrast between footer background and buttons
- make the header drag zone visually explicit
- add fields/state slots needed for sync settings form data

**Step 4: Run tests to verify they pass**

Run the same commands from Step 2.

**Step 5: Commit**

```bash
git add ui/components/sync-vault-modal.slint ui/app-window.slint src/shell/view_model.rs src/app/bootstrap.rs tests/sync_vault_modal_smoke.rs tests/assets_modal_render_spec.rs tests/assets_modal_ui_contract_smoke.sh
git commit -m "feat: convert sync modal into sync settings"
```

### Task 3: Add persisted sync settings for a primary target, optional mirror, and auto-sync

**Files:**
- Modify: `src/shell/view_model.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `src/app/vault/model.rs`
- Modify: `src/app/vault/bootstrap.rs`
- Test: `tests/vault_bootstrap_spec.rs`
- Test: `tests/bootstrap_smoke.rs`

**Step 1: Write the failing tests**

Add tests for:

- editing sync settings updates bootstrap/local vault bundle
- auto-sync enabled flag persists
- optional mirror target can be enabled and saved
- provider credentials are stored via bootstrap credential refs

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test vault_bootstrap_spec --test bootstrap_smoke -q
```

Expected:

- FAIL because no sync settings form state is persisted back into the vault bootstrap bundle

**Step 3: Write minimal implementation**

- introduce sync settings draft state in `ShellViewModel`
- allow bootstrap to build/update a bundle from the settings draft
- persist provider credentials with `persist_provider_credential`
- allow one primary Gitee target and one optional mirror target

**Step 4: Run tests to verify they pass**

Run the same command from Step 2.

**Step 5: Commit**

```bash
git add src/shell/view_model.rs src/app/bootstrap.rs src/app/vault/model.rs src/app/vault/bootstrap.rs tests/vault_bootstrap_spec.rs tests/bootstrap_smoke.rs
git commit -m "feat: persist sync settings targets and auto sync"
```

### Task 4: Wire real Gitee gist read/write support for the sync path

**Files:**
- Modify: `src/app/vault/provider/gitee_gist.rs`
- Test: `tests/vault_provider_gitee_spec.rs`
- Test: `tests/vault_sync_engine_spec.rs`

**Step 1: Write the failing tests**

Add API-level tests for:

- reading `vault-head.json` from a Gitee gist document
- writing updated bundled files back to a gist
- PAT auth is passed into the API layer

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test vault_provider_gitee_spec --test vault_sync_engine_spec -q
```

Expected:

- FAIL because Gitee provider still returns placeholder “not wired yet” errors

**Step 3: Write minimal implementation**

- add a small `GiteeGistApi` abstraction mirroring the GitHub provider shape
- implement gist read/update requests with `reqwest`
- parse and write the bundled files layout expected by the sync engine

**Step 4: Run tests to verify they pass**

Run the same command from Step 2.

**Step 5: Commit**

```bash
git add src/app/vault/provider/gitee_gist.rs tests/vault_provider_gitee_spec.rs tests/vault_sync_engine_spec.rs
git commit -m "feat: wire gitee gist sync provider"
```

### Task 5: Trigger auto-sync after unlock and after local asset mutations

**Files:**
- Modify: `src/app/bootstrap.rs`
- Test: `tests/bootstrap_smoke.rs`
- Test: `tests/sync_vault_modal_smoke.rs`

**Step 1: Write the failing tests**

Add coverage for:

- successful unlock triggers a sync when auto-sync is enabled
- asset create/edit/delete flows trigger sync when auto-sync is enabled and vault is unlocked
- auto-sync failures update sync status without forcing the settings modal open

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test bootstrap_smoke --test sync_vault_modal_smoke -q
```

Expected:

- FAIL because bootstrap does not trigger sync automatically after unlock or local mutations

**Step 3: Write minimal implementation**

- add a helper that conditionally syncs when the bundle has `auto_sync_enabled`
- call it after unlock and after successful asset mutation commits
- keep manual titlebar sync as an explicit override path

**Step 4: Run tests to verify they pass**

Run the same command from Step 2.

**Step 5: Commit**

```bash
git add src/app/bootstrap.rs tests/bootstrap_smoke.rs tests/sync_vault_modal_smoke.rs
git commit -m "feat: auto sync after unlock and asset changes"
```

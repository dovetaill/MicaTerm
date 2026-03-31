# Asset Sync Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace the current lock/unlock-heavy vault flow with always-on background asset sync that auto-recovers after restart, auto-decides push vs pull using durable sync metadata, and prunes remote revision history to the latest 10 revisions.

**Architecture:** Keep the existing `AppWindow -> bootstrap -> ShellViewModel -> vault/provider` pipeline, but split the work into four explicit layers: UI contract cleanup, durable local sync state, automatic sync decision + recovery backup, and provider-side retention cleanup. Reuse the shared credential store for vault auto-recovery key material, keep the snapshot-based vault format, and land the behavioral change through TDD so the new sync contract replaces the old locked modal contract instead of coexisting with it.

**Tech Stack:** Rust, Slint, Tokio timers, serde, bincode, existing `CredentialStore` chain (`SystemCredentialStore` + encrypted/file fallback), current vault provider modules (`gitee_gist`, `github_gist`, `s3`), existing smoke/unit test suites.

---

## Execution Notes

- Use `@test-driven-development` for every task.
- Use `@verification-before-completion` before claiming any task is done.
- Keep changes small and compile/test after every behavioral slice.
- Prefer adding helpers and new modules over making `src/app/bootstrap.rs` larger.

### Task 1: Lock The New Sync UX Contract In Tests

**Files:**
- Modify: `src/shell/view_model.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `ui/components/sync-vault-modal.slint`
- Modify: `ui/shell/titlebar.slint`
- Test: `tests/sync_vault_modal_smoke.rs`
- Test: `tests/top_status_bar_smoke.rs`
- Test: `tests/assets_modal_render_spec.rs`
- Test: `tests/top_status_bar_ui_contract_smoke.sh`

**Step 1: Write the failing tests**

Add or replace tests so they lock the new user-facing contract:

```rust
#[test]
fn sync_modal_never_enters_locked_mode_after_sync_is_enabled() {
    // enable sync once
    // reopen modal
    // assert modal mode stays ready/config-only and never returns "locked"
}

#[test]
fn restart_with_saved_sync_configuration_does_not_require_unlock() {
    // enable sync, persist runtime state, recreate app
    // assert titlebar sync does not route into unlock wording
}

#[test]
fn titlebar_sync_keeps_immediate_action_semantics_after_configuration() {
    // once sync is configured, invoke_sync_now_requested() should perform sync/check
    // instead of opening a lock/unlock flow
}
```

Also update render and contract tests so they stop expecting:

- `"Unlock"`
- `"Lock"`
- `SyncModalMode::Locked`
- `SyncModalMode::UnlockedButRemoteIncomplete`
- the auto-sync toggle row in the first-release UI

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test sync_vault_modal_smoke sync_modal_never_enters_locked_mode_after_sync_is_enabled -- --exact
cargo test --test top_status_bar_smoke titlebar_sync_keeps_immediate_action_semantics_after_configuration -- --exact
cargo test --test assets_modal_render_spec sync_modal_ -- --nocapture
bash tests/top_status_bar_ui_contract_smoke.sh
```

Expected:

- Rust tests fail because the current code still exposes `locked` mode and `Unlock` labels.
- The shell contract test fails because the old button/modal strings are still present.

**Step 3: Write the minimal implementation**

Implement the smallest UI contract change that makes the new tests meaningful:

```rust
pub enum SyncModalMode {
    NotConfigured,
    Ready,
    SyncError,
}

pub struct SyncModalViewState {
    pub open: bool,
    pub mode: SyncModalMode,
    pub title: String,
    pub headline: String,
    pub status_text: String,
    pub error_text: String,
    pub primary_action_label: String,
    pub secondary_action_label: String,
    // remove auto_sync_enabled and lock/unlock-driven fields from the first-release contract
}
```

Implementation checklist:

- Remove `Locked` and `UnlockedButRemoteIncomplete` from the visible `SyncModalMode`.
- Remove `auto_sync_enabled` from the first-release modal contract.
- Update the titlebar `Sync` action so the configured state always routes to immediate sync/check semantics.
- Keep the modal for configuration and diagnostics only; do not expose explicit `Lock` / `Unlock`.

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test sync_vault_modal_smoke -- --nocapture
cargo test --test top_status_bar_smoke -- --nocapture
cargo test --test assets_modal_render_spec -- --nocapture
bash tests/top_status_bar_ui_contract_smoke.sh
```

Expected:

- All sync modal and titlebar contract tests pass with no visible lock/unlock path left in the UI.

**Step 5: Commit**

```bash
git add src/shell/view_model.rs src/app/bootstrap.rs ui/components/sync-vault-modal.slint ui/shell/titlebar.slint tests/sync_vault_modal_smoke.rs tests/top_status_bar_smoke.rs tests/assets_modal_render_spec.rs tests/top_status_bar_ui_contract_smoke.sh
git commit -m "test: lock always-on sync ui contract"
```

### Task 2: Persist Vault Auto-Recovery Key Material

**Files:**
- Modify: `src/app/vault/bootstrap.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `src/app/ssh/credentials.rs`
- Test: `tests/vault_bootstrap_spec.rs`
- Test: `tests/bootstrap_smoke.rs`

**Step 1: Write the failing tests**

Add tests that prove sync can recover after restart without re-entering a master password:

```rust
#[test]
fn enabling_sync_persists_runtime_vault_key_material() {
    // enable sync once
    // assert runtime key credential ref exists in the credential store
}

#[test]
fn restart_recovers_vault_session_without_prompting_for_unlock() {
    // enable sync, recreate app with same runtime root + credential store
    // assert sync is immediately ready and asset projection is available
}
```

Add a bootstrap-level test for the helper API:

```rust
#[test]
fn runtime_vault_key_round_trips_through_credential_store() {
    // persist [u8; 32], load it back, compare bytes
}
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test vault_bootstrap_spec runtime_vault_key_round_trips_through_credential_store -- --exact
cargo test --test bootstrap_smoke restart_recovers_vault_session_without_prompting_for_unlock -- --exact
```

Expected:

- Tests fail because no runtime vault key helper or restart recovery path exists yet.

**Step 3: Write the minimal implementation**

Add explicit credential-store helpers for runtime auto-recovery:

```rust
pub fn vault_runtime_key_credential_ref(vault_id: &str) -> String {
    format!("vault/runtime-key/{vault_id}")
}

pub fn persist_runtime_vault_key(
    store: &dyn CredentialStore,
    vault_id: &str,
    key: &[u8; 32],
) -> Result<()>;

pub fn load_runtime_vault_key(
    store: &dyn CredentialStore,
    vault_id: &str,
) -> Result<Option<[u8; 32]>>;
```

Implementation checklist:

- Persist runtime vault key material when sync is first enabled or remote recovery succeeds.
- Reuse `shared_app_credential_store()` so the data lands in the existing system/fallback chain.
- On app bootstrap, if local vault state and runtime key both exist, attempt silent cache decryption before exposing modal state.
- If silent recovery fails, clear the saved runtime key material and surface a sync diagnostic instead of looping forever.

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test vault_bootstrap_spec -- --nocapture
cargo test --test bootstrap_smoke enabling_sync_persists_runtime_vault_key_material -- --exact
cargo test --test bootstrap_smoke restart_recovers_vault_session_without_prompting_for_unlock -- --exact
```

Expected:

- Runtime key helpers round-trip correctly.
- A restarted app recovers the configured vault session without explicit unlock UI.

**Step 5: Commit**

```bash
git add src/app/vault/bootstrap.rs src/app/bootstrap.rs src/app/ssh/credentials.rs tests/vault_bootstrap_spec.rs tests/bootstrap_smoke.rs
git commit -m "feat: persist vault auto-recovery key material"
```

### Task 3: Add Durable Local Sync State And New Remote Head Metadata

**Files:**
- Modify: `src/app/vault/model.rs`
- Modify: `src/app/vault/bootstrap.rs`
- Modify: `src/app/vault/engine.rs`
- Modify: `src/app/bootstrap.rs`
- Test: `tests/vault_model_spec.rs`
- Test: `tests/vault_sync_engine_spec.rs`
- Test: `tests/bootstrap_smoke.rs`

**Step 1: Write the failing tests**

Add model and engine tests that lock the new metadata shape:

```rust
#[test]
fn vault_head_serializes_real_commit_metadata() {
    // encode/decode head with committed_at + committed_by_device
}

#[test]
fn local_bootstrap_state_persists_durable_sync_state_fields() {
    // save/load LocalVaultBootstrapState and assert new fields survive round-trip
}

#[test]
fn sync_engine_writes_committed_metadata_instead_of_legacy_created_at() {
    // build SyncRequest and assert report.head contains committed_at
}
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test vault_model_spec vault_head_serializes_real_commit_metadata -- --exact
cargo test --test vault_sync_engine_spec sync_engine_writes_committed_metadata_instead_of_legacy_created_at -- --exact
```

Expected:

- Tests fail because `VaultHead` still exposes `created_at` and local sync state fields do not exist.

**Step 3: Write the minimal implementation**

Extend the schemas and remove the old fixed timestamp path:

```rust
pub struct VaultHead {
    pub format_version: u32,
    pub vault_id: String,
    pub vault_revision: String,
    pub parent_revision: Option<String>,
    pub device_id: String,
    pub committed_at: String,
    pub committed_by_device: String,
    pub payload_hash: String,
    pub manifest_ref: String,
    pub wrapped_vault_key: String,
    // ...
}

pub struct LocalVaultBootstrapState {
    pub bundle: BootstrapBundle,
    pub wrapped_vault_key: String,
    pub kdf: KdfConfig,
    pub current_revision: Option<String>,
    pub local_snapshot_hash: Option<String>,
    pub last_local_change_at: Option<String>,
    pub last_successful_push_at: Option<String>,
    pub last_successful_pull_at: Option<String>,
    pub last_sync_error: Option<String>,
}
```

Implementation checklist:

- Replace the fixed `created_at: "2026-03-28T00:00:00Z"` write path with a real timestamp helper.
- Preserve backward read compatibility if old local bootstrap files still contain only the legacy fields.
- Update `SyncRequest` and `SyncReport` to carry the new head metadata explicitly.

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test vault_model_spec -- --nocapture
cargo test --test vault_sync_engine_spec -- --nocapture
cargo test --test bootstrap_smoke missing_local_vault_state_recovers_from_primary_remote_without_uploading_empty_data -- --exact
```

Expected:

- Model round-trips pass.
- Sync engine writes real commit metadata.
- Remote recovery still works with the upgraded schema.

**Step 5: Commit**

```bash
git add src/app/vault/model.rs src/app/vault/bootstrap.rs src/app/vault/engine.rs src/app/bootstrap.rs tests/vault_model_spec.rs tests/vault_sync_engine_spec.rs tests/bootstrap_smoke.rs
git commit -m "feat: add durable sync metadata state"
```

### Task 4: Build Automatic Sync Decision And Recovery Backup

**Files:**
- Create: `src/app/vault/sync_decision.rs`
- Create: `src/app/vault/recovery.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `src/app/vault/mod.rs`
- Test: `tests/vault_sync_decision_spec.rs`
- Test: `tests/bootstrap_smoke.rs`

**Step 1: Write the failing tests**

Create a dedicated decision test file:

```rust
#[test]
fn newer_local_snapshot_pushes_and_stashes_remote_backup() {
    // same base revision, both changed, local committed_at newer
    // expect Push + remote backup request
}

#[test]
fn newer_remote_revision_pulls_and_stashes_local_backup() {
    // same base revision, both changed, remote committed_at newer
    // expect Pull + local backup request
}

#[test]
fn identical_hashes_short_circuit_to_noop() {
    // same payload hash => no sync action
}
```

Add a bootstrap smoke test that verifies the losing side is written to recovery before replacement.

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test vault_sync_decision_spec -- --nocapture
cargo test --test bootstrap_smoke recovery_ -- --nocapture
```

Expected:

- The new unit tests fail because there is no decision engine or recovery persistence module.

**Step 3: Write the minimal implementation**

Create explicit decision and recovery modules instead of embedding more policy into `bootstrap.rs`:

```rust
pub enum SyncAction {
    Noop,
    Push,
    Pull,
}

pub struct SyncDecision {
    pub action: SyncAction,
    pub backup_local_snapshot: bool,
    pub backup_remote_snapshot: bool,
}

pub fn decide_sync_action(local: &LocalSyncState, remote: Option<&VaultHead>) -> SyncDecision;
```

Implementation checklist:

- Compare `base_revision`, `local_snapshot_hash`, `payload_hash`, and `committed_at`.
- Before `Pull` replaces local projection, persist the current local snapshot into a recovery directory.
- Before `Push` overwrites an older remote head, persist the pulled remote snapshot locally as a recovery artifact.
- Keep the recovery format simple and local-only for this phase; do not add remote recovery UX yet.

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test vault_sync_decision_spec -- --nocapture
cargo test --test bootstrap_smoke -- --nocapture
```

Expected:

- The decision unit tests pass.
- End-to-end smoke coverage shows no silent overwrite without a local recovery artifact.

**Step 5: Commit**

```bash
git add src/app/vault/sync_decision.rs src/app/vault/recovery.rs src/app/vault/mod.rs src/app/bootstrap.rs tests/vault_sync_decision_spec.rs tests/bootstrap_smoke.rs
git commit -m "feat: add automatic sync decision and recovery backup"
```

### Task 5: Replace The Current Scheduler With Always-On Background Sync

**Files:**
- Modify: `src/app/bootstrap.rs`
- Modify: `src/shell/view_model.rs`
- Modify: `ui/components/sync-vault-modal.slint`
- Test: `tests/bootstrap_smoke.rs`
- Test: `tests/sync_vault_modal_smoke.rs`
- Test: `tests/top_status_bar_smoke.rs`

**Step 1: Write the failing tests**

Add or update tests that encode the always-on scheduler contract:

```rust
#[test]
fn asset_mutation_syncs_without_auto_sync_toggle() {
    // configure sync once
    // mutate asset
    // settle timer
    // assert push happened
}

#[test]
fn periodic_sync_pulls_remote_changes_even_without_local_dirty_state() {
    // no local change, remote head advanced
    // periodic timer should pull
}

#[test]
fn titlebar_sync_forces_foreground_check_when_scheduler_is_idle() {
    // invoke_sync_now_requested()
    // assert immediate foreground sync/check feedback
}
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test bootstrap_smoke asset_mutation_syncs_without_auto_sync_toggle -- --exact
cargo test --test bootstrap_smoke periodic_sync_pulls_remote_changes_even_without_local_dirty_state -- --exact
cargo test --test top_status_bar_smoke titlebar_sync_forces_foreground_check_when_scheduler_is_idle -- --exact
```

Expected:

- Tests fail because the current scheduler still gates on `auto_sync_enabled` and visible lock/unlock state.

**Step 3: Write the minimal implementation**

Refactor the scheduler contract instead of stacking more conditions onto `vault_auto_sync_ready()`:

```rust
fn vault_background_sync_ready(vault: &VaultSessionState) -> bool {
    vault.local_state.is_some() && vault.runtime_vault_key_available()
}

fn mark_local_vault_dirty_and_arm_sync(...) {
    scheduler.borrow_mut().dirty = true;
    debounce_timer.start(...);
}
```

Implementation checklist:

- Remove the `auto_sync_enabled` gate from first-release behavior.
- Treat successful asset writes as always-on dirty events once sync is configured.
- Keep the short debounce for push and the repeated periodic timer for pull/retry.
- Make `Sync now` bypass debounce and force foreground execution.

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test bootstrap_smoke -- --nocapture
cargo test --test sync_vault_modal_smoke -- --nocapture
cargo test --test top_status_bar_smoke -- --nocapture
```

Expected:

- Asset writes trigger background sync without any modal toggle dependency.
- Periodic pull/retry behavior still works.
- Titlebar feedback remains correct.

**Step 5: Commit**

```bash
git add src/app/bootstrap.rs src/shell/view_model.rs ui/components/sync-vault-modal.slint tests/bootstrap_smoke.rs tests/sync_vault_modal_smoke.rs tests/top_status_bar_smoke.rs
git commit -m "feat: make asset sync always-on in background"
```

### Task 6: Add Bounded Revision Retention To Providers

**Files:**
- Modify: `src/app/vault/provider/mod.rs`
- Modify: `src/app/vault/provider/gitee_gist.rs`
- Modify: `src/app/vault/provider/github_gist.rs`
- Modify: `src/app/vault/provider/s3.rs`
- Modify: `src/app/vault/provider/mock.rs`
- Test: `tests/vault_provider_gitee_spec.rs`
- Test: `tests/vault_provider_github_spec.rs`
- Test: `tests/vault_provider_s3_spec.rs`
- Test: `tests/vault_sync_engine_spec.rs`

**Step 1: Write the failing tests**

Add provider-level retention tests:

```rust
#[test]
fn gitee_provider_prunes_revisions_older_than_keep_latest_limit() {
    // seed >10 revisions, run retention, assert only latest 10 remain
}

#[test]
fn github_provider_prunes_revisions_older_than_keep_latest_limit() {
    // same contract for github gist
}

#[test]
fn s3_provider_prunes_revision_objects_older_than_keep_latest_limit() {
    // same contract for object-set provider
}
```

Add an engine-level test that retention runs after a successful sync report.

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test vault_provider_gitee_spec prune_ -- --nocapture
cargo test --test vault_provider_github_spec prune_ -- --nocapture
cargo test --test vault_provider_s3_spec prune_ -- --nocapture
cargo test --test vault_sync_engine_spec retention_ -- --nocapture
```

Expected:

- Tests fail because the provider trait has no retention hook and providers never delete old revisions.

**Step 3: Write the minimal implementation**

Extend the provider contract with a post-write retention hook:

```rust
pub trait VaultProvider: Send + Sync {
    fn remote_id(&self) -> &str;
    fn capabilities(&self) -> ProviderCapabilities;
    fn read_head(&self) -> Result<ProviderReadResult>;
    fn read_revision(&self, head: &VaultHead) -> Result<ProviderRevision>;
    fn write_revision(&self, request: &ProviderWriteRequest) -> Result<()>;
    fn prune_revisions(&self, keep_latest: usize, live_head: &VaultHead) -> Result<()>;
}
```

Implementation checklist:

- Call `prune_revisions(10, &report.head)` after primary write success and after successful mirror writes.
- For gist providers, derive revision sets from gist filenames.
- For S3, derive old revision object sets from manifest/head object naming.
- Make retention best-effort for mirrors; primary retention failures should surface as sync errors.

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test vault_provider_gitee_spec -- --nocapture
cargo test --test vault_provider_github_spec -- --nocapture
cargo test --test vault_provider_s3_spec -- --nocapture
cargo test --test vault_sync_engine_spec -- --nocapture
```

Expected:

- All provider suites confirm only the latest 10 revisions remain after retention.

**Step 5: Commit**

```bash
git add src/app/vault/provider/mod.rs src/app/vault/provider/gitee_gist.rs src/app/vault/provider/github_gist.rs src/app/vault/provider/s3.rs src/app/vault/provider/mock.rs tests/vault_provider_gitee_spec.rs tests/vault_provider_github_spec.rs tests/vault_provider_s3_spec.rs tests/vault_sync_engine_spec.rs
git commit -m "feat: prune retained vault revisions"
```

### Task 7: Run Full Regression And Remove Old Contract Debris

**Files:**
- Modify: `tests/bootstrap_smoke.rs`
- Modify: `tests/sync_vault_modal_smoke.rs`
- Modify: `tests/assets_modal_render_spec.rs`
- Modify: `tests/top_status_bar_smoke.rs`
- Modify: `tests/top_status_bar_ui_contract_smoke.sh`
- Modify: `docs/plans/2026-03-31-asset-sync-design.md` only if terminology drift needs correction

**Step 1: Write the final regression assertions**

Add or update final regression coverage for:

```rust
#[test]
fn restart_then_manual_sync_runs_without_password_prompt() {
    // enable sync, restart app, invoke sync now, assert no modal prompt
}

#[test]
fn remote_newer_revision_pulls_after_periodic_tick_and_preserves_local_recovery_copy() {
    // seed local + remote divergence and assert automatic pull + backup
}
```

**Step 2: Run the full regression set**

Run:

```bash
cargo test --test vault_bootstrap_spec -- --nocapture
cargo test --test vault_model_spec -- --nocapture
cargo test --test vault_sync_engine_spec -- --nocapture
cargo test --test vault_sync_decision_spec -- --nocapture
cargo test --test vault_provider_gitee_spec -- --nocapture
cargo test --test vault_provider_github_spec -- --nocapture
cargo test --test vault_provider_s3_spec -- --nocapture
cargo test --test sync_vault_modal_smoke -- --nocapture
cargo test --test bootstrap_smoke -- --nocapture
cargo test --test top_status_bar_smoke -- --nocapture
cargo test --test assets_modal_render_spec -- --nocapture
bash tests/top_status_bar_ui_contract_smoke.sh
```

Expected:

- No remaining test expects visible lock/unlock behavior.
- No remaining test depends on the first-release auto-sync toggle.
- Background sync, restart recovery, retention, and recovery backup all pass together.

**Step 3: Remove obsolete code paths**

Delete old contract debris only after the full suite is green:

```rust
// remove obsolete:
// - SyncModalMode::Locked
// - SyncModalMode::UnlockedButRemoteIncomplete
// - lock_local_vault(...) call sites used only for visible UI contract
// - first-release auto_sync_enabled modal bindings
```

**Step 4: Re-run the targeted regressions**

Run:

```bash
cargo test --test sync_vault_modal_smoke -- --nocapture
cargo test --test bootstrap_smoke -- --nocapture
bash tests/top_status_bar_ui_contract_smoke.sh
```

Expected:

- The targeted suite remains green after dead-code removal.

**Step 5: Commit**

```bash
git add tests/bootstrap_smoke.rs tests/sync_vault_modal_smoke.rs tests/assets_modal_render_spec.rs tests/top_status_bar_smoke.rs tests/top_status_bar_ui_contract_smoke.sh src/app/bootstrap.rs src/shell/view_model.rs ui/components/sync-vault-modal.slint
git commit -m "refactor: remove legacy sync lock contract"
```

## Final Verification Checklist

- Sync is configured once and persists across restart.
- No visible `Lock` / `Unlock` remains in the first-release sync flow.
- Titlebar `Sync` is always an immediate sync/check action.
- Local mutation triggers background sync without a modal toggle.
- Periodic timer can pull remote changes and retry failed push attempts.
- Automatic decision picks push or pull without asking the user.
- The losing side is written to local recovery storage before replacement.
- `vault-head.json` contains real commit metadata instead of the old fixed timestamp contract.
- Gitee, GitHub, and S3 providers all prune history to the latest 10 revisions.

## Suggested Execution Order

1. Task 1
2. Task 2
3. Task 3
4. Task 4
5. Task 5
6. Task 6
7. Task 7

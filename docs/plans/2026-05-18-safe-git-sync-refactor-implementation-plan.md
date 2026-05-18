# Safe Git Sync Refactor Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Incrementally land a safe, private-only Git repository sync flow for vault data using the existing vault / snapshot / provider / bootstrap / sync service architecture.

**Architecture:** Keep `ProviderKind::GitRepo` as the transport provider, but introduce a structured Git remote model plus a fail-closed repository validation layer for GitHub, GitLab, and Gitee. Route manual sync, debounced auto sync, periodic refresh, and sync-modal refresh through one `VaultSyncService` gate so push decisions always consult the latest remote safety state. Extend snapshot coverage to all cross-device user assets while excluding UI preferences, known hosts, local sync metadata, and provider credentials; keep encrypted payloads end-to-end and encrypt recovery persistence too.

**Tech Stack:** Rust, cargo test, git2, serde, existing vault bootstrap/snapshot/provider infrastructure, Slint UI.

---

### Task 0: Baseline and isolation

**Files:**
- Read: `AGENTS.md`
- Read: `src/app/vault/model.rs`
- Read: `src/app/vault/provider/mod.rs`
- Read: `src/app/vault/provider/git_repo.rs`
- Read: `src/app/vault/provider/gitee_gist.rs`
- Read: `src/app/vault/provider/github_gist.rs`
- Read: `src/app/vault/provider/gitlab_snippet.rs`
- Read: `src/app/vault/snapshot.rs`
- Read: `src/app/vault/recovery.rs`
- Read: `src/app/vault/bootstrap.rs`
- Read: `src/app/vault/sync_service.rs`
- Read: `src/app/bootstrap.rs`
- Read: `src/app/bootstrap/vault_sync.rs`
- Read: `src/shell/view_model.rs`
- Read: `src/shell/view_model/projection.rs`
- Read: `src/app/bootstrap/windowing.rs`
- Read: `ui/components/sync-vault-modal.slint`
- Read: `ui/app-window.slint`

**Step 1:** Work from a dedicated worktree rooted at `origin/master`.

**Step 2:** Verify baseline with:
- `cargo build -q`
- `cargo test -q`

**Step 3:** Record findings before code changes.

### Task 1: Provider validation tests first

**Files:**
- Modify: `tests/vault_provider_github_spec.rs`
- Modify: `tests/vault_provider_gitlab_spec.rs`
- Modify: `tests/vault_provider_gitee_spec.rs`
- Modify if needed: `tests/vault_provider_git_repo_spec.rs`
- Modify if needed: `src/app/vault/provider/mock.rs`

**Step 1: Write failing tests**
- `github_public_repo_is_rejected`
- `github_private_repo_is_accepted_when_writable`
- `github_private_repo_without_push_permission_is_rejected`
- `github_unknown_visibility_fails_closed`
- `gitlab_public_repo_is_rejected`
- `gitlab_internal_repo_is_rejected`
- `gitlab_private_repo_is_accepted_when_writable`
- `gitee_public_repo_is_rejected`
- `gitee_private_repo_is_accepted_when_writable`
- `configured_remote_revalidated_before_push_if_safety_status_stale`

**Step 2: Run tests to verify red**
- `cargo test --test vault_provider_github_spec -q`
- `cargo test --test vault_provider_gitlab_spec -q`
- `cargo test --test vault_provider_gitee_spec -q`

**Step 3: Commit**
- `git add -A`
- `git commit -m "test: add private repository validation specs"`

### Task 2: Minimal provider/model implementation

**Files:**
- Modify: `src/app/vault/model.rs`
- Modify: `src/app/vault/provider/mod.rs`
- Modify: `src/app/vault/provider/git_repo.rs`
- Add if needed: `src/app/vault/provider/git_host.rs`
- Add if needed: `src/app/vault/provider/git_repo_validation.rs`
- Modify if needed: `tests/vault_provider_git_repo_spec.rs`

**Step 1: Write/adjust failing contract tests for structured config and validation results**
- Prefer one test per rule: visibility mapping, writable mapping, stale safety revalidation.

**Step 2: Run targeted tests to verify red**
- `cargo test --test vault_provider_github_spec -q`
- `cargo test --test vault_provider_gitlab_spec -q`
- `cargo test --test vault_provider_gitee_spec -q`
- `cargo test --test vault_provider_git_repo_spec -q`

**Step 3: Write minimal implementation**
- Add structured Git remote locator/config fields beyond raw `remote_url`.
- Add fail-closed metadata model for visibility and write capability.
- Implement `fetch_repository_metadata(...)`, `ensure_private_repository(...)`, `ensure_writable(...)`, and `validate_remote_for_sync(...)`.
- Keep `ProviderKind::GitRepo`; branch by host/provider subtype.

**Step 4: Re-run the targeted tests until green**

**Step 5: Commit**
- `git add -A`
- `git commit -m "feat: validate git remotes before enabling sync"`

### Task 3: Unify sync safety gate and background scheduling

**Files:**
- Modify: `tests/vault_sync_service_spec.rs`
- Modify: `src/app/vault/sync_service.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `src/app/bootstrap/vault_sync.rs`
- Modify: `src/app/vault/bootstrap.rs`

**Step 1: Write failing tests**
- `opening_sync_modal_refreshes_remote_head_without_blocking`
- `remote_changed_to_public_pauses_sync`
- `local_mutation_does_not_push_to_unsafe_remote`
- `manual_sync_fails_closed_when_visibility_cannot_be_checked`
- `periodic_sync_retries_after_remote_revalidated_private`

**Step 2: Run red tests**
- `cargo test --test vault_sync_service_spec -q`

**Step 3: Write minimal implementation**
- Extend durable state with `base_revision`, `local_snapshot_hash`, `last_local_change_at`, `last_successful_push_at`, `last_successful_pull_at`, `last_sync_error`, `remote_safety_status`.
- Route manual sync, debounced auto sync, periodic refresh, and modal refresh through `VaultSyncService`.
- Revalidate before push, especially when safety status is stale.
- Pause sync on public/internal/unknown/unreadable/unwritable remotes.

**Step 4: Re-run sync tests and nearby smoke tests**

**Step 5: Commit**
- `git add -A`
- `git commit -m "refactor: unify background vault sync scheduling"`

### Task 4: Expand snapshot scope and exclude forbidden data

**Files:**
- Modify: `tests/vault_snapshot_spec.rs`
- Modify: `src/app/vault/model.rs`
- Modify: `src/app/vault/snapshot.rs`
- Modify: `src/app/bootstrap/vault_sync.rs`
- Modify: `src/shell/view_model.rs`
- Modify: `src/shell/view_model/projection.rs`

**Step 1: Write failing tests**
- `vault_snapshot_includes_all_user_sync_assets`
- `vault_snapshot_excludes_ui_preferences`
- `known_hosts_is_excluded_until_trust_policy_exists`
- `restore_rebuilds_asset_snippet_keychain_projection`
- `lock_clears_decrypted_asset_snippet_keychain_projection`
- `round_trip_ssh_connection_password_identity_keychain_snippet_folder_structure`

**Step 2: Run red tests**
- `cargo test --test vault_snapshot_spec -q`

**Step 3: Write minimal implementation**
- Include SSH assets, secret bundles, keychain catalog, keychain secrets, snippets, folder hierarchy/order/grouping, and required asset metadata.
- Exclude `ui_preferences`, `known_hosts`, local sync metadata, PAT/provider credentials.
- Ensure restore and lock clear projections consistently.

**Step 4: Re-run snapshot tests**

**Step 5: Commit**
- `git add -A`
- `git commit -m "fix: align vault snapshot sync scope"`

### Task 5: Encrypt recovery and fix retention behavior

**Files:**
- Modify: `src/app/vault/recovery.rs`
- Modify: `src/app/vault/provider/git_repo.rs`
- Modify: `src/app/vault/provider/mod.rs`
- Modify: `src/app/vault/engine.rs`
- Modify related tests in provider/sync/bootstrap specs

**Step 1: Write failing tests**
- `provider_keeps_latest_10_revisions`
- `cleanup_never_deletes_current_head_revision`
- `cleanup_failure_records_error_but_does_not_corrupt_successful_push`
- Add focused encrypted-recovery assertions where current smoke tests already cover recovery persistence.

**Step 2: Run red tests**
- targeted `cargo test --test ... -q` commands for updated suites

**Step 3: Write minimal implementation**
- Encrypt recovery snapshot persistence.
- Retain latest 10 revisions by default.
- Treat cleanup failure as degraded state, not push failure.

**Step 4: Re-run targeted tests**

**Step 5: Commit**
- `git add -A`
- `git commit -m "fix: encrypt recovery snapshots and soften retention cleanup failures"`

### Task 6: Sync modal and view-model updates

**Files:**
- Modify: `tests/sync_vault_modal_smoke.rs`
- Modify: `src/shell/view_model.rs`
- Modify: `src/shell/view_model/projection.rs`
- Modify: `src/app/bootstrap/windowing.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `src/app/bootstrap/vault_sync.rs`
- Modify: `ui/components/sync-vault-modal.slint`
- Modify: `ui/app-window.slint`

**Step 1: Write failing tests**
- `public_repo_validation_error_is_visible`
- `gitlab_internal_repo_validation_error_is_visible`
- `private_repo_validation_success_enables_setup`
- `sync_modal_security_pause_state_is_visible`

**Step 2: Run red tests**
- `cargo test --test sync_vault_modal_smoke -q`

**Step 3: Write minimal implementation**
- Replace the single `git_remote_url` model with structured provider config fields.
- Add provider selection for Gitee / GitHub / GitLab.
- Add background Validate flow plus visible validating/success/blocking states.
- Show paused state when the remote becomes unsafe; keep local data and block push.

**Step 4: Re-run modal smoke tests**

**Step 5: Commit**
- `git add -A`
- `git commit -m "feat: expose safe git sync validation in sync modal"`

### Task 7: Bootstrap integration and smoke coverage

**Files:**
- Modify: `tests/bootstrap_smoke.rs`
- Modify any touched bootstrap files only as required by failing tests

**Step 1: Write failing smoke tests**
- private repo happy path
- remote flips to public then pauses
- periodic refresh revalidates
- modal refresh stays backgrounded
- old-config compatibility if needed

**Step 2: Run red smoke test subset**
- `cargo test --test bootstrap_smoke -q`

**Step 3: Write minimal implementation**
- Wire validation, scheduling, restore, and paused-state behavior through the real bootstrap path.

**Step 4: Re-run smoke tests**

**Step 5: Commit**
- `git add -A`
- `git commit -m "test: cover safe git sync bootstrap flow"`

### Task 8: Final verification

**Files:**
- No intentional source edits unless verification failures require follow-up fixes

**Step 1: Run required acceptance commands**
- `cargo fmt --check`
- `cargo check`
- `cargo test --test vault_provider_gitee_spec -q`
- `cargo test --test vault_provider_github_spec -q`
- `cargo test --test vault_provider_gitlab_spec -q`
- `cargo test --test vault_snapshot_spec -q`
- `cargo test --test vault_sync_service_spec -q`
- `cargo test --test sync_vault_modal_smoke -q`
- `cargo test --test bootstrap_smoke -q`
- `bash tests/top_status_bar_ui_contract_smoke.sh`
- `bash tests/vault_settings_ui_contract_smoke.sh`

**Step 2:** If anything fails, add the smallest fix with a fresh commit.

**Step 3:** Report exact results, file list, provider support status, explicit non-support for OAuth unless fully implemented, and explicit ban on public repositories as sync remotes.

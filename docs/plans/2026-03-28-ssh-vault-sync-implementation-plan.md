# SSH Vault Sync Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build an end-to-end encrypted personal SSH vault that can sync SSH assets and secrets across devices, using one primary remote plus optional mirror remotes backed by S3-compatible object storage, GitHub Gists, GitLab Snippets, and Gitee Gists.

**Architecture:** Add a new `app::vault` subsystem with a clear split between `BootstrapBundle` and encrypted vault data. The vault uses `VaultHead -> VaultManifest -> EncryptedPack[]`, a master-password-derived KEK, and a randomly generated vault key. The sync engine writes to one primary remote with revision checks, then fans out the same ciphertext to mirror remotes. The UI surfaces this through a dedicated `Sync & Vault` settings panel in the existing right-side panel instead of overloading SSH asset modals.

**Tech Stack:** Rust, Slint, redb, keyring, Argon2id, XChaCha20-Poly1305, zstd, zeroize, reqwest, oauth2, AWS SDK for S3, Tokio, cargo test

**Execution Rules:** Use `@superpowers:test-driven-development` on every task. Before declaring the feature done, run `@superpowers:verification-before-completion`.

---

### Task 1: Lock the vault domain model and bootstrap schema with failing tests

**Files:**
- Create: `tests/vault_model_spec.rs`
- Create: `src/app/vault/mod.rs`
- Create: `src/app/vault/model.rs`
- Modify: `src/app/mod.rs`
- Reference: `src/shell/assets.rs`
- Reference: `src/app/ssh/credentials.rs`
- Reference: `src/app/ui_preferences.rs`

**Step 1: Write the failing vault model tests**

In `tests/vault_model_spec.rs`, add round-trip tests for:

- `VaultHead` with `vault_revision`, `parent_revision`, `payload_hash`, `manifest_ref`, and `wrapped_vault_key`;
- `VaultManifest` with multiple `PackRef` entries;
- `VaultSnapshot` containing:
  - asset catalog data
  - SSH secret bundles
  - known hosts
  - sync-related UI preferences;
- `BootstrapBundle` with:
  - one primary remote
  - one mirror remote
  - provider credential references.

Use concrete fields such as:

```rust
assert_eq!(bundle.remotes[0].role, RemoteRole::Primary);
assert_eq!(head.vault_revision, "rev-0001");
```

**Step 2: Run the focused tests to confirm failure**

Run:

```bash
cargo test --test vault_model_spec -- --nocapture
```

Expected:

- FAIL because the `app::vault` module and the new types do not exist yet.

**Step 3: Implement the minimal vault model**

Create `src/app/vault/model.rs` with the initial serializable types:

- `VaultHead`
- `VaultManifest`
- `PackRef`
- `VaultSnapshot`
- `BootstrapBundle`
- `BootstrapRemoteConfig`
- `RemoteRole`
- `ProviderKind`

Keep these types pure data models only. Do not add sync logic yet.

**Step 4: Run the focused tests to verify pass**

Run:

```bash
cargo test --test vault_model_spec -- --nocapture
```

Expected:

- PASS for serde round-trips and schema defaults.

**Step 5: Commit**

```bash
git add src/app/mod.rs src/app/vault/mod.rs src/app/vault/model.rs tests/vault_model_spec.rs
git commit -m "feat: add ssh vault domain model"
```

### Task 2: Add the crypto envelope and encrypted local cache with failing tests

**Files:**
- Modify: `Cargo.toml`
- Create: `src/app/vault/crypto.rs`
- Create: `src/app/vault/cache.rs`
- Create: `tests/vault_crypto_spec.rs`
- Reference: `src/app/vault/model.rs`

**Step 1: Write the failing crypto tests**

In `tests/vault_crypto_spec.rs`, add tests that assert:

- `Argon2id`-derived KEK can wrap and unwrap a random vault key;
- `VaultSnapshot` content is compressed before encryption and decrypts back to the original data;
- a wrong password fails unwrap cleanly;
- `EncryptedCache` round-trips through disk without leaving plaintext markers behind.

Use expectations such as:

```rust
assert!(decrypt_with_password("wrong-pass", &blob).is_err());
assert!(!encoded.contains("private_key_content"));
```

**Step 2: Run the focused tests to confirm failure**

Run:

```bash
cargo test --test vault_crypto_spec -- --nocapture
```

Expected:

- FAIL because the crypto and cache modules do not exist yet.

**Step 3: Add the crypto dependencies**

Add the smallest justified dependency set in `Cargo.toml`:

- `argon2`
- `chacha20poly1305`
- `rand_core`
- `sha2`
- `zstd`
- `zeroize`
- `secrecy`

Do not add HTTP or provider dependencies in this task.

**Step 4: Implement the crypto and cache modules**

In `src/app/vault/crypto.rs`, add:

- password-to-KEK derivation
- vault-key wrapping and unwrapping
- snapshot compression and encryption helpers
- snapshot decryption and decompression helpers

In `src/app/vault/cache.rs`, add:

- encrypted cache file read/write helpers
- deterministic cache file naming by `vault_id`

Use `XChaCha20-Poly1305` for the AEAD, and record KDF parameters in the header types rather than hardcoding magic constants in the caller.

**Step 5: Run the focused tests to verify pass**

Run:

```bash
cargo test --test vault_crypto_spec -- --nocapture
```

Expected:

- PASS for wrap/unwrap, encrypt/decrypt, and encrypted cache persistence.

**Step 6: Commit**

```bash
git add Cargo.toml src/app/vault/crypto.rs src/app/vault/cache.rs tests/vault_crypto_spec.rs
git commit -m "feat: add vault crypto envelope"
```

### Task 3: Extract and apply vault snapshots from the current app state with failing tests

**Files:**
- Create: `src/app/vault/snapshot.rs`
- Create: `tests/vault_snapshot_spec.rs`
- Modify: `src/app/assets_catalog/mapper.rs`
- Modify: `src/app/ssh/credentials.rs`
- Modify: `src/app/ssh/known_hosts.rs`
- Modify: `src/app/ui_preferences.rs`
- Reference: `src/app/vault/model.rs`
- Reference: `src/shell/view_model.rs`

**Step 1: Write the failing snapshot tests**

In `tests/vault_snapshot_spec.rs`, add tests proving that:

- a small asset catalog plus SSH secret bundle exports into `VaultSnapshot`;
- importing a `VaultSnapshot` recreates the asset catalog, SSH secrets, known-hosts entries, and sync preferences;
- empty secrets remain absent instead of becoming whitespace-filled secret entries.

Include a concrete expectation like:

```rust
assert_eq!(snapshot.secret_bundles.len(), 1);
assert_eq!(snapshot.assets.roots.len(), 2);
```

**Step 2: Run the focused tests to confirm failure**

Run:

```bash
cargo test --test vault_snapshot_spec -- --nocapture
```

Expected:

- FAIL because there is no snapshot import/export boundary yet.

**Step 3: Implement snapshot export/import helpers**

In `src/app/vault/snapshot.rs`, add focused helpers that can:

- export the current asset catalog and secret bundles into `VaultSnapshot`;
- import a `VaultSnapshot` back into:
  - asset catalog state
  - credential store
  - known hosts store
  - sync/UI preference store

Do not call remote providers here. This task is local-only.

**Step 4: Run the focused tests to verify pass**

Run:

```bash
cargo test --test vault_snapshot_spec -- --nocapture
```

Expected:

- PASS for snapshot extraction and application.

**Step 5: Commit**

```bash
git add src/app/vault/snapshot.rs src/app/assets_catalog/mapper.rs src/app/ssh/credentials.rs src/app/ssh/known_hosts.rs src/app/ui_preferences.rs tests/vault_snapshot_spec.rs
git commit -m "feat: add vault snapshot import export"
```

### Task 4: Harden local bootstrap storage and prefer the OS keychain with failing tests

**Files:**
- Create: `src/app/vault/bootstrap.rs`
- Create: `tests/vault_bootstrap_spec.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `src/app/ssh/credentials.rs`
- Modify: `tests/credential_store_spec.rs`
- Modify: `tests/bootstrap_smoke.rs`

**Step 1: Write the failing bootstrap storage tests**

In `tests/vault_bootstrap_spec.rs`, add tests that assert:

- bootstrap bundles can be saved to and loaded from an encrypted export file;
- keychain-backed bootstrap references load through the credential store abstraction;
- bootstrap export files are unreadable as plaintext JSON.

In `tests/credential_store_spec.rs`, add a test proving that the preferred runtime store now tries `SystemCredentialStore` before falling back to a local encrypted cache wrapper.

**Step 2: Run the focused tests to confirm failure**

Run:

```bash
cargo test --test vault_bootstrap_spec --test credential_store_spec -- --nocapture
```

Expected:

- FAIL because bootstrap storage does not exist and the shared credential-store selection is still file-first.

**Step 3: Implement bootstrap persistence and preferred local store selection**

In `src/app/vault/bootstrap.rs`, add:

- bootstrap export file read/write helpers
- provider credential reference helpers
- minimal validation for required primary remote metadata

In `src/app/bootstrap.rs`, update the shared local secret strategy so it becomes:

- `SystemCredentialStore` when available
- encrypted local cache fallback
- plain file store only as a last-resort test or recovery path

Do not change provider sync logic yet.

**Step 4: Run the focused tests to verify pass**

Run:

```bash
cargo test --test vault_bootstrap_spec --test credential_store_spec --test bootstrap_smoke -- --nocapture
```

Expected:

- PASS for encrypted bootstrap export
- PASS for the hardened local-store preference chain

**Step 5: Commit**

```bash
git add src/app/vault/bootstrap.rs src/app/bootstrap.rs src/app/ssh/credentials.rs tests/vault_bootstrap_spec.rs tests/credential_store_spec.rs tests/bootstrap_smoke.rs
git commit -m "feat: harden local vault bootstrap storage"
```

### Task 5: Add the primary-plus-mirror sync engine with mock provider tests

**Files:**
- Create: `src/app/vault/engine.rs`
- Create: `src/app/vault/provider/mod.rs`
- Create: `src/app/vault/provider/mock.rs`
- Create: `tests/vault_sync_engine_spec.rs`
- Reference: `src/app/vault/model.rs`
- Reference: `src/app/vault/crypto.rs`
- Reference: `src/app/vault/snapshot.rs`

**Step 1: Write the failing sync-engine tests**

In `tests/vault_sync_engine_spec.rs`, add tests covering:

- primary write success followed by mirror fan-out;
- mirror failure reporting degraded health without rolling back the primary;
- remote `parent_revision` mismatch surfacing a conflict error;
- S3-style conditional-write support being honored when the provider advertises it.

Use small fake revisions such as:

```rust
assert_eq!(result.primary_revision, "rev-0002");
assert_eq!(result.mirror_failures.len(), 1);
```

**Step 2: Run the focused tests to confirm failure**

Run:

```bash
cargo test --test vault_sync_engine_spec -- --nocapture
```

Expected:

- FAIL because the sync engine and provider traits do not exist yet.

**Step 3: Implement the sync engine and provider trait**

In `src/app/vault/provider/mod.rs`, define:

- `VaultProvider`
- `ProviderCapabilities`
- `ProviderReadResult`
- `ProviderWriteRequest`

In `src/app/vault/engine.rs`, implement:

- primary head read
- local revision comparison
- encrypted payload assembly
- primary write
- mirror fan-out
- sync report aggregation

Keep provider implementations mocked in this task.

**Step 4: Run the focused tests to verify pass**

Run:

```bash
cargo test --test vault_sync_engine_spec -- --nocapture
```

Expected:

- PASS for primary success, mirror degradation, and conflict surfacing.

**Step 5: Commit**

```bash
git add src/app/vault/engine.rs src/app/vault/provider/mod.rs src/app/vault/provider/mock.rs tests/vault_sync_engine_spec.rs
git commit -m "feat: add vault sync engine"
```

### Task 6: Add the S3-compatible provider as the default primary backend

**Files:**
- Modify: `Cargo.toml`
- Create: `src/app/vault/provider/s3.rs`
- Create: `tests/vault_provider_s3_spec.rs`
- Reference: `src/app/vault/provider/mod.rs`
- Reference: `src/app/vault/model.rs`

**Step 1: Write the failing S3 provider tests**

In `tests/vault_provider_s3_spec.rs`, add tests that assert:

- provider configuration can represent custom endpoint, region, bucket, prefix, and path-style settings;
- the request builder uses the standardized credential chain by default;
- conditional head writes are enabled for the S3 provider;
- multi-pack object names are stable and deterministic.

Do not hit the real network. Keep the tests at request-building and adapter level.

**Step 2: Run the focused tests to confirm failure**

Run:

```bash
cargo test --test vault_provider_s3_spec -- --nocapture
```

Expected:

- FAIL because the S3 provider does not exist yet.

**Step 3: Add the S3 dependencies and implement the provider**

Add the needed AWS crates in `Cargo.toml`:

- `aws-config`
- `aws-sdk-s3`

In `src/app/vault/provider/s3.rs`, implement:

- config construction from bootstrap data
- head read
- manifest and pack object read/write
- conditional head overwrite

Keep the provider generic enough to support S3-compatible endpoints, not just AWS-hosted S3.

**Step 4: Run the focused tests to verify pass**

Run:

```bash
cargo test --test vault_provider_s3_spec -- --nocapture
```

Expected:

- PASS for config parsing, object naming, and conditional-write behavior.

**Step 5: Commit**

```bash
git add Cargo.toml src/app/vault/provider/s3.rs tests/vault_provider_s3_spec.rs
git commit -m "feat: add s3 vault provider"
```

### Task 7: Add the GitHub Gist provider with device-flow and PAT bootstrap paths

**Files:**
- Modify: `Cargo.toml`
- Create: `src/app/vault/provider/github_gist.rs`
- Create: `src/app/vault/auth/oauth.rs`
- Create: `tests/vault_provider_github_spec.rs`
- Reference: `src/app/vault/provider/mod.rs`

**Step 1: Write the failing GitHub provider tests**

In `tests/vault_provider_github_spec.rs`, add tests that assert:

- bootstrap config can represent either `device_flow` or `pat`;
- pack layout falls back to bundled files suitable for gist limits;
- reads use `raw_url` when a gist file is marked truncated;
- the provider exposes `supports_conditional_head_write = false`.

**Step 2: Run the focused tests to confirm failure**

Run:

```bash
cargo test --test vault_provider_github_spec -- --nocapture
```

Expected:

- FAIL because the GitHub provider and shared OAuth helper do not exist yet.

**Step 3: Add HTTP and OAuth dependencies**

Add:

- `reqwest`
- `oauth2`

In `src/app/vault/auth/oauth.rs`, implement the reusable device-flow / PKCE plumbing needed by provider-specific adapters.

In `src/app/vault/provider/github_gist.rs`, implement:

- gist bootstrap config parsing
- gist metadata read
- bundled pack upload and download
- OAuth device-flow bootstrap path
- PAT fallback bootstrap path

Do not implement GitHub as a mirror-only provider. It must support both primary and mirror roles, while still reporting its weaker concurrency semantics.

**Step 4: Run the focused tests to verify pass**

Run:

```bash
cargo test --test vault_provider_github_spec -- --nocapture
```

Expected:

- PASS for GitHub gist capability and bootstrap behavior.

**Step 5: Commit**

```bash
git add Cargo.toml src/app/vault/auth/oauth.rs src/app/vault/provider/github_gist.rs tests/vault_provider_github_spec.rs
git commit -m "feat: add github gist vault provider"
```

### Task 8: Add the GitLab Snippet and Gitee Gist providers with provider-specific pack limits

**Files:**
- Create: `src/app/vault/provider/gitlab_snippet.rs`
- Create: `src/app/vault/provider/gitee_gist.rs`
- Create: `tests/vault_provider_gitlab_spec.rs`
- Create: `tests/vault_provider_gitee_spec.rs`
- Reference: `src/app/vault/auth/oauth.rs`
- Reference: `src/app/vault/provider/mod.rs`

**Step 1: Write the failing GitLab and Gitee provider tests**

In `tests/vault_provider_gitlab_spec.rs`, add tests that assert:

- GitLab pack layout respects the 10-file constraint;
- bootstrap auth can downgrade from `device_flow` to `pkce` or `pat`;
- provider capability flags mark GitLab as bundled-file transport without strict CAS.

In `tests/vault_provider_gitee_spec.rs`, add tests that assert:

- Gitee bootstrap config supports `pat` and standard OAuth code flow;
- the provider defaults to bundled-file layout;
- capability flags report non-conditional writes.

**Step 2: Run the focused tests to confirm failure**

Run:

```bash
cargo test --test vault_provider_gitlab_spec --test vault_provider_gitee_spec -- --nocapture
```

Expected:

- FAIL because neither provider exists yet.

**Step 3: Implement the GitLab and Gitee providers**

In `src/app/vault/provider/gitlab_snippet.rs`, implement:

- GitLab base-URL-aware config
- bundled upload layout with strict low pack count
- device-flow when supported
- PKCE fallback
- PAT fallback

In `src/app/vault/provider/gitee_gist.rs`, implement:

- Gitee config parsing
- bundled upload layout
- PAT-first bootstrap
- optional OAuth code-flow bootstrap

Reuse the shared HTTP and OAuth helpers from previous tasks instead of duplicating auth code per provider.

**Step 4: Run the focused tests to verify pass**

Run:

```bash
cargo test --test vault_provider_gitlab_spec --test vault_provider_gitee_spec -- --nocapture
```

Expected:

- PASS for provider limits, capability flags, and bootstrap fallbacks.

**Step 5: Commit**

```bash
git add src/app/vault/provider/gitlab_snippet.rs src/app/vault/provider/gitee_gist.rs tests/vault_provider_gitlab_spec.rs tests/vault_provider_gitee_spec.rs
git commit -m "feat: add gitlab and gitee vault providers"
```

### Task 9: Add the `Sync & Vault` UI and view-model state with failing tests

**Files:**
- Modify: `src/shell/view_model.rs`
- Modify: `src/app/ui_preferences.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `ui/app-window.slint`
- Modify: `ui/components/titlebar-menu.slint`
- Modify: `ui/shell/right-panel.slint`
- Create: `ui/components/vault-provider-card.slint`
- Create: `tests/vault_settings_smoke.rs`
- Create: `tests/vault_settings_ui_contract_smoke.sh`
- Modify: `tests/top_status_bar_smoke.rs`

**Step 1: Write the failing UI contract tests**

In `tests/vault_settings_smoke.rs` and `tests/vault_settings_ui_contract_smoke.sh`, add assertions that:

- the titlebar menu still exposes `Settings`, but selecting it routes the right panel to `Sync & Vault`;
- the right panel exposes:
  - vault lock state
  - master-password actions
  - provider cards with `Primary` or `Mirror` labels
  - `Sync now`
  - `Export bootstrap`
  - `Import bootstrap`;
- sync-specific panel state persists in `UiPreferences`.

**Step 2: Run the focused tests to confirm failure**

Run:

```bash
cargo test --test vault_settings_smoke --test top_status_bar_smoke -- --nocapture
bash tests/vault_settings_ui_contract_smoke.sh
```

Expected:

- FAIL because the right panel has no vault-specific state or UI yet.

**Step 3: Implement the new panel state and UI**

In `src/shell/view_model.rs`, add:

- right-panel subview state for `Sync & Vault`
- vault status projection
- provider health projection
- pending action state for:
  - lock / unlock
  - sync now
  - export bootstrap
  - import bootstrap

In `ui/shell/right-panel.slint`, add the panel structure. Keep the visual language consistent with the current shell and do not invent a second settings window.

In `ui/components/titlebar-menu.slint`, keep `Settings` as the label but route it to the vault/settings panel instead of a dead-end action.

**Step 4: Run the focused tests to verify pass**

Run:

```bash
cargo test --test vault_settings_smoke --test top_status_bar_smoke -- --nocapture
bash tests/vault_settings_ui_contract_smoke.sh
```

Expected:

- PASS for the right-panel settings contract and state persistence.

**Step 5: Commit**

```bash
git add src/shell/view_model.rs src/app/ui_preferences.rs src/app/bootstrap.rs ui/app-window.slint ui/components/titlebar-menu.slint ui/shell/right-panel.slint ui/components/vault-provider-card.slint tests/vault_settings_smoke.rs tests/vault_settings_ui_contract_smoke.sh tests/top_status_bar_smoke.rs
git commit -m "feat: add sync and vault settings panel"
```

### Task 10: Wire real vault unlock, sync triggers, and app integration

**Files:**
- Modify: `src/app/bootstrap.rs`
- Modify: `src/app/vault/engine.rs`
- Modify: `src/app/vault/bootstrap.rs`
- Modify: `src/app/vault/cache.rs`
- Modify: `tests/bootstrap_smoke.rs`
- Modify: `tests/ui_preferences.rs`
- Modify: `tests/vault_sync_engine_spec.rs`
- Reference: `src/app/vault/provider/*.rs`

**Step 1: Write the failing integration tests**

In `tests/bootstrap_smoke.rs`, add integration coverage for:

- creating a vault from the settings panel;
- unlocking an existing vault with the master password;
- triggering a manual sync that writes primary then mirrors;
- surfacing provider auth errors and mirror degradation in the UI;
- locking the vault and clearing decrypted in-memory state.

**Step 2: Run the focused tests to confirm failure**

Run:

```bash
cargo test --test bootstrap_smoke --test ui_preferences --test vault_sync_engine_spec -- --nocapture
```

Expected:

- FAIL because the UI is not yet connected to the vault engine or bootstrap persistence.

**Step 3: Implement the full app wiring**

In `src/app/bootstrap.rs`, connect the UI callbacks to:

- bootstrap load/save
- master-password unlock
- manual sync
- auto-sync on save
- startup cache restore
- mirror health refresh

Make sure the app:

- restores encrypted cache on startup when available;
- avoids loading decrypted secrets when the vault is locked;
- updates right-panel health state after sync results.

**Step 4: Run the focused integration tests**

Run:

```bash
cargo test --test bootstrap_smoke --test ui_preferences --test vault_sync_engine_spec -- --nocapture
```

Expected:

- PASS for unlock, sync, lock, and settings persistence.

**Step 5: Commit**

```bash
git add src/app/bootstrap.rs src/app/vault/engine.rs src/app/vault/bootstrap.rs src/app/vault/cache.rs tests/bootstrap_smoke.rs tests/ui_preferences.rs tests/vault_sync_engine_spec.rs
git commit -m "feat: wire vault sync into app bootstrap"
```

### Task 11: Run end-to-end verification and clean up any regressions

**Files:**
- No source changes required unless verification reveals regressions

**Step 1: Run the targeted vault test suite**

Run:

```bash
cargo test --test vault_model_spec --test vault_crypto_spec --test vault_snapshot_spec --test vault_bootstrap_spec --test vault_sync_engine_spec --test vault_provider_s3_spec --test vault_provider_github_spec --test vault_provider_gitlab_spec --test vault_provider_gitee_spec --test vault_settings_smoke --test bootstrap_smoke -- --nocapture
```

Expected:

- PASS

**Step 2: Run the broader regression suite that touches existing SSH and settings behavior**

Run:

```bash
cargo test --test credential_store_spec --test ssh_profile_spec --test ui_preferences --test top_status_bar_smoke --test shell_view_model -- --nocapture
```

Expected:

- PASS

**Step 3: Run workspace validation**

Run:

```bash
cargo check --workspace
```

Expected:

- PASS

**Step 4: Commit verification fixes if needed**

If any regression fixes were required, commit them with:

```bash
git add <fixed-files>
git commit -m "fix: resolve vault sync verification regressions"
```

**Step 5: Final handoff**

Document:

- which provider roles were verified as primary vs mirror;
- what bootstrap recovery paths were manually verified;
- any remaining limitations, especially around snippet-provider concurrency semantics.

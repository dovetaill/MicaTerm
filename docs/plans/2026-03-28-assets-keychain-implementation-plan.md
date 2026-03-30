# Assets Keychain Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a first-class `Keychain` module that manages reusable `Identity` and `SSH Key` records, lets SSH hosts choose either manual auth or a keychain identity, and syncs the new data through the existing vault pipeline without breaking legacy SSH assets.

**Architecture:** Keep `Keychain` as a separate top-level module with its own `KeychainCatalog`, dedicated secret bundle namespaces, and its own explorer projection. Extend SSH host specs with `auth_source` and `keychain_identity_id`, then add a resolver layer that expands a keychain identity into the existing `ConnectionProfile + CredentialStore` flow before runtime normalization. Reuse the current vault snapshot and credential backend, but add separate keychain snapshot sections instead of mixing keychain secrets into host-owned SSH secret bundles.

**Tech Stack:** Rust, Slint, Tokio, redb, keyring-backed `CredentialStore`, existing vault snapshot pipeline, cargo test, shell UI smoke tests

**Execution Rules:** Use `@superpowers:test-driven-development` on every task. Before declaring the feature done, run `@superpowers:verification-before-completion`.

---

### Task 1: Lock the keychain domain model and snapshot schema with failing tests

**Files:**
- Create: `src/app/keychain/mod.rs`
- Create: `src/app/keychain/model.rs`
- Modify: `src/app/mod.rs`
- Modify: `src/app/vault/model.rs`
- Create: `tests/keychain_model_spec.rs`
- Modify: `tests/vault_model_spec.rs`
- Reference: `src/shell/assets.rs`
- Reference: `docs/plans/2026-03-28-assets-keychain-design.md`

**Step 1: Write the failing keychain model tests**

In `tests/keychain_model_spec.rs`, add round-trip and defaulting tests for:

- `KeychainCatalog`
- `KeychainNode`
- `KeychainNodeKind`
- `KeychainIdentitySpec`
- `KeychainIdentityAuthKind`
- `KeychainSshKeySpec`

Also extend `tests/vault_model_spec.rs` so `VaultSnapshot` round-trips:

- `keychain_catalog`
- `keychain_identity_secret_bundles`
- `keychain_key_secret_bundles`

Use concrete expectations such as:

```rust
assert_eq!(catalog.root_ids.len(), 2);
assert_eq!(identity.username, "ops");
assert_eq!(ssh_key.algorithm, "ed25519");
```

**Step 2: Run the focused tests to confirm failure**

Run:

```bash
cargo test --test keychain_model_spec -- --nocapture
cargo test --test vault_model_spec -- --nocapture
```

Expected:

- FAIL because the `app::keychain` module and the new vault snapshot fields do not exist yet.

**Step 3: Implement the minimal domain model**

Create `src/app/keychain/model.rs` with serializable pure data types for:

- `KeychainCatalog`
- `KeychainNode`
- `KeychainNodeKind`
- `KeychainNodePayload`
- `KeychainIdentitySpec`
- `KeychainIdentityAuthKind`
- `KeychainSshKeySpec`

Extend `src/app/vault/model.rs` with new keychain snapshot sections and defaults, but do not implement persistence or runtime logic yet.

**Step 4: Run the focused tests to verify pass**

Run:

```bash
cargo test --test keychain_model_spec -- --nocapture
cargo test --test vault_model_spec -- --nocapture
```

Expected:

- PASS for serde round-trips and default snapshot initialization.

**Step 5: Commit**

```bash
git add src/app/mod.rs src/app/keychain/mod.rs src/app/keychain/model.rs src/app/vault/model.rs tests/keychain_model_spec.rs tests/vault_model_spec.rs
git commit -m "feat: add keychain domain model"
```

### Task 2: Add keychain persistence and snapshot import/export with failing tests

**Files:**
- Create: `src/app/keychain/repository.rs`
- Create: `src/app/keychain/redb_store.rs`
- Modify: `src/app/app_paths.rs`
- Modify: `src/app/vault/snapshot.rs`
- Create: `tests/keychain_store_spec.rs`
- Modify: `tests/vault_snapshot_spec.rs`
- Reference: `src/app/assets_catalog/redb_store.rs`
- Reference: `src/app/assets_catalog/repository.rs`

**Step 1: Write the failing persistence tests**

In `tests/keychain_store_spec.rs`, add tests proving that:

- a `KeychainCatalog` with folders, identities, and keys round-trips through the redb-backed store;
- node ordering and parent-child links survive reload;
- empty catalogs load as valid defaults.

In `tests/vault_snapshot_spec.rs`, add expectations that:

- exporting app state includes `keychain_catalog`;
- keychain identity secrets and key secrets are preserved in snapshot export/import.

Use concrete expectations such as:

```rust
assert_eq!(reloaded.nodes.len(), 3);
assert_eq!(snapshot.keychain_key_secret_bundles.len(), 1);
```

**Step 2: Run the focused tests to confirm failure**

Run:

```bash
cargo test --test keychain_store_spec -- --nocapture
cargo test --test vault_snapshot_spec -- --nocapture
```

Expected:

- FAIL because there is no keychain repository/store or snapshot bridge yet.

**Step 3: Implement the repository and snapshot wiring**

Create:

- `src/app/keychain/repository.rs`
- `src/app/keychain/redb_store.rs`

Add an app data path for the keychain catalog in `src/app/app_paths.rs`, then extend `src/app/vault/snapshot.rs` to export and restore:

- `keychain_catalog`
- `keychain_identity_secret_bundles`
- `keychain_key_secret_bundles`

Keep the store and snapshot helpers focused. Do not add any UI code in this task.

**Step 4: Run the focused tests to verify pass**

Run:

```bash
cargo test --test keychain_store_spec -- --nocapture
cargo test --test vault_snapshot_spec -- --nocapture
```

Expected:

- PASS for redb round-trips and vault snapshot import/export.

**Step 5: Commit**

```bash
git add src/app/app_paths.rs src/app/keychain/repository.rs src/app/keychain/redb_store.rs src/app/vault/snapshot.rs tests/keychain_store_spec.rs tests/vault_snapshot_spec.rs
git commit -m "feat: add keychain persistence and snapshot support"
```

### Task 3: Add keychain secret bundles and credential reference helpers with failing tests

**Files:**
- Modify: `src/app/ssh/credentials.rs`
- Create: `tests/keychain_secret_store_spec.rs`
- Modify: `tests/credential_store_spec.rs`
- Reference: `docs/plans/2026-03-28-assets-keychain-design.md`

**Step 1: Write the failing credential helper tests**

In `tests/keychain_secret_store_spec.rs`, add tests for:

- `keychain_identity_credential_ref(identity_id)` namespace generation;
- `keychain_key_credential_ref(key_id)` namespace generation;
- persisting and loading identity password bundles;
- persisting and loading key bundles with `private_key_content` and `passphrase`;
- deleting empty bundles instead of leaving stale entries.

Add compatibility assertions in `tests/credential_store_spec.rs` proving the existing SSH helper behavior remains unchanged.

Use concrete expectations such as:

```rust
assert_eq!(keychain_identity_credential_ref("id-1"), "keychain/identity/id-1");
assert_eq!(bundle.private_key_content.as_deref(), Some("-----BEGIN OPENSSH PRIVATE KEY-----"));
```

**Step 2: Run the focused tests to confirm failure**

Run:

```bash
cargo test --test keychain_secret_store_spec -- --nocapture
cargo test --test credential_store_spec -- --nocapture
```

Expected:

- FAIL because the new keychain credential refs and bundle helpers do not exist yet.

**Step 3: Implement keychain secret helpers**

Extend `src/app/ssh/credentials.rs` with:

- credential ref builders for keychain identities and keys;
- `StoredKeychainIdentitySecretBundle`;
- `StoredKeychainKeySecretBundle`;
- persist/load/snapshot/restore helpers mirroring the current SSH helper style.

Do not change existing SSH bundle semantics in this task.

**Step 4: Run the focused tests to verify pass**

Run:

```bash
cargo test --test keychain_secret_store_spec -- --nocapture
cargo test --test credential_store_spec -- --nocapture
```

Expected:

- PASS for keychain secret round-trips and SSH compatibility.

**Step 5: Commit**

```bash
git add src/app/ssh/credentials.rs tests/keychain_secret_store_spec.rs tests/credential_store_spec.rs
git commit -m "feat: add keychain secret bundle helpers"
```

### Task 4: Extend SSH host specs and add the identity resolver with failing tests

**Files:**
- Create: `src/app/keychain/resolver.rs`
- Modify: `src/shell/assets.rs`
- Modify: `src/app/assets_catalog/model.rs`
- Modify: `src/app/assets_catalog/mapper.rs`
- Modify: `src/app/ssh/profile.rs`
- Modify: `src/app/ssh/runtime.rs`
- Create: `tests/keychain_resolver_spec.rs`
- Modify: `tests/ssh_profile_spec.rs`
- Modify: `tests/bootstrap_smoke.rs`

**Step 1: Write the failing resolver tests**

In `tests/keychain_resolver_spec.rs`, add tests proving that:

- a password-based identity resolves to the same runtime auth shape as manual password auth;
- an SSH-key-based identity resolves to the same runtime auth shape as manual inline private key auth;
- missing identity and missing key references fail with explicit diagnostics.

Extend `tests/ssh_profile_spec.rs` and `tests/bootstrap_smoke.rs` so SSH hosts round-trip:

- `auth_source = "manual"`
- `auth_source = "keychain-identity"`
- `keychain_identity_id`

Use concrete expectations such as:

```rust
assert_eq!(resolved.user.as_deref(), Some("ops"));
assert_eq!(saved_spec.keychain_identity_id.as_deref(), Some("identity-prod"));
```

**Step 2: Run the focused tests to confirm failure**

Run:

```bash
cargo test --test keychain_resolver_spec -- --nocapture
cargo test --test ssh_profile_spec -- --nocapture
cargo test --test bootstrap_smoke -- --nocapture
```

Expected:

- FAIL because host specs do not yet carry keychain auth fields and no resolver exists.

**Step 3: Implement the host schema and resolver**

Add to SSH host specs:

- `auth_source`
- `keychain_identity_id`

Create `src/app/keychain/resolver.rs` to turn a host plus keychain references into the same secret-bearing runtime shape the SSH stack already understands.

Keep the resolver strictly before runtime normalization. Do not fork the SSH runtime into a separate keychain path.

**Step 4: Run the focused tests to verify pass**

Run:

```bash
cargo test --test keychain_resolver_spec -- --nocapture
cargo test --test ssh_profile_spec -- --nocapture
cargo test --test bootstrap_smoke -- --nocapture
```

Expected:

- PASS for host schema round-trips, resolver success cases, and compatibility with existing manual assets.

**Step 5: Commit**

```bash
git add src/app/keychain/resolver.rs src/shell/assets.rs src/app/assets_catalog/model.rs src/app/assets_catalog/mapper.rs src/app/ssh/profile.rs src/app/ssh/runtime.rs tests/keychain_resolver_spec.rs tests/ssh_profile_spec.rs tests/bootstrap_smoke.rs
git commit -m "feat: resolve ssh hosts from keychain identities"
```

### Task 5: Add keychain explorer projection and CRUD state with failing tests

**Files:**
- Create: `src/shell/keychain.rs`
- Modify: `src/shell/mod.rs`
- Modify: `src/shell/view_model.rs`
- Modify: `src/shell/sidebar.rs`
- Create: `tests/keychain_projection_spec.rs`
- Modify: `tests/sidebar_navigation_spec.rs`
- Modify: `tests/shell_view_model.rs`
- Reference: `src/shell/assets.rs`

**Step 1: Write the failing explorer and state tests**

In `tests/keychain_projection_spec.rs`, add tests for:

- tree projection of `Folder / Identity / SSH Key`;
- search matching by title, username, fingerprint, and public key comment;
- deletion blocking when a referenced identity or key is targeted.

Extend `tests/sidebar_navigation_spec.rs` and `tests/shell_view_model.rs` to assert:

- `Keychain` toolbar uses a create popover instead of a single `new-keychain` action;
- keychain selection state is independent from console asset tree state;
- default create names work within a folder scope.

Use concrete expectations such as:

```rust
assert_eq!(rows[0].kind.id(), "folder");
assert!(delete_result.is_err());
assert_eq!(descriptor.primary_create_tooltip, "Create Keychain Item");
```

**Step 2: Run the focused tests to confirm failure**

Run:

```bash
cargo test --test keychain_projection_spec -- --nocapture
cargo test --test shell_view_model -- --nocapture
cargo test --test sidebar_navigation_spec -- --nocapture
```

Expected:

- FAIL because no keychain projection or CRUD state exists yet.

**Step 3: Implement keychain projection and view-model state**

Create `src/shell/keychain.rs` for keychain-specific projection helpers and state utilities, then extend `src/shell/view_model.rs` and `src/shell/sidebar.rs` so the shell owns:

- a `KeychainCatalog` state source;
- create/rename/delete operations for keychain items;
- independent search text and selection state;
- a create popover definition with:
  - `New Folder`
  - `New Identity`
  - `New SSH Key`

Do not build the Slint forms yet.

**Step 4: Run the focused tests to verify pass**

Run:

```bash
cargo test --test keychain_projection_spec -- --nocapture
cargo test --test shell_view_model -- --nocapture
cargo test --test sidebar_navigation_spec -- --nocapture
```

Expected:

- PASS for projection, create-state, and toolbar-descriptor behavior.

**Step 5: Commit**

```bash
git add src/shell/keychain.rs src/shell/mod.rs src/shell/view_model.rs src/shell/sidebar.rs tests/keychain_projection_spec.rs tests/sidebar_navigation_spec.rs tests/shell_view_model.rs
git commit -m "feat: add keychain explorer state"
```

### Task 6: Build the keychain panel and dedicated modals with failing UI tests

**Files:**
- Create: `ui/components/assets-keychain-identity-modal.slint`
- Create: `ui/components/assets-keychain-ssh-key-modal.slint`
- Modify: `ui/app-window.slint`
- Modify: `ui/shell/assets-sidebar.slint`
- Modify: `ui/components/assets-create-menu.slint`
- Modify: `ui/components/sidebar-nav-button.slint`
- Modify: `src/app/bootstrap.rs`
- Create: `tests/keychain_modal_smoke.rs`
- Create: `tests/keychain_ui_contract_smoke.sh`
- Modify: `tests/assets_modal_smoke.rs`

**Step 1: Write the failing UI tests**

In `tests/keychain_modal_smoke.rs`, add smoke tests proving:

- the `Keychain` panel shows a tree instead of placeholder copy;
- identity modal exposes `Name`, `Username`, `Password` or `SSH Key` auth controls;
- ssh key modal exposes private key import/paste, public key import/paste, generate, and copy actions.

In `tests/keychain_ui_contract_smoke.sh`, assert that rendered UI text includes:

- `Identity`
- `SSH Key`
- `Generate Key Pair`
- `Copy Public Key`
- `Use Existing Keychain Identity`
- `Manual`
- `Keychain Identity`
- `Authentication Summary`

**Step 2: Run the focused tests to confirm failure**

Run:

```bash
cargo test --test keychain_modal_smoke -- --nocapture
bash tests/keychain_ui_contract_smoke.sh
```

Expected:

- FAIL because the dedicated keychain panel and modals do not exist yet.

**Step 3: Implement the Slint UI shells and bootstrap bindings**

Create:

- `ui/components/assets-keychain-identity-modal.slint`
- `ui/components/assets-keychain-ssh-key-modal.slint`

Then wire them through:

- `ui/app-window.slint`
- `ui/shell/assets-sidebar.slint`
- `src/app/bootstrap.rs`

Make the keychain panel render tree rows instead of placeholder text, and use the create popover rather than the old single `new-keychain` action.

**Step 4: Run the focused tests to verify pass**

Run:

```bash
cargo test --test keychain_modal_smoke -- --nocapture
bash tests/keychain_ui_contract_smoke.sh
```

Expected:

- PASS for panel rendering, modal shells, and required copy strings.

**Step 5: Commit**

```bash
git add ui/components/assets-keychain-identity-modal.slint ui/components/assets-keychain-ssh-key-modal.slint ui/app-window.slint ui/shell/assets-sidebar.slint ui/components/assets-create-menu.slint ui/components/sidebar-nav-button.slint src/app/bootstrap.rs tests/keychain_modal_smoke.rs tests/keychain_ui_contract_smoke.sh tests/assets_modal_smoke.rs
git commit -m "feat: add keychain panel and modals"
```

### Task 7: Add key import, public-key handling, and key generation flows with failing tests

**Files:**
- Modify: `src/app/bootstrap.rs`
- Modify: `src/app/keychain/resolver.rs`
- Modify: `src/shell/view_model.rs`
- Create: `tests/keychain_key_actions_spec.rs`
- Modify: `tests/bootstrap_smoke.rs`
- Modify: `tests/assets_modal_smoke.rs`
- Reference: `src/app/ssh/runtime.rs`

**Step 1: Write the failing action tests**

In `tests/keychain_key_actions_spec.rs`, add tests for:

- importing a private key file into a key draft;
- importing a public key file into a key draft;
- generating an `ed25519` key pair and populating `public_key` plus `fingerprint`;
- copying public key text from a saved key;
- keeping host auth behavior unchanged when a manual SSH host imports an inline key.

Use concrete expectations such as:

```rust
assert!(draft.public_key.starts_with("ssh-ed25519 "));
assert!(!draft.fingerprint.is_empty());
```

**Step 2: Run the focused tests to confirm failure**

Run:

```bash
cargo test --test keychain_key_actions_spec -- --nocapture
cargo test --test bootstrap_smoke -- --nocapture
```

Expected:

- FAIL because the key actions and generation flow do not exist yet.

**Step 3: Implement the key actions**

Extend bootstrap and view-model action handlers so keychain key drafts support:

- import private key
- import public key
- paste private key
- paste public key
- generate `ed25519` key pair
- derive `public_key` and `fingerprint` where possible
- copy public key text

Do not require public key input for SSH host connections.

**Step 4: Run the focused tests to verify pass**

Run:

```bash
cargo test --test keychain_key_actions_spec -- --nocapture
cargo test --test bootstrap_smoke -- --nocapture
```

Expected:

- PASS for key import/generation flows and manual host compatibility.

**Step 5: Commit**

```bash
git add src/app/bootstrap.rs src/app/keychain/resolver.rs src/shell/view_model.rs tests/keychain_key_actions_spec.rs tests/bootstrap_smoke.rs tests/assets_modal_smoke.rs
git commit -m "feat: add keychain key actions"
```

### Task 8: Wire SSH host modal source switching and end-to-end compatibility with failing tests

**Files:**
- Modify: `ui/components/assets-ssh-connection-modal.slint`
- Modify: `ui/app-window.slint`
- Modify: `src/app/bootstrap.rs`
- Modify: `src/shell/view_model.rs`
- Modify: `tests/assets_modal_smoke.rs`
- Modify: `tests/assets_modal_ui_contract_smoke.sh`
- Modify: `tests/shell_view_model.rs`
- Modify: `tests/ssh_session_manager_spec.rs`
- Modify: `tests/bootstrap_smoke.rs`
- Modify: `tests/ssh_profile_spec.rs`

**Step 1: Write the failing host-modal tests**

Extend the existing SSH modal and session tests to prove:

- host auth source toggles between `Manual` and `Keychain Identity`;
- choosing a keychain identity hides or disables manual auth inputs;
- manual mode still supports password and legacy private key path assets;
- identity-backed hosts connect through the existing runtime path after resolver expansion.

Use concrete expectations such as:

```rust
assert_eq!(draft.auth_source, "keychain-identity");
assert_eq!(draft.keychain_identity_id.as_deref(), Some("identity-prod"));
```

**Step 2: Run the focused tests to confirm failure**

Run:

```bash
cargo test --test assets_modal_smoke -- --nocapture
bash tests/assets_modal_ui_contract_smoke.sh
cargo test --test ssh_session_manager_spec -- --nocapture
cargo test --test bootstrap_smoke -- --nocapture
```

Expected:

- FAIL because the SSH host modal does not yet expose keychain auth source selection.

**Step 3: Implement the SSH modal integration**

Update the host modal and shell state so:

- `Manual` remains the default;
- `Keychain Identity` exposes an identity picker with username and auth summary;
- switching to keychain mode clears manual-only secret draft fields before save;
- switching back to manual preserves legacy compatibility rules for old assets.

Keep all runtime auth work behind the resolver added earlier.

**Step 4: Run the focused tests to verify pass**

Run:

```bash
cargo test --test assets_modal_smoke -- --nocapture
bash tests/assets_modal_ui_contract_smoke.sh
cargo test --test ssh_session_manager_spec -- --nocapture
cargo test --test bootstrap_smoke -- --nocapture
cargo test --test ssh_profile_spec -- --nocapture
```

Expected:

- PASS for host modal behavior, runtime compatibility, and legacy asset coverage.

**Step 5: Commit**

```bash
git add ui/components/assets-ssh-connection-modal.slint ui/app-window.slint src/app/bootstrap.rs src/shell/view_model.rs tests/assets_modal_smoke.rs tests/assets_modal_ui_contract_smoke.sh tests/shell_view_model.rs tests/ssh_session_manager_spec.rs tests/ssh_profile_spec.rs
git add ui/components/assets-ssh-connection-modal.slint ui/app-window.slint src/app/bootstrap.rs src/shell/view_model.rs tests/assets_modal_smoke.rs tests/assets_modal_ui_contract_smoke.sh tests/shell_view_model.rs tests/ssh_session_manager_spec.rs tests/bootstrap_smoke.rs tests/ssh_profile_spec.rs
git commit -m "feat: support ssh hosts backed by keychain identities"
```

### Task 9: Run full keychain verification and update planning docs

**Files:**
- Modify: `docs/plans/2026-03-28-assets-keychain-design.md`
- Modify: `docs/plans/2026-03-28-assets-keychain-implementation-plan.md`
- Reference: `tests/keychain_model_spec.rs`
- Reference: `tests/keychain_store_spec.rs`
- Reference: `tests/keychain_secret_store_spec.rs`
- Reference: `tests/keychain_resolver_spec.rs`
- Reference: `tests/keychain_projection_spec.rs`
- Reference: `tests/keychain_modal_smoke.rs`
- Reference: `tests/keychain_key_actions_spec.rs`

**Step 1: Run the focused Rust test set**

Run:

```bash
cargo test --test keychain_model_spec -- --nocapture
cargo test --test keychain_store_spec -- --nocapture
cargo test --test keychain_secret_store_spec -- --nocapture
cargo test --test keychain_resolver_spec -- --nocapture
cargo test --test keychain_projection_spec -- --nocapture
cargo test --test keychain_modal_smoke -- --nocapture
cargo test --test keychain_key_actions_spec -- --nocapture
```

Expected:

- PASS for all new focused keychain tests.

**Step 2: Run the compatibility and UI smoke set**

Run:

```bash
cargo test --test assets_modal_smoke -- --nocapture
cargo test --test shell_view_model -- --nocapture
cargo test --test ssh_profile_spec -- --nocapture
cargo test --test ssh_session_manager_spec -- --nocapture
cargo test --test bootstrap_smoke -- --nocapture
cargo test --test vault_snapshot_spec -- --nocapture
bash tests/keychain_ui_contract_smoke.sh
bash tests/assets_modal_ui_contract_smoke.sh
bash tests/sidebar_ui_contract_smoke.sh
```

Expected:

- PASS with no regressions in manual SSH host flows, sidebar navigation, or snapshot restore behavior.

**Step 3: Refresh the docs if implementation changed any minor naming details**

Update the design doc and plan only if the implemented field names or test entrypoints differ from the plan in a material way. Do not rewrite the design intent.

**Step 4: Commit**

```bash
git add docs/plans/2026-03-28-assets-keychain-design.md docs/plans/2026-03-28-assets-keychain-implementation-plan.md
git commit -m "docs: finalize assets keychain planning"
```

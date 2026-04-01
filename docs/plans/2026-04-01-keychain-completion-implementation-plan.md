# Keychain Completion Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Finish the keychain feature so identities, SSH keys, local persistence, and vault sync all behave as first-class assets.

**Architecture:** Keep keychain as its own catalog and secret namespace. Fill the missing UI, view-model, bootstrap, persistence, and sync seams instead of folding keychain back into the console asset tree. Reuse the existing `CredentialStore`, `KeychainCatalog`, and vault snapshot abstractions so restart recovery and sync share one data model.

**Tech Stack:** Rust, Slint, redb, existing `CredentialStore`, existing vault/sync pipeline, `cargo test`, shell UI smoke scripts

---

### Task 1: Lock the missing keychain UI and context-menu behavior with failing tests

**Files:**
- Modify: `tests/keychain_modal_smoke.rs`
- Modify: `tests/assets_context_menu_smoke.rs`
- Modify: `tests/keychain_ui_contract_smoke.sh`
- Reference: `ui/shell/assets-sidebar.slint`
- Reference: `ui/components/asset-node-row.slint`
- Reference: `src/shell/context_menu.rs`

**Step 1: Write the failing tests**

Add focused coverage for:

- keychain blank area right-click using a keychain-specific menu target;
- keychain `identity` and `ssh-key` items exposing `Edit / Rename / Delete`;
- keychain item icons no longer using the console SSH icon;
- `New Identity` remaining a modal entry point instead of immediate node creation.

```rust
assert_eq!(app.get_asset_modal_open(), true);
assert_eq!(app.get_asset_modal_kind().as_str(), "new-keychain-identity");
assert!(menu_labels.contains(&"Edit".to_string()));
assert!(menu_labels.contains(&"Delete".to_string()));
assert!(!key_icon_source.contains("window-console"));
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test keychain_modal_smoke -- --nocapture`
Expected: FAIL because identity create still inserts a node directly and keychain menu/icon routing is incomplete.

Run: `cargo test --test assets_context_menu_smoke -- --nocapture`
Expected: FAIL because keychain rows still resolve through generic blank/folder/ssh menu paths.

Run: `bash tests/keychain_ui_contract_smoke.sh`
Expected: FAIL because the keychain-specific menu and icon copy are not fully present.

**Step 3: Write minimal implementation**

- Add keychain-specific context target parsing and action trees.
- Make keychain blank area right-click resolve to keychain create actions only.
- Make keychain `identity` / `ssh-key` rows advertise the correct node kinds and icons.
- Make `new-identity` open a modal instead of mutating the catalog immediately.

```text
parse_context_target_kind("blank", Keychain) -> KeychainBlankArea
parse_context_target_kind("identity", Keychain) -> KeychainIdentity
parse_context_target_kind("ssh-key", Keychain) -> KeychainSshKey
```

**Step 4: Run test to verify it passes**

Run: `cargo test --test keychain_modal_smoke -- --nocapture`
Expected: PASS

Run: `cargo test --test assets_context_menu_smoke -- --nocapture`
Expected: PASS

Run: `bash tests/keychain_ui_contract_smoke.sh`
Expected: PASS

**Step 5: Commit**

```bash
git add tests/keychain_modal_smoke.rs tests/assets_context_menu_smoke.rs tests/keychain_ui_contract_smoke.sh ui/shell/assets-sidebar.slint ui/components/asset-node-row.slint src/shell/context_menu.rs src/app/bootstrap.rs src/shell/view_model.rs
git commit -m "test: lock keychain menu and icon contracts"
```

### Task 2: Add keychain identity modal state, create flow, and edit flow

**Files:**
- Modify: `src/shell/view_model.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `ui/app-window.slint`
- Modify: `ui/components/assets-keychain-identity-modal.slint`
- Modify: `tests/assets_modal_smoke.rs`
- Modify: `tests/bootstrap_smoke.rs`
- Create: `tests/keychain_identity_actions_spec.rs`

**Step 1: Write the failing test**

Add tests proving:

- `New Identity` opens `new-keychain-identity`;
- confirming the modal creates a catalog node only after validation passes;
- editing an existing identity reopens the modal with populated fields;
- switching auth kind between `password` and `ssh-key` preserves shared metadata and updates the correct secret/reference fields.

```rust
assert_eq!(app.get_asset_modal_kind().as_str(), "new-keychain-identity");
assert_eq!(app.get_keychain_asset_items().row_count(), 0);
app.invoke_confirm_asset_modal_requested();
assert_eq!(app.get_keychain_asset_items().row_count(), 1);
assert_eq!(app.get_keychain_identity_modal_username().as_str(), "ops");
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test keychain_identity_actions_spec -- --nocapture`
Expected: FAIL because `AssetModalState::NewKeychainIdentity` and bootstrap wiring do not exist yet.

Run: `cargo test --test assets_modal_smoke -- --nocapture`
Expected: FAIL because the identity modal fields do not drive a real create/edit flow.

Run: `cargo test --test bootstrap_smoke -- --nocapture`
Expected: FAIL because `keychain-identity-modal-*` callbacks are not connected.

**Step 3: Write minimal implementation**

- Add `KeychainIdentityDraft`.
- Add `AssetModalState::NewKeychainIdentity { parent_id, editing_item_id, draft }`.
- Implement:
  - `open_new_keychain_identity_modal(...)`
  - `open_edit_keychain_identity_modal(...)`
  - `update_keychain_identity_modal_field(...)`
  - identity validation helpers
  - confirm logic to create/update the node only on modal confirm.
- Wire `keychain-identity-modal-draft-changed` and `keychain-identity-modal-action-requested` through bootstrap.

```text
if editing_item_id.is_some():
    update existing identity payload
else:
    create identity node after validation

if auth_kind == "password":
    persist identity password secret
else:
    clear password secret and store ssh_key_id
```

**Step 4: Run test to verify it passes**

Run: `cargo test --test keychain_identity_actions_spec -- --nocapture`
Expected: PASS

Run: `cargo test --test assets_modal_smoke -- --nocapture`
Expected: PASS

Run: `cargo test --test bootstrap_smoke -- --nocapture`
Expected: PASS

**Step 5: Commit**

```bash
git add src/shell/view_model.rs src/app/bootstrap.rs ui/app-window.slint ui/components/assets-keychain-identity-modal.slint tests/keychain_identity_actions_spec.rs tests/assets_modal_smoke.rs tests/bootstrap_smoke.rs
git commit -m "feat: add keychain identity modal flows"
```

### Task 3: Finish SSH key modal actions and saved-key edit behavior

**Files:**
- Modify: `src/shell/view_model.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `src/app/keychain/resolver.rs`
- Modify: `tests/keychain_key_actions_spec.rs`
- Modify: `tests/bootstrap_smoke.rs`
- Reference: `ui/components/assets-keychain-ssh-key-modal.slint`

**Step 1: Write the failing test**

Extend coverage for:

- editing a saved SSH key rehydrates private/public key data into the modal;
- importing or pasting a private key derives public key and fingerprint;
- importing or pasting a public key only populates public metadata;
- generating a key pair creates an ed25519 key and enables `Copy Public Key`;
- saving a public-only key remains allowed as metadata, but selecting it from an identity requiring a private key fails validation.

```rust
assert!(app.get_keychain_ssh_key_modal_public_key().starts_with("ssh-ed25519 "));
assert!(app.get_keychain_ssh_key_modal_fingerprint().starts_with("SHA256:"));
assert_eq!(clipboard, app.get_keychain_ssh_key_modal_public_key().to_string());
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test keychain_key_actions_spec -- --nocapture`
Expected: FAIL because saved-key edit hydration and all action variants are not complete.

Run: `cargo test --test bootstrap_smoke -- --nocapture`
Expected: FAIL because edit/copy flows for saved SSH key entries are incomplete.

**Step 3: Write minimal implementation**

- Add `open_edit_keychain_ssh_key_modal(...)`.
- Rehydrate SSH key draft from catalog + `CredentialStore`.
- Keep the existing action ids and finish all handler branches.
- Persist updated key metadata into `KeychainCatalog` and private key material into `CredentialStore`.
- Keep resolver validation strict when a selected SSH key lacks private key material.

```text
load draft metadata from catalog
load secret bundle from keychain/key/<id>
on save:
    update algorithm/fingerprint/public_key/comment in catalog
    persist private_key_content/passphrase in credential store
```

**Step 4: Run test to verify it passes**

Run: `cargo test --test keychain_key_actions_spec -- --nocapture`
Expected: PASS

Run: `cargo test --test bootstrap_smoke -- --nocapture`
Expected: PASS

**Step 5: Commit**

```bash
git add src/shell/view_model.rs src/app/bootstrap.rs src/app/keychain/resolver.rs tests/keychain_key_actions_spec.rs tests/bootstrap_smoke.rs
git commit -m "feat: complete keychain ssh key actions"
```

### Task 4: Wire keychain-specific rename, delete, and local repo persistence

**Files:**
- Modify: `src/app/bootstrap.rs`
- Modify: `src/shell/view_model.rs`
- Modify: `src/shell/context_menu.rs`
- Modify: `src/app/keychain/repository.rs`
- Modify: `src/app/keychain/redb_store.rs`
- Modify: `tests/keychain_store_spec.rs`
- Modify: `tests/bootstrap_smoke.rs`
- Modify: `tests/assets_context_menu_smoke.rs`

**Step 1: Write the failing test**

Add tests proving:

- keychain rename and delete actions operate on keychain nodes, not console assets;
- deleting an identity or SSH key clears associated secrets only when reference checks pass;
- a persisted keychain catalog reloads after restart and preserves folder/identity/key relationships;
- bootstrap saves the keychain catalog after create/edit/rename/delete.

```rust
assert!(state.delete_keychain_item("identity-ops").is_err());
assert_eq!(reloaded.nodes["identity-ops"].title, "Ops");
assert_eq!(save_attempts.len(), 1);
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test keychain_store_spec -- --nocapture`
Expected: FAIL because bootstrap is not using the keychain repo on mutation/restart paths.

Run: `cargo test --test bootstrap_smoke -- --nocapture`
Expected: FAIL because keychain mutations do not save through a `KeychainCatalogRepository`.

Run: `cargo test --test assets_context_menu_smoke -- --nocapture`
Expected: FAIL because keychain rename/delete still route through console-asset assumptions.

**Step 3: Write minimal implementation**

- Introduce keychain repo loading/saving into bootstrap alongside asset repo loading/saving.
- Add `save_keychain_catalog_if_available(...)`.
- Keep rename/delete confirmation logic domain-aware:
  - console/snippet targets keep current path;
  - keychain targets use keychain node ids and keychain repo save hooks.
- Ensure successful delete also clears secret namespace entries.

```text
on startup:
    load keychain repo
    state.replace_keychain_catalog(loaded_catalog)

on keychain mutate:
    save keychain repo
    if delete identity/key:
        delete keychain namespace secret bundle
```

**Step 4: Run test to verify it passes**

Run: `cargo test --test keychain_store_spec -- --nocapture`
Expected: PASS

Run: `cargo test --test bootstrap_smoke -- --nocapture`
Expected: PASS

Run: `cargo test --test assets_context_menu_smoke -- --nocapture`
Expected: PASS

**Step 5: Commit**

```bash
git add src/app/bootstrap.rs src/shell/view_model.rs src/shell/context_menu.rs src/app/keychain/repository.rs src/app/keychain/redb_store.rs tests/keychain_store_spec.rs tests/bootstrap_smoke.rs tests/assets_context_menu_smoke.rs
git commit -m "feat: persist keychain catalog locally"
```

### Task 5: Finish vault dirty-marking, export/import, and sync restore for keychain mutations

**Files:**
- Modify: `src/app/bootstrap.rs`
- Modify: `src/app/vault/snapshot.rs`
- Modify: `tests/vault_snapshot_spec.rs`
- Modify: `tests/vault_bootstrap_spec.rs`
- Modify: `tests/vault_attach_merge_spec.rs`
- Reference: `src/app/vault/merge.rs`

**Step 1: Write the failing test**

Add coverage proving:

- identity and SSH key create/edit/delete mutations mark the local vault dirty;
- exported snapshots include updated keychain catalog + identity/key secret bundles;
- applying a snapshot restores keychain projection after restart/attach;
- merge/remap logic preserves:
  - identity -> key references
  - host -> identity references
  - keychain secret namespace ownership.

```rust
assert!(!snapshot.keychain_catalog.nodes.is_empty());
assert!(snapshot.keychain_identity_secret_bundles.contains_key("identity-ops"));
assert!(snapshot.keychain_key_secret_bundles.contains_key("key-prod"));
assert_eq!(restored_spec.keychain_identity_id.as_deref(), Some("identity-ops"));
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test vault_snapshot_spec -- --nocapture`
Expected: FAIL because not every keychain mutation path marks dirty or persists current state before export.

Run: `cargo test --test vault_bootstrap_spec -- --nocapture`
Expected: FAIL because restart/apply flows do not fully rebuild local keychain projection from repo + snapshot.

Run: `cargo test --test vault_attach_merge_spec -- --nocapture`
Expected: FAIL because merged snapshots may not preserve all keychain references after local edits.

**Step 3: Write minimal implementation**

- Make every successful keychain mutation call the same dirty/sync arming path used by console assets.
- Keep export/import logic in `vault/snapshot.rs` as the single serializer/deserializer seam.
- Ensure bootstrap restore order is:
  1. load/apply snapshot
  2. rebuild console + snippet + keychain projection
  3. rehydrate selected modal/projection state from restored catalog.

```text
if keychain mutation succeeded:
    save keychain repo
    mark local vault dirty
    arm auto sync
```

**Step 4: Run test to verify it passes**

Run: `cargo test --test vault_snapshot_spec -- --nocapture`
Expected: PASS

Run: `cargo test --test vault_bootstrap_spec -- --nocapture`
Expected: PASS

Run: `cargo test --test vault_attach_merge_spec -- --nocapture`
Expected: PASS

**Step 5: Commit**

```bash
git add src/app/bootstrap.rs src/app/vault/snapshot.rs tests/vault_snapshot_spec.rs tests/vault_bootstrap_spec.rs tests/vault_attach_merge_spec.rs
git commit -m "feat: sync keychain assets through vault"
```

### Task 6: Run focused regressions and one end-to-end keychain pass

**Files:**
- Modify: `docs/plans/2026-04-01-keychain-completion-design.md`
- Modify: `docs/plans/2026-04-01-keychain-completion-implementation-plan.md`
- Reference: `tests/keychain_modal_smoke.rs`
- Reference: `tests/keychain_identity_actions_spec.rs`
- Reference: `tests/keychain_key_actions_spec.rs`
- Reference: `tests/assets_context_menu_smoke.rs`
- Reference: `tests/assets_modal_smoke.rs`
- Reference: `tests/bootstrap_smoke.rs`
- Reference: `tests/keychain_store_spec.rs`
- Reference: `tests/vault_snapshot_spec.rs`
- Reference: `tests/vault_bootstrap_spec.rs`
- Reference: `tests/vault_attach_merge_spec.rs`

**Step 1: Run the focused regression set**

Run:

```bash
cargo test --test keychain_modal_smoke -- --nocapture
cargo test --test keychain_identity_actions_spec -- --nocapture
cargo test --test keychain_key_actions_spec -- --nocapture
cargo test --test assets_context_menu_smoke -- --nocapture
cargo test --test assets_modal_smoke -- --nocapture
cargo test --test bootstrap_smoke -- --nocapture
cargo test --test keychain_store_spec -- --nocapture
cargo test --test vault_snapshot_spec -- --nocapture
cargo test --test vault_bootstrap_spec -- --nocapture
cargo test --test vault_attach_merge_spec -- --nocapture
bash tests/keychain_ui_contract_smoke.sh
```

Expected: all PASS

**Step 2: Run one broader Rust verification**

Run: `cargo test -- --nocapture`
Expected: PASS, or only unrelated pre-existing failures documented explicitly.

**Step 3: Update docs with any divergence**

- If implementation differs from this plan, update:
  - `docs/plans/2026-04-01-keychain-completion-design.md`
  - `docs/plans/2026-04-01-keychain-completion-implementation-plan.md`
- Keep acceptance criteria and file references aligned with shipped behavior.

**Step 4: Commit**

```bash
git add docs/plans/2026-04-01-keychain-completion-design.md docs/plans/2026-04-01-keychain-completion-implementation-plan.md
git commit -m "docs: finalize keychain completion plan"
```

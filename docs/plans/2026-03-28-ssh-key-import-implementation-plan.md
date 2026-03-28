# SSH Key Import Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a first-class SSH key login flow that supports pasting private key text and importing a private key file through a system file picker, while preserving compatibility with legacy file-path SSH assets.

**Architecture:** Keep the existing `Password` and `PrivateKeyContent` / `PrivateKeyPath` runtime branches intact, but reshape the modal and bootstrap bridge so new SSH key flows always land in `private_key_content`. Add a native file picker on the Rust side, feed the imported file content back into modal draft state, and preserve `path` mode only for editing existing legacy assets.

**Tech Stack:** Rust, Slint, native file picker crate, credential store, `russh`

---

### Task 1: Lock the new SSH key modal behavior with failing tests

**Files:**
- Modify: `tests/assets_modal_smoke.rs`
- Modify: `tests/shell_view_model.rs`
- Modify: `tests/bootstrap_smoke.rs`
- Reference: `ui/components/assets-ssh-connection-modal.slint`
- Reference: `src/shell/view_model.rs`

**Step 1: Write the failing UI contract test**

In `tests/assets_modal_smoke.rs`, add assertions that the SSH modal contract exposes:

- an `Import` trigger for the private key field;
- explanatory copy stating that only the private key is needed locally;
- a legacy path label for old path-based assets.

Use string-based contract checks in the existing style.

**Step 2: Write the failing view-model behavior tests**

In `tests/shell_view_model.rs`, add tests that assert:

- a new SSH key draft defaults to `private_key_source = "content"`;
- editing a saved `path` asset still keeps the legacy path field visible and valid;
- applying imported private key content switches the draft to `content` mode.

**Step 3: Write the failing bootstrap import feedback tests**

In `tests/bootstrap_smoke.rs`, add tests that assert:

- import success updates the window-facing `asset_ssh_modal_private_key_content`;
- import failure sets SSH modal feedback to an error message;
- import cancellation leaves the draft unchanged.

**Step 4: Run the focused tests to confirm failure**

Run:

```bash
cargo test --test assets_modal_smoke --test shell_view_model --test bootstrap_smoke -- --nocapture
```

Expected:

- FAIL because the modal does not yet expose import behavior or legacy path UX.

**Step 5: Commit**

```bash
git add tests/assets_modal_smoke.rs tests/shell_view_model.rs tests/bootstrap_smoke.rs
git commit -m "test: lock ssh key import modal behavior"
```

### Task 2: Update the SSH modal UI contract for paste/import-first SSH key flows

**Files:**
- Modify: `ui/components/assets-ssh-connection-modal.slint`
- Modify: `ui/app-window.slint`
- Test: `tests/assets_modal_smoke.rs`
- Test: `tests/assets_modal_ui_contract_smoke.sh`

**Step 1: Add the new SSH key explanatory copy**

In `ui/components/assets-ssh-connection-modal.slint`, render a short note inside the `SSH Key` section:

```text
Only the private key is needed here. The public key must already be installed on the server.
```

**Step 2: Add an import trigger to the private key content field**

Extend the existing private key content field so it can fire a dedicated import request. Keep direct text pasting intact.

Use the existing modal callback pattern instead of inventing a side channel.

**Step 3: Add a legacy path presentation for old assets**

When the modal is editing a legacy path-based asset, show a `Legacy File Path` field instead of surfacing the old `File Path` choice as a first-class new-connection option.

**Step 4: Bridge any new UI properties/callbacks through `ui/app-window.slint`**

Project the new callback and any new window properties needed by bootstrap.

**Step 5: Run the focused modal contract tests**

Run:

```bash
cargo test --test assets_modal_smoke -- --nocapture
bash tests/assets_modal_ui_contract_smoke.sh
```

Expected:

- PASS for the new UI contract checks.

**Step 6: Commit**

```bash
git add ui/components/assets-ssh-connection-modal.slint ui/app-window.slint tests/assets_modal_smoke.rs tests/assets_modal_ui_contract_smoke.sh
git commit -m "feat: update ssh modal for key import flow"
```

### Task 3: Extend the shell view model for imported-key draft transitions

**Files:**
- Modify: `src/shell/view_model.rs`
- Test: `tests/shell_view_model.rs`

**Step 1: Add a view-model entry point for imported private key content**

In `src/shell/view_model.rs`, add a focused helper that applies imported key content to the active SSH modal draft.

The helper should:

- no-op if the SSH modal is not open;
- switch `auth_method` to `private-key`;
- switch `private_key_source` to `content`;
- write `private_key_content`;
- clear stale validation or feedback only as needed.

**Step 2: Preserve legacy path mode for unchanged saved assets**

Keep the existing `path` validation and save behavior when a user opens an old path-based asset and does not replace it with imported or pasted content.

**Step 3: Run the focused view-model tests**

Run:

```bash
cargo test --test shell_view_model -- --nocapture
```

Expected:

- PASS for new imported-key transitions;
- PASS for existing SSH modal validation tests.

**Step 4: Commit**

```bash
git add src/shell/view_model.rs tests/shell_view_model.rs
git commit -m "feat: support imported ssh key drafts"
```

### Task 4: Add native private key file import handling in bootstrap

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/app/bootstrap.rs`
- Test: `tests/bootstrap_smoke.rs`
- Reference: `src/app/ssh/credentials.rs`

**Step 1: Add the native file picker dependency**

Add a small native file picker crate appropriate for Slint + desktop Rust. Keep the dependency surface minimal.

**Step 2: Add a tiny file-import abstraction**

In `src/app/bootstrap.rs`, introduce a small abstraction that can be replaced in tests. It should support:

- open file picker;
- return selected path or cancellation;
- read file content as text.

Do not hardwire direct OS dialogs into test-only code paths.

**Step 3: Handle the new SSH modal import action**

Wire the new modal callback/action so bootstrap:

- opens the file picker;
- handles cancellation without error;
- reads the selected file content;
- applies it to the modal draft through the view model;
- sets error feedback if file open/read fails.

**Step 4: Keep secret persistence unchanged**

Ensure imported content still persists through the existing saved-secret bundle path, not in asset metadata.

**Step 5: Run the focused bootstrap tests**

Run:

```bash
cargo test --test bootstrap_smoke -- --nocapture
```

Expected:

- PASS for import success, cancellation, and failure coverage;
- PASS for existing SSH modal save/connect tests.

**Step 6: Commit**

```bash
git add Cargo.toml src/app/bootstrap.rs tests/bootstrap_smoke.rs
git commit -m "feat: import ssh private keys from file picker"
```

### Task 5: Verify profile and credential persistence still normalize to content mode

**Files:**
- Modify: `tests/ssh_profile_spec.rs`
- Modify: `tests/credential_store_spec.rs`
- Reference: `src/app/ssh/profile.rs`
- Reference: `src/app/ssh/credentials.rs`

**Step 1: Add imported-key normalization coverage**

In `tests/ssh_profile_spec.rs`, add a case that simulates imported private key content and asserts the profile normalizes to `SshAuthMethod::PrivateKeyContent`.

**Step 2: Add secret persistence coverage**

In `tests/credential_store_spec.rs`, add a case that verifies imported content and passphrase persist through the existing secret bundle path exactly like pasted content.

**Step 3: Run the focused profile/credential tests**

Run:

```bash
cargo test --test ssh_profile_spec --test credential_store_spec -- --nocapture
```

Expected:

- PASS

**Step 4: Commit**

```bash
git add tests/ssh_profile_spec.rs tests/credential_store_spec.rs
git commit -m "test: cover imported ssh key persistence"
```

### Task 6: Run end-to-end regression verification

**Files:**
- No source changes required unless verification reveals a regression

**Step 1: Run the SSH modal and persistence regression suite**

Run:

```bash
cargo test --test assets_modal_smoke --test shell_view_model --test bootstrap_smoke --test ssh_profile_spec --test credential_store_spec -- --nocapture
```

Expected:

- PASS

**Step 2: Run workspace validation**

Run:

```bash
cargo check --workspace
```

Expected:

- PASS

**Step 3: Commit if verification required fixes**

If Task 6 required follow-up edits:

```bash
git add <touched-files>
git commit -m "fix: resolve ssh key import regressions"
```

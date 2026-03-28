# SSH Proxy Dropdown / HTTP Proxy Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace free-form SSH proxy inputs with dropdown-driven selection, add HTTP CONNECT proxy support, and prevent self-proxy selection while editing an SSH asset.

**Architecture:** The modal stays Rust-driven through `ShellViewModel` and `bootstrap` projection. Proxy support extends the existing saved asset schema, secret bundle handling, profile normalization, recursive proxy-chain resolution, and runtime transport setup.

**Tech Stack:** Rust, Slint, Tokio, russh, redb, serde

---

### Task 1: Add failing UI and view-model tests

**Files:**
- Modify: `tests/assets_modal_smoke.rs`
- Modify: `tests/bootstrap_smoke.rs`
- Modify: `tests/shell_view_model.rs`

**Step 1: Write the failing tests**

- Add a modal round-trip test for HTTP proxy fields.
- Add a bootstrap projection test for upstream SSH dropdown options excluding the edited asset itself.
- Add a view-model test for selecting `http` proxy draft fields.

**Step 2: Run test to verify it fails**

Run: `cargo test --test assets_modal_smoke --test bootstrap_smoke --test shell_view_model`

Expected: FAIL because HTTP fields and SSH dropdown option projection do not exist yet.

**Step 3: Write minimal implementation**

- Add new Slint/AppWindow properties for HTTP fields and SSH proxy option labels.
- Add view-model helpers that derive eligible upstream SSH options from the asset tree.

**Step 4: Run test to verify it passes**

Run: `cargo test --test assets_modal_smoke --test bootstrap_smoke --test shell_view_model`

Expected: PASS for the new cases.

### Task 2: Add failing proxy model and persistence tests

**Files:**
- Modify: `tests/ssh_profile_spec.rs`
- Modify: `tests/assets_catalog_domain.rs`
- Modify: `tests/credential_store_spec.rs`

**Step 1: Write the failing tests**

- Add HTTP draft normalization and saved-asset normalization tests.
- Add persistence round-trip coverage for HTTP proxy spec.
- Add saved-secret bundle coverage for HTTP proxy password merge/persist behavior.

**Step 2: Run test to verify it fails**

Run: `cargo test --test ssh_profile_spec --test assets_catalog_domain --test credential_store_spec`

Expected: FAIL because `http` proxy types and password bundle fields are not modeled yet.

**Step 3: Write minimal implementation**

- Extend runtime/persisted asset proxy enums with `http`.
- Extend secret bundle storage with `proxy_http_password`.
- Normalize/build HTTP proxy specs from drafts and saved assets.

**Step 4: Run test to verify it passes**

Run: `cargo test --test ssh_profile_spec --test assets_catalog_domain --test credential_store_spec`

Expected: PASS.

### Task 3: Add failing runtime HTTP CONNECT tests

**Files:**
- Modify: `tests/ssh_session_manager_spec.rs`
- Modify: `src/app/ssh/runtime.rs`

**Step 1: Write the failing tests**

- Add a fake HTTP CONNECT proxy server helper.
- Add one success test for unauthenticated HTTP CONNECT.
- Add one rejection test for bad HTTP proxy credentials.

**Step 2: Run test to verify it fails**

Run: `cargo test --test ssh_session_manager_spec http_proxy`

Expected: FAIL because runtime only supports SOCKS5 and SSH upstream transport hops.

**Step 3: Write minimal implementation**

- Add HTTP proxy hop/profile variants.
- Open a TCP stream to the HTTP proxy, send `CONNECT host:port HTTP/1.1`, optionally emit `Proxy-Authorization: Basic ...`, and accept only successful `2xx` responses.
- Treat HTTP proxy hops as outermost-only transports like SOCKS5.

**Step 4: Run test to verify it passes**

Run: `cargo test --test ssh_session_manager_spec http_proxy`

Expected: PASS.

### Task 4: Integrate modal UX and bootstrap projection

**Files:**
- Modify: `ui/components/assets-ssh-connection-modal.slint`
- Modify: `ui/app-window.slint`
- Modify: `src/app/bootstrap.rs`

**Step 1: Write the failing test**

- Add/extend UI contract assertions so the modal uses `ComboBox` for proxy type and upstream SSH connection selection.

**Step 2: Run test to verify it fails**

Run: `cargo test --test assets_modal_smoke && bash tests/assets_modal_ui_contract_smoke.sh`

Expected: FAIL because the modal still uses segmented buttons and free-text upstream SSH input.

**Step 3: Write minimal implementation**

- Replace proxy type buttons with a `ComboBox`.
- Render HTTP fields parallel to SOCKS5 fields.
- Replace upstream SSH text field with a `ComboBox` fed by bootstrap-projected labels.

**Step 4: Run test to verify it passes**

Run: `cargo test --test assets_modal_smoke && bash tests/assets_modal_ui_contract_smoke.sh`

Expected: PASS.

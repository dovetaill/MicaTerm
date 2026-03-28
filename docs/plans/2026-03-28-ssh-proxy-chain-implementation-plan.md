# SSH Proxy Chain Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add first-class SSH proxy support with single-upstream recursion, allowing each SSH asset to use either a SOCKS5 proxy or another saved SSH asset as its upstream, and automatically expand chains such as `SOCKS5 -> B -> A -> C` at connect time.

**Architecture:** Replace the legacy `proxy_method` string with structured proxy metadata in the asset catalog and modal draft. Normalize direct proxy settings into `ConnectionProfile`, resolve recursive SSH upstream references into a flat runtime hop list before opening or probing sessions, then build the transport chain in `SshSessionRuntime` with `russh::client::connect_stream` plus `channel_open_direct_tcpip`. Keep proxy passwords in the credential store, not in asset metadata.

**Tech Stack:** Rust, Slint, redb, serde, keyring-backed credential store, Tokio async IO, `russh`, cargo test

**Execution Rules:** Use `@superpowers:test-driven-development` on every task. Before declaring the feature done, run `@superpowers:verification-before-completion`.

---

### Task 1: Lock the structured proxy schema and migration rules with failing tests

**Files:**
- Modify: `tests/assets_catalog_domain.rs`
- Modify: `tests/assets_catalog_store.rs`
- Modify: `src/shell/assets.rs`
- Modify: `src/app/assets_catalog/model.rs`
- Modify: `src/app/assets_catalog/mapper.rs`
- Modify: `src/app/assets_catalog/redb_store.rs`

**Step 1: Write the failing asset-domain tests**

In `tests/assets_catalog_domain.rs`, add coverage for the new structured proxy spec on SSH assets. Add assertions for:

- `proxy = None` as the default;
- SOCKS5 proxy metadata round-tripping through runtime asset nodes;
- SSH upstream asset references round-tripping through runtime asset nodes.

Use concrete values such as:

```rust
AssetSshProxySpec::Socks5(AssetSocks5ProxySpec {
    host: "proxy.example.net".into(),
    port: "1080".into(),
    username: "ops-proxy".into(),
    password_credential_ref: Some("ssh/saved-secrets/asset-a".into()),
})
```

**Step 2: Write the failing store/migration tests**

In `tests/assets_catalog_store.rs`, add:

- a round-trip test for a persisted SOCKS5 proxy spec;
- a round-trip test for an SSH upstream asset reference;
- a migration test proving an older node with only `proxy_method` loads as `proxy = None`.

**Step 3: Run the focused tests to confirm failure**

Run:

```bash
cargo test --test assets_catalog_domain --test assets_catalog_store -- --nocapture
```

Expected:

- FAIL because the catalog models and serde layer still only know about `proxy_method: String`.

**Step 4: Implement the structured proxy schema**

Make these model changes:

- In `src/shell/assets.rs`, replace `proxy_method: String` with:

```rust
pub enum AssetSshProxySpec {
    None,
    Socks5(AssetSocks5ProxySpec),
    SshAsset { asset_id: String },
}

pub struct AssetSocks5ProxySpec {
    pub host: String,
    pub port: String,
    pub username: String,
    pub password_credential_ref: Option<String>,
}
```

- Mirror the same structure in `src/app/assets_catalog/model.rs`;
- Bump `ASSET_CATALOG_SCHEMA_VERSION`;
- In `src/app/assets_catalog/redb_store.rs`, keep backward compatibility by deserializing missing proxy fields as `None`, and stop writing `proxy_method`.

**Step 5: Run the focused schema tests**

Run:

```bash
cargo test --test assets_catalog_domain --test assets_catalog_store -- --nocapture
```

Expected:

- PASS for new structured proxy round-trips;
- PASS for old-schema compatibility loading as `None`.

**Step 6: Commit**

```bash
git add src/shell/assets.rs src/app/assets_catalog/model.rs src/app/assets_catalog/mapper.rs src/app/assets_catalog/redb_store.rs tests/assets_catalog_domain.rs tests/assets_catalog_store.rs
git commit -m "feat: add structured ssh proxy schema"
```

### Task 2: Lock the SSH modal proxy UI contract with failing tests

**Files:**
- Modify: `tests/assets_modal_smoke.rs`
- Modify: `tests/assets_modal_ui_contract_smoke.sh`
- Modify: `tests/shell_view_model.rs`
- Modify: `ui/components/assets-ssh-connection-modal.slint`
- Modify: `ui/app-window.slint`
- Modify: `src/shell/view_model.rs`

**Step 1: Write the failing modal contract tests**

In `tests/assets_modal_smoke.rs` and `tests/assets_modal_ui_contract_smoke.sh`, add assertions that the SSH modal now exposes:

- a `Proxy` section header;
- a `Proxy Type` control with `None`, `SOCKS5`, and `Existing SSH Connection`;
- conditional SOCKS5 fields:
  - `SOCKS5 Host`
  - `SOCKS5 Port`
  - `Username`
  - `Password`
- a conditional upstream SSH selector field.

**Step 2: Write the failing view-model draft tests**

In `tests/shell_view_model.rs`, add tests asserting:

- a new SSH draft defaults to `proxy_type = "none"`;
- selecting SOCKS5 updates only SOCKS5 draft fields;
- selecting `ssh-asset` stores the chosen upstream asset ID;
- switching proxy type clears stale validation text.

**Step 3: Run the focused tests to confirm failure**

Run:

```bash
cargo test --test assets_modal_smoke --test shell_view_model -- --nocapture
bash tests/assets_modal_ui_contract_smoke.sh
```

Expected:

- FAIL because the modal and view-model still expose only legacy non-functional proxy placeholders.

**Step 4: Implement the modal and draft contract**

Update `src/shell/view_model.rs` and `ui/components/assets-ssh-connection-modal.slint` so the SSH modal draft includes:

```rust
pub proxy_type: String, // "none" | "socks5" | "ssh-asset"
pub proxy_socks5_host: String,
pub proxy_socks5_port: String,
pub proxy_socks5_username: String,
pub proxy_socks5_password: String,
pub proxy_socks5_password_visible: bool,
pub proxy_ssh_asset_id: String,
```

Wire the new fields through `ui/app-window.slint`, keep the current modal layout style, and avoid reintroducing top-level tabs.

**Step 5: Run the focused UI tests**

Run:

```bash
cargo test --test assets_modal_smoke --test shell_view_model -- --nocapture
bash tests/assets_modal_ui_contract_smoke.sh
```

Expected:

- PASS for modal contract and draft transitions.

**Step 6: Commit**

```bash
git add ui/components/assets-ssh-connection-modal.slint ui/app-window.slint src/shell/view_model.rs tests/assets_modal_smoke.rs tests/assets_modal_ui_contract_smoke.sh tests/shell_view_model.rs
git commit -m "feat: add ssh proxy modal fields"
```

### Task 3: Persist SOCKS5 secrets and normalize proxy-aware connection profiles

**Files:**
- Modify: `src/app/ssh/credentials.rs`
- Modify: `src/app/ssh/profile.rs`
- Modify: `tests/credential_store_spec.rs`
- Modify: `tests/ssh_profile_spec.rs`
- Reference: `src/shell/assets.rs`
- Reference: `src/shell/view_model.rs`

**Step 1: Write the failing credential-store tests**

In `tests/credential_store_spec.rs`, add coverage proving:

- SOCKS5 proxy passwords persist in the saved secret bundle;
- blank SOCKS5 passwords clear the stored secret;
- existing SSH auth secrets still behave exactly as before.

Add a new field expectation such as:

```rust
assert_eq!(bundle.proxy_socks5_password.as_deref(), Some("proxy-secret"));
```

**Step 2: Write the failing profile normalization tests**

In `tests/ssh_profile_spec.rs`, add tests asserting that:

- a draft with `proxy_type = "socks5"` normalizes into a typed SOCKS5 proxy profile;
- a saved asset with `proxy = SshAsset { asset_id: ... }` preserves the upstream asset reference;
- invalid SOCKS5 ports fail early during profile normalization.

**Step 3: Run the focused tests to confirm failure**

Run:

```bash
cargo test --test credential_store_spec --test ssh_profile_spec -- --nocapture
```

Expected:

- FAIL because neither the secret bundle nor `ConnectionProfile` currently models proxy data.

**Step 4: Implement the proxy-aware profile model**

Extend `StoredSshSecretBundle` with:

```rust
pub proxy_socks5_password: Option<String>,
```

Extend `ConnectionProfile` with a direct proxy field and an initially-empty resolved hop list:

```rust
pub enum ConnectionProxyProfile {
    None,
    Socks5 {
        host: String,
        port: u16,
        username: Option<String>,
        password: Option<String>,
        credential_ref: Option<String>,
    },
    SshAsset { asset_id: String },
}

pub enum ResolvedProxyHop {
    Socks5 {
        host: String,
        port: u16,
        username: Option<String>,
        password: Option<String>,
    },
    Ssh(Box<ConnectionProfile>),
}
```

Have `from_draft` / `from_saved_asset` populate only the direct `proxy` field and leave `resolved_proxy_hops` empty. Keep existing SSH auth normalization intact.

**Step 5: Run the focused profile/secret tests**

Run:

```bash
cargo test --test credential_store_spec --test ssh_profile_spec -- --nocapture
```

Expected:

- PASS for proxy password persistence;
- PASS for direct proxy normalization and early validation.

**Step 6: Commit**

```bash
git add src/app/ssh/credentials.rs src/app/ssh/profile.rs tests/credential_store_spec.rs tests/ssh_profile_spec.rs
git commit -m "feat: normalize proxy-aware ssh profiles"
```

### Task 4: Resolve recursive upstream SSH references into a flat runtime hop list

**Files:**
- Create: `src/app/ssh/proxy.rs`
- Modify: `src/app/ssh/mod.rs`
- Modify: `tests/ssh_profile_spec.rs`
- Modify: `src/app/bootstrap.rs`
- Reference: `src/shell/assets.rs`
- Reference: `src/app/ssh/profile.rs`

**Step 1: Write the failing proxy-chain resolver tests**

In `tests/ssh_profile_spec.rs`, add resolver-level tests for:

- `B = SOCKS5`, `A = SSH(B)`, `C = SSH(A)` expanding to hops `[SOCKS5, SSH(B), SSH(A)]`;
- `A -> B -> A` reporting a cycle;
- a missing upstream asset reporting a missing-reference error;
- a chain deeper than 8 hops reporting `too deep`.

Use a small in-memory `AssetTree` with concrete asset IDs such as `asset-b`, `asset-a`, and `asset-c`.

**Step 2: Run the focused tests to confirm failure**

Run:

```bash
cargo test --test ssh_profile_spec -- --nocapture
```

Expected:

- FAIL because there is no recursive proxy resolver yet.

**Step 3: Implement the resolver module**

Create `src/app/ssh/proxy.rs` with a focused API:

```rust
pub fn resolve_proxy_chain(
    tree: &AssetTree,
    profile: &ConnectionProfile,
    max_depth: usize,
) -> anyhow::Result<Vec<ResolvedProxyHop>>
```

Rules:

- recurse only through saved SSH assets;
- reject self-reference and cycles;
- cap depth at `8`;
- for each upstream SSH asset, normalize it with `ConnectionProfile::from_saved_asset`, then push it as `ResolvedProxyHop::Ssh(Box::new(upstream_profile_without_hops))`;
- for SOCKS5, materialize a `ResolvedProxyHop::Socks5`.

**Step 4: Wire bootstrap to resolve hops before probe/open**

In `src/app/bootstrap.rs`, add a helper that:

- builds the direct `ConnectionProfile`;
- resolves `resolved_proxy_hops` from the asset tree;
- returns the runtime-ready profile used for `probe_connection` and `open_session`.

Do not resolve proxy chains when merely hydrating the edit modal.

**Step 5: Run the focused resolver tests**

Run:

```bash
cargo test --test ssh_profile_spec -- --nocapture
```

Expected:

- PASS for recursive chain expansion and all error cases.

**Step 6: Commit**

```bash
git add src/app/ssh/mod.rs src/app/ssh/proxy.rs src/app/bootstrap.rs tests/ssh_profile_spec.rs
git commit -m "feat: resolve recursive ssh proxy chains"
```

### Task 5: Wire bootstrap feedback and selection behavior for proxy-aware SSH assets

**Files:**
- Modify: `src/app/bootstrap.rs`
- Modify: `tests/bootstrap_smoke.rs`
- Modify: `tests/assets_modal_smoke.rs`
- Reference: `src/shell/view_model.rs`

**Step 1: Write the failing bootstrap smoke tests**

In `tests/bootstrap_smoke.rs`, add coverage for:

- editing a saved SOCKS5-backed SSH asset and seeing all proxy fields projected into the window;
- editing an upstream-SSH-backed asset and seeing the upstream asset ID projected correctly;
- attempting to test/connect with a missing upstream asset and receiving inline feedback;
- attempting to save a self-referential upstream SSH asset and being blocked before runtime launch.

**Step 2: Run the focused tests to confirm failure**

Run:

```bash
cargo test --test bootstrap_smoke --test assets_modal_smoke -- --nocapture
```

Expected:

- FAIL because bootstrap still only bridges the old proxy placeholder fields.

**Step 3: Implement bootstrap projection and validation**

Update `src/app/bootstrap.rs` so the window bridge:

- sets and reads all new proxy properties;
- hydrates proxy passwords from the credential store when editing;
- persists proxy passwords back into the secret bundle on save;
- surfaces resolver errors in the modal feedback banner instead of silently failing.

Keep `None` proxy assets as the fast path.

**Step 4: Run the focused bootstrap tests**

Run:

```bash
cargo test --test bootstrap_smoke --test assets_modal_smoke -- --nocapture
```

Expected:

- PASS for edit hydration, save validation, and modal feedback coverage.

**Step 5: Commit**

```bash
git add src/app/bootstrap.rs tests/bootstrap_smoke.rs tests/assets_modal_smoke.rs
git commit -m "feat: wire bootstrap ssh proxy behavior"
```

### Task 6: Implement SOCKS5 transport setup before the target SSH handshake

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/app/ssh/runtime.rs`
- Modify: `tests/ssh_session_manager_spec.rs`
- Reference: `src/app/ssh/profile.rs`

**Step 1: Write the failing SOCKS5 runtime tests**

In `tests/ssh_session_manager_spec.rs`, add runtime-level tests for:

- unauthenticated SOCKS5 CONNECT to a test SSH server;
- username/password SOCKS5 CONNECT to a test SSH server;
- SOCKS5 auth rejection surfacing as a runtime error.

Keep the fake SOCKS5 server minimal:

- parse greeting;
- negotiate either no-auth or username/password;
- forward bytes to the requested target address.

**Step 2: Run the focused tests to confirm failure**

Run:

```bash
cargo test --test ssh_session_manager_spec socks5 -- --nocapture
```

Expected:

- FAIL because runtime still always opens the target SSH connection directly via `client::connect`.

**Step 3: Implement SOCKS5 dial support**

In `src/app/ssh/runtime.rs`:

- add `tokio` features needed for `TcpStream` and async read/write helpers;
- extract a helper that opens the outermost transport stream;
- implement a minimal SOCKS5 handshake covering:
  - no-auth;
  - username/password auth;
  - CONNECT to the next host/port.

Use concrete helpers such as:

```rust
async fn connect_via_socks5(
    proxy_host: &str,
    proxy_port: u16,
    username: Option<&str>,
    password: Option<&str>,
    target_host: &str,
    target_port: u16,
) -> Result<TcpStream>
```

Then feed the returned stream into `russh::client::connect_stream`.

**Step 4: Run the focused SOCKS5 tests**

Run:

```bash
cargo test --test ssh_session_manager_spec socks5 -- --nocapture
```

Expected:

- PASS for no-auth and username/password SOCKS5 paths;
- PASS for explicit auth-failure reporting.

**Step 5: Commit**

```bash
git add Cargo.toml src/app/ssh/runtime.rs tests/ssh_session_manager_spec.rs
git commit -m "feat: add socks5 transport for ssh runtime"
```

### Task 7: Implement SSH upstream direct-tcpip hops and multi-hop chain traversal

**Files:**
- Modify: `src/app/ssh/runtime.rs`
- Modify: `tests/ssh_session_manager_spec.rs`
- Modify: `tests/bootstrap_smoke.rs`
- Reference: `src/app/ssh/proxy.rs`

**Step 1: Write the failing multi-hop runtime tests**

In `tests/ssh_session_manager_spec.rs`, add tests for:

- a single upstream SSH asset using `direct-tcpip` to reach the final target;
- `SOCKS5 -> SSH(B) -> SSH(A) -> target` succeeding end-to-end;
- an upstream server rejecting `direct-tcpip` surfacing as a clear runtime error.

Reuse the existing SSH server harness in this file instead of inventing a second one.

**Step 2: Run the focused tests to confirm failure**

Run:

```bash
cargo test --test ssh_session_manager_spec direct_tcpip -- --nocapture
cargo test --test ssh_session_manager_spec multi_hop -- --nocapture
```

Expected:

- FAIL because resolved proxy hops are not yet consumed by runtime transport setup.

**Step 3: Implement hop-by-hop SSH transport chaining**

In `src/app/ssh/runtime.rs`, change the connect path so it:

1. Starts from either:
   - direct TCP to the first SSH host; or
   - SOCKS5-connected stream from Task 6;
2. For each `ResolvedProxyHop::Ssh(upstream)`:
   - open an SSH client on the current stream with `connect_stream`;
   - authenticate using the upstream profile;
   - request `channel_open_direct_tcpip` to the next host/port;
   - use that channel as the stream for the next hop;
3. Opens the final target SSH session only after all proxy hops are in place.

Keep all intermediate handles/channels alive in a small guard struct so dropping the runtime does not orphan the chain.

**Step 4: Run the focused multi-hop tests**

Run:

```bash
cargo test --test ssh_session_manager_spec direct_tcpip -- --nocapture
cargo test --test ssh_session_manager_spec multi_hop -- --nocapture
```

Expected:

- PASS for single-hop upstream SSH;
- PASS for recursive `SOCKS5 -> SSH -> SSH -> target`;
- PASS for direct-tcpip rejection handling.

**Step 5: Commit**

```bash
git add src/app/ssh/runtime.rs tests/ssh_session_manager_spec.rs tests/bootstrap_smoke.rs
git commit -m "feat: support multi-hop ssh proxy chains"
```

### Task 8: Run full regression verification

**Files:**
- No source changes required unless verification reveals regressions

**Step 1: Run the focused SSH proxy regression suite**

Run:

```bash
cargo test --test assets_catalog_domain --test assets_catalog_store --test credential_store_spec --test ssh_profile_spec --test assets_modal_smoke --test shell_view_model --test bootstrap_smoke --test ssh_session_manager_spec -- --nocapture
```

Expected:

- PASS

**Step 2: Run a workspace compile check**

Run:

```bash
cargo check --workspace
```

Expected:

- PASS

**Step 3: Commit only if verification required follow-up fixes**

```bash
git add <touched-files>
git commit -m "fix: resolve ssh proxy chain regressions"
```

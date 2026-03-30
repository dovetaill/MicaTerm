# SSH Connection Timeline Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace the blocking SSH open flow with an asynchronous, stepwise connection timeline in the workspace so slow SSH, proxy, and jump-host chains stay responsive and visibly explain what the client is doing.

**Architecture:** Add a structured connection-progress model shared by the SSH runtime, session manager, bootstrap projection, and workspace UI. The runtime will emit hop-aware step events instead of exposing only coarse `Connected` / `Error` transitions, and the workspace host will render those events as a connection timeline with diagnostics, cancel, retry, and inline host-key decisions.

**Tech Stack:** Rust, Tokio, Slint, existing `russh` SSH runtime, existing SSH proxy-chain support, focused cargo tests, existing workspace UI contract tests

---

### Task 1: Lock The New Non-Blocking Connection Contract In Tests

**Files:**
- Modify: `tests/ssh_session_manager_spec.rs`
- Modify: `tests/bootstrap_smoke.rs`
- Modify: `tests/workspace_tabs_spec.rs`
- Reference: `src/app/ssh/session_manager.rs`
- Reference: `src/app/bootstrap.rs`
- Reference: `src/shell/view_model.rs`

**Step 1: Write the failing test**

Add tests that lock these new expectations:

- opening an SSH asset returns immediately and does not synchronously wait for `probe_connection()`;
- a newly opened slow SSH session projects into a workspace `connection-progress` host mode instead of `terminal`;
- connecting tabs still appear immediately even if the launcher delays actual runtime attachment.

Suggested coverage shape:

```rust
#[test]
fn opening_slow_ssh_session_returns_before_runtime_connects() { /* ... */ }

#[test]
fn connecting_workspace_session_uses_connection_progress_mode() { /* ... */ }
```

Use a fake launcher that delays completion so the tests can assert state before the runtime resolves.

**Step 2: Run test to verify it fails**

Run: `cargo test --test ssh_session_manager_spec --test bootstrap_smoke --test workspace_tabs_spec opening_slow -- --nocapture`

Expected: FAIL because the current open-session path still blocks on `probe_connection()` and the workspace still projects `connecting` into the terminal host.

**Step 3: Write minimal implementation**

Make the smallest code changes necessary to express the new contract:

- stop asserting the old `connecting -> terminal` projection;
- add any tiny fake-launcher seams required for delayed runtime attachment tests;
- do not implement the full timeline model yet.

**Step 4: Run test to verify it passes or fails for the next right reason**

Run: `cargo test --test ssh_session_manager_spec --test bootstrap_smoke --test workspace_tabs_spec opening_slow -- --nocapture`

Expected: tests now fail because the application still has no connection-progress model or UI, not because the contract is underspecified.

**Step 5: Commit**

```bash
git add tests/ssh_session_manager_spec.rs tests/bootstrap_smoke.rs tests/workspace_tabs_spec.rs
git commit -m "test: lock ssh connection timeline opening contract"
```

### Task 2: Add A Structured Connection Progress Domain Model

**Files:**
- Create: `src/app/ssh/connection_progress.rs`
- Modify: `src/app/ssh/mod.rs`
- Modify: `src/app/ssh/session_manager.rs`
- Modify: `src/shell/tabs.rs`
- Modify: `src/shell/view_model.rs`
- Test: `tests/ssh_session_manager_spec.rs`
- Test: `tests/workspace_tabs_spec.rs`

**Step 1: Write the failing test**

Add coverage for a shared connection model that can represent:

- top-level session states such as `connecting`, `waiting-user`, `connected`, `cancelled`, and `error`;
- ordered timeline steps with `pending`, `running`, `done`, `blocked`, `failed`, and `cancelled`;
- per-attempt diagnostics buffering and attempt scoping;
- workspace host projection to `connection-progress`.

Suggested API shape:

```rust
pub enum ConnectionHeadlineState { Connecting, WaitingUser, Connected, Cancelled, Error }

pub enum ConnectionStepState { Pending, Running, Done, Blocked, Failed, Cancelled }

pub struct ConnectionAttemptState {
    pub attempt_id: Uuid,
    pub headline: ConnectionHeadlineState,
    pub steps: Vec<ConnectionStepStateItem>,
    pub diagnostics: Vec<ConnectionDiagnosticLine>,
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test ssh_session_manager_spec --test workspace_tabs_spec connection_progress -- --nocapture`

Expected: FAIL because there is no shared connection-progress module or workspace projection yet.

**Step 3: Write minimal implementation**

Implement the model layer and wire it into the session manager registry and tab/view-model projection:

- add `connection_progress.rs` as the shared data model;
- store per-session attempt state in `SessionRegistry`;
- extend `WorkspaceTab` and `ShellViewModel` projection so connecting sessions no longer pretend they already have a terminal surface;
- add a `connection-progress` workspace host mode.

Keep this step purely structural. Do not emit real runtime steps yet.

**Step 4: Run test to verify it passes**

Run: `cargo test --test ssh_session_manager_spec --test workspace_tabs_spec connection_progress -- --nocapture`

Expected: PASS

**Step 5: Commit**

```bash
git add src/app/ssh/connection_progress.rs src/app/ssh/mod.rs src/app/ssh/session_manager.rs src/shell/tabs.rs src/shell/view_model.rs tests/ssh_session_manager_spec.rs tests/workspace_tabs_spec.rs
git commit -m "feat: add ssh connection progress state model"
```

### Task 3: Emit Hop-Aware Runtime Step Events From The SSH Pipeline

**Files:**
- Modify: `src/app/ssh/runtime.rs`
- Modify: `src/app/ssh/session_manager.rs`
- Create: `tests/ssh_connection_timeline_spec.rs`
- Reference: `src/app/ssh/connection_progress.rs`
- Reference: `src/app/ssh/profile.rs`

**Step 1: Write the failing test**

Add focused runtime/session-manager tests proving that direct, proxy, and jump-host flows emit meaningful ordered steps, for example:

- `resolve-profile`
- `connect-proxy`
- `connect-jump-host`
- `authenticate-jump-host`
- `open-direct-tcpip`
- `connect-target`
- `verify-host-key`
- `authenticate-target`
- `request-pty`
- `request-shell`

Also lock that step failure pins the correct hop label and message instead of collapsing into a generic error.

Suggested test shape:

```rust
#[tokio::test]
async fn multi_hop_connection_emits_timeline_steps_in_order() { /* ... */ }

#[tokio::test]
async fn jump_host_failure_is_reported_on_the_failing_hop() { /* ... */ }
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test ssh_connection_timeline_spec --test ssh_session_manager_spec multi_hop_connection -- --nocapture`

Expected: FAIL because the runtime currently emits only coarse `Connected` / `Error` events.

**Step 3: Write minimal implementation**

In `src/app/ssh/runtime.rs`:

- add structured attempt and step events alongside existing surface events;
- emit step start/finish/failure markers at these boundaries:
  - profile resolution;
  - TCP connect to proxy;
  - proxy negotiation;
  - SSH connect/auth per jump host;
  - `direct-tcpip` open to the next hop;
  - target connect/auth;
  - host key verification;
  - session channel / PTY / shell startup;
- append concise diagnostics strings for copy/export later;
- scope events to an attempt ID so retries cannot corrupt older attempt state.

In `src/app/ssh/session_manager.rs`:

- consume the new events into the stored `ConnectionAttemptState`;
- update the top-level session state to `waiting-user`, `connected`, or `error` as appropriate.

**Step 4: Run test to verify it passes**

Run: `cargo test --test ssh_connection_timeline_spec --test ssh_session_manager_spec multi_hop_connection -- --nocapture`

Expected: PASS

**Step 5: Commit**

```bash
git add src/app/ssh/runtime.rs src/app/ssh/session_manager.rs src/app/ssh/connection_progress.rs tests/ssh_connection_timeline_spec.rs tests/ssh_session_manager_spec.rs
git commit -m "feat: emit ssh connection timeline runtime events"
```

### Task 4: Replace The Empty Connecting Terminal With A Timeline UI

**Files:**
- Modify: `ui/app-window.slint`
- Modify: `ui/shell/workspace-pane.slint`
- Modify: `ui/shell/terminal-session-host.slint`
- Modify: `src/app/bootstrap.rs`
- Modify: `tests/workspace_tabs_spec.rs`
- Modify: `tests/ssh_connect_tabs_ui_contract_smoke.sh`
- Reference: `src/shell/view_model.rs`

**Step 1: Write the failing test**

Add UI contract coverage for:

- the new `connection-progress` host mode;
- timeline rows in the terminal host;
- current-step detail text;
- diagnostics toggle;
- footer action buttons such as `Cancel`, `Retry`, and `Copy Diagnostics`.

Suggested assertions:

```rust
assert!(workspace_pane.contains("connection-progress"));
assert!(terminal_host.contains("Show Diagnostics"));
assert!(terminal_host.contains("Copy Diagnostics"));
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test workspace_tabs_spec connection_progress -- --nocapture && bash tests/ssh_connect_tabs_ui_contract_smoke.sh`

Expected: FAIL because the Slint workspace still has only `welcome`, `terminal`, and `session-error`.

**Step 3: Write minimal implementation**

In the Slint layer:

- add a new workspace/terminal host branch for `connection-progress`;
- define item projection properties for timeline rows and diagnostics lines;
- render:
  - title and subtitle;
  - elapsed time;
  - vertical step timeline;
  - current detail text;
  - collapsible diagnostics panel;
  - footer action buttons.

In `src/app/bootstrap.rs`:

- project the stored connection attempt model into Slint models and scalar properties;
- keep terminal surface projection untouched for the `connected` case;
- ensure the transition to the live terminal clears timeline-only transient state cleanly.

**Step 4: Run test to verify it passes**

Run: `cargo test --test workspace_tabs_spec connection_progress -- --nocapture && bash tests/ssh_connect_tabs_ui_contract_smoke.sh`

Expected: PASS

**Step 5: Commit**

```bash
git add ui/app-window.slint ui/shell/workspace-pane.slint ui/shell/terminal-session-host.slint src/app/bootstrap.rs tests/workspace_tabs_spec.rs tests/ssh_connect_tabs_ui_contract_smoke.sh
git commit -m "feat: add workspace ssh connection timeline UI"
```

### Task 5: Add Inline Host-Key Decisions And Retry / Cancel Controls

**Files:**
- Modify: `src/app/ssh/runtime.rs`
- Modify: `src/app/ssh/session_manager.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `ui/shell/terminal-session-host.slint`
- Modify: `tests/bootstrap_smoke.rs`
- Modify: `tests/ssh_connection_timeline_spec.rs`
- Modify: `tests/workspace_tabs_spec.rs`
- Reference: `src/app/ssh/known_hosts.rs`

**Step 1: Write the failing test**

Add coverage for these workspace-session flows:

- unknown host key blocks the `verify-host-key` step and exposes inline decision controls;
- accepting the host key writes it and restarts a fresh attempt in the same tab;
- rejecting the host key ends the attempt with preserved diagnostics;
- cancelling a running attempt marks the timeline as cancelled and keeps `Retry` available.

Suggested test shape:

```rust
#[test]
fn unknown_host_key_blocks_connection_in_workspace_timeline() { /* ... */ }

#[test]
fn trusting_host_key_retries_connection_in_same_tab() { /* ... */ }
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test bootstrap_smoke --test ssh_connection_timeline_spec --test workspace_tabs_spec host_key -- --nocapture`

Expected: FAIL because host key handling still routes through the old modal prompt state.

**Step 3: Write minimal implementation**

Implement workspace-session controls:

- add inline prompt projection for blocked `verify-host-key` steps;
- remove the workspace open path’s dependency on the global host-key modal;
- on accept:
  - persist the key through `KnownHostsService`;
  - start a fresh asynchronous retry attempt;
- on reject:
  - finalize the attempt as cancelled with preserved diagnostics while keeping the same tab on the timeline surface;
- add cancel and retry callbacks from the connection timeline footer back into bootstrap/session-manager actions.

Do not change modal `Test Connection` in this task.

**Step 4: Run test to verify it passes**

Run: `cargo test --test bootstrap_smoke --test ssh_connection_timeline_spec --test workspace_tabs_spec host_key -- --nocapture`

Expected: PASS

**Step 5: Commit**

```bash
git add src/app/ssh/runtime.rs src/app/ssh/session_manager.rs src/app/bootstrap.rs ui/shell/terminal-session-host.slint tests/bootstrap_smoke.rs tests/ssh_connection_timeline_spec.rs tests/workspace_tabs_spec.rs
git commit -m "feat: add ssh timeline retry cancel and host key flows"
```

### Task 6: Final Verification And Documentation Alignment

**Files:**
- Modify: `docs/plans/2026-03-30-ssh-connection-timeline-design.md`
- Modify: `docs/plans/2026-03-30-ssh-connection-timeline-implementation-plan.md`
- Reference: `tests/ssh_connection_timeline_spec.rs`
- Reference: `tests/ssh_session_manager_spec.rs`
- Reference: `tests/bootstrap_smoke.rs`
- Reference: `tests/workspace_tabs_spec.rs`

**Step 1: Run the focused verification suite**

Run:

```bash
cargo test --test ssh_connection_timeline_spec --test ssh_session_manager_spec --test bootstrap_smoke --test workspace_tabs_spec -- --nocapture
bash tests/ssh_connect_tabs_ui_contract_smoke.sh
```

Expected: PASS

**Step 2: Run a broader smoke pass if the focused suite is green**

Run:

```bash
cargo test --test ssh_terminal_interaction_spec --test terminal_scrollback_spec -- --nocapture
```

Expected: PASS

**Step 3: Perform manual verification**

Verify these scenarios against real hosts or controlled repro targets:

- slow direct SSH connect keeps the window responsive;
- SOCKS5 proxy connect shows proxy steps;
- one jump host shows per-hop connect/auth/direct-tcpip steps;
- unknown host key blocks inline and can be trusted/rejected;
- wrong password fails on the correct hop;
- cancel stops the attempt and retry starts fresh;
- successful connection fades into the live terminal without blank intermediate frames.

**Step 4: Update any final notes if verification changed details**

If any implementation detail differs from the design or plan, update the docs to match the shipped behavior before closing the work.

Shipped alignment notes:

- workspace unknown-host-key prompts now stay inline in `connection-progress`; modal host-key confirmation remains only for modal `Test Connection`;
- `cancelled` workspace attempts keep rendering on the timeline page so `Retry` stays local to the same tab;
- generic asynchronous launch failures may project as `connecting` until the next workspace sync tick, then settle into `session-error`;
- automated verification is complete in-repo; manual real-host verification remains an external follow-up.

**Step 5: Commit**

```bash
git add docs/plans/2026-03-30-ssh-connection-timeline-design.md docs/plans/2026-03-30-ssh-connection-timeline-implementation-plan.md
git commit -m "docs: finalize ssh connection timeline plan"
```

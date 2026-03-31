# Default Enhanced Remote Session Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make SSH sessions automatically attempt a temporary enhanced shell-integration bootstrap for bash/zsh/fish, while safely falling back to a plain terminal without modifying remote dotfiles.

**Architecture:** Introduce a dedicated SSH shell-integration module that owns protocol parsing and bootstrap generation, extend the SSH runtime/session manager with explicit enhancement state, perform a one-shot shell probe plus session-only bootstrap after ordinary shell startup, and surface the resulting mode and opt-out controls in the workspace UI. Use standard `OSC 7` / `OSC 133` semantics first, with a gated `mica-term` private OSC channel for extra actions.

**Tech Stack:** Rust, russh, Slint, existing SSH runtime/session manager tests, focused TDD with new protocol tests

---

### Task 1: Lock The Shell Integration Protocol Contract

**Files:**
- Create: `src/app/ssh/shell_integration.rs`
- Modify: `src/app/ssh/mod.rs`
- Create: `tests/ssh_shell_integration_spec.rs`

**Step 1: Write the failing test**

Add a new focused protocol spec that proves `mica-term` can parse the baseline escape sequences and generate gated bootstrap payloads.

```rust
#[test]
fn parser_extracts_standard_and_private_shell_integration_events() {
    let input = concat!(
        "\u{1b}]7;file://remote/tmp/project\u{7}",
        "\u{1b}]133;A\u{7}",
        "\u{1b}]133;B\u{7}",
        "\u{1b}]133;C\u{7}",
        "\u{1b}]133;D;0\u{7}",
        "\u{1b}]1337;CurrentDir=/tmp/project\u{7}",
        "\u{1b}]9001;mterm;open;/tmp/readme.md\u{7}",
    );

    let events = parse_shell_integration_events(input.as_bytes());

    assert!(events.contains(&ShellIntegrationEvent::CurrentDirectory("/tmp/project".into())));
    assert!(events.contains(&ShellIntegrationEvent::PromptStart));
    assert!(events.contains(&ShellIntegrationEvent::PromptEnd));
    assert!(events.contains(&ShellIntegrationEvent::CommandStart));
    assert!(events.contains(&ShellIntegrationEvent::CommandFinished(Some(0))));
    assert!(events.contains(&ShellIntegrationEvent::PrivateAction(
        MicaPrivateAction::OpenPath("/tmp/readme.md".into())
    )));
}

#[test]
fn bash_bootstrap_builder_prefers_standard_markers_and_gates_private_channel() {
    let script = build_shell_bootstrap(ShellKind::Bash, BootstrapOptions::default());

    assert!(script.contains("OSC 133").not());
    assert!(script.contains("\\033]133;A"));
    assert!(script.contains("\\033]7;file://"));
    assert!(script.contains("TERM_PROGRAM=mica-term"));
    assert!(script.contains("MICA_TERM_ENHANCED=1"));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test ssh_shell_integration_spec -q`

Expected: FAIL because the protocol module and spec file do not exist yet.

**Step 3: Write minimal implementation**

Create the protocol module with:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellKind {
    Bash,
    Zsh,
    Fish,
    Unsupported(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MicaPrivateAction {
    OpenPath(String),
    EditPath(String),
    DownloadPath(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellIntegrationEvent {
    CurrentDirectory(String),
    PromptStart,
    PromptEnd,
    CommandStart,
    CommandFinished(Option<i32>),
    PrivateAction(MicaPrivateAction),
}

pub fn parse_shell_integration_events(bytes: &[u8]) -> Vec<ShellIntegrationEvent> { /* ... */ }
pub fn build_shell_bootstrap(shell: ShellKind, options: BootstrapOptions) -> String { /* ... */ }
```

Keep the first pass intentionally small:

- support `OSC 7`
- support `OSC 133 A/B/C/D`
- support iTerm2 `OSC 1337;CurrentDir`
- support a gated `mica-term` private OSC family

**Step 4: Run test to verify it passes**

Run: `cargo test --test ssh_shell_integration_spec -q`

Expected: PASS

**Step 5: Commit**

```bash
git add src/app/ssh/mod.rs src/app/ssh/shell_integration.rs tests/ssh_shell_integration_spec.rs
git commit -m "test: lock ssh shell integration protocol contract"
```

### Task 2: Extend Session And Tab State For Enhancement Mode

**Files:**
- Modify: `src/app/ssh/runtime.rs`
- Modify: `src/app/ssh/session_manager.rs`
- Modify: `src/shell/tabs.rs`
- Modify: `tests/ssh_session_manager_spec.rs`
- Modify: `tests/workspace_tabs_spec.rs`

**Step 1: Write the failing test**

Add state-projection coverage proving the session manager can track enhancement mode separately from ordinary connection state.

```rust
#[test]
fn session_manager_tracks_enhanced_remote_session_state_changes() {
    let session_id = Uuid::new_v4();
    let attempt_id = Uuid::new_v4();
    let registry = Arc::new(Mutex::new(SessionRegistry::default()));

    seed_connected_session(&registry, session_id, attempt_id);

    apply_runtime_event(
        &registry,
        session_id,
        attempt_id,
        SessionRuntimeEvent::EnhancedSessionStateChanged(EnhancedSessionState::Enhanced),
    );

    let session = registry
        .lock()
        .expect("lock registry")
        .sessions
        .get(&session_id)
        .cloned()
        .expect("session");

    assert_eq!(session.enhanced_session_state, EnhancedSessionState::Enhanced);
}

#[test]
fn workspace_tab_projects_enhanced_session_state_badge() {
    let handle = SessionHandle {
        session_id: Uuid::new_v4(),
        asset_id: "asset-prod".into(),
        title: "Prod".into(),
        subtitle: "ops@10.0.0.12:22".into(),
        state: SessionState::Connected,
        can_reconnect: false,
        enhanced_session_state: EnhancedSessionState::Fallback,
    };

    let tab = WorkspaceTab::from_session(&handle);

    assert_eq!(tab.enhanced_session_state, "fallback");
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test ssh_session_manager_spec session_manager_tracks_enhanced_remote_session_state_changes -q`

Run: `cargo test --test workspace_tabs_spec workspace_tab_projects_enhanced_session_state_badge -q`

Expected: FAIL because there is no enhancement-state contract yet.

**Step 3: Write minimal implementation**

Extend the shared contracts:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnhancedSessionState {
    Plain,
    Enhanced,
    Fallback,
}

pub enum SessionRuntimeEvent {
    /* existing variants */
    EnhancedSessionStateChanged(EnhancedSessionState),
}

pub struct SessionHandle {
    /* existing fields */
    pub enhanced_session_state: EnhancedSessionState,
}
```

Project the new state through:

- `apply_runtime_event`
- session initialization defaults
- `WorkspaceTab::from_session`

Do not wire UI yet; only lock the domain contract in this task.

**Step 4: Run test to verify it passes**

Run: `cargo test --test ssh_session_manager_spec session_manager_tracks_enhanced_remote_session_state_changes -q`

Run: `cargo test --test workspace_tabs_spec workspace_tab_projects_enhanced_session_state_badge -q`

Expected: PASS

**Step 5: Commit**

```bash
git add src/app/ssh/runtime.rs src/app/ssh/session_manager.rs src/shell/tabs.rs tests/ssh_session_manager_spec.rs tests/workspace_tabs_spec.rs
git commit -m "feat: add enhanced ssh session state contract"
```

### Task 3: Add One-Shot Shell Probe And Bootstrap Policy To The SSH Runtime

**Files:**
- Modify: `src/app/ssh/runtime.rs`
- Modify: `src/app/ssh/shell_integration.rs`
- Modify: `tests/ssh_session_manager_spec.rs`

**Step 1: Write the failing test**

Use the runtime-backed shell server test harness to prove the runtime tries enhancement once for a supported shell and stops retrying after fallback.

```rust
#[test]
fn ssh_runtime_attempts_supported_shell_bootstrap_once() {
    let runtime = AppAsyncRuntime::new().expect("runtime");
    let (server_task, addr, private_key_path, server_public_key, server_state) =
        runtime.block_on(async { spawn_publickey_shell_server(Duration::from_millis(10)).await });

    configure_known_host(&addr, &server_public_key);

    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    let _runtime = runtime.block_on(async {
        SshSessionRuntime::connect(sample_publickey_profile("asset-prod", addr.ip().to_string(), addr.port(), private_key_path.display().to_string()), Uuid::new_v4(), Uuid::new_v4(), event_tx)
            .await
            .expect("connect runtime")
    });

    let states = collect_enhancement_states(&runtime, &mut event_rx);

    assert_eq!(states, vec![EnhancedSessionState::Enhanced]);
    assert_eq!(server_state.bootstrap_attempts(), 1);
}

#[test]
fn ssh_runtime_marks_session_fallback_after_failed_bootstrap_without_retry() {
    /* same harness, but force bootstrap rejection */
    assert_eq!(states, vec![EnhancedSessionState::Fallback]);
    assert_eq!(server_state.bootstrap_attempts(), 1);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test ssh_session_manager_spec ssh_runtime_attempts_supported_shell_bootstrap_once -q`

Run: `cargo test --test ssh_session_manager_spec ssh_runtime_marks_session_fallback_after_failed_bootstrap_without_retry -q`

Expected: FAIL because the runtime does not yet probe or bootstrap.

**Step 3: Write minimal implementation**

Add a small state machine to `SshSessionRuntime::connect` and the channel pump:

```rust
enum EnhancementAttemptState {
    Unknown,
    Plain,
    Enhanced,
    Fallback,
}

async fn detect_remote_shell(/* ... */) -> ShellKind { /* side-channel probe */ }
async fn attempt_shell_bootstrap(/* ... */) -> Result<EnhancedSessionState> { /* one shot */ }
```

Rules to implement:

- connect normally first
- detect the remote shell out-of-band
- only attempt bootstrap for bash/zsh/fish
- emit `EnhancedSessionStateChanged`
- never retry in the same session after fallback

Keep this task focused on policy and lifecycle. Do not add UI or private actions yet.

**Step 4: Run test to verify it passes**

Run: `cargo test --test ssh_session_manager_spec ssh_runtime_attempts_supported_shell_bootstrap_once -q`

Run: `cargo test --test ssh_session_manager_spec ssh_runtime_marks_session_fallback_after_failed_bootstrap_without_retry -q`

Expected: PASS

**Step 5: Commit**

```bash
git add src/app/ssh/runtime.rs src/app/ssh/shell_integration.rs tests/ssh_session_manager_spec.rs
git commit -m "feat: auto-bootstrap supported ssh shells"
```

### Task 4: Parse Prompt And Command Markers Into Runtime Metadata

**Files:**
- Modify: `src/app/ssh/runtime.rs`
- Modify: `src/app/ssh/session_manager.rs`
- Modify: `src/app/ssh/shell_integration.rs`
- Modify: `tests/ssh_shell_integration_spec.rs`
- Modify: `tests/ssh_session_manager_spec.rs`

**Step 1: Write the failing test**

Lock the parser/runtime bridge so shell markers become session metadata rather than visible garbage in the terminal.

```rust
#[test]
fn runtime_extracts_cwd_and_command_marks_from_shell_integration_sequences() {
    let bytes = concat!(
        "\u{1b}]133;A\u{7}",
        "\u{1b}]7;file://remote/tmp/project\u{7}",
        "\u{1b}]133;B\u{7}",
        "\u{1b}]133;C\u{7}",
        "\u{1b}]133;D;0\u{7}",
    )
    .as_bytes();

    let parsed = runtime_shell_events(bytes);

    assert!(parsed.cwd.as_deref() == Some("/tmp/project"));
    assert_eq!(parsed.command_finish_exit_code, Some(0));
}

#[test]
fn runtime_does_not_leave_private_control_sequences_in_visible_terminal_rows() {
    let surface = apply_output_and_snapshot(
        "\u{1b}]9001;mterm;open;/tmp/readme.md\u{7}\r\nprompt$ ".as_bytes(),
    );

    assert!(
        surface
            .visible_lines
            .iter()
            .all(|line| !line.contains("9001;mterm"))
    );
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test ssh_shell_integration_spec runtime_extracts_cwd_and_command_marks_from_shell_integration_sequences -q`

Run: `cargo test --test ssh_session_manager_spec runtime_does_not_leave_private_control_sequences_in_visible_terminal_rows -q`

Expected: FAIL because marker parsing is not wired through the runtime yet.

**Step 3: Write minimal implementation**

Teach the runtime pump to:

- parse shell-integration events before writing bytes into the terminal grid;
- continue emitting `CurrentDirectoryChanged`;
- add a small internal command-metadata struct for prompt/command lifecycle;
- swallow `mica-term` private OSC records so they are not rendered as text.

Use the smallest event bridge necessary to support the current plan; avoid speculative UI features.

**Step 4: Run test to verify it passes**

Run: `cargo test --test ssh_shell_integration_spec runtime_extracts_cwd_and_command_marks_from_shell_integration_sequences -q`

Run: `cargo test --test ssh_session_manager_spec runtime_does_not_leave_private_control_sequences_in_visible_terminal_rows -q`

Expected: PASS

**Step 5: Commit**

```bash
git add src/app/ssh/runtime.rs src/app/ssh/session_manager.rs src/app/ssh/shell_integration.rs tests/ssh_shell_integration_spec.rs tests/ssh_session_manager_spec.rs
git commit -m "feat: parse ssh shell integration markers"
```

### Task 5: Surface Enhancement State, Local Opt-Out, And Host Cache In The UI

**Files:**
- Modify: `src/app/ssh/session_manager.rs`
- Modify: `src/shell/tabs.rs`
- Modify: `src/shell/view_model.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `ui/shell/tabbar.slint`
- Modify: `ui/shell/terminal-session-host.slint`
- Modify: `tests/workspace_tabs_spec.rs`
- Modify: `tests/bootstrap_smoke.rs`

**Step 1: Write the failing test**

Add UI-contract coverage for the new session mode indicator and host/session opt-out behavior.

```rust
#[test]
fn workspace_session_host_projects_enhanced_state_and_disable_action() {
    let app = AppWindow::new().expect("app");

    assert!(
        read_file("ui/shell/terminal-session-host.slint")
            .contains("workspace-session-enhanced-state")
    );
    assert!(
        read_file("ui/shell/terminal-session-host.slint")
            .contains("disable-enhanced-session")
    );
}

#[test]
fn session_manager_skips_auto_bootstrap_for_cached_fallback_host() {
    let manager = seeded_session_manager_with_fallback_cache("ops@10.0.0.12:22", "bash");
    let policy = manager.enhancement_policy_for(sample_profile());

    assert_eq!(policy, EnhancementPolicy::SkipAutoBootstrap);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test workspace_tabs_spec workspace_session_host_projects_enhanced_state_and_disable_action -q`

Run: `cargo test --test bootstrap_smoke session_manager_skips_auto_bootstrap_for_cached_fallback_host -q`

Expected: FAIL because the UI and cache plumbing do not exist yet.

**Step 3: Write minimal implementation**

Add:

- a local enhancement-policy cache keyed by host/user/shell fingerprint;
- a per-session disable action;
- a per-host disable action for the current app run;
- tab/session host projection of `plain` / `enhanced` / `fallback`.

Recommended minimal shapes:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct EnhancementCacheKey {
    user: String,
    host: String,
    port: u16,
    shell: String,
}

enum EnhancementPolicy {
    AutoTry,
    SkipAutoBootstrap,
}
```

Keep this first cut local-only. Do not add persistent settings storage in the same change.

**Step 4: Run test to verify it passes**

Run: `cargo test --test workspace_tabs_spec workspace_session_host_projects_enhanced_state_and_disable_action -q`

Run: `cargo test --test bootstrap_smoke session_manager_skips_auto_bootstrap_for_cached_fallback_host -q`

Expected: PASS

**Step 5: Commit**

```bash
git add src/app/ssh/session_manager.rs src/shell/tabs.rs src/shell/view_model.rs src/app/bootstrap.rs ui/shell/tabbar.slint ui/shell/terminal-session-host.slint tests/workspace_tabs_spec.rs tests/bootstrap_smoke.rs
git commit -m "feat: surface enhanced ssh session controls"
```

### Task 6: Full Verification And Docs Sweep

**Files:**
- Reference: `src/app/ssh/shell_integration.rs`
- Reference: `src/app/ssh/runtime.rs`
- Reference: `src/app/ssh/session_manager.rs`
- Reference: `src/shell/tabs.rs`
- Reference: `src/shell/view_model.rs`
- Reference: `src/app/bootstrap.rs`
- Reference: `ui/shell/tabbar.slint`
- Reference: `ui/shell/terminal-session-host.slint`
- Reference: `tests/ssh_shell_integration_spec.rs`
- Reference: `tests/ssh_session_manager_spec.rs`
- Reference: `tests/workspace_tabs_spec.rs`
- Reference: `tests/bootstrap_smoke.rs`

**Step 1: Run focused tests**

Run:

```bash
cargo test --test ssh_shell_integration_spec --test ssh_session_manager_spec --test workspace_tabs_spec --test bootstrap_smoke -q
```

Expected: PASS

**Step 2: Run compile verification**

Run:

```bash
cargo check
```

Expected: PASS

**Step 3: Review diff**

Run:

```bash
git diff -- src/app/ssh/mod.rs src/app/ssh/shell_integration.rs src/app/ssh/runtime.rs src/app/ssh/session_manager.rs src/shell/tabs.rs src/shell/view_model.rs src/app/bootstrap.rs ui/shell/tabbar.slint ui/shell/terminal-session-host.slint tests/ssh_shell_integration_spec.rs tests/ssh_session_manager_spec.rs tests/workspace_tabs_spec.rs tests/bootstrap_smoke.rs
```

Expected: only Enhanced Remote Session changes plus any tight test scaffolding required for them

**Step 4: Update docs if the UI labels changed materially**

If the implementation lands user-visible wording such as `Enhanced`, `Plain`, or `Fallback`, update the nearest user-facing terminal/SSH docs in the same final pass.

**Step 5: Commit**

```bash
git add src/app/ssh/mod.rs src/app/ssh/shell_integration.rs src/app/ssh/runtime.rs src/app/ssh/session_manager.rs src/shell/tabs.rs src/shell/view_model.rs src/app/bootstrap.rs ui/shell/tabbar.slint ui/shell/terminal-session-host.slint tests/ssh_shell_integration_spec.rs tests/ssh_session_manager_spec.rs tests/workspace_tabs_spec.rs tests/bootstrap_smoke.rs
git commit -m "feat: add default enhanced remote sessions"
```

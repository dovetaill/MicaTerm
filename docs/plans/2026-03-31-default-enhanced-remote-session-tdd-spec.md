# Default Enhanced Remote Session TDD Spec

## Scope

This document captures the implemented contracts for the default enhanced remote session work completed on 2026-03-31.

The implementation covers:

- shell integration protocol contracts and bootstrap generation
- SSH runtime shell probing and one-shot bootstrap attempt
- runtime parsing of shell markers into session metadata
- session/tab/UI projection of enhancement state
- local-only opt-out and fallback host cache contracts

## Core Structs

### `ShellKind`

Location: `src/app/ssh/shell_integration.rs`

Represents the detected interactive shell family:

- `Bash`
- `Zsh`
- `Fish`
- `Unsupported(String)`

Used by runtime probing and bootstrap selection.

### `BootstrapOptions`

Location: `src/app/ssh/shell_integration.rs`

Defines runtime bootstrap generation options:

- `term_program`
- `enhanced_flag`
- `private_channel_tag`
- `private_actions_enabled`

### `RuntimeShellEvents`

Location: `src/app/ssh/shell_integration.rs`

Normalized parsed shell metadata emitted from raw OSC traffic:

- `cwd`
- `prompt_started`
- `prompt_ended`
- `command_started`
- `command_finish_exit_code`
- `sanitized_bytes`

`sanitized_bytes` is the visible stream that remains after recognized integration control sequences are removed.

### `EnhancedSessionState`

Location: `src/app/ssh/session_manager.rs`

Runtime/UI enhancement state:

- `Plain`
- `Enhanced`
- `Fallback`

This is intentionally separate from transport connection state.

### `EnhancementCacheKey`

Location: `src/app/ssh/session_manager.rs`

Local-only cache key for enhancement incompatibility / opt-out decisions:

- `user`
- `host`
- `port`
- `shell`

Current implementation stores the shell field, but manager policy lookup currently matches by `user/host/port` because shell identity is not yet threaded through the full decision path.

### `SessionHandle`

Location: `src/app/ssh/session_manager.rs`

Extended with:

- `enhanced_session_state: EnhancedSessionState`

This is projected into workspace tabs and active session host state.

## Traits And Interface Contracts

### `SessionRuntimeControl`

Location: `src/app/ssh/session_manager.rs`

Owns live session control methods:

- `disconnect`
- `send_text_input`
- `send_key_input`
- `send_mouse_input`
- `send_paste`
- `resize`
- optional surface/theme/sftp helpers

### `SessionRuntimeLauncher`

Location: `src/app/ssh/session_manager.rs`

Async launcher seam used by `SessionManager`:

- `launch(profile, session_id, attempt_id, event_tx)`
- `probe(profile)`

This keeps runtime startup mockable in tests and isolates session registry logic from transport setup.

### `SessionRuntimeEvent`

Location: `src/app/ssh/runtime.rs`

Relevant new contract:

- `EnhancedSessionStateChanged(EnhancedSessionState)`

This event is emitted after one-shot bootstrap evaluation and consumed by the session manager.

### `SessionManager` enhancement APIs

Location: `src/app/ssh/session_manager.rs`

Implemented manager-level contracts:

- `remember_enhancement_fallback(&ConnectionProfile, &str)`
- `enhancement_policy_for(&ConnectionProfile) -> EnhancementPolicy`
- `disable_enhancement_for_session(Uuid) -> Result<SessionHandle>`
- `disable_enhancement_for_host(Uuid, &str) -> Result<SessionHandle>`

`EnhancementPolicy` currently exposes:

- `AutoTry`
- `SkipAutoBootstrap`

## Slint Callbacks, Global State, And Bindings

### App-level properties

Location: `ui/app-window.slint`

Added / used properties:

- `workspace-session-enhanced-state`
- `workspace-tab-items[*].enhanced_session_state`

### Callback path

Callback flow:

1. `TerminalSessionHost.local-action-requested(string)`
2. `WorkspacePane.local-action-requested(string)`
3. `AppWindow.workspace-session-local-action-requested(string)`
4. `src/app/bootstrap.rs` action handler
5. `SessionManager` mutation
6. projection refresh back into Slint properties

### Session host bindings

Location: `ui/shell/terminal-session-host.slint`

The active session host now binds:

- `workspace-session-enhanced-state`
- `"disable-enhanced-session"` action
- `"disable-enhanced-session-host"` action

The host renders enhancement status text in both:

- terminal mode
- session-error mode

### View-model projection

Location: `src/shell/view_model.rs`

Relevant accessor:

- `active_workspace_session_enhanced_state() -> &str`

This accessor feeds the active host property sync in bootstrap.

## Tokio Tasks, Channels, And Actor-Like Interaction

### Session startup path

1. `SessionManager::open_session` creates `SessionHandle` and stores `ConnectionProfile`.
2. `SessionManager::spawn_session_attempt` creates `mpsc::unbounded_channel()`.
3. Event task drains `SessionRuntimeEvent` and applies registry mutations.
4. Launcher task creates the concrete runtime via `SessionRuntimeLauncher::launch`.
5. `SshSessionRuntime::connect` requests shell, performs side-channel shell probe, and attempts bootstrap once.

### Runtime event flow

Runtime emits:

- `Connected`
- `ConnectionProgress(...)`
- `EnhancedSessionStateChanged(...)`
- `CurrentDirectoryChanged(...)`
- `SurfaceChanged(...)`
- `SurfaceDirty`

Manager consumes those events under `Arc<Mutex<SessionRegistry>>`.

### UI thread synchronization

Bootstrap updates Slint state through the existing shell-state sync path and uses `slint::invoke_from_event_loop` for event-loop-safe UI refresh where needed.

## State Flow

### Enhancement decision flow

1. SSH session opens in `Connecting`.
2. Runtime requests shell.
3. Runtime opens a short-lived side channel and runs `shell_probe_command()`.
4. Probe output is normalized into `ShellKind`.
5. If shell is supported, runtime injects one bootstrap payload into the main shell channel.
6. Runtime waits for one bootstrap acknowledgment window.
7. Runtime emits:
   - `EnhancedSessionState::Enhanced` on accepted ack
   - `EnhancedSessionState::Fallback` on rejection / timeout / failed bootstrap path
   - `EnhancedSessionState::Plain` if no supported shell bootstrap is attempted
8. Session manager stores the enhancement state separately from connection state.
9. Bootstrap projects the active tab state into Slint.

### Local opt-out flow

For `"disable-enhanced-session"`:

1. UI action reaches bootstrap callback.
2. Bootstrap resolves active session id.
3. `SessionManager::disable_enhancement_for_session` marks the session disabled.
4. Session enhancement state is forced to `Plain`.
5. Future runtime enhancement updates for that session are ignored.

For `"disable-enhanced-session-host"`:

1. UI action reaches bootstrap callback.
2. Bootstrap resolves active session id.
3. `SessionManager::disable_enhancement_for_host` records a local fallback cache entry.
4. Session enhancement state is forced to `Plain`.
5. Manager policy queries for matching host/user/port return `SkipAutoBootstrap`.

## Key Error Handling Strategies

- Unsupported or undetectable shells fall back without breaking normal terminal use.
- Bootstrap request failure returns `Fallback` rather than failing the SSH session.
- Parsed private shell integration sequences are stripped from visible terminal output.
- Unknown host key flows remain separate from enhancement flows.
- `current_attempt_matches(...)` prevents stale runtime events from old attempts mutating a retried session.
- Session-level disable takes precedence over later runtime enhancement updates.

## Edge Cases

### Tokio channel blocking or message accumulation

- `SessionManager` uses `mpsc::unbounded_channel`, so there is no built-in backpressure.
- Surface events are explicitly coalesced to reduce churn.
- Remaining risk: non-surface events can still accumulate if producer rate exceeds consumer/UI sync rate.

### UI thread update timing

- Active workspace properties and tab items must be refreshed together.
- If only tab data or only active-host data is refreshed, the host badge can temporarily show stale enhancement state.

### Data races or shared-state inconsistency

- Registry state is centralized behind `Arc<Mutex<SessionRegistry>>`.
- Consistency risk remains if future code mutates session state outside the projection helpers.

### Resource release sequencing

- `close_session` and `retry_session` remove runtime controls before disconnecting old runtimes.
- Disabled-session bookkeeping is cleared on close.
- Future additions must keep cache/session cleanup symmetric.

### Async task cancellation or window close with pending callbacks

- Runtime tasks may still emit events during retry/close windows.
- `current_attempt_matches(...)` is the primary guard against stale updates after replacement attempts.

### Slint model update and source-of-truth drift

- `workspace-tab-items` and `workspace-session-enhanced-state` are derived views, not the source of truth.
- If bootstrap sync is skipped after a manager mutation, UI state can drift from registry state.

### Host cache granularity

- `EnhancementCacheKey` stores `shell`, but current policy lookup uses `user/host/port`.
- This means one cached fallback may suppress auto-bootstrap more broadly than a future shell-specific design intends.

### Local host disable action shell context

- The current UI action does not carry detected shell identity.
- Bootstrap therefore records host disable with an empty shell string and relies on coarse profile matching.

## Recommended Future Tests

### Unit tests

- `EnhancementCacheKey` matching semantics, especially host normalization and shell handling
- session-level disable precedence over later `EnhancedSessionStateChanged` events
- coarse host-cache policy behavior vs future shell-specific behavior

### Integration tests

- runtime skips bootstrap when manager policy is `SkipAutoBootstrap` after full wiring is added
- retrying a disabled session does not re-enable enhancement automatically
- host-level disable survives multiple tabs for the same host within the same app run

### UI interaction tests

- clicking `"Disable This Session"` updates active host badge without reconnect
- clicking `"Disable For Host"` updates current session and affects a subsequent reconnect/open
- badge copy stays correct when switching among `plain`, `enhanced`, and `fallback` tabs

### Regression tests

- parsed OSC private actions never leak into visible terminal rows
- bootstrap rejection produces exactly one `Fallback` state and no repeated retries
- tab projection and active host projection remain consistent after retry, reconnect, and close flows

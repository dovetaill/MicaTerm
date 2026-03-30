# SSH Connection Timeline TDD Handoff

## Scope

This document captures the shipped test-driving surface for the workspace SSH connection timeline implemented on 2026-03-30.

Automation status:

- focused timeline and workspace suites passed
- broader terminal interaction smoke suites passed
- real-host manual verification is still pending outside this repository environment

## Core Structs

`ConnectionAttemptState`

- owns the active `attempt_id`
- stores `headline`, ordered `steps`, buffered `diagnostics`
- carries optional `prompt: Option<ConnectionHostKeyPrompt>` for inline host-key decisions

`ConnectionHostKeyPrompt`

- contains `host`, `port`, `fingerprint`, `public_key_openssh`
- is internal state for retrying after host-key trust
- UI currently projects only host and fingerprint

`ConnectionStepStateItem`

- stable step record with `step_id`, `step_kind`, `title`, `detail`, `hop_label`, `state`
- used by runtime emission, session aggregation, and Slint projection

`ConnectionDiagnosticLine`

- stores `attempt_id`-scoped human-readable diagnostics
- used for inline diagnostics display and copy/export actions

`SessionHandle`

- tab-facing projection of `session_id`, `asset_id`, titles, `SessionState`, reconnect affordance

`SessionState`

- shipped values now include `Connecting`, `WaitingUser`, `Connected`, `Cancelled`, `Disconnected`, `Error(String)`
- `WaitingUser` and `Cancelled` are required for timeline-only workspace states

`ConnectionProgressReporter` and `ConnectionProgressStep`

- runtime-side helpers that emit `AttemptStarted`, `HeadlineChanged`, `StepUpdated`, `DiagnosticAppended`
- `ConnectionProgressStep::block()` is used for inline host-key waits

## Traits And Interface Contracts

`SessionRuntimeLauncher`

- `launch(profile, session_id, attempt_id, event_tx)` must start a fresh runtime attempt
- attempt scoping is mandatory; stale events must be ignorable
- `probe(profile)` remains available for modal `Test Connection` only

`SessionRuntimeControl`

- provides live session controls after a runtime is attached
- `disconnect()` is used both for user disconnect and cancelling an in-flight attempt

`SessionRuntimeEvent`

- `Connected`, `Disconnected`, `Error`
- `ConnectionProgress(ConnectionProgressEvent)`
- `SurfaceChanged`, `SurfaceDirty`

`ConnectionProgressEvent`

- `AttemptStarted`
- `HeadlineChanged`
- `StepUpdated`
- `DiagnosticAppended`

Bootstrap local action contract:

- `cancel-connection-attempt`
- `retry-connection-attempt`
- `trust-host-key`
- `reject-host-key`
- `copy-connection-diagnostics`
- `edit-connection`

## Slint Callbacks, Global State, And Bindings

App window state projected for the active workspace session:

- `workspace-session-connection-headline`
- `workspace-session-connection-current-hop`
- `workspace-session-connection-current-detail`
- `workspace-session-host-key-prompt-host`
- `workspace-session-host-key-prompt-fingerprint`
- `workspace-session-connection-steps`
- `workspace-session-connection-diagnostics`

Binding chain:

- `AppWindow` forwards workspace timeline properties into `WorkspacePane`
- `WorkspacePane` forwards them into `TerminalSessionHost`
- `TerminalSessionHost` renders the `connection-progress` branch

`TerminalSessionHost` callback usage:

- footer actions use `local-action-requested(...)`
- inline host-key buttons emit `trust-host-key` and `reject-host-key`
- retry/cancel stay in the same callback family to keep workspace actions centralized in bootstrap

Global modal behavior:

- modal host-key state in `ShellViewModel` remains for modal `Test Connection`
- workspace open-session host-key UX no longer depends on the modal path

## Tokio Tasks, Channels, And Actor-Like Interactions

`SessionManager::open_session(...)`

- creates a `SessionHandle`
- stores the resolved `ConnectionProfile`
- creates initial `ConnectionAttemptState`
- spawns an event-pump task for `SessionRuntimeEvent`
- spawns a launch task that calls `SessionRuntimeLauncher::launch(...)`

Event pump behavior:

- consumes an unbounded Tokio channel
- coalesces `SurfaceChanged` and `SurfaceDirty` bursts
- routes all events through `apply_runtime_event(...)`
- drops stale events when `attempt_id` no longer matches the current attempt

Retry behavior:

- `retry_session(session_id)` replaces the current attempt with a fresh `attempt_id`
- clears stale terminal surfaces
- reuses the same `session_id` and tab
- disconnects any attached runtime control before starting the fresh attempt

Host-key trust flow:

- bootstrap reads `ConnectionHostKeyPrompt`
- persists trust through `KnownHostsService`
- immediately calls `retry_session(session_id)`

## State Flow

Open path:

1. saved SSH asset is activated
2. `SessionManager::open_session(...)` returns immediately
3. workspace tab enters `connection-progress`
4. runtime emits timeline steps
5. successful connect changes headline to `connected`
6. workspace host switches to `terminal`

Unknown host key:

1. runtime reaches `verify-host-key`
2. runtime emits a blocked step and `waiting-user`
3. `SessionManager` stores `ConnectionHostKeyPrompt`
4. workspace shows inline host-key card
5. `trust-host-key` writes `known_hosts` and starts a fresh attempt in the same tab
6. `reject-host-key` finalizes the attempt as `cancelled` and preserves diagnostics

Cancel:

1. user triggers `cancel-connection-attempt`
2. current attempt headline becomes `cancelled`
3. current running or blocked step becomes `cancelled`
4. tab remains on `connection-progress`
5. `Retry` remains available

Generic async launch failure:

1. tab may appear as `connecting` for one workspace projection tick
2. session manager settles to `Error(String)`
3. workspace host switches to `session-error`

## Error Handling Strategy

Unknown host key:

- special-cased via `UnknownHostKeyError`
- converted into `waiting-user` plus inline prompt payload
- does not collapse into a generic `Error(String)` for workspace open-session UX

Stale retries:

- all runtime events are gated by `attempt_id`
- old attempt events are ignored once a new retry starts

Runtime attachment race:

- `attach_runtime_control(...)` checks both session existence and current `attempt_id`
- outdated runtime controls are disconnected instead of attaching to the wrong attempt

Known-host persistence failures:

- bootstrap logs the failure and leaves the attempt unresolved rather than silently mutating state

Diagnostics:

- append human-readable lines at step transitions and blocked states
- preserve failure context for retry/copy flows

## Edge Cases

Tokio channel backpressure or message pile-up:

- current implementation uses an unbounded channel
- this avoids deadlocks on UI-adjacent control flow but can still grow if runtime logging becomes too noisy
- future tests should stress long retry loops and noisy proxy chains

UI thread update timing:

- workspace projections depend on Slint timer/event-loop refresh
- async launch failures can transiently show `connecting` before the next projection pass

Data races or shared-state inconsistency:

- `SessionRegistry` is mutex-backed
- attempt and session mutations must remain ordered to avoid mixed `SessionState` and `ConnectionAttemptState`

Resource release sequencing:

- retry and cancel remove runtime controls before re-launching or finalizing
- outdated runtime controls must be disconnected after removal

Async task cancellation or closed UI callbacks:

- `disconnect()` may arrive after a retry already replaced the attempt
- stale event rejection by `attempt_id` is required to prevent ghost updates

Slint model updates diverging from the source:

- connection steps and diagnostics use `VecModel` reconciliation
- prompt host/fingerprint must be cleared whenever the active mode is not `connection-progress`

Inline host-key duplication:

- runtime may emit a blocked `verify-host-key` step before launch returns the typed error
- session aggregation must upsert the same logical step instead of rendering duplicates

Cancelled timeline persistence:

- `cancelled` is intentionally still projected as `connection-progress`
- tests should guard against accidental fallback to `session-error`

## Recommended Next Tests

Unit tests:

- verify `retry_session(...)` preserves `session_id` while rotating `attempt_id`
- verify `reject_host_key_prompt(...)` preserves the last blocked step and appends diagnostics exactly once
- verify `attach_runtime_control(...)` rejects stale runtime attachments after rapid retries

Integration tests:

- simulate repeated trust/retry cycles with multiple stale event bursts
- exercise host-key blocking on jump hosts, not only the final target
- verify generic async launch failures settle from `connecting` into `session-error`

UI interaction tests:

- assert inline host-key card visibility toggles with prompt projection
- assert `Cancel` and `Retry` button visibility across `connecting`, `waiting-user`, `cancelled`, `error`
- assert diagnostics copy text matches the projected diagnostics model

Manual verification targets:

- slow direct SSH host
- SOCKS5 proxy with and without auth
- at least one jump-host chain
- unknown host key accept/reject against a disposable host

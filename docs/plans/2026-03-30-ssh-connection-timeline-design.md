# SSH Connection Timeline Design

## Goal

Eliminate UI hangs during slow SSH connections and replace the current opaque `connecting` state with a polished, multi-hop-aware connection timeline inside the workspace terminal page.

This work is scoped to workspace SSH session opening. The immediate target is the path triggered by double-clicking or otherwise opening a saved SSH asset into a tab.

## Current State

- [`src/app/bootstrap.rs`](../../src/app/bootstrap.rs) still calls into a synchronous probe path before opening a workspace SSH session.
- [`src/app/ssh/session_manager.rs`](../../src/app/ssh/session_manager.rs) exposes `probe_connection()` by calling `runtime_handle.block_on(...)`, which can block the UI thread during DNS, TCP connect, proxy negotiation, jump-host auth, host key verification, or shell startup.
- [`src/app/ssh/runtime.rs`](../../src/app/ssh/runtime.rs) already has real multi-hop runtime logic for:
  - direct TCP connect;
  - SOCKS5 and HTTP proxy negotiation;
  - recursive SSH jump hosts through `direct-tcpip`;
  - host key verification;
  - PTY and shell startup.
- [`ui/shell/terminal-session-host.slint`](../../ui/shell/terminal-session-host.slint) has `welcome`, `terminal`, and `session-error` visual states, but no dedicated connection-progress surface.
- [`src/shell/tabs.rs`](../../src/shell/tabs.rs) still treats `connecting` as “use terminal surface”, which means the workspace shows an empty terminal canvas while the runtime is still establishing the connection.

## Root Cause

The “window becomes Not Responding” problem is primarily architectural, not cosmetic.

The current open-session path does this:

1. UI callback handles asset activation.
2. The callback calls `attempt_open_session_with_profile(...)`.
3. That path calls `probe_connection(...)`.
4. `probe_connection(...)` synchronously waits on the Tokio runtime with `block_on(...)`.

If the remote side is slow, or if the chain includes a proxy or one or more jump hosts, the main thread stops pumping window events. Windows then marks the process as unresponsive.

Even when the window does not hard-freeze, the product still feels poor because the UI exposes only a coarse `connecting` state and provides no insight into which hop or step is currently running.

## Product Principles

This design follows the same broad product pattern used by mature SSH and remote-development tools:

- keep connection work asynchronous so the window never stops responding;
- expose high-level progress while the connection is being established;
- keep detailed diagnostics available without forcing them into the primary visual focus;
- treat jump hosts and proxies as first-class connection steps rather than hiding them behind one spinner.

Reference material:

- [VS Code Remote - SSH](https://code.visualstudio.com/docs/remote/ssh)
- [JetBrains Gateway](https://www.jetbrains.com/help/idea/remote-development-a.html)
- [Termius Jump Hosts](https://termius.com/documentation/jump-hosts)
- [Termius Proxy](https://termius.com/documentation/proxy)
- [SecureCRT Trace / Debug Logging](https://www.vandyke.com/support/tips/configure-trace-options-debug-logging-in-securecrt.html)

## Constraints

- The window must never block on connection setup.
- The tab should appear immediately after the user opens an SSH asset.
- The workspace must show a purposeful connection page, not a blank terminal image.
- Multi-hop chains must show hop-aware progress.
- Terminal rendering stays a single atlas-backed workspace surface once connected.
- Error, cancel, retry, and host key decision flows must remain within the workspace context for open-session UX.
- Existing runtime features for proxies, jump hosts, terminal input, and host key verification should be reused rather than rewritten into a second connection stack.

## Approved Approach

### 1. Remove synchronous probing from the workspace open path

- Opening an SSH asset must create a tab immediately and return control to the UI thread immediately.
- The workspace open path must no longer call `probe_connection()` before session creation.
- The connection attempt should run entirely in the Tokio runtime and stream progress back into UI state.

### 2. Introduce a connection-attempt timeline model

- Keep the existing top-level session lifecycle, but add a richer per-attempt timeline model.
- Each connection attempt should own:
  - a stable attempt identifier;
  - the current session headline state;
  - an ordered list of timeline steps;
  - a structured diagnostics buffer;
  - optional user-decision prompt payload;
  - timestamps for total elapsed time and per-step durations.

Suggested session headline states:

- `connecting`
- `waiting-user`
- `connected`
- `cancelled`
- `disconnected`
- `error`

Suggested timeline step states:

- `pending`
- `running`
- `done`
- `blocked`
- `failed`
- `cancelled`

### 3. Model connection work as explicit steps, not generic strings

The runtime must emit structured step events for meaningful boundaries, not just raw log text.

Representative step kinds:

- `resolve-profile`
- `connect-proxy`
- `proxy-negotiate`
- `connect-jump-host`
- `authenticate-jump-host`
- `open-direct-tcpip`
- `connect-target`
- `verify-host-key`
- `authenticate-target`
- `open-session-channel`
- `request-pty`
- `request-shell`

Each step carries:

- a stable `step_id`;
- a `step_kind`;
- a user-facing `title`;
- a short `detail`;
- a `hop_label`, such as `Proxy`, `Jump Host 1`, `Jump Host 2`, or `Target`;
- timestamps and final status.

### 4. Render a dedicated connection-progress page inside the terminal workspace

The terminal host gets a new visual mode, separate from the real terminal surface and separate from the generic error page.

The connection page should have three layers:

- header:
  - session title;
  - target summary;
  - current hop summary;
  - elapsed time;
- main body:
  - a vertical timeline of connection steps;
  - a highlighted current step;
  - a concise current detail line;
- footer:
  - `Cancel` while running;
  - `Retry` and `Edit Connection` after failure or cancellation;
  - `Copy Diagnostics`;
  - `Show Diagnostics` / `Hide Diagnostics`.

The visual target is calm and premium rather than flashy:

- no giant empty spinner;
- subtle active-step motion;
- stable completed-step geometry;
- a smooth fade/slide from connection page into the real terminal once connected.

### 5. Keep diagnostics available but secondary

- The default workspace view shows only the timeline and the current step detail.
- Diagnostics are available through an expandable panel in the same page.
- Diagnostics should be structured, human-readable, and copyable.

Example diagnostics lines:

- `TCP connected to SOCKS5 proxy 10.0.0.2:1080`
- `SSH handshake started with bastion-a`
- `Host key verification required for target`
- `Authentication rejected by jump host bastion-b`

The primary UI should not dump raw Rust error chains until the user explicitly expands or copies diagnostics.

### 6. Handle user-decision steps inline for workspace sessions

Open-session UX should not jump out into a separate modal when the connection flow is already on-screen.

For workspace sessions:

- `unknown host key` blocks the `verify-host-key` step in place;
- the connection page displays the host, fingerprint, and action buttons;
- the user can choose `Trust and Continue` or `Reject`.

To keep runtime control flow robust, the runtime does not pause and resume an in-flight handshake. Instead:

1. the attempt transitions to a blocked `waiting-user` state;
2. the page shows the decision UI;
3. accepting the host key writes to `known_hosts`;
4. the system starts a fresh asynchronous retry attempt automatically;
5. the new attempt reuses the same tab and connection page context.

This keeps the UX continuous while avoiding brittle “suspend and resume” runtime state.

### 7. Keep modal `Test Connection` out of scope for the full-page timeline

This design is intentionally focused on the workspace open path.

- The asset modal can continue using compact busy/success/error feedback for `Test Connection`.
- The same low-level connection step model may later be reused there, but this iteration does not require rendering the full timeline inside the modal.

## Interaction Flow

### Open Session

1. User opens a saved SSH asset.
2. A tab appears immediately.
3. The workspace host enters `connection-progress`.
4. The timeline begins with `Resolve profile`.
5. As proxy and jump-host steps advance, the timeline updates in place.
6. On success, the page transitions into the live terminal.

### Unknown Host Key

1. Timeline reaches `Verify host key`.
2. The step becomes `blocked`.
3. Workspace host enters `waiting-user`.
4. Inline action area shows:
   - host;
   - port;
   - fingerprint;
   - `Trust and Continue`;
   - `Reject`.
5. `Trust and Continue` stores the key and restarts the attempt asynchronously.
6. `Reject` ends the attempt as `cancelled`, preserves diagnostics, and keeps the same tab on the connection-progress surface so `Retry` stays local to the timeline.

### Failure

1. The failing step becomes `failed`.
2. The page keeps the full timeline intact.
3. The footer switches to:
   - `Retry`
   - `Edit Connection`
   - `Copy Diagnostics`
4. The error page is no longer a contextless red message; it is the timeline itself with a pinned failure point.

### Cancel

1. User clicks `Cancel`.
2. The active attempt is asked to stop.
3. The current running step becomes `cancelled`.
4. The page shows a cancelled summary with `Retry`.

## Runtime Architecture

### Connection Orchestrator

Add an application-level connection orchestrator on top of the existing SSH runtime pipeline.

Responsibilities:

- create attempt state and timeline steps;
- emit progress events at runtime boundaries;
- accumulate diagnostics lines;
- surface user-decision requirements;
- attach the real `SessionRuntimeControl` only after the shell is ready;
- ignore stale events from earlier attempts once a retry begins.

### Event Flow

Suggested event family:

- `ConnectionAttemptStarted`
- `ConnectionStepStarted`
- `ConnectionStepUpdated`
- `ConnectionStepFinished`
- `ConnectionStepBlocked`
- `ConnectionStepFailed`
- `ConnectionAttemptCancelled`
- `ConnectionAttemptConnected`
- `ConnectionDiagnosticsAppended`

This event family should sit alongside existing surface and disconnect events rather than overloading them into generic text messages.

### Session Manager Integration

`SessionManager::open_session(...)` should:

1. create the session handle in `connecting`;
2. register initial empty timeline state;
3. spawn the connection attempt in the runtime;
4. return immediately.

It should not:

- block on a probe;
- wait for host key checks;
- wait for authentication;
- wait for PTY or shell startup.

### Workspace Projection

The workspace projection should distinguish these content modes:

- `welcome`
- `connection-progress`
- `terminal`
- `session-error`

Mapping:

- `connecting`, `waiting-user`, or `cancelled` -> `connection-progress`
- `connected` -> `terminal`
- `error` or `disconnected` -> `session-error`

The connection page itself can still live inside [`ui/shell/terminal-session-host.slint`](../../ui/shell/terminal-session-host.slint), but it must be rendered via a dedicated visual branch, not as an empty terminal frame.

Shipped alignment note:

- asynchronous launch failures can render one projection tick as `connecting` before the workspace sync loop projects the final `error` state into `session-error`; this keeps the open path non-blocking while still converging to the correct failure surface.

## Files

Likely touched files for implementation:

- Create [`src/app/ssh/connection_progress.rs`](../../src/app/ssh/connection_progress.rs)
- Modify [`src/app/ssh/mod.rs`](../../src/app/ssh/mod.rs)
- Modify [`src/app/ssh/runtime.rs`](../../src/app/ssh/runtime.rs)
- Modify [`src/app/ssh/session_manager.rs`](../../src/app/ssh/session_manager.rs)
- Modify [`src/app/bootstrap.rs`](../../src/app/bootstrap.rs)
- Modify [`src/shell/tabs.rs`](../../src/shell/tabs.rs)
- Modify [`src/shell/view_model.rs`](../../src/shell/view_model.rs)
- Modify [`ui/app-window.slint`](../../ui/app-window.slint)
- Modify [`ui/shell/workspace-pane.slint`](../../ui/shell/workspace-pane.slint)
- Modify [`ui/shell/terminal-session-host.slint`](../../ui/shell/terminal-session-host.slint)
- Modify [`tests/ssh_session_manager_spec.rs`](../../tests/ssh_session_manager_spec.rs)
- Modify [`tests/bootstrap_smoke.rs`](../../tests/bootstrap_smoke.rs)
- Modify [`tests/workspace_tabs_spec.rs`](../../tests/workspace_tabs_spec.rs)
- Optionally create [`tests/ssh_connection_timeline_spec.rs`](../../tests/ssh_connection_timeline_spec.rs)

## Risks

- Event ordering can become noisy or inconsistent if multiple retries write into one timeline without attempt scoping.
- Diagnostics volume can explode on long-lived retries unless bounded or grouped.
- Host key accept/retry flows can regress if stale blocked-state UI is not cleared when a new attempt begins.
- UI polish can regress into flicker if the switch from connection page to terminal surface is tied to unstable intermediate states.

## Validation

- Opening a slow SSH session must never freeze the window or mark it as unresponsive.
- A jump-host chain must show per-hop progress in the workspace.
- Unknown host key handling must stay within the workspace connection page for open-session UX.
- Failure must preserve step context and diagnostics.
- Connected state must transition cleanly into the real terminal without blank intermediate frames.
- Focused automated tests passed in-repo.
- Targeted manual verification on slow or multi-hop hosts still needs to be executed against real or controlled repro targets outside this repository environment.

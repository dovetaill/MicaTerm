# Dedicated exec ZMODEM modal lifecycle design

Date: 2026-07-17
Status: approved

## Problem restatement

Dedicated-exec uploads and interactive-terminal ZMODEM use different
`ZmodemController` instances, but their modal controls currently share commands
handled only by the interactive terminal pump. The exec task can publish a
Completed modal, yet Done, close, and Cancel are routed to a controller that
does not own that transfer. The manager therefore never receives the state
change needed to remove the visible modal.

The fix must separate two responsibilities:

1. `SessionManager` owns the durable projection shown by the application and
   must reliably dismiss terminal states.
2. `SshSessionRuntime` owns live transport work and must route running-state
   cancellation to the dedicated exec task that owns the SSH channel.

## Invariants

1. A running transfer is never hidden by dismissal. Running and selection
   phases use Cancel; Completed, Failed, and Cancelled use Dismiss.
2. A modal terminal state can be removed even when its exec task has already
   exited or the runtime command channel is unavailable.
3. A lifecycle command from an older transfer cannot clear or cancel a newer
   transfer in the same SSH session.
4. Exactly one dedicated-exec ZMODEM upload may own the per-session exec
   lifecycle slot at a time.
5. The task that registered an exec lifecycle slot may clear only its own
   generation.
6. Cancelling an active exec upload queues and writes the standard ZMODEM abort
   wire when the protocol and channel still permit it, then settles the exec
   channel and publishes a terminal state.
7. Natural completion, failure, cancellation, task drop, and runtime drop all
   release the exec lifecycle registration.
8. Interactive-PTY ZMODEM continues to be owned by the existing terminal pump.

## Considered approaches

### A. Manager-owned terminal dismissal plus runtime-owned exec cancel (chosen)

The manager conditionally removes terminal modal projections. The runtime keeps
a generation-scoped command sender only while a dedicated exec task is alive,
and routes Cancel through it. This addresses both broken paths without keeping
a completed upload task alive solely to wait for Done.

### B. Clear only the manager projection

This would make Done and close appear to work, but running Cancel would still
target the interactive controller. It would also leave the interactive
controller's terminal modal state uncleared. It is insufficient.

### C. Give every ZMODEM event a public transfer id and unify both paths under a
single actor

This offers a general multi-transfer architecture, but changes all modal event
and controller contracts for a defect confined to dedicated exec ownership. It
is larger than the requested fix. The chosen design uses private projection
revisions and exec generations instead.

## Ownership model

| Concern | Owner | Lifetime |
| --- | --- | --- |
| Visible modal projection | `SessionManager` registry | Until terminal dismissal or session cleanup |
| Interactive controller and PTY channel | Main SSH channel pump | SSH session lifetime |
| Dedicated exec controller and channel | Exec upload task | One upload attempt |
| Active exec Cancel routing | `SshSessionRuntime` lifecycle slot | Same exec task generation |

No UI component becomes a source of truth. The Slint modal continues to project
the state returned by `SessionManager::zmodem_state`.

## Terminal-state dismissal

### Revisioned manager projection

Store each projected ZMODEM state with a private monotonically increasing
revision:

```text
ProjectedZmodemTransfer
  revision: u64
  state: ZmodemTransferState
```

Every `ZmodemStateChanged(Some(state))` installs a new revision. Public callers
still receive only `ZmodemTransferState`; no public event or UI model changes.

`SessionManager::dismiss_zmodem_transfer(session_id)` performs this sequence:

1. Snapshot the current projected record and runtime control without holding a
   registry lock across a runtime call.
2. If no state exists, return success as an idempotent no-op.
3. Reject AwaitingUploadSelection, AwaitingDownloadDirectory, or Running. Those
   phases must use Cancel.
4. For Completed, Failed, or Cancelled, request best-effort cleanup of the
   runtime's matching internal terminal state.
5. Re-lock the registry and remove the projection only if its revision is still
   the snapshotted revision.

A missing runtime, closed command channel, or no-op cleanup is logged but does
not prevent a terminal projection from closing. A newer revision is retained.

### Interactive controller cleanup

The runtime dismissal command carries the expected terminal state. The main
pump may clear its internal controller only when that controller has no active
session and its modal state matches the expected state.

Manager dismissal is authoritative for projection removal, so processing this
cleanup command must consume the controller's local dirty `None` rather than
publishing an unconditional `ZmodemStateChanged(None)`. Otherwise a delayed
cleanup event could remove a newer exec or interactive projection. Natural
non-dismissal state transitions continue to use the existing event path.

This makes dedicated exec cleanup a safe no-op in the interactive pump while
still releasing a completed interactive controller's internal modal state.

## Dedicated exec lifecycle control

### Runtime slot

Add a session-local slot shared by `SshSessionRuntime` and weakly referenced by
spawned exec tasks:

```text
ExecZmodemTransferSlot
  next_generation: u64
  active:
    generation: u64
    command_tx: UnboundedSender<ExecZmodemCommand>

ExecZmodemCommand
  Cancel
```

Starting a dedicated exec upload allocates a generation, creates a command
channel, registers the sender, and passes the receiver to the spawned task. A
second start is rejected while an open sender is registered. A closed sender is
treated as stale and conditionally removed before a new generation is created.

The task holds a cleanup guard containing a `Weak` slot reference and its
generation. On every exit path, the guard clears `active` only when the stored
generation still matches. The weak reference avoids keeping the runtime and its
sender alive after session teardown.

### Cancel routing

`SshSessionRuntime::cancel_zmodem_transfer` checks the exec slot first:

```text
active exec sender accepts Cancel -> return success (owner=exec)
active sender is closed           -> clear same generation, then fall through
no active exec sender             -> existing main-pump Cancel (owner=interactive)
```

The generation comparison prevents a failed send from clearing a registration
installed by a newer task. The existing interactive cancellation command and
controller behavior remain unchanged.

## Exec task cancellation

`run_zmodem_exec_upload_inner` selects between SSH channel messages and
`ExecZmodemCommand` throughout the handshake and transfer loops.

When Cancel arrives after a protocol session exists:

1. Call `ZmodemController::cancel`, producing Cancelled state and queuing the
   existing exact `ZMODEM_ABORT_WIRE`.
2. Emit the Cancelled modal update.
3. Drive the controller so the abort wire is written to the dedicated exec
   channel when transport is still writable.
4. Send EOF and close the exec channel on a best-effort basis.
5. Return a Cancelled task outcome rather than an error.

If Cancel arrives before the remote handshake creates a controller session,
the task publishes a synthetic Cancelled upload state, writes the same abort
wire when the accepted channel permits it, and closes the channel. It must not
wait for the handshake timeout.

Use an explicit internal outcome so the outer wrapper distinguishes expected
cancellation from failure:

```text
Completed       -> retain existing success state and completion log
Cancelled       -> retain Cancelled state; do not overwrite it with Failed
RuntimeClosed   -> close transport; session cleanup owns projection removal
Err(error)      -> publish the existing Failed upload state
```

If the exec control receiver closes because the runtime/session is dropped, the
task settles the channel and exits without resurrecting a modal after disconnect.

## State and command matrix

| Projected phase | Done / primary | Close / Escape | Cancel / secondary |
| --- | --- | --- | --- |
| Awaiting upload selection | Existing file selection | Cancel owner task | Cancel owner task |
| Awaiting download directory | Existing folder selection | Cancel interactive task | Cancel interactive task |
| Running | No dismiss | Cancel owner task | Cancel owner task |
| Completed | Revision-checked dismiss | Revision-checked dismiss | Existing reveal action |
| Failed | No primary action | Revision-checked dismiss | Revision-checked dismiss |
| Cancelled | No primary action | Revision-checked dismiss | Revision-checked dismiss |

No Slint behavior change is expected. The title-bar X and Escape already emit
`zmodem-transfer-modal-close-requested`, and the Rust callbacks already route
terminal phases to manager dismissal.

## Failure and race behavior

- Repeated terminal dismissal after removal returns success and does nothing.
- A runtime cleanup failure cannot reopen or retain an otherwise dismissible
  terminal projection.
- A projection update that races with dismissal wins when its revision differs.
- An old task's cleanup guard cannot clear a newer exec generation.
- A closed exec control sender is removed only after matching its generation;
  cancellation then falls back to the interactive pump.
- A Cancel accepted just as the exec task naturally completes may resolve as
  Completed or Cancelled, but it cannot remain Running or become an unrelated
  session error solely because the command targeted the wrong controller.
- Session disconnect keeps the existing manager cleanup behavior and drops the
  runtime-owned sender, waking the exec task through receiver closure.

## Diagnostics

Add structured `app.zmodem` lifecycle logs with safe fields:

- `session_id`
- `transfer_generation` for dedicated exec work
- `lifecycle_command`: `cancel` or `dismiss`
- `owner`: `exec`, `interactive`, or `projection`
- `outcome`: `routed`, `cleared`, `ignored`, `stale`, or `failed`
- current transfer phase when available

Keep existing path count, remote directory, byte count, and file count logs.
Do not log file contents, credentials, protocol payloads, or remote command
output.

## Test design

### Session manager projection tests

1. No projected state: dismiss succeeds idempotently.
2. Awaiting or Running: dismiss is rejected and state remains.
3. Completed, Failed, and Cancelled: dismiss removes the projection even when
   runtime cleanup is a no-op or returns an error.
4. Install a newer revision between snapshot and conditional removal; the newer
   state survives.

### Bootstrap modal regressions

Replace the masking assumption in the modal fixture: its runtime dismiss method
returns success but does not emit `ZmodemStateChanged(None)`. Then prove:

1. Done closes a Completed dedicated-exec modal and it stays closed across
   multiple projection flushes.
2. The close callback produces the same result.
3. Failed and Cancelled close successfully.
4. The Slint source contract keeps Escape wired to the same close callback.

### Exec lifecycle unit tests

1. Cancel is delivered to the active generation.
2. A stale cleanup guard cannot clear a newer registration.
3. A failed send clears only the matching stale generation and falls back to
   interactive Cancel.
4. Overlapping active exec starts are rejected; a start after cleanup succeeds.

### Live russh cancellation regression

Extend the in-process SSH server fixture to accept the dedicated `rz -q` exec,
emit a valid ZRINIT header, and capture client channel data. Start an upload,
wait for Running, call manager/runtime Cancel, and assert:

- the visible state becomes Cancelled rather than remaining Running;
- the server receives the exact existing ZMODEM abort wire;
- the exec channel reaches EOF/close; and
- the interactive pump controller is not required for the result.

The existing controller unit test remains the byte-level contract for the abort
wire; the live test proves correct ownership and routing.

## Compatibility and scope

- No persisted schema, public modal model, layout, labels, or automatic-close
  policy changes.
- No changes to cwd probing, `rz` detection, `rz -q` command construction,
  transfer framing, SFTP fallback, or Bash wildcard handling.
- Interactive upload/download protocol processing remains on the main pump.
- Expected production edits are limited to SSH runtime lifecycle routing,
  manager modal projection bookkeeping, and focused diagnostics. UI code should
  require tests only unless a wiring discrepancy is found.

## Rollback boundaries

- Manager revisioned dismissal can be reverted independently of exec Cancel
  routing.
- The exec lifecycle slot and task command receiver are internal and require no
  migration or persisted cleanup.
- Interactive cleanup remains isolated in the main pump command handler.

## Acceptance mapping

- PRD AC1-AC3: manager and bootstrap terminal-dismissal regressions.
- PRD AC4: exec lifecycle unit coverage plus live russh abort-wire regression.
- PRD AC5: projection revision and exec generation race tests.
- PRD AC6: existing interactive ZMODEM, final-wire, drag-routing, and terminal
  interaction test suites.
- PRD AC7: formatting, focused targets, owning integration targets, build
  checks, and Trellis quality verification before completion.

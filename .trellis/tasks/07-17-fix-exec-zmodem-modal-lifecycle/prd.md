# Fix dedicated exec ZMODEM modal lifecycle

## Goal

Make every control on a dedicated-exec ZMODEM upload act on the transfer that
owns the visible modal. After completion, Done, the title-bar close button, and
Escape must close the modal permanently. While the upload is running, Cancel
must reach the dedicated exec controller, abort the remote transfer, and leave a
dismissible terminal state.

## Background

- The 2026-07-17 runtime log proves routing and transfer completed correctly:
  remote cwd `/root/1`, `rz` probe exit status 0, `upload_method=zmodem_exec`,
  one file completed, and 160320 bytes transferred.
- `SshSessionRuntime::start_zmodem_upload_to_remote_dir` spawns
  `run_zmodem_exec_upload` and retains no lifecycle control for that task
  (`src/app/ssh/runtime.rs:444`).
- `run_zmodem_exec_upload_inner` creates a private `ZmodemController` owned by
  the spawned exec task (`src/app/ssh/runtime/pump.rs:789`, controller at
  `src/app/ssh/runtime/pump.rs:806`).
- Done and completed-state close both call
  `SessionManager::dismiss_zmodem_transfer` (`src/app/bootstrap.rs:8285`,
  `src/app/bootstrap.rs:8366`). The runtime converts that to
  `RuntimeCommand::DismissZmodem` (`src/app/ssh/runtime.rs:478`).
- `RuntimeCommand::DismissZmodem` is handled by the interactive terminal pump's
  different `ZmodemController` (`src/app/ssh/runtime/pump.rs:340`). With no
  modal state there, `ZmodemController::dismiss` returns false and publishes no
  state change (`src/app/ssh/runtime/zmodem.rs:414`).
- SessionManager removes the visible transfer only after receiving
  `ZmodemStateChanged(None)` (`src/app/ssh/session_manager.rs:1434`). Because no
  such event is emitted, periodic projection sees the stored Completed state
  and reopens the modal (`src/app/bootstrap.rs:4459`,
  `src/app/bootstrap.rs:6770`).
- The existing bootstrap tests use `ZmodemModalRuntimeControl`, whose fake
  dismiss/cancel methods directly emit `ZmodemStateChanged(None)`
  (`tests/bootstrap_smoke.rs:2427`). They bypass the real split-controller path
  and therefore cannot detect this defect.
- The same ownership split means running-state Cancel is sent to the interactive
  pump rather than the dedicated exec upload controller.

## Requirements

- R1: A dedicated exec upload must expose one session-scoped lifecycle control
  path that targets the controller and SSH channel owned by that upload task.
- R2: Done, title-bar close, and Escape on Completed must clear the persisted
  transfer state and keep the modal closed across subsequent projection ticks.
- R3: Close on Failed or Cancelled must use the same reliable dismissal path.
- R4: Cancel during a running dedicated exec upload must request cancellation
  from the owning exec task, queue/write the ZMODEM abort wire when possible,
  close or settle the exec channel, and publish a terminal Cancelled or Failed
  state. It must not target only the interactive terminal controller.
- R5: Dismiss must never hide an actually running transfer. Running states use
  cancellation; only terminal states may be dismissed.
- R6: Interactive-PTY ZMODEM upload/download behavior, ordinary terminal pump
  ownership, `rz -q`, transfer framing, and SFTP fallback must remain unchanged.
- R7: Lifecycle completion, task failure, session disconnect, and repeated
  dismissal must not leave a stale control handle or resurrect an old modal.
- R8: Add structured `app.zmodem` diagnostics for lifecycle command routing and
  ignored/stale commands without logging file contents or credentials.

## Acceptance Criteria

- [x] AC1: A regression representing a real dedicated exec Completed upload
  proves Done removes `SessionManager::zmodem_state(session_id)` and the modal
  stays closed after additional projection flushes.
- [x] AC2: The title-bar close callback and Escape path have the same Completed
  dismissal result as Done.
- [x] AC3: Failed and Cancelled dedicated exec terminal states can be dismissed
  without relying on the interactive pump controller.
- [x] AC4: A running dedicated exec upload receives Cancel through its own
  lifecycle channel and emits the expected abort/terminal-state evidence.
- [x] AC5: A stale or repeated dismiss/cancel command is idempotent and cannot
  remove or cancel a newer transfer for the same session.
- [x] AC6: Existing interactive ZMODEM modal, final-wire, drag-routing, and
  terminal interaction tests remain green.
- [x] AC7: Focused tests, owning integration targets, formatting, build checks,
  and applicable Trellis quality checks pass.

Verification qualification: all focused and owning test targets, formatting,
`cargo check`, and diff checks pass. Repo-wide Clippy remains blocked by 35
pre-existing library diagnostics (with additional test-target baseline
diagnostics) outside the changed lines. Windows interaction remains the manual
follow-up recorded in `implement.md`; cancellation transport behavior is
covered by the in-process russh tests.

## Out Of Scope

- Redesigning the ZMODEM modal or changing button labels.
- Automatically closing successful transfers without user action.
- Moving ZMODEM downloads onto dedicated exec channels.
- Changing remote cwd discovery, `rz` availability probing, SFTP fallback, or
  Bash wildcard behavior.
- General multi-transfer queueing beyond preventing lifecycle commands from
  affecting the wrong transfer generation.

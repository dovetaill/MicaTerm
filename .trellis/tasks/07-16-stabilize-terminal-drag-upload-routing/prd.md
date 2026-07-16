# Stabilize terminal drag upload routing across remote cwd changes

## Goal

Make single-file drag uploads choose a deterministic transfer path while the user
changes directories inside one remote SSH shell. A sequence such as remote
directory A -> B -> B -> C must not alternate between the ZMODEM modal and the
Transfer Center because SSH exec messages arrived in a different valid order.

## Background

- The directories A, B, and C are remote shell working directories. The dragged
  item is one local file on every attempt.
- The current routing contract prefers a dedicated ZMODEM exec upload when the
  remote cwd is known and `rz` is available. SFTP/Transfer Center is the fallback
  when `rz` is confirmed unavailable; interactive ZMODEM is retained for the
  existing safe-cwd-unavailable fallback.
- `terminal_current_working_directory_for_drop` re-probes the remote cwd when
  live cwd tracking is unavailable
  (`src/app/bootstrap/sftp.rs:1660`).
- `schedule_terminal_zmodem_drop_from_paths` probes `rz` on each eligible drop
  and selects ZMODEM or SFTP from that result
  (`src/app/bootstrap/sftp.rs:1725`).
- Both the cwd probe and the `rz` probe require `exit_status == Some(0)`
  (`src/app/ssh/runtime/pump.rs:691`, `src/app/ssh/runtime/pump.rs:709`).
- Their shared SSH exec collector currently stops on either `ChannelMsg::Eof`
  or `ChannelMsg::Close` (`src/app/ssh/runtime/pump.rs:907`). SSH EOF only ends
  data in one direction; exit status is a separate channel request and may
  validly arrive after EOF (RFC 4254 sections 5.3, 6.10, and 6.5).
- Consequently, a valid `Data -> EOF -> ExitStatus(0) -> Close` sequence loses
  the exit status. The cwd probe can then reject a valid path, or the `rz` probe
  can report `false`, causing the same session and same remote directory to
  alternate transfer UI based on message timing.
- Existing routing tests cover explicit cwd/rz availability changes, including
  re-probing a probe-derived cwd, but do not cover valid EOF-before-exit-status
  ordering or repeated A -> B -> B -> C drops with stable remote capability.

## Requirements

- R1: Collect remote exec output through the channel's actual terminal condition
  so a valid exit status sent after EOF is not discarded.
- R2: Preserve the distinction between a confirmed successful probe, a confirmed
  non-zero result, and an incomplete/failed probe. An incomplete probe must not
  silently masquerade as confirmed command absence.
- R3: Keep the established upload routing contract: known cwd plus confirmed
  `rz` uses dedicated ZMODEM; confirmed missing `rz` uses SFTP/Transfer Center;
  the existing safe interactive fallback remains available when cwd cannot be
  resolved.
- R4: Preserve the exact remote cwd for each upload. In A -> B -> B -> C, the
  upload commands must target A, B, B, and C respectively.
- R5: Add diagnostics that identify the cwd source, raw probe outcome, and chosen
  upload method without logging file contents or credentials.
- R6: Keep the change scoped to SSH exec result collection and terminal-drop
  routing coverage. Do not change ZMODEM framing, shell glob behavior, or the
  general SFTP upload workflow.

## Acceptance Criteria

- [ ] AC1: Tests accept `ExitStatus(0) -> EOF -> Close`,
  `EOF -> ExitStatus(0) -> Close`, and
  `Data -> EOF -> ExitStatus(0) -> Close` without losing status or stdout.
- [ ] AC2: A live russh test server that sends EOF before exit status still
  produces `Ok(true)` for a successful `remote_command_exists` probe, while a
  genuinely missing exit status is surfaced as an incomplete probe/error rather
  than `Ok(false)`.
- [ ] AC3: Four consecutive single-file drops after remote cwd changes
  A -> B -> B -> C all select dedicated ZMODEM when `rz` remains available.
- [ ] AC4: The four dedicated ZMODEM commands target A, B, B, and C in order.
- [ ] AC5: A confirmed non-zero `rz` probe still selects SFTP/Transfer Center.
- [ ] AC6: Existing cwd-unavailable interactive fallback behavior remains
  covered and unchanged.
- [ ] AC7: Focused Rust unit/integration tests, formatting, and the applicable
  project checks pass.

## Out Of Scope

- Changing Bash wildcard expansion or prompt rendering.
- Replacing ZMODEM with SFTP as the preferred terminal-drop path.
- Redesigning the ZMODEM modal or Transfer Center UI.
- Adding a persistent cross-session remote capability cache.

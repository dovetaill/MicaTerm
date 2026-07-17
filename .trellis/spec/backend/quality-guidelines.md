# Quality Guidelines

> Code quality standards for backend development.

---

## Overview

<!--
Document your project's quality standards here.

Questions to answer:
- What patterns are forbidden?
- What linting rules do you enforce?
- What are your testing requirements?
- What code review standards apply?
-->

(To be filled by the team)

---

## Forbidden Patterns

<!-- Patterns that should never be used and why -->

(To be filled by the team)

---

## Required Patterns

<!-- Patterns that must always be used -->

(To be filled by the team)

---

## Testing Requirements

<!-- What level of testing is expected -->

(To be filled by the team)

---

## Code Review Checklist

<!-- What reviewers should check -->

(To be filled by the team)

## Scenario: ZMODEM, SSH Exec Probes, and External Drop Transfer Boundaries

### 1. Scope / Trigger

- Trigger: terminal transfers cross protocol runtime, SSH channel IO, Slint modal state, OS drag/drop, and SFTP upload scheduling.
- Trigger: short-lived cwd and command probes cross russh channel-message ordering and terminal-drop routing; valid SSH EOF, exit-status, and close messages may arrive separately.
- Applies when modifying `src/app/ssh/runtime.rs`, `src/app/ssh/runtime/zmodem.rs`, `src/app/ssh/runtime/pump.rs`, `src/app/ssh/session_manager.rs`, `src/app/bootstrap.rs`, `src/app/bootstrap/windowing.rs`, `src/app/bootstrap/sftp.rs`, `ui/components/zmodem-transfer-modal.slint`, or `vendor/winit/src/platform_impl/windows/drop_handler.rs`.

### 2. Signatures

- `ZmodemController::cancel() -> Result<()>` must queue wire-level abort bytes and publish a local cancelled modal state.
- `ZmodemTransferState` carries UI progress plus local completion targets: `local_file_path: Option<PathBuf>` and `local_reveal_path: Option<PathBuf>`.
- `window.on_workspace_terminal_external_drop_requested` consumes `workspace-terminal-external-drop-paths`, probes `rz` for ordinary files, then falls back to `schedule_terminal_cwd_upload_from_paths` only when ZMODEM is unavailable or unsuitable.
- `SessionRuntimeControl::resolve_current_working_directory(&self) -> Result<Option<String>>` is the non-interactive runtime escape hatch for terminal drops when no OSC7/shell-integration cwd snapshot exists.
- `SessionManager::resolve_current_working_directory(session_id) -> Result<Option<String>>` delegates to the active runtime and caches a successful cwd in `current_working_directories`.
- `SessionManager::remote_command_exists(session_id, "rz") -> Result<bool>` probes lrzsz through a dedicated SSH exec channel with the same transfer PATH setup used by exec-channel upload; it must not call `send_session_text_input`.
- `RemoteExecOutput::push_message(&mut self, ChannelMsg, &'static str) -> Result<bool>` is the single transition owner for short-lived exec-probe stdout, EOF, exit status, request acceptance, and close; `true` means enough channel facts exist to finish collection.
- `require_remote_exec_exit_status(Option<u32>, &'static str) -> Result<u32>` converts a closed/incomplete exec result with no exit status into an error instead of command absence.
- `remote_command_exists(handle, command_name) -> Result<bool>` maps exit status `0` to `true`, a confirmed non-zero status to `false`, and missing status to `Err`.
- `SessionManager::start_zmodem_upload_to_remote_dir(session_id, local_paths, remote_dir) -> Result<()>` starts drag-triggered ZMODEM uploads through a dedicated SSH exec channel running `<transfer PATH setup>; cd <quoted remote_dir> && rz -q`.
- `SessionManager::start_interactive_zmodem_upload(session_id, local_paths) -> Result<()>` starts the active-PTY fallback for drag-triggered ZMODEM upload when the active terminal cwd cannot be resolved and the terminal mode allows interaction.
- `SessionRuntimeControl::dismiss_zmodem_transfer(&self, expected_state: ZmodemTransferState) -> Result<()>` performs identity-safe cleanup of a matching inactive runtime controller; it is not the source of truth for removing the visible terminal-state modal.
- `SessionManager::dismiss_zmodem_transfer(session_id) -> Result<()>` snapshots the private projected revision, releases the registry lock, requests best-effort runtime cleanup, then removes only the same revision for Completed, Failed, or Cancelled.
- A dedicated exec upload registers one `ExecZmodemTransferContext` containing its monotonic generation, `UnboundedReceiver<ExecZmodemCommand>`, and weak cleanup registration. `SshSessionRuntime::cancel_zmodem_transfer()` routes `ExecZmodemCommand::Cancel` to that context before falling back to `RuntimeCommand::CancelZmodem` for interactive PTY ownership.
- `SessionRegistry::runtime_controls` stores `Arc<Mutex<Box<dyn SessionRuntimeControl>>>`; callers must clone the shared control while holding the registry lock, release the registry lock, then invoke runtime methods under the per-control mutex.
- `SftpTransferBackgroundMessage` carries UI intent flags `open_queue_drawer: bool` and `open_transfer_center: bool` so background terminal-drop SFTP fallback can ask the main thread to show transfer UI without mutating `ShellViewModel` off-thread.
- `negotiated_terminal_environment()` must include cwd-only bash `PROMPT_COMMAND` tracking that emits OSC7 with `$PWD`; this is not the retired enhanced bootstrap and must not include `MICA_TERM_ENHANCED`.
- `ZmodemController::intercept_remote_bytes()` must strip lrzsz's `rz\r` / `rz\n` download autostart text only when it is immediately followed by a detected `ZRQINIT` prefix.
- `ZmodemController::expect_automatic_rz_echo()` arms one exact app-generated interactive upload echo. `intercept_remote_bytes()` may strip `" rz -q\r"` (plus CRLF and legacy in-flight variants) only while armed and only when a CRC-valid `ZRINIT` header immediately follows; identical unarmed manual text is terminal output.
- `ZmodemController::intercept_remote_bytes()` must treat the six-byte `**\x18B00` / `**\x18B01` marker as tentative and validate the complete 18-byte ZHEX header with `zmodem2` before starting a session. Leading candidate stars may be rendered immediately, then repainted with `\x08 \x08` only after validation proves they belong to the protocol header.
- `SenderTransfer` and `ReceiverTransfer` treat `SessionCompleted` as "completion event observed", not "all final wire bytes written"; they stay alive until the protocol emits no more `WriteWire`.
- `ZmodemSession::take_pending_wire() -> Vec<u8>` transfers parser-unconsumed bytes before a completed session is dropped. `drive_zmodem() -> Option<Vec<u8>>` returns released terminal bytes to the ordinary SSH output path after final protocol wire has been written.
- The vendored Windows winit `IDropTarget` must implement COM `QueryInterface` for `IUnknown` and `IDropTarget`; `unimplemented!()` at this boundary can prevent shell drag/drop from reaching Slint/winit.
- The vendored Windows winit `IDropTargetVtbl` must match the Win32 COM ABI: `DragEnter`, `DragOver`, and `Drop` receive `POINTL pt` by value, not `*const POINTL`; `IDataObject` is passed as `IDataObject *`.

### 3. Contracts

- ZMODEM cancel is a wire contract: local state changes are not enough; remote `rz` / `sz` must receive an abort frame.
- ZMODEM modal state is a projection ownership contract. `SessionManager` owns visible terminal-state dismissal; a completed dedicated exec task must not remain alive waiting for Done, X, or Escape.
- Dismiss and Cancel are distinct. AwaitingUploadSelection, AwaitingDownloadDirectory, and Running use Cancel and remain visible until a terminal outcome arrives. Completed, Failed, and Cancelled use Dismiss.
- Runtime dismiss cleanup must match the expected controller state and consume its local dirty `None` without publishing an unconditional delayed `ZmodemStateChanged(None)`, which could erase a newer transfer projection.
- Dedicated exec lifecycle cleanup is generation-conditional. A task guard, failed send, or stale command may clear only the generation it captured; runtime drop closes the sender so the exec task settles its channel without resurrecting a modal.
- Dedicated exec upload loops must select between SSH channel messages and lifecycle commands before and after ZRINIT. Cancel queues/writes the shared `ZMODEM_ABORT_WIRE` when possible, publishes Cancelled, and sends EOF/Close on the exec channel; it must not be sent only to the interactive pump's controller.
- ZMODEM receiver `FileStarted` is not a one-shot UI event; the runtime must treat repeated metadata for the currently active file as idempotent and must not create/truncate the local target twice.
- ZMODEM receiver `FileStarted` can also repeat after `FileCompleted` and before `SessionCompleted`; repeated metadata for the last completed file must be ignored so the UI does not show "file 1 of at least 2" for a one-file transfer.
- ZMODEM session completion is a final-wire contract: `zmodem2::poll()` returns pending events before pending final wire (`ZFIN` / `OO`), so both `finished = true` and the public `Completed` modal phase must wait for the following idle poll after all `WriteWire` actions were acknowledged.
- Tentative detection is lossless: local input, paste, timeout, resize, failure, and transport close must never clear unseen ordinary bytes. A provisionally rendered prefix is emitted once; flush paths return only the unseen suffix.
- Completed-session cleanup is direction-aware. Both directions consume only the unconsumed ZFIN `\r` plus `\n`/`\x8a` trailer; download then consumes exactly one remote `OO`, while upload expects no inbound `OO` because the local sender writes it. Any mismatch is released unchanged, and broad filtering by prefixes such as `OO`, `rz`, stars, ZHEX-looking text, or control bytes is forbidden.
- Same-batch prompt bytes extracted from a completed session must be returned through `process_ready_remote_output()` immediately. Waiting for a later SSH batch can make a completed upload appear stalled indefinitely.
- Completed downloads with exactly one local file should expose both `Open` and `Open Folder`; multi-file downloads should at least expose `Open Folder`.
- ZMODEM download conflict handling must not silently map the default `Ask` policy to auto-rename; until a per-file ZMODEM conflict modal exists, `Ask` should reuse the requested filename and `AutoRename` must remain opt-in.
- External file drops may emit `HoveredFileCancelled` after `DroppedFile`; once a drop flush is pending, cancellation must not clear queued paths.
- External drop routing must not silently discard terminal drops when exact pointer hit testing fails; if the workspace host is terminal and there is an active terminal session, route to terminal as a fallback and log the miss at `info`.
- Windows external drop support depends on the vendored winit COM drop target. `QueryInterface` must return the same drop-target interface pointer with `AddRef` for `IID_IUnknown` and `IID_IDropTarget`, and return `E_NOINTERFACE` for unsupported interfaces.
- Windows external drop support must release `STGMEDIUM` returned by `IDataObject::GetData` after enumerating `CF_HDROP` paths; the handler should log `winit.drop` registration, enter, and drop boundaries so a missing `app.drop` log can be traced to the OS/winit boundary.
- Terminal external drops prefer ZMODEM `rz` for regular files and use `SessionManager::current_working_directory(session_id)` only for exec-channel ZMODEM cwd selection or SFTP fallback, not the SFTP panel path.
- Drag-triggered automatic ZMODEM upload must not inject command-probe snippets such as `if command -v rz ...` into the interactive terminal. Those snippets are echoed, can enter shell history, and confuse users after upload completion.
- Drag-triggered automatic ZMODEM upload may use a dedicated SSH exec channel for `rz -q` because exec requests do not touch the interactive PTY scrollback or shell history. The remote working directory must be shell-quoted before `cd`; `-q` suppresses lrzsz's non-protocol `rz waiting to receive.` stderr banner.
- Drag-triggered `rz` detection must account for non-interactive exec PATH differences by exporting a transfer PATH containing `$HOME/.local/bin`, `$HOME/bin`, `/usr/local/bin`, `/usr/bin`, and standard sbin/bin directories before `command -v rz` and before exec-channel `rz`.
- Short-lived SSH exec collectors must treat `ChannelMsg::Eof`, `ChannelMsg::ExitStatus`, and `ChannelMsg::Close` as independent facts. EOF ends remote stdout but does not close the channel; a valid exit status can arrive after EOF.
- Exec-probe collection may finish on `Close`, stream end, or once both EOF and exit status are known. If exit status arrives before EOF, keep collecting so late stdout is not discarded; if EOF arrives first, keep collecting so the status is not discarded.
- A missing exec-probe exit status is incomplete transport metadata, not proof that `rz` is absent. Surface it as an error so terminal-drop routing can log `rz_probe_error`; reserve `Ok(false)` and `rz_missing` for a confirmed non-zero command probe.
- Do not add retries or per-session capability caches to conceal an incorrect exec collector. Fix message ownership at `remote_exec_output`; retries add latency and caches leave cwd probes broken or stale.
- When the interactive cwd cannot be resolved for ordinary file drops and the terminal mode allows interaction, drag-triggered automatic ZMODEM upload must fall back to the active PTY by sending only the minimal history-friendly command ` rz -q\r`. Do not let an exec-channel `command -v rz` false result veto this fallback: exec channels can see a different PATH or restricted shell behavior than the active PTY. This is the only allowed interactive auto-start command; its echo must be stripped from visible terminal output when the `ZRINIT` handshake follows.
- Interactive `rz` fallback must publish a ZMODEM upload failure state if `ZRINIT` is not observed within the handshake timeout, so a genuinely missing remote `rz` does not silently stall the UI.
- Application cursor-key mode alone must not disable drag-triggered `rz` fallback. Alternate screen and mouse-grabbed states can block interactive `rz`; application cursor-key mode is too broad and can be set on ordinary prompts.
- A missing active terminal surface snapshot at drop scheduling time is not proof that interactive `rz` is unsafe. If the session is active and cwd resolution returns `None`, allow the interactive `rz` fallback unless an observed surface explicitly reports alternate screen or mouse-grabbed state.
- Terminal drops must not require shell-integration markers. If `SessionManager::current_working_directory(session_id)` is empty, the drop scheduler must call `SessionManager::resolve_current_working_directory(session_id)` before probing `rz` or falling back to SFTP.
- Terminal drop UI callbacks must only snapshot UI state, clear hover/drop paths, and enqueue background work. They must not call `remote_command_exists`, `resolve_current_working_directory`, `sftp_read_dir`, `sftp_execute_queued_transfers*`, or `sync_shell_side_regions(..., Some(manager))` after scheduling a background probe, because those paths can block the Slint event loop.
- Background terminal-drop SFTP fallback may validate the remote cwd and enqueue `schedule_sftp_upload_paths`, but UI changes such as opening the SFTP queue drawer or transfer center must return through `SftpTransferBackgroundMessage` and be applied by `drain_sftp_transfer_background_messages`.
- `SessionManager` must not hold the registry mutex while calling any `SessionRuntimeControl` method that can perform I/O, block, or wait on a runtime channel. Holding the registry lock around runtime calls can freeze unrelated UI projection reads while a background SSH probe is slow.
- Runtime cwd tracking should be established at session startup with a minimal environment-level bash `PROMPT_COMMAND` that emits OSC7. This avoids drag-time shell command injection while keeping `SessionManager::current_working_directory` fresh after `cd`.
- The SSH runtime cwd resolver must use a dedicated exec channel and must not write bytes to the interactive terminal. On Linux/OpenSSH it may inspect `/proc`, `SSH_CONNECTION`, and the sibling interactive shell process to recover the shell cwd.
- Terminal drop scheduling must treat a runtime cwd probe error or timeout the same as an unresolved cwd: log it at `app.drop` and continue to interactive `rz` fallback when the terminal mode allows it.
- A cwd resolved through the runtime probe must be cached in `current_working_directories` so a subsequent SFTP fallback uses the same target directory.
- A cwd resolved through the runtime probe is a point-in-time fallback, not live shell tracking. If the session has not emitted a runtime `CurrentDirectoryChanged` event from terminal output, the next terminal drop must probe cwd again instead of trusting a previous probe cache that may still point at the login directory.
- Terminal drop failures must log an `app.drop` warning with `path_count` and `error`; a failed cwd probe must not look like a silent no-op in packaged logs.

### 4. Validation & Error Matrix

- No active terminal session -> reject terminal drop with user-visible feedback.
- Missing terminal cwd + runtime cwd probe succeeds -> continue with the resolved cwd and cache it.
- Probe-derived cwd cache + no live terminal cwd tracking -> refresh cwd through the runtime probe on the next terminal drop before choosing exec-channel `rz` or SFTP fallback.
- Missing terminal cwd + runtime cwd probe returns none or times out + terminal mode allows interaction -> start interactive `rz` fallback with `SessionManager::start_interactive_zmodem_upload` without requiring a successful exec-channel `rz` probe.
- Missing active terminal surface snapshot + runtime cwd probe returns none -> start interactive `rz` fallback; do not surface a cwd preflight error and do not enqueue an SFTP transfer task.
- Missing terminal cwd + runtime cwd probe returns none or times out + terminal mode explicitly blocks interaction -> reject terminal drop with user-visible feedback and an `app.drop` warning because SFTP fallback has no target directory and PTY `rz` is unsafe.
- `remote_command_exists(session_id, "rz") == false` + known terminal cwd -> fall back to SFTP upload into that cwd.
- Exec probe receives `ExitStatus(0) -> EOF -> Close` -> retain status 0 and finish at EOF without waiting for close.
- Exec probe receives `Data -> EOF -> ExitStatus(0) -> Close` -> retain all stdout, wait past EOF, retain status 0, and finish without the three-second timeout.
- Exec probe receives `ExitStatus(nonzero) -> Close` -> classify the command as confirmed unavailable; do not turn it into an incomplete-probe error.
- Exec probe receives `EOF -> Close` without exit status -> return an incomplete-probe error; terminal-drop routing may use its existing safe fallback but must log the probe-error reason.
- Dedicated exec-channel `rz` handshake not observed before timeout -> show a failed ZMODEM upload state; do not write recovery commands into the interactive shell.
- Interactive fallback command sent + `ZRINIT` handshake not observed before timeout -> show a failed ZMODEM upload state.
- Partial or CRC-invalid ZHEX initialization header -> replay every non-protocol byte exactly once; do not create a session from the six-byte marker alone.
- Completed upload tail is `\r\n` or `\r\x8a` followed by prompt bytes -> consume only the trailer and release the prompt immediately; a prompt beginning with `OO` remains intact.
- Completed download tail is `\r\nOO` or `\r\x8aOO` followed by prompt bytes -> consume exactly that tail; an `O` mismatch such as `Ox-service#` is released unchanged after the proven trailer.
- Remote cwd not readable through SFTP fallback -> reject terminal drop with user-visible feedback.
- Slow `remote_command_exists`, cwd probe, or SFTP preflight during terminal drop -> the drop callback and the next UI projection tick must still return without waiting for the remote operation.
- Cancel clicked during pending/running ZMODEM -> send abort wire, mark modal `Cancelled`, and allow the shell to continue.
- Dismiss with no projected ZMODEM state -> return success as an idempotent no-op.
- Dismiss during AwaitingUploadSelection, AwaitingDownloadDirectory, or Running -> return an error and retain the projection; use Cancel instead.
- Dismiss Completed, Failed, or Cancelled + runtime cleanup is unavailable/fails/no-ops -> remove the snapshotted projection revision and log cleanup failure without reopening the modal.
- Dismiss terminal revision N + revision N+1 arrives before removal -> preserve N+1 and log the dismissal as stale.
- Dedicated exec Cancel before ZRINIT -> send the abort wire when the channel permits it, publish Cancelled before the handshake timeout, then EOF/Close the exec channel.
- Dedicated exec Cancel while Running -> route to the registered generation, publish Cancelled, write the exact abort wire, and EOF/Close that exec channel without requiring the interactive controller.
- Exec cancel sender is closed -> compare-and-clear only its generation, then fall back to interactive PTY Cancel.
- Completed local file missing -> hide or disable `Open`; keep `Open Folder` only when reveal target still exists.

### 5. Good/Base/Bad Cases

- Good: `sz file` completes, modal shows `Done`, `Open Folder`, and `Open`; `Done` dismisses without sending anything to the terminal.
- Good: dedicated exec upload completes, then Done, title-bar X, or Escape removes the manager projection permanently even though the completed task no longer owns a controller command receiver.
- Good: a large dedicated exec upload is cancelled while Running; the remote sees the exact abort wire and exec EOF/Close, the modal shows Cancelled, and the interactive terminal remains owned by its main pump.
- Good: ordinary `*`, `**`, `***`, `*.log`, `a*b`, quoted stars, escaped stars, and pasted star runs appear immediately and remain byte-for-byte intact when no CRC-valid ZMODEM header follows.
- Good: final ZMODEM response and `root@host:~# ` arrive in one SSH batch; final wire is written, the modal becomes `Completed`, and the prompt is released in the same pump turn.
- Base: drag a regular file over the terminal, overlay appears, drop probes `rz` through an exec channel, ZMODEM upload starts on a dedicated exec channel when `ZRINIT` is observed, and no text is written to the interactive terminal.
- Base with bash prompt tracking: after connection, the negotiated `PROMPT_COMMAND` emits OSC7 cwd markers on each prompt, markers are stripped from visible terminal output, and terminal drops use the tracked cwd without any drag-time probe.
- Base without shell markers: drag a regular file before any OSC7 cwd snapshot exists; if the runtime resolves the interactive shell cwd through a dedicated exec probe, start exec-channel `rz` in that directory.
- Base without cwd resolution: drag a regular file when no cwd snapshot/probe result is available; start the interactive `rz` fallback with exactly ` rz -q\r`, then auto-start the local ZMODEM sender when `ZRINIT` is detected. Do not require exec-channel `rz` detection in this branch.
- Base with cwd probe failure: drag a regular file when the runtime cwd probe errors or times out; log the probe failure and start interactive `rz` fallback rather than surfacing the probe error as the transfer result.
- Base with delayed surface projection: drag a regular file after the terminal session is active but before `TerminalSurfaceState` is available; if cwd cannot be resolved, start interactive `rz` fallback instead of treating the missing surface as unsafe.
- Base with exec probe false negative: drag a regular file when cwd is unavailable and the exec-channel `rz` probe would report false; start interactive `rz` fallback because the active PTY may still resolve `rz` through interactive shell configuration.
- Fallback: drag a regular file when `rz` is unavailable, the exec probe returns false, and SFTP uploads into the active terminal cwd.
- Fallback without shell markers: when the cwd snapshot is missing and `rz` is unavailable, resolve cwd first, cache it, then SFTP uploads into that resolved cwd.
- Base after probe-only cwd cache: if a prior drop probed `/home/user` but no live cwd marker has arrived, a later drop must re-probe and use the new result such as `/srv/app/releases` instead of uploading to the stale login directory.
- Good: an SSH server sends stdout, EOF, exit status 0, and close in that order; cwd and `rz` probes retain the successful status and choose the same route they would choose if status arrived before EOF.
- Base: one SSH session uploads the same local file after remote cwd changes A -> B -> B -> C; with stable `rz` availability, all four drops use dedicated ZMODEM and target A, B, B, and C respectively.
- Bad: breaking the exec-probe loop on `ChannelMsg::Eof`, returning `exit_status == None`, and comparing it with `Some(0)`; this turns valid SSH message timing into random SFTP/Transfer Center fallback.
- Base with application cursor mode: if the terminal is not in alternate screen and is not mouse-grabbed, application cursor-key mode alone must still allow `rz` fallback rather than forcing SFTP or an exec-channel availability veto.
- Base with stale shell markers: if shell integration says input is not active or a command is running, but the terminal is not in alternate screen and is not mouse-grabbed, allow interactive `rz` fallback; shell marker state is advisory and can be stale when environment-based integration is rejected.
- Bad: creating a failed SFTP transfer-center task when no target cwd was determined; this is a preflight failure, not a file-transfer failure.
- Bad: calling `zmodem2::Sender::abort()` or `Receiver::abort()` only and assuming the remote process exits; those APIs only create local events.
- Bad: sending dedicated exec Done/Cancel only as `RuntimeCommand::DismissZmodem` / `CancelZmodem` to the main terminal pump; that pump owns a different controller and cannot close or abort the exec transfer.
- Bad: making a bootstrap fake emit `ZmodemStateChanged(None)` from Dismiss and treating the resulting green test as proof that a completed dedicated exec modal can close.
- Bad: creating a new destination file every time `Event::FileStarted` is observed; remote retries or queued metadata can repeat the event for the same active file.
- Bad: clearing `external_drop_paths` on every `HoveredFileCancelled`; Windows can emit cancel after drop.
- Bad: making terminal drag-hover overlay depend on `rz` auto-injection eligibility; hover should only communicate that an active terminal can accept a drop, while the drop scheduler decides between ZMODEM and SFTP fallback.
- Bad: hand-writing the Windows `IDropTargetVtbl` with `pt: *const POINTL`; the Win32 header uses `POINTL pt` by value, and treating coordinates as a pointer can stop file drops before Slint receives `HoveredFile`.
- Bad: using `send_session_text_input("if command -v rz ...")` for drag upload. This pollutes scrollback/history and leaves confusing prompt output after completion.
- Bad: clearing tentative detector bytes on local input or transport failure; the remote shell can receive the command while its echo silently disappears.
- Bad: deleting output because it starts with `OO`, `rz`, stars, control bytes, or a ZHEX-looking sequence without direction and completed-state proof.

### 6. Tests Required

- Unit test: controller cancel queues the exact ZMODEM abort wire and finishes local state.
- Unit test: receiver waits for destination directory before consuming a queued file offer.
- Unit test: receiver ignores duplicate `FileStarted` for the active file and keeps `files_started` at one.
- Unit test: receiver ignores duplicate `FileStarted` after `FileCompleted` for the same wire filename and size.
- Unit test: sender and receiver do not become `finished` or publish `Completed` immediately on `SessionCompleted`; final wire drain and the following idle poll must happen first.
- Unit test: lrzsz `rz\r` before `ZRQINIT` is stripped from terminal output while preserving earlier prompt text.
- Unit test: every partial marker length, false continuation, local-input interleaving, and transport-close flush reproduces ordinary bytes exactly once.
- Unit test: valid `ZRQINIT` and `ZRINIT` headers are detected only after all 18 CRC-checked ZHEX bytes, across every split; invalid CRC is replayed.
- Unit test: completed upload consumes only the ZFIN trailer; completed download additionally consumes exactly `OO`; prompts beginning with `O`, `OO`, `rz`, stars, or control-like bytes are preserved.
- Unit test: same-chunk completed-session pending wire is extracted before session drop and returned by `drive_zmodem()` without waiting for another SSH batch.
- Unit test: overwrite conflict policy reuses the existing ZMODEM download path rather than producing an auto-renamed sibling.
- Integration test: terminal file drop with `rz` available records a dedicated exec ZMODEM upload call, records `remote_command_exists("rz")`, and records no terminal text input.
- Integration test: negotiated terminal environment includes cwd-only `PROMPT_COMMAND`, excludes `MICA_TERM_ENHANCED`, and still records zero bootstrap attempts for supported shells.
- Integration test: the negotiated `PROMPT_COMMAND` emits an OSC7 cwd marker in a live bash PTY, and `runtime_shell_events` strips that marker from visible output.
- Integration test: terminal file drop with no tracked cwd records one runtime cwd probe, then records a dedicated exec ZMODEM upload call to the probed cwd, and records no terminal text input.
- Integration test: terminal file drop with no tracked cwd and no probe result records one runtime cwd probe, records an interactive ZMODEM upload fallback, records no exec-channel `remote_command_exists("rz")` veto, and records no SFTP upload.
- Integration test: terminal file drop with no tracked cwd and a cwd probe error records one runtime cwd probe, records an interactive ZMODEM upload fallback, records no exec-channel `remote_command_exists("rz")` veto, and records no SFTP upload or `Transfer failed` feedback.
- Integration test: terminal file drop with no tracked cwd, no probe result, and application cursor-key mode set still records an interactive ZMODEM upload fallback without an exec-channel availability veto.
- Integration test: terminal file drop with no tracked cwd, no probe result, and stale shell marker state (`has_markers=true`, `command_running=true` or `input_active=false`) still records an interactive ZMODEM upload fallback without an exec-channel availability veto.
- Integration test: terminal file drop with no projected terminal surface and no tracked/probed cwd records an interactive ZMODEM upload fallback, no terminal text probe snippet, no exec-channel `remote_command_exists("rz")` veto, no SFTP upload, and no `Transfer failed` feedback.
- Unit test: interactive `rz` fallback command is exactly ` rz -q\r`, does not contain `command -v` or `if`, and uses quiet mode.
- Unit test: remote command probe command includes the shared transfer PATH setup before `command -v`.
- Unit test: short-lived exec accumulation covers status-before-EOF, EOF-before-status, data plus EOF-before-status, status plus close without EOF, and close with no status. Assert stdout/status preservation, completion only at the defined terminal condition, and an error for missing status.
- Live runtime test: an in-process russh server sends data, EOF, exit status 0, then close; `SshSessionRuntime::remote_command_exists("rz")` must return `true` rather than `false` or timing out.
- Unit test: explicitly armed ZMODEM prefix interception strips the echoed `" rz -q\r"` fallback command before a CRC-valid `ZRINIT` across every split; identical unarmed manual text remains visible.
- Integration test: terminal file drop with `rz` missing records no terminal text input and falls back to SFTP into `/srv/app` or the active cwd fixture.
- Integration test: terminal file drop with `rz` missing and a delayed `remote_command_exists` probe returns from `invoke_workspace_terminal_external_drop_requested()` quickly, and a subsequent `flush_runtime_projection()` also returns quickly while the background probe is still sleeping.
- Integration test: terminal file drop with no tracked cwd and `rz` missing records one runtime cwd probe, no exec ZMODEM upload, and an SFTP upload into the probed cwd.
- Integration test: terminal file drop after a previous probe-derived cwd cache records a fresh runtime cwd probe when no live cwd tracking has arrived, then uses the refreshed cwd for exec-channel `rz`.
- Integration test: four consecutive single-file drops in one session after remote cwd changes A -> B -> B -> C, with stable `rz` availability, record four cwd probes, four `rz` probes, dedicated ZMODEM targets A/B/B/C, no interactive upload, and no SFTP upload.
- Integration test: terminal file drop with no tracked/probed cwd and an exec-channel `rz` false result still records interactive ZMODEM upload fallback because the probe is not authoritative for the active PTY.
- Unit/runtime test: interactive `rz` fallback publishes a failed ZMODEM upload state when no `ZRINIT` handshake appears before the timeout.
- Integration test: completed download modal exposes `Done`, `Open Folder`, and `Open` when a local completed file exists.
- Unit test: projected revision N cannot remove a newer revision N+1, and repeated removal of N is an idempotent no-op.
- Unit test: exec Cancel reaches the active generation; a closed sender and stale task guard cannot clear a newer generation; overlapping live registrations are rejected.
- Integration test: Done, X, Failed, and Cancelled terminal modal states close and stay closed across repeated projection flushes when the fake runtime Dismiss returns success without emitting `ZmodemStateChanged(None)`.
- Integration/source test: Escape and title-bar close both route to `zmodem-transfer-modal-close-requested`.
- Live runtime test: an in-process russh server emits ZRINIT, observes the exact ZMODEM abort wire plus EOF/Close after Running Cancel, and the runtime emits Cancelled rather than Failed.
- Live runtime test: Cancel before ZRINIT emits Cancelled and settles the exec channel before the four-second handshake timeout.
- Integration/source test: Windows drop handler source must not contain `unimplemented!()` in `QueryInterface` and must expose `IID_IDROPTARGET` / `E_NOINTERFACE` handling.
- Integration/source test: Windows drop handler source must use `pt: POINTL`, must not use `pt: *const POINTL`, and must call `ReleaseStgMedium`.

### 7. Wrong vs Correct

#### Wrong

```rust
while let Some(message) = channel.wait().await {
    match message {
        ChannelMsg::ExitStatus { exit_status } => status = Some(exit_status),
        ChannelMsg::Eof | ChannelMsg::Close => break,
        _ => {}
    }
}
Ok(status == Some(0))
```

EOF does not close an SSH channel. If the server sends `EOF` before
`ExitStatus(0)`, this code loses the successful status and reports that the
remote command is missing.

#### Correct

```rust
match message {
    ChannelMsg::ExitStatus { exit_status } => {
        output.exit_status = Some(exit_status);
        complete = output.saw_eof;
    }
    ChannelMsg::Eof => {
        output.saw_eof = true;
        complete = output.exit_status.is_some();
    }
    ChannelMsg::Close => complete = true,
    _ => {}
}

let status = output
    .exit_status
    .ok_or_else(|| anyhow!("remote command probe closed without an exit status"))?;
Ok(status == 0)
```

Collect EOF and exit status independently, finish once both are known or the
channel actually closes, and distinguish incomplete metadata from a confirmed
non-zero command result.

#### Wrong

```rust
fn cancel(&mut self) {
    self.protocol.abort();
}
```

This only changes the local zmodem state machine; the remote `rz` / `sz` can keep waiting and leave the terminal stuck.

#### Correct

```rust
fn cancel(&mut self) -> Result<()> {
    self.session.take().expect("active transfer").cancel();
    self.pending_control_wire = Some(ZMODEM_ABORT_WIRE.to_vec());
    self.set_modal_state(Some(cancelled_state));
    Ok(())
}
```

The UI settles immediately and the SSH pump writes the abort frame to the remote protocol peer.

#### Wrong

```rust
Event::SessionCompleted => {
    self.finished = true;
}
```

`zmodem2` can still have final wire queued after the completion event, so this leaves remote `sz` / `rz` waiting and leaks frames such as `**B08...` into the terminal.

#### Correct

```rust
Event::SessionCompleted => {
    self.session_complete_pending = true;
}

Action::Idle if self.session_complete_pending => {
    self.complete_session();
    self.finished = true;
}
```

Keep polling until final `WriteWire` actions are written and only publish `Completed` after the protocol reaches idle. Before dropping the finished session, move `pending_wire` into the direction-aware tail parser and return any prompt bytes through the ordinary terminal-output path.

#### Wrong

```rust
let remote_dir = manager.current_working_directory(session_id)
    .ok_or_else(|| anyhow!("the active terminal has not reported its current working directory yet"))?;
```

This makes drag upload depend on shell-integration markers. Real SSH sessions can have a visible prompt and usable `pwd` without emitting OSC7, so the drop appears to do nothing.

#### Correct

```rust
match manager
    .current_working_directory(session_id)
    .or_else(|| manager.resolve_current_working_directory(session_id).ok().flatten())
{
    Some(remote_dir) if manager.remote_command_exists(session_id, "rz")? => {
        manager.start_zmodem_upload_to_remote_dir(session_id, local_paths, remote_dir)?
    }
    Some(remote_dir) => {
        schedule_terminal_cwd_upload_from_paths(manager, session_id, remote_dir, tx, local_paths)?
    }
    None => manager.start_interactive_zmodem_upload(session_id, local_paths)?,
}
```

Resolve the cwd through a non-interactive runtime probe before choosing the
upload path. If cwd resolution still fails, use the dedicated interactive
fallback path that sends only ` rz -q\r`; do not let an exec-channel `rz` probe
veto this branch, and never recover by injecting `pwd`, `command -v`, or shell
snippets into the interactive terminal.

#### Wrong

```rust
let registry = self.registry.lock().expect("lock session registry");
let runtime_control = registry.runtime_controls.get(&session_id).unwrap();
runtime_control.remote_command_exists("rz".into())?;
```

This holds the global session registry while an SSH exec probe can block. A
background drag-upload probe can then freeze the Slint UI if projection code
tries to read session state.

#### Correct

```rust
let runtime_control = {
    let registry = self.registry.lock().expect("lock session registry");
    registry.runtime_controls.get(&session_id).cloned().unwrap()
};
runtime_control
    .lock()
    .expect("lock session runtime control")
    .remote_command_exists("rz".into())?;
```

Keep the global registry lock scoped to handle lookup only. Serialize calls for
the individual session with the per-runtime-control mutex.

#### Wrong

```rust
pub fn dismiss_zmodem_transfer(&self, session_id: Uuid) -> Result<()> {
    self.runtime_control_for_session(session_id)?
        .lock()
        .expect("lock runtime")
        .dismiss_zmodem_transfer()
}
```

This assumes the main terminal pump owns every visible modal. A dedicated exec
task has a different controller and has already exited after completion, so no
`None` event is emitted and periodic projection reopens the modal.

#### Correct

```rust
let (projected, runtime_control) = {
    let registry = self.registry.lock().expect("lock registry");
    let projected = registry
        .zmodem_transfers
        .get(&session_id)
        .cloned()
        .ok_or_else(|| anyhow!("no projected zmodem transfer"))?;
    let runtime_control = registry.runtime_controls.get(&session_id).cloned();
    (projected, runtime_control)
};
if !zmodem_phase_is_dismissible(projected.state.phase) {
    return Err(anyhow!("active zmodem transfers must be cancelled"));
}
if let Some(runtime_control) = runtime_control {
    let _ = runtime_control
        .lock()
        .expect("lock runtime")
        .dismiss_zmodem_transfer(projected.state.clone());
}
self.registry
    .lock()
    .expect("lock registry")
    .remove_zmodem_projection_if_revision(session_id, projected.revision);
```

The manager owns terminal projection removal, releases the registry lock during
runtime cleanup, and removes only the captured revision. The runtime command is
best-effort internal cleanup and must not publish a delayed unconditional
`ZmodemStateChanged(None)`.

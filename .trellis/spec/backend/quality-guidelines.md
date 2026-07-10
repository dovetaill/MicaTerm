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

## Scenario: ZMODEM and External Drop Transfer Boundaries

### 1. Scope / Trigger

- Trigger: terminal transfers cross protocol runtime, SSH channel IO, Slint modal state, OS drag/drop, and SFTP upload scheduling.
- Applies when modifying `src/app/ssh/runtime/zmodem.rs`, `src/app/ssh/runtime/pump.rs`, `src/app/bootstrap/windowing.rs`, `src/app/bootstrap/sftp.rs`, `ui/components/zmodem-transfer-modal.slint`, or `vendor/winit/src/platform_impl/windows/drop_handler.rs`.

### 2. Signatures

- `ZmodemController::cancel() -> Result<()>` must queue wire-level abort bytes and publish a local cancelled modal state.
- `ZmodemTransferState` carries UI progress plus local completion targets: `local_file_path: Option<PathBuf>` and `local_reveal_path: Option<PathBuf>`.
- `window.on_workspace_terminal_external_drop_requested` consumes `workspace-terminal-external-drop-paths`, probes `rz` for ordinary files, then falls back to `schedule_terminal_cwd_upload_from_paths` only when ZMODEM is unavailable or unsuitable.
- `SessionRuntimeControl::resolve_current_working_directory(&self) -> Result<Option<String>>` is the non-interactive runtime escape hatch for terminal drops when no OSC7/shell-integration cwd snapshot exists.
- `SessionManager::resolve_current_working_directory(session_id) -> Result<Option<String>>` delegates to the active runtime and caches a successful cwd in `current_working_directories`.
- `SessionManager::remote_command_exists(session_id, "rz") -> Result<bool>` probes lrzsz through a dedicated SSH exec channel with the same transfer PATH setup used by exec-channel upload; it must not call `send_session_text_input`.
- `SessionManager::start_zmodem_upload_to_remote_dir(session_id, local_paths, remote_dir) -> Result<()>` starts drag-triggered ZMODEM uploads through a dedicated SSH exec channel running `<transfer PATH setup>; cd <quoted remote_dir> && rz -q`.
- `SessionManager::start_interactive_zmodem_upload(session_id, local_paths) -> Result<()>` starts the active-PTY fallback for drag-triggered ZMODEM upload when the active terminal cwd cannot be resolved and the terminal mode allows interaction.
- `SessionRegistry::runtime_controls` stores `Arc<Mutex<Box<dyn SessionRuntimeControl>>>`; callers must clone the shared control while holding the registry lock, release the registry lock, then invoke runtime methods under the per-control mutex.
- `SftpTransferBackgroundMessage` carries UI intent flags `open_queue_drawer: bool` and `open_transfer_center: bool` so background terminal-drop SFTP fallback can ask the main thread to show transfer UI without mutating `ShellViewModel` off-thread.
- `negotiated_terminal_environment()` must include cwd-only bash `PROMPT_COMMAND` tracking that emits OSC7 with `$PWD`; this is not the retired enhanced bootstrap and must not include `MICA_TERM_ENHANCED`.
- `ZmodemController::intercept_remote_bytes()` must strip lrzsz's `rz\r` / `rz\n` download autostart text only when it is immediately followed by a detected `ZRQINIT` prefix.
- `ZmodemController::intercept_remote_bytes()` must strip the history-friendly interactive upload fallback echo (`" rz -q\r"`, legacy `" rz\r"`, and CRLF variants) when immediately followed by a detected `ZRINIT` prefix.
- ZMODEM prefix detection must not hide a single terminal `*` while waiting for a possible `**\x18B00` / `**\x18B01` frame. It may render the first star immediately, then send a local `\x08 \x08` repaint only if a later chunk confirms the protocol prefix.
- `SenderTransfer` and `ReceiverTransfer` treat `SessionCompleted` as "completion event observed", not "all final wire bytes written"; they stay alive until the protocol emits no more `WriteWire`.
- The vendored Windows winit `IDropTarget` must implement COM `QueryInterface` for `IUnknown` and `IDropTarget`; `unimplemented!()` at this boundary can prevent shell drag/drop from reaching Slint/winit.
- The vendored Windows winit `IDropTargetVtbl` must match the Win32 COM ABI: `DragEnter`, `DragOver`, and `Drop` receive `POINTL pt` by value, not `*const POINTL`; `IDataObject` is passed as `IDataObject *`.

### 3. Contracts

- ZMODEM cancel is a wire contract: local state changes are not enough; remote `rz` / `sz` must receive an abort frame.
- ZMODEM receiver `FileStarted` is not a one-shot UI event; the runtime must treat repeated metadata for the currently active file as idempotent and must not create/truncate the local target twice.
- ZMODEM receiver `FileStarted` can also repeat after `FileCompleted` and before `SessionCompleted`; repeated metadata for the last completed file must be ignored so the UI does not show "file 1 of at least 2" for a one-file transfer.
- ZMODEM session completion is a final-wire contract: `zmodem2::poll()` returns pending events before pending final wire (`ZFIN` / `OO`), so the runtime must not remove the session when it first observes `Event::SessionCompleted`.
- ZMODEM post-session cleanup must absorb only known protocol tail bytes (`OO`, ZHEX frames ending in XON, CAN/backspace/XON noise, and lrzsz `rz` autostart residue). The next local input must clear this drain state so a new user-initiated `sz` / `rz` is not swallowed.
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
- Dedicated exec-channel `rz` handshake not observed before timeout -> show a failed ZMODEM upload state; do not write recovery commands into the interactive shell.
- Interactive fallback command sent + `ZRINIT` handshake not observed before timeout -> show a failed ZMODEM upload state.
- Remote cwd not readable through SFTP fallback -> reject terminal drop with user-visible feedback.
- Slow `remote_command_exists`, cwd probe, or SFTP preflight during terminal drop -> the drop callback and the next UI projection tick must still return without waiting for the remote operation.
- Cancel clicked during pending/running ZMODEM -> send abort wire, mark modal `Cancelled`, and allow the shell to continue.
- Completed local file missing -> hide or disable `Open`; keep `Open Folder` only when reveal target still exists.

### 5. Good/Base/Bad Cases

- Good: `sz file` completes, modal shows `Done`, `Open Folder`, and `Open`; `Done` dismisses without sending anything to the terminal.
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
- Base with application cursor mode: if the terminal is not in alternate screen and is not mouse-grabbed, application cursor-key mode alone must still allow `rz` fallback rather than forcing SFTP or an exec-channel availability veto.
- Base with stale shell markers: if shell integration says input is not active or a command is running, but the terminal is not in alternate screen and is not mouse-grabbed, allow interactive `rz` fallback; shell marker state is advisory and can be stale when environment-based integration is rejected.
- Bad: creating a failed SFTP transfer-center task when no target cwd was determined; this is a preflight failure, not a file-transfer failure.
- Bad: calling `zmodem2::Sender::abort()` or `Receiver::abort()` only and assuming the remote process exits; those APIs only create local events.
- Bad: creating a new destination file every time `Event::FileStarted` is observed; remote retries or queued metadata can repeat the event for the same active file.
- Bad: clearing `external_drop_paths` on every `HoveredFileCancelled`; Windows can emit cancel after drop.
- Bad: making terminal drag-hover overlay depend on `rz` auto-injection eligibility; hover should only communicate that an active terminal can accept a drop, while the drop scheduler decides between ZMODEM and SFTP fallback.
- Bad: hand-writing the Windows `IDropTargetVtbl` with `pt: *const POINTL`; the Win32 header uses `POINTL pt` by value, and treating coordinates as a pointer can stop file drops before Slint receives `HoveredFile`.
- Bad: using `send_session_text_input("if command -v rz ...")` for drag upload. This pollutes scrollback/history and leaves confusing prompt output after completion.

### 6. Tests Required

- Unit test: controller cancel queues the exact ZMODEM abort wire and finishes local state.
- Unit test: receiver waits for destination directory before consuming a queued file offer.
- Unit test: receiver ignores duplicate `FileStarted` for the active file and keeps `files_started` at one.
- Unit test: receiver ignores duplicate `FileStarted` after `FileCompleted` for the same wire filename and size.
- Unit test: sender and receiver do not become `finished` immediately on `SessionCompleted`; final wire drain must happen first.
- Unit test: lrzsz `rz\r` before `ZRQINIT` is stripped from terminal output while preserving earlier prompt text.
- Unit test: post-session drain strips `OO` / ZHEX tail bytes and then releases plain prompt text.
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
- Unit test: ZMODEM prefix interception strips the echoed `" rz -q\r"` fallback command before `ZRINIT` and renders then rewinds a split ordinary `*` only when the following bytes confirm a ZMODEM prefix.
- Integration test: terminal file drop with `rz` missing records no terminal text input and falls back to SFTP into `/srv/app` or the active cwd fixture.
- Integration test: terminal file drop with `rz` missing and a delayed `remote_command_exists` probe returns from `invoke_workspace_terminal_external_drop_requested()` quickly, and a subsequent `flush_runtime_projection()` also returns quickly while the background probe is still sleeping.
- Integration test: terminal file drop with no tracked cwd and `rz` missing records one runtime cwd probe, no exec ZMODEM upload, and an SFTP upload into the probed cwd.
- Integration test: terminal file drop after a previous probe-derived cwd cache records a fresh runtime cwd probe when no live cwd tracking has arrived, then uses the refreshed cwd for exec-channel `rz`.
- Integration test: terminal file drop with no tracked/probed cwd and an exec-channel `rz` false result still records interactive ZMODEM upload fallback because the probe is not authoritative for the active PTY.
- Unit/runtime test: interactive `rz` fallback publishes a failed ZMODEM upload state when no `ZRINIT` handshake appears before the timeout.
- Integration test: completed download modal exposes `Done`, `Open Folder`, and `Open` when a local completed file exists.
- Integration/source test: Windows drop handler source must not contain `unimplemented!()` in `QueryInterface` and must expose `IID_IDROPTARGET` / `E_NOINTERFACE` handling.
- Integration/source test: Windows drop handler source must use `pt: POINTL`, must not use `pt: *const POINTL`, and must call `ReleaseStgMedium`.

### 7. Wrong vs Correct

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
    self.finished = true;
}
```

Keep polling until final `WriteWire` actions are written and only finish after the protocol reaches idle.

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

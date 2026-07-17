//! SSH runtime channel pump and output coalescing helpers.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};
use russh::client;
use russh::{Channel, ChannelMsg, Disconnect};
use tokio::sync::mpsc;
use tokio::time::{Sleep, sleep, timeout};
use uuid::Uuid;

use crate::app::ssh::shell_integration::runtime_shell_events;

use super::auth::RuntimeClientHandler;
use super::terminal::{TerminalSession, apply_remote_output, snapshot_terminal_surface};
use super::transport::TransportChainGuard;
use super::zmodem::{ZMODEM_ABORT_WIRE, ZmodemAdvanceOutcome, ZmodemController};
use super::{
    ExecZmodemCommand, ExecZmodemTransferContext, FAST_SURFACE_DIRTY_NOTIFICATION_INTERVAL,
    INPUT_ACTIVE_SURFACE_DIRTY_WINDOW, RuntimeCommand, SURFACE_DIRTY_NOTIFICATION_INTERVAL,
    SessionRuntimeEvent, WORKING_SET_TRIM_IDLE_INTERVAL, WORKING_SET_TRIM_MIN_OUTPUT_BYTES,
};

const ZMODEM_EXEC_UPLOAD_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(4);
const INTERACTIVE_ZMODEM_UPLOAD_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(4);
const REMOTE_COMMAND_PROBE_TIMEOUT: Duration = Duration::from_secs(3);
const REMOTE_CWD_PROBE_TIMEOUT: Duration = Duration::from_secs(3);
const INTERACTIVE_RZ_UPLOAD_COMMAND: &[u8] = b" rz -q\r";
const REMOTE_INTERACTIVE_SHELL_CWD_PROBE: &str = r#"if [ -d /proc ]; then
__mica_term_probe_emit_cwd() {
    __mica_term_probe_pid="$1"
    __mica_term_probe_dir="/proc/$__mica_term_probe_pid"
    [ -r "$__mica_term_probe_dir/cwd" ] || return 1
    __mica_term_probe_comm="$(cat "$__mica_term_probe_dir/comm" 2>/dev/null || true)"
    case "$__mica_term_probe_comm" in
        sh|bash|zsh|fish|dash|ksh|mksh|csh|tcsh) ;;
        *) return 1 ;;
    esac
    __mica_term_probe_tty="$(ps -o tty= -p "$__mica_term_probe_pid" 2>/dev/null | tr -d '[:space:]')"
    [ -n "$__mica_term_probe_tty" ] && [ "$__mica_term_probe_tty" != "?" ] || return 1
    __mica_term_probe_cwd="$(readlink "$__mica_term_probe_dir/cwd" 2>/dev/null || true)"
    case "$__mica_term_probe_cwd" in
        /*) printf '%s\n' "$__mica_term_probe_cwd"; exit 0 ;;
    esac
    return 1
}
if [ -n "${SSH_CONNECTION:-}" ]; then
    for __mica_term_probe_env in /proc/[0-9]*/environ; do
        __mica_term_probe_pid="${__mica_term_probe_env%/environ}"
        __mica_term_probe_pid="${__mica_term_probe_pid##*/}"
        [ "$__mica_term_probe_pid" = "$$" ] && continue
        [ -r "$__mica_term_probe_env" ] || continue
        if tr '\000' '\n' < "$__mica_term_probe_env" 2>/dev/null | grep -Fx "SSH_CONNECTION=$SSH_CONNECTION" >/dev/null; then
            __mica_term_probe_emit_cwd "$__mica_term_probe_pid"
        fi
    done
fi
__mica_term_probe_parent="$(ps -o ppid= -p "$$" 2>/dev/null | tr -d '[:space:]')"
if [ -n "$__mica_term_probe_parent" ]; then
    for __mica_term_probe_pid in $(ps -o pid= --ppid "$__mica_term_probe_parent" 2>/dev/null); do
        [ "$__mica_term_probe_pid" = "$$" ] && continue
        __mica_term_probe_emit_cwd "$__mica_term_probe_pid"
    done
fi
fi
exit 1"#;

#[derive(Debug)]
struct PendingInteractiveZmodemUpload {
    local_paths: Vec<std::path::PathBuf>,
    started_at: Instant,
}

impl PendingInteractiveZmodemUpload {
    fn new(local_paths: Vec<std::path::PathBuf>) -> Self {
        Self {
            local_paths,
            started_at: Instant::now(),
        }
    }

    fn remaining_timeout(&self, now: Instant) -> Duration {
        INTERACTIVE_ZMODEM_UPLOAD_HANDSHAKE_TIMEOUT
            .checked_sub(now.saturating_duration_since(self.started_at))
            .unwrap_or(Duration::ZERO)
    }

    fn timed_out(&self, now: Instant) -> bool {
        self.remaining_timeout(now).is_zero()
    }
}

pub(super) async fn run_channel_pump(
    session_id: Uuid,
    handle: Arc<client::Handle<RuntimeClientHandler>>,
    mut channel: Channel<client::Msg>,
    terminal: Arc<Mutex<TerminalSession>>,
    event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    mut command_rx: mpsc::UnboundedReceiver<RuntimeCommand>,
    _transport_chain_guard: TransportChainGuard,
) {
    let mut command_channel_open = true;
    let mut pending_channel_messages = VecDeque::new();
    let mut dirty_notifier = SurfaceDirtyNotifier::default();
    let mut dirty_timer: Option<std::pin::Pin<Box<Sleep>>> = None;
    let mut dirty_timer_interval: Option<std::time::Duration> = None;
    let mut working_set_trim_scheduler = WorkingSetTrimScheduler::default();
    let mut working_set_trim_timer: Option<std::pin::Pin<Box<Sleep>>> = None;
    let mut shell_integration = super::TerminalShellIntegrationState::default();
    let mut synchronized_output = SynchronizedOutputBatcher::default();
    let mut zmodem = ZmodemController::default();
    let mut pending_interactive_zmodem_upload: Option<PendingInteractiveZmodemUpload> = None;

    'pump: loop {
        let pending_interactive_zmodem_upload_timeout = pending_interactive_zmodem_upload
            .as_ref()
            .map(|pending| pending.remaining_timeout(Instant::now()));
        tokio::select! {
            maybe_command = command_rx.recv(), if command_channel_open => {
                match maybe_command {
                    Some(RuntimeCommand::TextInput(text)) => {
                        pending_interactive_zmodem_upload = None;
                        zmodem.note_local_input();
                        dirty_notifier.note_local_input(Instant::now());
                        let bytes = text.into_bytes();
                        if let Err(bytes) = handle.data(channel.id(), bytes).await {
                            let _ = event_tx.send(SessionRuntimeEvent::Error(format!(
                                "failed to write {} bytes to SSH channel",
                                bytes.len()
                            )));
                            break;
                        }
                    }
                    Some(RuntimeCommand::KeyInput(event)) => {
                        pending_interactive_zmodem_upload = None;
                        zmodem.note_local_input();
                        dirty_notifier.note_local_input(Instant::now());
                        let bytes = match terminal.lock() {
                            Ok(mut terminal) => match terminal.send_key_event(event) {
                                Ok(bytes) => bytes,
                                Err(err) => {
                                    let _ = event_tx.send(SessionRuntimeEvent::Error(format!(
                                        "failed to encode key input for SSH channel: {err}"
                                    )));
                                    break;
                                }
                            },
                            Err(_) => {
                                let _ = event_tx.send(SessionRuntimeEvent::Error(
                                    "failed to lock terminal for key input".into()
                                ));
                                break;
                            }
                        };
                        if bytes.is_empty() {
                            continue;
                        }
                        if let Err(bytes) = handle.data(channel.id(), bytes).await {
                            let _ = event_tx.send(SessionRuntimeEvent::Error(format!(
                                "failed to write {} key bytes to SSH channel",
                                bytes.len()
                            )));
                            break;
                        }
                    }
                    Some(RuntimeCommand::MouseInput(event)) => {
                        let bytes = match terminal.lock() {
                            Ok(mut terminal) => match terminal.send_mouse_input(event) {
                                Ok(bytes) => bytes,
                                Err(err) => {
                                    let _ = event_tx.send(SessionRuntimeEvent::Error(format!(
                                        "failed to encode mouse input for SSH channel: {err}"
                                    )));
                                    break;
                                }
                            },
                            Err(_) => {
                                let _ = event_tx.send(SessionRuntimeEvent::Error(
                                    "failed to lock terminal for mouse input".into()
                                ));
                                break;
                            }
                        };
                        if bytes.is_empty() {
                            continue;
                        }
                        if let Err(bytes) = handle.data(channel.id(), bytes).await {
                            let _ = event_tx.send(SessionRuntimeEvent::Error(format!(
                                "failed to write {} mouse bytes to SSH channel",
                                bytes.len()
                            )));
                            break;
                        }
                    }
                    Some(RuntimeCommand::Paste(text)) => {
                        pending_interactive_zmodem_upload = None;
                        zmodem.note_local_input();
                        dirty_notifier.note_local_input(Instant::now());
                        let bytes = match terminal.lock() {
                            Ok(mut terminal) => match terminal.encode_paste(&text) {
                                Ok(bytes) => bytes,
                                Err(err) => {
                                    let _ = event_tx.send(SessionRuntimeEvent::Error(format!(
                                        "failed to encode paste for SSH channel: {err}"
                                    )));
                                    break;
                                }
                            },
                            Err(_) => {
                                let _ = event_tx.send(SessionRuntimeEvent::Error(
                                    "failed to lock terminal for paste".into()
                                ));
                                break;
                            }
                        };
                        if bytes.is_empty() {
                            continue;
                        }
                        if let Err(bytes) = handle.data(channel.id(), bytes).await {
                            let _ = event_tx.send(SessionRuntimeEvent::Error(format!(
                                "failed to write {} paste bytes to SSH channel",
                                bytes.len()
                            )));
                            break;
                        }
                    }
                    Some(RuntimeCommand::StartInteractiveZmodemUpload { local_paths }) => {
                        if local_paths.is_empty() {
                            let _ = event_tx.send(SessionRuntimeEvent::Error(
                                "zmodem upload requires at least one local file".into()
                            ));
                            continue;
                        }
                        tracing::info!(
                            target: "app.zmodem",
                            session_id = %session_id,
                            path_count = local_paths.len(),
                            "starting zmodem upload through interactive rz fallback"
                        );
                        pending_interactive_zmodem_upload =
                            Some(PendingInteractiveZmodemUpload::new(local_paths));
                        zmodem.note_local_input();
                        zmodem.expect_automatic_rz_echo();
                        dirty_notifier.note_local_input(Instant::now());
                        if let Err(bytes) = handle
                            .data(channel.id(), INTERACTIVE_RZ_UPLOAD_COMMAND.to_vec())
                            .await
                        {
                            let _ = event_tx.send(SessionRuntimeEvent::Error(format!(
                                "failed to write {} interactive rz bytes to SSH channel",
                                bytes.len()
                            )));
                            break;
                        }
                    }
                    Some(RuntimeCommand::StartZmodemUpload { local_paths }) => {
                        if let Err(err) = zmodem.start_upload(local_paths) {
                            zmodem.surface_error(err.to_string());
                        }
                        emit_zmodem_state_changes(&mut zmodem, &event_tx);
                        let Some(terminal_bytes) = drive_zmodem(
                            &handle,
                            channel.id(),
                            &event_tx,
                            &mut zmodem,
                        ).await else {
                            break 'pump;
                        };
                        if !terminal_bytes.is_empty() {
                            process_ready_remote_output(
                                terminal_bytes.as_slice(),
                                &terminal,
                                &event_tx,
                                &mut shell_integration,
                                &mut dirty_notifier,
                                &mut dirty_timer,
                                &mut dirty_timer_interval,
                                &mut working_set_trim_scheduler,
                                &mut working_set_trim_timer,
                            );
                        }
                    }
                    Some(RuntimeCommand::StartZmodemDownload {
                        local_dir,
                        conflict_policy,
                    }) => {
                        if let Err(err) = zmodem.start_download(local_dir, conflict_policy) {
                            zmodem.surface_error(err.to_string());
                        }
                        emit_zmodem_state_changes(&mut zmodem, &event_tx);
                        let Some(terminal_bytes) = drive_zmodem(
                            &handle,
                            channel.id(),
                            &event_tx,
                            &mut zmodem,
                        ).await else {
                            break 'pump;
                        };
                        if !terminal_bytes.is_empty() {
                            process_ready_remote_output(
                                terminal_bytes.as_slice(),
                                &terminal,
                                &event_tx,
                                &mut shell_integration,
                                &mut dirty_notifier,
                                &mut dirty_timer,
                                &mut dirty_timer_interval,
                                &mut working_set_trim_scheduler,
                                &mut working_set_trim_timer,
                            );
                        }
                    }
                    Some(RuntimeCommand::CancelZmodem) => {
                        let _ = zmodem.cancel();
                        emit_zmodem_state_changes(&mut zmodem, &event_tx);
                        let Some(terminal_bytes) = drive_zmodem(
                            &handle,
                            channel.id(),
                            &event_tx,
                            &mut zmodem,
                        ).await else {
                            break 'pump;
                        };
                        if !terminal_bytes.is_empty() {
                            process_ready_remote_output(
                                terminal_bytes.as_slice(),
                                &terminal,
                                &event_tx,
                                &mut shell_integration,
                                &mut dirty_notifier,
                                &mut dirty_timer,
                                &mut dirty_timer_interval,
                                &mut working_set_trim_scheduler,
                                &mut working_set_trim_timer,
                            );
                        }
                    }
                    Some(RuntimeCommand::DismissZmodem { expected_state }) => {
                        let dismissed = zmodem.dismiss_if_matches(expected_state.as_ref());
                        if dismissed {
                            let _ = zmodem.take_modal_state_change();
                        }
                        tracing::debug!(
                            target: "app.zmodem",
                            session_id = %session_id,
                            lifecycle_command = "dismiss",
                            owner = "interactive",
                            outcome = if dismissed { "cleared" } else { "ignored" },
                            "processed interactive zmodem controller cleanup"
                        );
                    }
                    Some(RuntimeCommand::Resize {
                        rows,
                        cols,
                        pixel_width,
                        pixel_height,
                    }) => {
                        if let Ok(mut terminal) = terminal.lock() {
                            terminal.resize(rows as usize, cols as usize);
                        }
                        if let Some(surface) = snapshot_terminal_surface(&terminal, session_id) {
                            tracing::trace!(
                                target: "app.terminal",
                                requested_rows = rows,
                                requested_cols = cols,
                                surface_rows = surface.rows,
                                surface_cols = surface.cols,
                                surface_seqno = surface.seqno,
                                "ssh runtime applied terminal resize before publishing surface"
                            );
                            let _ = event_tx.send(SessionRuntimeEvent::SurfaceChanged(surface));
                        }
                        if let Err(err) = channel
                            .window_change(cols, rows, pixel_width, pixel_height)
                            .await
                        {
                            let _ = event_tx.send(SessionRuntimeEvent::Error(format!(
                                "failed to resize SSH PTY: {err}"
                            )));
                            break;
                        }
                    }
                    Some(RuntimeCommand::Disconnect) => {
                        if let Some(ready_bytes) = synchronized_output.finish() {
                            let mut terminal_bytes = zmodem.intercept_remote_bytes(ready_bytes.as_slice());
                            emit_zmodem_state_changes(&mut zmodem, &event_tx);
                            start_pending_interactive_zmodem_upload(
                                &mut zmodem,
                                &mut pending_interactive_zmodem_upload,
                                &event_tx,
                            );
                            let Some(released_terminal_bytes) =
                                drive_zmodem(&handle, channel.id(), &event_tx, &mut zmodem).await
                            else {
                                break 'pump;
                            };
                            terminal_bytes.extend(released_terminal_bytes);
                            if !terminal_bytes.is_empty() {
                                process_ready_remote_output(
                                    terminal_bytes.as_slice(),
                                    &terminal,
                                    &event_tx,
                                    &mut shell_integration,
                                    &mut dirty_notifier,
                                    &mut dirty_timer,
                                    &mut dirty_timer_interval,
                                    &mut working_set_trim_scheduler,
                                    &mut working_set_trim_timer,
                                );
                            }
                        }
                        zmodem.mark_transport_closed();
                        let trailing_terminal_bytes = zmodem.flush_terminal_bytes();
                        if !trailing_terminal_bytes.is_empty() {
                            process_ready_remote_output(
                                trailing_terminal_bytes.as_slice(),
                                &terminal,
                                &event_tx,
                                &mut shell_integration,
                                &mut dirty_notifier,
                                &mut dirty_timer,
                                &mut dirty_timer_interval,
                                &mut working_set_trim_scheduler,
                                &mut working_set_trim_timer,
                            );
                        }
                        emit_zmodem_state_changes(&mut zmodem, &event_tx);
                        if dirty_notifier.take_pending() {
                            let _ = event_tx.send(SessionRuntimeEvent::SurfaceDirty);
                        }
                        let _ = channel.eof().await;
                        let _ = channel.close().await;
                        let _ = handle
                            .disconnect(Disconnect::ByApplication, "session closed", "en-US")
                            .await;
                        let _ = event_tx.send(SessionRuntimeEvent::Disconnected);
                        break;
                    }
                    None => {
                        command_channel_open = false;
                    }
                }
            }
            maybe_message = async {
                if let Some(message) = pending_channel_messages.pop_front() {
                    Some(message)
                } else {
                    channel.wait().await
                }
            } => {
                match maybe_message {
                    Some(message @ ChannelMsg::Data { .. }) | Some(message @ ChannelMsg::ExtendedData { .. }) => {
                        let output_batch =
                            collect_ready_output_batch(message, &mut channel, &mut pending_channel_messages)
                                .await;
                        for ready_bytes in synchronized_output.push_bytes(output_batch.raw_bytes.as_slice()) {
                            let mut terminal_bytes = zmodem.intercept_remote_bytes(ready_bytes.as_slice());
                            emit_zmodem_state_changes(&mut zmodem, &event_tx);
                            start_pending_interactive_zmodem_upload(
                                &mut zmodem,
                                &mut pending_interactive_zmodem_upload,
                                &event_tx,
                            );
                            let Some(released_terminal_bytes) =
                                drive_zmodem(&handle, channel.id(), &event_tx, &mut zmodem).await
                            else {
                                break 'pump;
                            };
                            terminal_bytes.extend(released_terminal_bytes);
                            if !terminal_bytes.is_empty() {
                                process_ready_remote_output(
                                    terminal_bytes.as_slice(),
                                    &terminal,
                                    &event_tx,
                                    &mut shell_integration,
                                    &mut dirty_notifier,
                                    &mut dirty_timer,
                                    &mut dirty_timer_interval,
                                    &mut working_set_trim_scheduler,
                                    &mut working_set_trim_timer,
                                );
                            }
                        }
                    }
                    Some(ChannelMsg::Close) | Some(ChannelMsg::Eof) | None => {
                        if let Some(ready_bytes) = synchronized_output.finish() {
                            let mut terminal_bytes = zmodem.intercept_remote_bytes(ready_bytes.as_slice());
                            emit_zmodem_state_changes(&mut zmodem, &event_tx);
                            start_pending_interactive_zmodem_upload(
                                &mut zmodem,
                                &mut pending_interactive_zmodem_upload,
                                &event_tx,
                            );
                            let Some(released_terminal_bytes) =
                                drive_zmodem(&handle, channel.id(), &event_tx, &mut zmodem).await
                            else {
                                break 'pump;
                            };
                            terminal_bytes.extend(released_terminal_bytes);
                            if !terminal_bytes.is_empty() {
                                process_ready_remote_output(
                                    terminal_bytes.as_slice(),
                                    &terminal,
                                    &event_tx,
                                    &mut shell_integration,
                                    &mut dirty_notifier,
                                    &mut dirty_timer,
                                    &mut dirty_timer_interval,
                                    &mut working_set_trim_scheduler,
                                    &mut working_set_trim_timer,
                                );
                            }
                        }
                        zmodem.mark_transport_closed();
                        let trailing_terminal_bytes = zmodem.flush_terminal_bytes();
                        if !trailing_terminal_bytes.is_empty() {
                            process_ready_remote_output(
                                trailing_terminal_bytes.as_slice(),
                                &terminal,
                                &event_tx,
                                &mut shell_integration,
                                &mut dirty_notifier,
                                &mut dirty_timer,
                                &mut dirty_timer_interval,
                                &mut working_set_trim_scheduler,
                                &mut working_set_trim_timer,
                            );
                        }
                        emit_zmodem_state_changes(&mut zmodem, &event_tx);
                        if dirty_notifier.take_pending() {
                            let _ = event_tx.send(SessionRuntimeEvent::SurfaceDirty);
                        }
                        let _ = event_tx.send(SessionRuntimeEvent::Disconnected);
                        break;
                    }
                    Some(ChannelMsg::Failure) => {
                        if let Some(ready_bytes) = synchronized_output.finish() {
                            let mut terminal_bytes = zmodem.intercept_remote_bytes(ready_bytes.as_slice());
                            emit_zmodem_state_changes(&mut zmodem, &event_tx);
                            start_pending_interactive_zmodem_upload(
                                &mut zmodem,
                                &mut pending_interactive_zmodem_upload,
                                &event_tx,
                            );
                            let Some(released_terminal_bytes) =
                                drive_zmodem(&handle, channel.id(), &event_tx, &mut zmodem).await
                            else {
                                break 'pump;
                            };
                            terminal_bytes.extend(released_terminal_bytes);
                            if !terminal_bytes.is_empty() {
                                process_ready_remote_output(
                                    terminal_bytes.as_slice(),
                                    &terminal,
                                    &event_tx,
                                    &mut shell_integration,
                                    &mut dirty_notifier,
                                    &mut dirty_timer,
                                    &mut dirty_timer_interval,
                                    &mut working_set_trim_scheduler,
                                    &mut working_set_trim_timer,
                                );
                            }
                        }
                        zmodem.mark_transport_closed();
                        let trailing_terminal_bytes = zmodem.flush_terminal_bytes();
                        if !trailing_terminal_bytes.is_empty() {
                            process_ready_remote_output(
                                trailing_terminal_bytes.as_slice(),
                                &terminal,
                                &event_tx,
                                &mut shell_integration,
                                &mut dirty_notifier,
                                &mut dirty_timer,
                                &mut dirty_timer_interval,
                                &mut working_set_trim_scheduler,
                                &mut working_set_trim_timer,
                            );
                        }
                        emit_zmodem_state_changes(&mut zmodem, &event_tx);
                        if dirty_notifier.take_pending() {
                            let _ = event_tx.send(SessionRuntimeEvent::SurfaceDirty);
                        }
                        let _ = event_tx.send(SessionRuntimeEvent::Error(
                            "remote SSH channel reported failure".into()
                        ));
                        break;
                    }
                    Some(_) => {}
                }
            }
            () = async { if let Some(timer) = dirty_timer.as_mut() { timer.await } }, if dirty_timer.is_some() => {
                dirty_timer = None;
                dirty_timer_interval = None;
                if dirty_notifier.flush_due() {
                    let _ = event_tx.send(SessionRuntimeEvent::SurfaceDirty);
                }
            }
            () = sleep(pending_interactive_zmodem_upload_timeout.unwrap_or(Duration::ZERO)),
                if pending_interactive_zmodem_upload_timeout.is_some() => {
                if pending_interactive_zmodem_upload
                    .as_ref()
                    .is_some_and(|pending| pending.timed_out(Instant::now()))
                {
                    let pending = pending_interactive_zmodem_upload
                        .take()
                        .expect("pending interactive zmodem upload timed out");
                    tracing::warn!(
                        target: "app.zmodem",
                        session_id = %session_id,
                        path_count = pending.local_paths.len(),
                        "interactive rz fallback did not emit a ZMODEM upload handshake"
                    );
                    let _ = event_tx.send(SessionRuntimeEvent::ZmodemStateChanged(Some(
                        failed_zmodem_upload_state(
                            "remote rz did not emit a ZMODEM upload handshake after the interactive fallback command".into(),
                        ),
                    )));
                    zmodem.cancel_automatic_rz_echo_expectation();
                    let terminal_bytes = zmodem.flush_terminal_bytes();
                    if !terminal_bytes.is_empty() {
                        process_ready_remote_output(
                            terminal_bytes.as_slice(),
                            &terminal,
                            &event_tx,
                            &mut shell_integration,
                            &mut dirty_notifier,
                            &mut dirty_timer,
                            &mut dirty_timer_interval,
                            &mut working_set_trim_scheduler,
                            &mut working_set_trim_timer,
                        );
                    }
                }
            }
            () = async { if let Some(timer) = working_set_trim_timer.as_mut() { timer.await } }, if working_set_trim_timer.is_some() => {
                working_set_trim_timer = None;
                if let Some(pending_output_bytes) = working_set_trim_scheduler.take_trim_request_bytes() {
                    let profile = crate::app::runtime_profile::AppRuntimeProfile::packaged();
                    let idle_interval_ms = WORKING_SET_TRIM_IDLE_INTERVAL.as_millis() as u64;
                    let before_memory = crate::app::memory::current_process_memory_snapshot();
                    crate::app::logging::runtime::emit_memory_diagnostics_event(
                        profile,
                        crate::app::logging::runtime::MemoryDiagnosticsEvent {
                            event_name: "trim-request",
                            trigger_reason: Some("large-output-idle"),
                            active_renderer_mode: Some(profile.terminal_render_mode_label()),
                            pending_output_bytes: Some(pending_output_bytes),
                            idle_interval_ms: Some(idle_interval_ms),
                            before_memory,
                            ..crate::app::logging::runtime::MemoryDiagnosticsEvent::default()
                        },
                    );
                    let trim_succeeded = crate::app::memory::trim_process_working_set();
                    let after_memory = crate::app::memory::current_process_memory_snapshot();
                    crate::app::logging::runtime::emit_memory_diagnostics_event(
                        profile,
                        crate::app::logging::runtime::MemoryDiagnosticsEvent {
                            event_name: "trim-executed",
                            trigger_reason: Some("large-output-idle"),
                            active_renderer_mode: Some(profile.terminal_render_mode_label()),
                            pending_output_bytes: Some(pending_output_bytes),
                            idle_interval_ms: Some(idle_interval_ms),
                            trim_succeeded: Some(trim_succeeded),
                            before_memory,
                            after_memory,
                            ..crate::app::logging::runtime::MemoryDiagnosticsEvent::default()
                        },
                    );
                }
            }
        }
    }
}

fn start_pending_interactive_zmodem_upload(
    zmodem: &mut ZmodemController,
    pending_local_paths: &mut Option<PendingInteractiveZmodemUpload>,
    event_tx: &mpsc::UnboundedSender<SessionRuntimeEvent>,
) {
    let Some(state) = zmodem.current_state() else {
        return;
    };
    if state.direction != super::ZmodemTransferDirection::Upload
        || state.phase != super::ZmodemTransferPhase::AwaitingUploadSelection
    {
        return;
    }

    let Some(pending) = pending_local_paths.take() else {
        return;
    };
    if let Err(err) = zmodem.start_upload(pending.local_paths) {
        zmodem.surface_error(err.to_string());
    }
    emit_zmodem_state_changes(zmodem, event_tx);
}

pub(super) async fn remote_command_exists(
    handle: Arc<client::Handle<RuntimeClientHandler>>,
    command_name: String,
) -> Result<bool> {
    if !is_safe_remote_command_name(command_name.as_str()) {
        bail!("remote command name is not safe to probe: `{command_name}`");
    }

    let command = remote_command_probe_command(command_name.as_str());
    let status = timeout(
        REMOTE_COMMAND_PROBE_TIMEOUT,
        remote_exec_exit_status(handle, command),
    )
    .await
    .context("remote command probe timed out")??;
    tracing::debug!(
        target: "app.ssh",
        command_name = command_name.as_str(),
        exit_status = ?status,
        "remote command probe completed"
    );
    let status = require_remote_exec_exit_status(status, "remote command probe")?;
    Ok(status == 0)
}

pub(super) async fn resolve_remote_current_working_directory(
    handle: Arc<client::Handle<RuntimeClientHandler>>,
) -> Result<Option<String>> {
    let output = timeout(
        REMOTE_CWD_PROBE_TIMEOUT,
        remote_exec_output(
            handle,
            REMOTE_INTERACTIVE_SHELL_CWD_PROBE.to_string(),
            "remote cwd probe",
        ),
    )
    .await
    .context("remote cwd probe timed out")??;

    tracing::debug!(
        target: "app.ssh",
        exit_status = ?output.exit_status,
        stdout_bytes = output.stdout.len(),
        "remote cwd probe completed"
    );
    let status = require_remote_exec_exit_status(output.exit_status, "remote cwd probe")?;
    if status != 0 {
        return Ok(None);
    }

    let stdout = String::from_utf8_lossy(output.stdout.as_slice());
    let Some(cwd) = stdout
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with('/') && !line.is_empty())
    else {
        return Ok(None);
    };
    Ok(Some(cwd.to_string()))
}

pub(super) async fn run_zmodem_exec_upload(
    session_id: Uuid,
    handle: Arc<client::Handle<RuntimeClientHandler>>,
    event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    local_paths: Vec<std::path::PathBuf>,
    remote_dir: String,
    exec_transfer: ExecZmodemTransferContext,
) {
    let ExecZmodemTransferContext {
        generation,
        command_rx: exec_command_rx,
        registration,
    } = exec_transfer;
    let path_count = local_paths.len();
    tracing::info!(
        target: "app.zmodem",
        session_id = %session_id,
        transfer_generation = generation,
        remote_dir = remote_dir.as_str(),
        path_count,
        "starting zmodem upload over dedicated SSH exec channel"
    );
    let outcome = run_zmodem_exec_upload_inner(
        session_id,
        Arc::clone(&handle),
        event_tx.clone(),
        local_paths,
        remote_dir.clone(),
        generation,
        exec_command_rx,
    )
    .await;
    drop(registration);
    match outcome {
        Ok(ExecZmodemUploadOutcome::Completed) => {}
        Ok(ExecZmodemUploadOutcome::Cancelled) => {
            tracing::info!(
                target: "app.zmodem",
                session_id = %session_id,
                transfer_generation = generation,
                remote_dir = remote_dir.as_str(),
                lifecycle_command = "cancel",
                owner = "exec",
                outcome = "cleared",
                "cancelled zmodem exec upload"
            );
        }
        Ok(ExecZmodemUploadOutcome::RuntimeClosed) => {
            tracing::debug!(
                target: "app.zmodem",
                session_id = %session_id,
                transfer_generation = generation,
                remote_dir = remote_dir.as_str(),
                outcome = "ignored",
                "zmodem exec upload stopped with runtime lifecycle"
            );
        }
        Err(err) => {
            tracing::warn!(
                target: "app.zmodem",
                session_id = %session_id,
                transfer_generation = generation,
                remote_dir = remote_dir.as_str(),
                error = %err,
                "zmodem exec upload failed"
            );
            let _ = event_tx.send(SessionRuntimeEvent::ZmodemStateChanged(Some(
                failed_zmodem_upload_state(err.to_string()),
            )));
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExecZmodemUploadOutcome {
    Completed,
    Cancelled,
    RuntimeClosed,
}

enum ExecZmodemInput {
    Channel(Option<ChannelMsg>),
    Command(Option<ExecZmodemCommand>),
}

async fn run_zmodem_exec_upload_inner(
    session_id: Uuid,
    handle: Arc<client::Handle<RuntimeClientHandler>>,
    event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    local_paths: Vec<std::path::PathBuf>,
    remote_dir: String,
    generation: u64,
    mut exec_command_rx: mpsc::UnboundedReceiver<ExecZmodemCommand>,
) -> Result<ExecZmodemUploadOutcome> {
    let mut channel = handle
        .channel_open_session()
        .await
        .context("failed to open SSH exec channel for ZMODEM upload")?;
    let command = zmodem_exec_upload_command(remote_dir.as_str());
    channel
        .exec(true, command)
        .await
        .context("failed to request remote rz exec")?;

    let mut zmodem = ZmodemController::default();
    let mut pending_local_paths = Some(local_paths);
    let mut upload_started = false;
    let mut exec_accepted = false;
    let mut exit_status = None;
    let handshake_started = Instant::now();

    loop {
        let input = if upload_started {
            tokio::select! {
                biased;
                command = exec_command_rx.recv() => ExecZmodemInput::Command(command),
                message = channel.wait() => ExecZmodemInput::Channel(message),
            }
        } else {
            let remaining = ZMODEM_EXEC_UPLOAD_HANDSHAKE_TIMEOUT
                .checked_sub(handshake_started.elapsed())
                .unwrap_or(Duration::ZERO);
            if remaining.is_zero() {
                bail!("remote rz did not emit a ZMODEM upload handshake");
            }
            tokio::select! {
                biased;
                command = exec_command_rx.recv() => ExecZmodemInput::Command(command),
                result = timeout(remaining, channel.wait()) => {
                    ExecZmodemInput::Channel(
                        result.context("remote rz did not emit a ZMODEM upload handshake")?
                    )
                }
            }
        };

        let message = match input {
            ExecZmodemInput::Command(Some(ExecZmodemCommand::Cancel)) => {
                return cancel_zmodem_exec_upload(
                    session_id,
                    generation,
                    &handle,
                    &event_tx,
                    &mut channel,
                    &mut zmodem,
                )
                .await;
            }
            ExecZmodemInput::Command(None) => {
                settle_zmodem_exec_channel(&mut channel).await;
                return Ok(ExecZmodemUploadOutcome::RuntimeClosed);
            }
            ExecZmodemInput::Channel(Some(message)) => message,
            ExecZmodemInput::Channel(None) => break,
        };

        match message {
            ChannelMsg::Success => {
                exec_accepted = true;
            }
            ChannelMsg::Failure if !exec_accepted => {
                bail!("remote SSH server rejected the rz exec request");
            }
            ChannelMsg::Data { data } => {
                let _ignored_exec_output = zmodem.intercept_remote_bytes(data.as_ref());
                emit_zmodem_state_changes(&mut zmodem, &event_tx);
                if !upload_started
                    && zmodem.current_state().is_some_and(|state| {
                        state.direction == super::ZmodemTransferDirection::Upload
                            && state.phase == super::ZmodemTransferPhase::AwaitingUploadSelection
                    })
                {
                    let local_paths = pending_local_paths
                        .take()
                        .ok_or_else(|| anyhow!("zmodem upload files were already consumed"))?;
                    zmodem
                        .start_upload(local_paths)
                        .context("failed to start local ZMODEM upload")?;
                    upload_started = true;
                    emit_zmodem_state_changes(&mut zmodem, &event_tx);
                }
                if drive_zmodem(&handle, channel.id(), &event_tx, &mut zmodem)
                    .await
                    .is_none()
                {
                    bail!("failed to write ZMODEM upload bytes to SSH exec channel");
                }
            }
            ChannelMsg::ExtendedData { .. } => {}
            ChannelMsg::ExitStatus {
                exit_status: status,
            } => {
                exit_status = Some(status);
            }
            ChannelMsg::ExitSignal {
                signal_name,
                error_message,
                ..
            } => {
                bail!("remote rz exited by signal {signal_name:?}: {error_message}");
            }
            ChannelMsg::Eof | ChannelMsg::Close => {
                break;
            }
            _ => {}
        }
    }

    zmodem.mark_transport_closed();
    emit_zmodem_state_changes(&mut zmodem, &event_tx);

    let Some(state) = zmodem.current_state() else {
        bail!("remote rz closed before starting a ZMODEM upload");
    };
    if state.phase != super::ZmodemTransferPhase::Completed {
        if let Some(status) = exit_status {
            bail!("remote rz exited with status {status}");
        }
        bail!("remote rz closed before the ZMODEM upload completed");
    }

    tracing::info!(
        target: "app.zmodem",
        session_id = %session_id,
        transfer_generation = generation,
        files_completed = state.files_completed,
        bytes_transferred = state.bytes_transferred,
        "zmodem exec upload completed"
    );
    Ok(ExecZmodemUploadOutcome::Completed)
}

async fn cancel_zmodem_exec_upload(
    session_id: Uuid,
    generation: u64,
    handle: &Arc<client::Handle<RuntimeClientHandler>>,
    event_tx: &mpsc::UnboundedSender<SessionRuntimeEvent>,
    channel: &mut Channel<client::Msg>,
    zmodem: &mut ZmodemController,
) -> Result<ExecZmodemUploadOutcome> {
    if let Some(state) = zmodem.current_state() {
        match state.phase {
            super::ZmodemTransferPhase::Completed => {
                settle_zmodem_exec_channel(channel).await;
                return Ok(ExecZmodemUploadOutcome::Completed);
            }
            super::ZmodemTransferPhase::Failed | super::ZmodemTransferPhase::Cancelled => {
                settle_zmodem_exec_channel(channel).await;
                return Ok(ExecZmodemUploadOutcome::Cancelled);
            }
            super::ZmodemTransferPhase::AwaitingUploadSelection
            | super::ZmodemTransferPhase::AwaitingDownloadDirectory
            | super::ZmodemTransferPhase::Running => {}
        }
        zmodem
            .cancel()
            .context("cancel dedicated exec zmodem upload")?;
        emit_zmodem_state_changes(zmodem, event_tx);
        if drive_zmodem(handle, channel.id(), event_tx, zmodem)
            .await
            .is_none()
        {
            bail!("failed to write ZMODEM abort bytes to SSH exec channel");
        }
    } else {
        if let Err(bytes) = handle.data(channel.id(), ZMODEM_ABORT_WIRE.to_vec()).await {
            tracing::warn!(
                target: "app.zmodem",
                session_id = %session_id,
                transfer_generation = generation,
                lifecycle_command = "cancel",
                owner = "exec",
                outcome = "failed",
                unwritten_bytes = bytes.len(),
                "failed to write pre-handshake zmodem abort wire"
            );
        }
        let _ = event_tx.send(SessionRuntimeEvent::ZmodemStateChanged(Some(
            cancelled_zmodem_upload_state(),
        )));
    }
    settle_zmodem_exec_channel(channel).await;
    Ok(ExecZmodemUploadOutcome::Cancelled)
}

async fn settle_zmodem_exec_channel(channel: &mut Channel<client::Msg>) {
    let _ = channel.eof().await;
    let _ = channel.close().await;
}

async fn remote_exec_exit_status(
    handle: Arc<client::Handle<RuntimeClientHandler>>,
    command: String,
) -> Result<Option<u32>> {
    Ok(remote_exec_output(handle, command, "remote command probe")
        .await?
        .exit_status)
}

#[derive(Debug, Default)]
struct RemoteExecOutput {
    exit_status: Option<u32>,
    stdout: Vec<u8>,
    saw_eof: bool,
    exec_accepted: bool,
}

impl RemoteExecOutput {
    fn push_message(&mut self, message: ChannelMsg, request_label: &'static str) -> Result<bool> {
        match message {
            ChannelMsg::Success => self.exec_accepted = true,
            ChannelMsg::Failure if !self.exec_accepted => {
                bail!("remote SSH server rejected the {request_label} request");
            }
            ChannelMsg::Data { data } => {
                self.stdout.extend_from_slice(data.as_ref());
            }
            ChannelMsg::ExitStatus {
                exit_status: status,
            } => {
                self.exit_status = Some(status);
                return Ok(self.saw_eof);
            }
            ChannelMsg::ExitSignal { .. } => {
                self.exit_status = Some(255);
                return Ok(true);
            }
            ChannelMsg::Eof => {
                self.saw_eof = true;
                return Ok(self.exit_status.is_some());
            }
            ChannelMsg::Close => return Ok(true),
            _ => {}
        }
        Ok(false)
    }
}

fn require_remote_exec_exit_status(
    exit_status: Option<u32>,
    request_label: &'static str,
) -> Result<u32> {
    exit_status.ok_or_else(|| anyhow!("{request_label} closed without an exit status"))
}

async fn remote_exec_output(
    handle: Arc<client::Handle<RuntimeClientHandler>>,
    command: String,
    request_label: &'static str,
) -> Result<RemoteExecOutput> {
    let mut channel = handle
        .channel_open_session()
        .await
        .with_context(|| format!("failed to open SSH exec channel for {request_label}"))?;
    channel
        .exec(true, command)
        .await
        .with_context(|| format!("failed to request {request_label}"))?;

    let mut output = RemoteExecOutput::default();
    while let Some(message) = channel.wait().await {
        if output.push_message(message, request_label)? {
            break;
        }
    }
    Ok(output)
}

fn is_safe_remote_command_name(command_name: &str) -> bool {
    !command_name.is_empty()
        && command_name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn remote_transfer_path_setup() -> &'static str {
    r#"PATH="${PATH:-}${HOME:+:$HOME/.local/bin:$HOME/bin}:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"; export PATH"#
}

fn remote_command_probe_command(command_name: &str) -> String {
    format!(
        "__mica_term_probe_command={}; {}; command -v \"$__mica_term_probe_command\" >/dev/null 2>&1",
        shell_quote_posix(command_name),
        remote_transfer_path_setup(),
    )
}

fn zmodem_exec_upload_command(remote_dir: &str) -> String {
    format!(
        "{}; cd {} && rz -q",
        remote_transfer_path_setup(),
        shell_quote_posix(remote_dir)
    )
}

fn shell_quote_posix(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn failed_zmodem_upload_state(error_text: String) -> super::ZmodemTransferState {
    super::ZmodemTransferState {
        direction: super::ZmodemTransferDirection::Upload,
        phase: super::ZmodemTransferPhase::Failed,
        title: "ZMODEM Upload".into(),
        headline: "Upload failed".into(),
        status_text: "The drag upload could not be completed with rz.".into(),
        detail_text: String::new(),
        error_text,
        current_file_name: String::new(),
        files_completed: 0,
        files_total: None,
        bytes_transferred: 0,
        bytes_total: None,
        local_file_path: None,
        local_reveal_path: None,
    }
}

fn cancelled_zmodem_upload_state() -> super::ZmodemTransferState {
    super::ZmodemTransferState {
        direction: super::ZmodemTransferDirection::Upload,
        phase: super::ZmodemTransferPhase::Cancelled,
        title: "ZMODEM Upload".into(),
        headline: "Upload cancelled".into(),
        status_text: "The transfer was cancelled before all files were sent.".into(),
        detail_text: "The remote shell was told to abort the ZMODEM session.".into(),
        error_text: String::new(),
        current_file_name: String::new(),
        files_completed: 0,
        files_total: None,
        bytes_transferred: 0,
        bytes_total: None,
        local_file_path: None,
        local_reveal_path: None,
    }
}

fn emit_zmodem_state_changes(
    zmodem: &mut ZmodemController,
    event_tx: &mpsc::UnboundedSender<SessionRuntimeEvent>,
) {
    while let Some(state) = zmodem.take_modal_state_change() {
        let _ = event_tx.send(SessionRuntimeEvent::ZmodemStateChanged(state));
    }
}

async fn drive_zmodem(
    handle: &Arc<client::Handle<RuntimeClientHandler>>,
    channel_id: russh::ChannelId,
    event_tx: &mpsc::UnboundedSender<SessionRuntimeEvent>,
    zmodem: &mut ZmodemController,
) -> Option<Vec<u8>> {
    loop {
        emit_zmodem_state_changes(zmodem, event_tx);
        match zmodem.advance() {
            ZmodemAdvanceOutcome::WriteWire(bytes) => {
                let written = bytes.len();
                if let Err(bytes) = handle.data(channel_id, bytes).await {
                    let _ = event_tx.send(SessionRuntimeEvent::Error(format!(
                        "failed to write {} zmodem bytes to SSH channel",
                        bytes.len()
                    )));
                    return None;
                }
                zmodem.note_wire_written(written);
            }
            ZmodemAdvanceOutcome::Continue => {}
            ZmodemAdvanceOutcome::Idle => {
                emit_zmodem_state_changes(zmodem, event_tx);
                return Some(zmodem.take_released_terminal_bytes());
            }
        }
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
struct ReadyOutputBatch {
    raw_bytes: Vec<u8>,
    chunk_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedOutputBatch {
    sanitized_bytes: Vec<u8>,
    cwd: Option<String>,
    next_shell_state: super::TerminalShellIntegrationState,
    shell_integration_changed: bool,
}

#[derive(Debug, Default)]
struct SynchronizedOutputBatcher {
    plain_buffer: Vec<u8>,
    sync_buffer: Vec<u8>,
    sync_active: bool,
}

impl SynchronizedOutputBatcher {
    fn push_bytes(&mut self, bytes: &[u8]) -> Vec<Vec<u8>> {
        let mut ready_batches = Vec::new();
        if self.sync_active {
            self.sync_buffer.extend_from_slice(bytes);
            self.process_sync_buffer(&mut ready_batches);
        } else {
            self.plain_buffer.extend_from_slice(bytes);
            self.process_plain_buffer(&mut ready_batches);
        }
        ready_batches
    }

    fn finish(&mut self) -> Option<Vec<u8>> {
        if self.sync_active {
            self.sync_active = false;
            if self.sync_buffer.is_empty() {
                None
            } else {
                Some(std::mem::take(&mut self.sync_buffer))
            }
        } else if self.plain_buffer.is_empty() {
            None
        } else {
            Some(std::mem::take(&mut self.plain_buffer))
        }
    }

    fn process_plain_buffer(&mut self, ready_batches: &mut Vec<Vec<u8>>) {
        loop {
            let Some(sync_start) =
                find_subsequence(self.plain_buffer.as_slice(), SYNC_OUTPUT_START)
            else {
                let keep_suffix_len =
                    partial_marker_suffix_len(self.plain_buffer.as_slice(), SYNC_OUTPUT_START);
                let emit_len = self.plain_buffer.len().saturating_sub(keep_suffix_len);
                if emit_len > 0 {
                    ready_batches.push(self.plain_buffer.drain(..emit_len).collect());
                }
                return;
            };

            if sync_start > 0 {
                ready_batches.push(self.plain_buffer.drain(..sync_start).collect());
            }
            self.sync_active = true;
            self.sync_buffer.extend(self.plain_buffer.drain(..));
            self.process_sync_buffer(ready_batches);
            if !self.sync_active {
                continue;
            }
            return;
        }
    }

    fn process_sync_buffer(&mut self, ready_batches: &mut Vec<Vec<u8>>) {
        loop {
            let Some(sync_end) = find_subsequence(self.sync_buffer.as_slice(), SYNC_OUTPUT_END)
            else {
                return;
            };
            let emit_len = sync_end + SYNC_OUTPUT_END.len();
            ready_batches.push(self.sync_buffer.drain(..emit_len).collect());
            self.sync_active = false;
            if self.sync_buffer.is_empty() {
                return;
            }
            self.plain_buffer.extend(self.sync_buffer.drain(..));
            self.process_plain_buffer(ready_batches);
            return;
        }
    }
}

const SYNC_OUTPUT_START: &[u8] = b"\x1b[?2026h";
const SYNC_OUTPUT_END: &[u8] = b"\x1b[?2026l";

async fn collect_ready_output_batch(
    initial_message: ChannelMsg,
    channel: &mut Channel<client::Msg>,
    pending_channel_messages: &mut VecDeque<ChannelMsg>,
) -> ReadyOutputBatch {
    let mut backlog = VecDeque::from([initial_message]);

    while let Ok(Some(message)) = timeout(Duration::ZERO, channel.wait()).await {
        backlog.push_back(message);
        if !matches!(
            backlog.back(),
            Some(ChannelMsg::Data { .. } | ChannelMsg::ExtendedData { .. })
        ) {
            break;
        }
    }

    let Some(first_message) = backlog.pop_front() else {
        return ReadyOutputBatch::default();
    };
    let output_batch = take_contiguous_output_messages(first_message, &mut backlog);
    pending_channel_messages.extend(backlog);
    output_batch
}

fn take_contiguous_output_messages(
    first_message: ChannelMsg,
    backlog: &mut VecDeque<ChannelMsg>,
) -> ReadyOutputBatch {
    let mut batch = ReadyOutputBatch::default();
    let mut next_message = Some(first_message);

    while let Some(message) = next_message.take() {
        let Some(bytes) = channel_output_bytes(&message) else {
            backlog.push_front(message);
            break;
        };
        batch.raw_bytes.extend_from_slice(bytes);
        batch.chunk_count = batch.chunk_count.saturating_add(1);

        match backlog.front() {
            Some(ChannelMsg::Data { .. } | ChannelMsg::ExtendedData { .. }) => {
                next_message = backlog.pop_front();
            }
            _ => break,
        }
    }

    batch
}

fn process_ready_remote_output(
    ready_bytes: &[u8],
    terminal: &Arc<Mutex<TerminalSession>>,
    event_tx: &mpsc::UnboundedSender<SessionRuntimeEvent>,
    shell_integration: &mut super::TerminalShellIntegrationState,
    dirty_notifier: &mut SurfaceDirtyNotifier,
    dirty_timer: &mut Option<std::pin::Pin<Box<Sleep>>>,
    dirty_timer_interval: &mut Option<std::time::Duration>,
    working_set_trim_scheduler: &mut WorkingSetTrimScheduler,
    working_set_trim_timer: &mut Option<std::pin::Pin<Box<Sleep>>>,
) {
    let parsed = parse_output_chunks(*shell_integration, &[ready_bytes]);

    if let Some(cwd) = parsed.cwd.as_ref() {
        let _ = event_tx.send(SessionRuntimeEvent::CurrentDirectoryChanged(cwd.clone()));
    }
    if parsed.shell_integration_changed {
        *shell_integration = parsed.next_shell_state;
        let _ = event_tx.send(SessionRuntimeEvent::ShellIntegrationChanged(
            *shell_integration,
        ));
    }
    if !parsed.sanitized_bytes.is_empty() {
        apply_remote_output(terminal, &parsed.sanitized_bytes);
        working_set_trim_scheduler.record_output(parsed.sanitized_bytes.len());
        *working_set_trim_timer = Some(Box::pin(sleep(WORKING_SET_TRIM_IDLE_INTERVAL)));
        let now = Instant::now();
        let (should_arm, preferred_interval) = dirty_notifier.record_output(now);
        let should_speed_up_timer = dirty_timer_interval
            .is_some_and(|current_interval| preferred_interval < current_interval);
        if should_arm || should_speed_up_timer {
            *dirty_timer = Some(Box::pin(sleep(preferred_interval)));
            *dirty_timer_interval = Some(preferred_interval);
        }
    }
}

fn parse_output_chunks(
    initial_shell_state: super::TerminalShellIntegrationState,
    chunks: &[&[u8]],
) -> ParsedOutputBatch {
    let mut merged_bytes = Vec::with_capacity(chunks.iter().map(|chunk| chunk.len()).sum());
    for chunk in chunks {
        merged_bytes.extend_from_slice(chunk);
    }
    let parsed = runtime_shell_events(merged_bytes.as_slice());
    let mut next_shell_state = initial_shell_state;
    let shell_integration_changed = apply_shell_integration_events(&mut next_shell_state, &parsed);

    ParsedOutputBatch {
        sanitized_bytes: parsed.sanitized_bytes,
        cwd: parsed.cwd,
        next_shell_state,
        shell_integration_changed,
    }
}

fn channel_output_bytes(message: &ChannelMsg) -> Option<&[u8]> {
    match message {
        ChannelMsg::Data { data } | ChannelMsg::ExtendedData { data, .. } => Some(data.as_ref()),
        _ => None,
    }
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn partial_marker_suffix_len(bytes: &[u8], marker: &[u8]) -> usize {
    let max_suffix = bytes.len().min(marker.len().saturating_sub(1));
    (1..=max_suffix)
        .rev()
        .find(|&suffix_len| bytes.ends_with(&marker[..suffix_len]))
        .unwrap_or(0)
}

fn apply_shell_integration_events(
    state: &mut super::TerminalShellIntegrationState,
    parsed: &crate::app::ssh::shell_integration::RuntimeShellEvents,
) -> bool {
    let before = *state;

    if parsed.prompt_started {
        state.has_markers = true;
        state.input_active = false;
        state.command_running = false;
    }
    if parsed.prompt_ended {
        state.has_markers = true;
        state.input_active = true;
        state.command_running = false;
    }
    if parsed.command_started {
        state.has_markers = true;
        state.input_active = false;
        state.command_running = true;
    }
    if parsed.command_finished {
        state.has_markers = true;
        state.input_active = false;
        state.command_running = false;
        state.last_command_exit_code = parsed.command_finish_exit_code;
    }

    *state != before
}

#[derive(Debug, Default)]
struct SurfaceDirtyNotifier {
    dirty: bool,
    notification_armed: bool,
    input_active_until: Option<Instant>,
}

impl SurfaceDirtyNotifier {
    fn note_local_input(&mut self, now: Instant) {
        self.input_active_until = Some(now + INPUT_ACTIVE_SURFACE_DIRTY_WINDOW);
    }

    fn preferred_interval(&self, now: Instant) -> std::time::Duration {
        if self
            .input_active_until
            .is_some_and(|deadline| deadline > now)
        {
            FAST_SURFACE_DIRTY_NOTIFICATION_INTERVAL
        } else {
            SURFACE_DIRTY_NOTIFICATION_INTERVAL
        }
    }

    fn record_output(&mut self, now: Instant) -> (bool, std::time::Duration) {
        self.dirty = true;
        let should_arm = !self.notification_armed;
        self.notification_armed = true;
        (should_arm, self.preferred_interval(now))
    }

    fn flush_due(&mut self) -> bool {
        self.notification_armed = false;
        if self.dirty {
            self.dirty = false;
            true
        } else {
            false
        }
    }

    fn take_pending(&mut self) -> bool {
        self.notification_armed = false;
        if self.dirty {
            self.dirty = false;
            true
        } else {
            false
        }
    }
}

#[derive(Debug, Default)]
struct WorkingSetTrimScheduler {
    pending_output_bytes: usize,
}

impl WorkingSetTrimScheduler {
    fn record_output(&mut self, bytes: usize) {
        self.pending_output_bytes = self.pending_output_bytes.saturating_add(bytes);
    }

    fn take_trim_request_bytes(&mut self) -> Option<usize> {
        let pending_output_bytes = self.pending_output_bytes;
        self.pending_output_bytes = 0;
        (pending_output_bytes >= WORKING_SET_TRIM_MIN_OUTPUT_BYTES).then_some(pending_output_bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn collect_remote_exec_test_messages(
        messages: impl IntoIterator<Item = ChannelMsg>,
    ) -> RemoteExecOutput {
        let mut output = RemoteExecOutput::default();
        for message in messages {
            if output
                .push_message(message, "test remote exec")
                .expect("collect test remote exec message")
            {
                break;
            }
        }
        output
    }

    #[test]
    fn interactive_rz_fallback_uses_quiet_history_friendly_command() {
        assert_eq!(INTERACTIVE_RZ_UPLOAD_COMMAND, b" rz -q\r");
        let command = String::from_utf8_lossy(INTERACTIVE_RZ_UPLOAD_COMMAND);
        assert!(!command.contains("command -v"));
        assert!(!command.contains("if "));
    }

    #[test]
    fn exec_zmodem_upload_uses_quiet_rz() {
        let command = zmodem_exec_upload_command("/srv/releases");

        assert!(command.ends_with("cd '/srv/releases' && rz -q"));
    }

    #[test]
    fn pending_interactive_rz_fallback_times_out_after_handshake_window() {
        let started_at = Instant::now();
        let pending = PendingInteractiveZmodemUpload {
            local_paths: vec![std::path::PathBuf::from("release.env")],
            started_at,
        };

        assert_eq!(
            pending.remaining_timeout(started_at + Duration::from_secs(1)),
            Duration::from_secs(3)
        );
        assert!(!pending.timed_out(started_at + Duration::from_secs(3)));
        assert!(pending.timed_out(
            started_at + INTERACTIVE_ZMODEM_UPLOAD_HANDSHAKE_TIMEOUT + Duration::from_millis(1)
        ));
    }

    #[test]
    fn surface_dirty_notifier_coalesces_repeated_output_until_flush() {
        let mut notifier = SurfaceDirtyNotifier::default();
        let now = Instant::now();

        assert_eq!(
            notifier.record_output(now),
            (true, SURFACE_DIRTY_NOTIFICATION_INTERVAL)
        );
        assert_eq!(
            notifier.record_output(now),
            (false, SURFACE_DIRTY_NOTIFICATION_INTERVAL)
        );
        assert_eq!(
            notifier.record_output(now),
            (false, SURFACE_DIRTY_NOTIFICATION_INTERVAL)
        );
        assert!(notifier.flush_due());
        assert!(!notifier.flush_due());
    }

    #[test]
    fn surface_dirty_notifier_rearms_after_flush() {
        let mut notifier = SurfaceDirtyNotifier::default();
        let now = Instant::now();

        assert_eq!(
            notifier.record_output(now),
            (true, SURFACE_DIRTY_NOTIFICATION_INTERVAL)
        );
        assert!(notifier.flush_due());
        assert_eq!(
            notifier.record_output(now),
            (true, SURFACE_DIRTY_NOTIFICATION_INTERVAL)
        );
        assert!(notifier.take_pending());
        assert!(!notifier.take_pending());
    }

    #[test]
    fn surface_dirty_notifier_prefers_fast_interval_during_local_input_window() {
        let mut notifier = SurfaceDirtyNotifier::default();
        let now = Instant::now();

        notifier.note_local_input(now);

        assert_eq!(
            notifier.preferred_interval(now),
            FAST_SURFACE_DIRTY_NOTIFICATION_INTERVAL
        );
        assert_eq!(
            notifier.preferred_interval(
                now + INPUT_ACTIVE_SURFACE_DIRTY_WINDOW + Duration::from_millis(1)
            ),
            SURFACE_DIRTY_NOTIFICATION_INTERVAL
        );
        assert_eq!(
            notifier.record_output(now),
            (true, FAST_SURFACE_DIRTY_NOTIFICATION_INTERVAL)
        );
    }

    #[test]
    fn working_set_trim_scheduler_ignores_small_idle_output() {
        let mut scheduler = WorkingSetTrimScheduler::default();

        scheduler.record_output(WORKING_SET_TRIM_MIN_OUTPUT_BYTES / 4);

        assert_eq!(scheduler.take_trim_request_bytes(), None);
        assert_eq!(scheduler.take_trim_request_bytes(), None);
    }

    #[test]
    fn working_set_trim_scheduler_requests_trim_after_large_idle_output() {
        let mut scheduler = WorkingSetTrimScheduler::default();

        scheduler.record_output(WORKING_SET_TRIM_MIN_OUTPUT_BYTES / 2);
        scheduler.record_output(WORKING_SET_TRIM_MIN_OUTPUT_BYTES / 2);
        scheduler.record_output(1);

        assert_eq!(
            scheduler.take_trim_request_bytes(),
            Some(WORKING_SET_TRIM_MIN_OUTPUT_BYTES + 1)
        );
        assert_eq!(scheduler.take_trim_request_bytes(), None);
    }

    #[test]
    fn working_set_trim_scheduler_reports_pending_bytes_for_trim_diagnostics() {
        let mut scheduler = WorkingSetTrimScheduler::default();

        scheduler.record_output(WORKING_SET_TRIM_MIN_OUTPUT_BYTES / 2);
        scheduler.record_output(WORKING_SET_TRIM_MIN_OUTPUT_BYTES / 2);
        scheduler.record_output(1);

        assert_eq!(
            scheduler.take_trim_request_bytes(),
            Some(WORKING_SET_TRIM_MIN_OUTPUT_BYTES + 1)
        );
        assert_eq!(scheduler.take_trim_request_bytes(), None);
    }

    #[test]
    fn parse_output_chunks_merges_split_shell_integration_sequences() {
        let parsed = parse_output_chunks(
            crate::app::ssh::runtime::TerminalShellIntegrationState::default(),
            &[
                b"\x1b]133;".as_slice(),
                b"B\x07prompt ready".as_slice(),
                b"\x1b]7;file://remote/home/tester\x07".as_slice(),
            ],
        );

        assert_eq!(parsed.sanitized_bytes, b"prompt ready");
        assert_eq!(parsed.cwd.as_deref(), Some("/home/tester"));
        assert!(parsed.shell_integration_changed);
        assert!(parsed.next_shell_state.has_markers);
        assert!(parsed.next_shell_state.input_active);
        assert!(!parsed.next_shell_state.command_running);
    }

    #[test]
    fn remote_command_probe_uses_transfer_path_setup() {
        let command = remote_command_probe_command("rz");

        assert!(command.contains("__mica_term_probe_command='rz'"));
        assert!(command.contains("${HOME:+:$HOME/.local/bin:$HOME/bin}"));
        assert!(command.contains("/usr/local/bin"));
        assert!(command.contains("command -v \"$__mica_term_probe_command\""));
    }

    #[test]
    fn remote_exec_output_completes_when_eof_follows_exit_status() {
        let output = collect_remote_exec_test_messages([
            ChannelMsg::ExitStatus { exit_status: 0 },
            ChannelMsg::Eof,
            ChannelMsg::Close,
        ]);

        assert_eq!(output.exit_status, Some(0));
        assert!(output.saw_eof);
    }

    #[test]
    fn remote_exec_output_waits_for_exit_status_after_eof() {
        let output = collect_remote_exec_test_messages([
            ChannelMsg::Eof,
            ChannelMsg::ExitStatus { exit_status: 0 },
            ChannelMsg::Close,
        ]);

        assert_eq!(output.exit_status, Some(0));
        assert!(output.saw_eof);
    }

    #[test]
    fn remote_exec_output_preserves_data_before_eof_and_late_status() {
        let output = collect_remote_exec_test_messages([
            ChannelMsg::Data {
                data: b"/srv/b\n".as_slice().into(),
            },
            ChannelMsg::Eof,
            ChannelMsg::ExitStatus { exit_status: 0 },
            ChannelMsg::Close,
        ]);

        assert_eq!(output.stdout, b"/srv/b\n");
        assert_eq!(output.exit_status, Some(0));
    }

    #[test]
    fn remote_exec_output_accepts_close_without_eof() {
        let output = collect_remote_exec_test_messages([
            ChannelMsg::ExitStatus { exit_status: 0 },
            ChannelMsg::Close,
        ]);

        assert_eq!(output.exit_status, Some(0));
        assert!(!output.saw_eof);
    }

    #[test]
    fn remote_exec_output_reports_missing_exit_status_as_incomplete() {
        let output = collect_remote_exec_test_messages([ChannelMsg::Eof, ChannelMsg::Close]);
        let error = require_remote_exec_exit_status(output.exit_status, "test remote exec")
            .expect_err("missing exit status must be incomplete");

        assert!(error.to_string().contains("without an exit status"));
    }

    #[test]
    fn synchronized_output_batcher_waits_for_sync_end_across_chunks() {
        let mut batcher = SynchronizedOutputBatcher::default();

        let first = batcher.push_bytes(b"prefix\x1b[?2026hframe-1");
        let second = batcher.push_bytes(b"-continued\x1b[?2026l");

        assert_eq!(first, vec![b"prefix".to_vec()]);
        assert_eq!(
            second,
            vec![b"\x1b[?2026hframe-1-continued\x1b[?2026l".to_vec()]
        );
    }

    #[test]
    fn collect_output_batch_keeps_output_chunks_until_first_control_message() {
        let mut backlog = std::collections::VecDeque::from([
            ChannelMsg::ExtendedData {
                data: b" stderr".as_slice().into(),
                ext: 1,
            },
            ChannelMsg::Close,
        ]);

        let batch = take_contiguous_output_messages(
            ChannelMsg::Data {
                data: b"stdout".as_slice().into(),
            },
            &mut backlog,
        );

        assert_eq!(batch.raw_bytes, b"stdout stderr");
        assert_eq!(batch.chunk_count, 2);
        assert!(matches!(backlog.front(), Some(ChannelMsg::Close)));
    }
}

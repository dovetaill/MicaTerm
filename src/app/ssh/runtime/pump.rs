//! SSH runtime channel pump and output coalescing helpers.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use russh::client;
use russh::{Channel, ChannelMsg, Disconnect};
use tokio::sync::mpsc;
use tokio::time::{Sleep, sleep, timeout};
use uuid::Uuid;

use crate::app::ssh::shell_integration::runtime_shell_events;

use super::auth::RuntimeClientHandler;
use super::terminal::{TerminalSession, apply_remote_output, snapshot_terminal_surface};
use super::transport::TransportChainGuard;
use super::zmodem::{ZmodemAdvanceOutcome, ZmodemController};
use super::{
    FAST_SURFACE_DIRTY_NOTIFICATION_INTERVAL, INPUT_ACTIVE_SURFACE_DIRTY_WINDOW, RuntimeCommand,
    SURFACE_DIRTY_NOTIFICATION_INTERVAL, SessionRuntimeEvent, WORKING_SET_TRIM_IDLE_INTERVAL,
    WORKING_SET_TRIM_MIN_OUTPUT_BYTES,
};

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

    'pump: loop {
        tokio::select! {
            maybe_command = command_rx.recv(), if command_channel_open => {
                match maybe_command {
                    Some(RuntimeCommand::TextInput(text)) => {
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
                    Some(RuntimeCommand::StartZmodemUpload { local_paths }) => {
                        if let Err(err) = zmodem.start_upload(local_paths) {
                            zmodem.surface_error(err.to_string());
                        }
                        emit_zmodem_state_changes(&mut zmodem, &event_tx);
                        if !drive_zmodem(
                            &handle,
                            channel.id(),
                            &event_tx,
                            &mut zmodem,
                        ).await {
                            break 'pump;
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
                        if !drive_zmodem(
                            &handle,
                            channel.id(),
                            &event_tx,
                            &mut zmodem,
                        ).await {
                            break 'pump;
                        }
                    }
                    Some(RuntimeCommand::CancelZmodem) => {
                        let _ = zmodem.cancel();
                        emit_zmodem_state_changes(&mut zmodem, &event_tx);
                        if !drive_zmodem(
                            &handle,
                            channel.id(),
                            &event_tx,
                            &mut zmodem,
                        ).await {
                            break 'pump;
                        }
                    }
                    Some(RuntimeCommand::DismissZmodem) => {
                        zmodem.dismiss();
                        emit_zmodem_state_changes(&mut zmodem, &event_tx);
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
                            let terminal_bytes = zmodem.intercept_remote_bytes(ready_bytes.as_slice());
                            emit_zmodem_state_changes(&mut zmodem, &event_tx);
                            if !drive_zmodem(&handle, channel.id(), &event_tx, &mut zmodem).await {
                                break 'pump;
                            }
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
                        zmodem.mark_transport_closed();
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
                            let terminal_bytes = zmodem.intercept_remote_bytes(ready_bytes.as_slice());
                            emit_zmodem_state_changes(&mut zmodem, &event_tx);
                            if !drive_zmodem(&handle, channel.id(), &event_tx, &mut zmodem).await {
                                break 'pump;
                            }
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
                            let terminal_bytes = zmodem.intercept_remote_bytes(ready_bytes.as_slice());
                            emit_zmodem_state_changes(&mut zmodem, &event_tx);
                            if !drive_zmodem(&handle, channel.id(), &event_tx, &mut zmodem).await {
                                break 'pump;
                            }
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
                        zmodem.mark_transport_closed();
                        emit_zmodem_state_changes(&mut zmodem, &event_tx);
                        if dirty_notifier.take_pending() {
                            let _ = event_tx.send(SessionRuntimeEvent::SurfaceDirty);
                        }
                        let _ = event_tx.send(SessionRuntimeEvent::Disconnected);
                        break;
                    }
                    Some(ChannelMsg::Failure) => {
                        if let Some(ready_bytes) = synchronized_output.finish() {
                            let terminal_bytes = zmodem.intercept_remote_bytes(ready_bytes.as_slice());
                            emit_zmodem_state_changes(&mut zmodem, &event_tx);
                            if !drive_zmodem(&handle, channel.id(), &event_tx, &mut zmodem).await {
                                break 'pump;
                            }
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
                        zmodem.mark_transport_closed();
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
) -> bool {
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
                    return false;
                }
                zmodem.note_wire_written(written);
            }
            ZmodemAdvanceOutcome::Continue => {}
            ZmodemAdvanceOutcome::Idle => {
                emit_zmodem_state_changes(zmodem, event_tx);
                return true;
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

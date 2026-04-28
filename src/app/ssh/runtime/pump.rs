//! SSH runtime channel pump and output coalescing helpers.

use std::sync::{Arc, Mutex};
use std::time::Instant;

use russh::client;
use russh::{Channel, ChannelMsg, Disconnect};
use tokio::sync::mpsc;
use tokio::time::{Sleep, sleep};
use uuid::Uuid;

use crate::app::ssh::shell_integration::runtime_shell_events;

use super::auth::RuntimeClientHandler;
use super::terminal::{TerminalSession, apply_remote_output, snapshot_terminal_surface};
use super::transport::TransportChainGuard;
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
    let mut dirty_notifier = SurfaceDirtyNotifier::default();
    let mut dirty_timer: Option<std::pin::Pin<Box<Sleep>>> = None;
    let mut dirty_timer_interval: Option<std::time::Duration> = None;
    let mut working_set_trim_scheduler = WorkingSetTrimScheduler::default();
    let mut working_set_trim_timer: Option<std::pin::Pin<Box<Sleep>>> = None;
    let mut shell_integration = super::TerminalShellIntegrationState::default();

    loop {
        tokio::select! {
            maybe_command = command_rx.recv(), if command_channel_open => {
                match maybe_command {
                    Some(RuntimeCommand::TextInput(text)) => {
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
            maybe_message = channel.wait() => {
                match maybe_message {
                    Some(ChannelMsg::Data { data }) | Some(ChannelMsg::ExtendedData { data, .. }) => {
                        let parsed = runtime_shell_events(data.as_ref());
                        if let Some(cwd) = parsed.cwd.as_ref() {
                            let _ = event_tx.send(SessionRuntimeEvent::CurrentDirectoryChanged(cwd.clone()));
                        }
                        if apply_shell_integration_events(&mut shell_integration, &parsed) {
                            let _ = event_tx.send(SessionRuntimeEvent::ShellIntegrationChanged(
                                shell_integration,
                            ));
                        }
                        if !parsed.sanitized_bytes.is_empty() {
                            apply_remote_output(&terminal, &parsed.sanitized_bytes);
                            working_set_trim_scheduler.record_output(parsed.sanitized_bytes.len());
                            working_set_trim_timer =
                                Some(Box::pin(sleep(WORKING_SET_TRIM_IDLE_INTERVAL)));
                            let now = Instant::now();
                            let (should_arm, preferred_interval) = dirty_notifier.record_output(now);
                            let should_speed_up_timer = dirty_timer_interval
                                .is_some_and(|current_interval| preferred_interval < current_interval);
                            if should_arm || should_speed_up_timer {
                                dirty_timer = Some(Box::pin(sleep(preferred_interval)));
                                dirty_timer_interval = Some(preferred_interval);
                            }
                        }
                    }
                    Some(ChannelMsg::Close) | Some(ChannelMsg::Eof) | None => {
                        if dirty_notifier.take_pending() {
                            let _ = event_tx.send(SessionRuntimeEvent::SurfaceDirty);
                        }
                        let _ = event_tx.send(SessionRuntimeEvent::Disconnected);
                        break;
                    }
                    Some(ChannelMsg::Failure) => {
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
                if working_set_trim_scheduler.trim_due() {
                    let _ = crate::app::memory::trim_process_working_set();
                }
            }
        }
    }
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

    fn trim_due(&mut self) -> bool {
        let should_trim = self.pending_output_bytes >= WORKING_SET_TRIM_MIN_OUTPUT_BYTES;
        self.pending_output_bytes = 0;
        should_trim
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

        assert!(!scheduler.trim_due());
        assert!(!scheduler.trim_due());
    }

    #[test]
    fn working_set_trim_scheduler_requests_trim_after_large_idle_output() {
        let mut scheduler = WorkingSetTrimScheduler::default();

        scheduler.record_output(WORKING_SET_TRIM_MIN_OUTPUT_BYTES / 2);
        scheduler.record_output(WORKING_SET_TRIM_MIN_OUTPUT_BYTES / 2);
        scheduler.record_output(1);

        assert!(scheduler.trim_due());
        assert!(!scheduler.trim_due());
    }
}

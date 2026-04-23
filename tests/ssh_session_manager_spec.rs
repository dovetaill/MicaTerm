use std::fs;
use std::future::Future;
use std::io::{Cursor, Read, Seek, SeekFrom, Write};
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow};
use mica_term::app::async_runtime::AppAsyncRuntime;
use mica_term::app::sftp::{
    BoxedSftpReader, BoxedSftpWriter, SftpBackend, SftpDirectoryEntry, SftpRemoteMetadata,
    SftpRuntimeHandle, SftpWriteMode,
};
use mica_term::app::ssh::connection_progress::{
    ConnectionHeadlineState, ConnectionProgressEvent, ConnectionStepState,
};
use mica_term::app::ssh::known_hosts::KnownHostsService;
use mica_term::app::ssh::profile::{
    ConnectionProfile, ConnectionProxyProfile, ResolvedProxyHop, SshAuthMethod,
};
use mica_term::app::ssh::runtime::{
    SessionRuntimeEvent, SshSessionRuntime, TerminalKeyEvent, TerminalMouseButton,
    TerminalMouseEventKind, TerminalMouseInput, TerminalSession, TerminalSurfaceState,
    UnknownHostKeyError, negotiated_terminal_environment,
};
use mica_term::app::ssh::session_manager::{
    EnhancedSessionState, OpenSessionMode, SessionManager, SessionRuntimeControl,
    SessionRuntimeLauncher, SessionState,
};
use mica_term::app::ssh::shell_integration::runtime_shell_events;
use mica_term::app::terminal_theme::preset_for_theme;
use mica_term::theme::{ThemeMode, ThemeVariant};
use russh::keys::PrivateKey;
use russh::keys::ssh_key::rand_core::OsRng;
use russh::server::{Auth, Session};
use russh::{Channel, ChannelId, server};
use tokio::io::{
    AsyncRead, AsyncReadExt, AsyncSeek, AsyncWrite, AsyncWriteExt, ReadBuf, copy_bidirectional,
};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use uuid::Uuid;

static KNOWN_HOSTS_ENV_LOCK: Mutex<()> = Mutex::new(());
const BOOTSTRAP_ACK_ACCEPTED: &str = "__MICA_TERM_BOOTSTRAP_OK__";
const BOOTSTRAP_ACK_REJECTED: &str = "__MICA_TERM_BOOTSTRAP_REJECT__";

fn lock_known_hosts_env() -> std::sync::MutexGuard<'static, ()> {
    KNOWN_HOSTS_ENV_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[derive(Clone)]
struct FakeLauncher {
    behavior: FakeLauncherBehavior,
}

#[derive(Clone, Default)]
struct RuntimeBackedLauncher;

#[derive(Clone)]
struct TrackingLauncher {
    disconnects: Arc<AtomicUsize>,
    terminal_releases: Arc<AtomicUsize>,
}

struct NoopFileHandle {
    cursor: Cursor<Vec<u8>>,
}

impl NoopFileHandle {
    fn new() -> Self {
        Self {
            cursor: Cursor::new(Vec::new()),
        }
    }
}

impl AsyncRead for NoopFileHandle {
    fn poll_read(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let mut chunk = vec![0; buf.remaining()];
        let read = Read::read(&mut self.cursor, &mut chunk)?;
        buf.put_slice(&chunk[..read]);
        Poll::Ready(Ok(()))
    }
}

impl AsyncWrite for NoopFileHandle {
    fn poll_write(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Poll::Ready(Write::write(&mut self.cursor, buf))
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

impl AsyncSeek for NoopFileHandle {
    fn start_seek(mut self: Pin<&mut Self>, position: SeekFrom) -> std::io::Result<()> {
        Seek::seek(&mut self.cursor, position)?;
        Ok(())
    }

    fn poll_complete(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<u64>> {
        Poll::Ready(Ok(self.cursor.position()))
    }
}

#[derive(Clone, Default)]
struct InteractiveTrackingState {
    text_inputs: Arc<Mutex<Vec<String>>>,
    key_inputs: Arc<Mutex<Vec<TerminalKeyEvent>>>,
    paste_inputs: Arc<Mutex<Vec<String>>>,
    resizes: Arc<Mutex<Vec<(u32, u32)>>>,
    mouse_inputs: Arc<Mutex<Vec<TerminalMouseInput>>>,
}

#[derive(Clone, Default)]
struct ScrollTrackingState {
    surface: Arc<Mutex<Option<TerminalSurfaceState>>>,
    scroll_deltas: Arc<Mutex<Vec<i32>>>,
}

#[derive(Clone)]
struct InteractiveTrackingLauncher {
    state: InteractiveTrackingState,
}

#[derive(Clone)]
struct ScrollTrackingLauncher {
    state: ScrollTrackingState,
}

#[derive(Clone)]
struct SurfacePullLauncher {
    surface: TerminalSurfaceState,
}

#[derive(Clone, Default)]
struct ThemeTrackingLauncher;

#[derive(Clone)]
struct DelayedTrackingLauncher {
    disconnects: Arc<AtomicUsize>,
    terminal_releases: Arc<AtomicUsize>,
    ready_delay: Duration,
}

#[derive(Clone)]
struct DelayedInteractiveTrackingLauncher {
    state: InteractiveTrackingState,
    ready_delay: Duration,
}

#[derive(Clone, Default)]
struct EnhancedStateLauncher;

#[derive(Clone)]
struct TrackingRuntimeControl {
    disconnects: Arc<AtomicUsize>,
    terminal_releases: Arc<AtomicUsize>,
}

#[derive(Clone)]
struct InteractiveTrackingRuntimeControl {
    state: InteractiveTrackingState,
}

#[derive(Clone)]
struct ScrollTrackingRuntimeControl {
    state: ScrollTrackingState,
}

#[derive(Clone)]
struct SurfacePullRuntimeControl {
    surface: TerminalSurfaceState,
}

struct ThemeTrackingRuntimeControl {
    session_id: Uuid,
    terminal: Arc<Mutex<TerminalSession>>,
}

#[derive(Default)]
struct NoopSftpBackend;

#[derive(Clone)]
struct SftpCapableLauncher {
    backend: Arc<dyn SftpBackend>,
}

#[derive(Clone)]
struct SftpCapableRuntimeControl {
    runtime: SftpRuntimeHandle,
}

#[derive(Clone, Copy)]
enum FakeLauncherBehavior {
    StayConnecting,
    FailImmediately,
}

#[derive(Clone)]
enum FakeSocks5AuthMode {
    NoAuth,
    UsernamePassword { username: String, password: String },
    RejectAuthentication,
}

#[derive(Clone)]
enum FakeHttpProxyAuthMode {
    NoAuth,
    Basic { username: String, password: String },
    RejectAuthentication,
}

fn encode_basic_auth_header(username: &str, password: &str) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let input = format!("{username}:{password}");
    let bytes = input.as_bytes();
    let mut encoded = String::new();
    let mut index = 0;
    while index < bytes.len() {
        let first = bytes[index];
        let second = bytes.get(index + 1).copied();
        let third = bytes.get(index + 2).copied();

        encoded.push(TABLE[(first >> 2) as usize] as char);
        encoded.push(TABLE[(((first & 0x03) << 4) | (second.unwrap_or(0) >> 4)) as usize] as char);
        match second {
            Some(second) => {
                encoded.push(
                    TABLE[(((second & 0x0f) << 2) | (third.unwrap_or(0) >> 6)) as usize] as char,
                );
            }
            None => encoded.push('='),
        }
        match third {
            Some(third) => encoded.push(TABLE[(third & 0x3f) as usize] as char),
            None => encoded.push('='),
        }

        index += 3;
    }
    encoded
}

#[derive(Clone)]
struct DirectTcpipRoute {
    requested_host: String,
    requested_port: u16,
    target_addr: std::net::SocketAddr,
}

#[derive(Clone, Default)]
struct DirectTcpipBehavior {
    routes: Vec<DirectTcpipRoute>,
    reject_requests: bool,
}

impl FakeLauncher {
    fn stay_connecting() -> Self {
        Self {
            behavior: FakeLauncherBehavior::StayConnecting,
        }
    }

    fn fail_immediately() -> Self {
        Self {
            behavior: FakeLauncherBehavior::FailImmediately,
        }
    }
}

impl TrackingLauncher {
    fn new(disconnects: Arc<AtomicUsize>, terminal_releases: Arc<AtomicUsize>) -> Self {
        Self {
            disconnects,
            terminal_releases,
        }
    }
}

impl InteractiveTrackingLauncher {
    fn new(state: InteractiveTrackingState) -> Self {
        Self { state }
    }
}

impl ScrollTrackingLauncher {
    fn new(state: ScrollTrackingState) -> Self {
        Self { state }
    }
}

impl SurfacePullLauncher {
    fn new(surface: TerminalSurfaceState) -> Self {
        Self { surface }
    }
}

impl DelayedTrackingLauncher {
    fn new(
        disconnects: Arc<AtomicUsize>,
        terminal_releases: Arc<AtomicUsize>,
        ready_delay: Duration,
    ) -> Self {
        Self {
            disconnects,
            terminal_releases,
            ready_delay,
        }
    }
}

impl DelayedInteractiveTrackingLauncher {
    fn new(state: InteractiveTrackingState, ready_delay: Duration) -> Self {
        Self { state, ready_delay }
    }
}

impl SessionRuntimeLauncher for EnhancedStateLauncher {
    fn launch(
        &self,
        _profile: ConnectionProfile,
        _session_id: Uuid,
        _attempt_id: Uuid,
        event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn SessionRuntimeControl>>> + Send + 'static>>
    {
        Box::pin(async move {
            let _ = event_tx.send(SessionRuntimeEvent::Connected);
            let _ = event_tx.send(SessionRuntimeEvent::EnhancedSessionStateChanged(
                EnhancedSessionState::Enhanced,
            ));
            Ok(Box::new(TrackingRuntimeControl {
                disconnects: Arc::new(AtomicUsize::new(0)),
                terminal_releases: Arc::new(AtomicUsize::new(0)),
            }) as Box<dyn SessionRuntimeControl>)
        })
    }

    fn probe(
        &self,
        _profile: ConnectionProfile,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>> {
        Box::pin(async move { Ok(()) })
    }
}

impl SessionRuntimeLauncher for FakeLauncher {
    fn launch(
        &self,
        _profile: ConnectionProfile,
        _session_id: Uuid,
        _attempt_id: Uuid,
        event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn SessionRuntimeControl>>> + Send + 'static>>
    {
        let behavior = self.behavior;
        Box::pin(async move {
            match behavior {
                FakeLauncherBehavior::StayConnecting => {
                    tokio::time::sleep(Duration::from_millis(25)).await;
                    Ok(Box::new(TrackingRuntimeControl {
                        disconnects: Arc::new(AtomicUsize::new(0)),
                        terminal_releases: Arc::new(AtomicUsize::new(0)),
                    }) as Box<dyn SessionRuntimeControl>)
                }
                FakeLauncherBehavior::FailImmediately => {
                    event_tx
                        .send(SessionRuntimeEvent::Error("authentication failed".into()))
                        .expect("send runtime error");
                    Err(anyhow!("authentication failed"))
                }
            }
        })
    }

    fn probe(
        &self,
        _profile: ConnectionProfile,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>> {
        let behavior = self.behavior;
        Box::pin(async move {
            match behavior {
                FakeLauncherBehavior::StayConnecting => Ok(()),
                FakeLauncherBehavior::FailImmediately => Err(anyhow!("authentication failed")),
            }
        })
    }
}

impl SessionRuntimeLauncher for RuntimeBackedLauncher {
    fn launch(
        &self,
        profile: ConnectionProfile,
        session_id: Uuid,
        attempt_id: Uuid,
        event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn SessionRuntimeControl>>> + Send + 'static>>
    {
        Box::pin(async move {
            let runtime =
                SshSessionRuntime::connect(profile, session_id, attempt_id, event_tx).await?;
            Ok(Box::new(runtime) as Box<dyn SessionRuntimeControl>)
        })
    }

    fn probe(
        &self,
        profile: ConnectionProfile,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>> {
        Box::pin(async move {
            let (event_tx, _event_rx) = mpsc::unbounded_channel();
            let runtime =
                SshSessionRuntime::connect(profile, Uuid::new_v4(), Uuid::new_v4(), event_tx)
                    .await?;
            runtime.disconnect()?;
            Ok(())
        })
    }
}

impl SessionRuntimeLauncher for TrackingLauncher {
    fn launch(
        &self,
        _profile: ConnectionProfile,
        _session_id: Uuid,
        _attempt_id: Uuid,
        _event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn SessionRuntimeControl>>> + Send + 'static>>
    {
        let disconnects = Arc::clone(&self.disconnects);
        let terminal_releases = Arc::clone(&self.terminal_releases);
        Box::pin(async move {
            Ok(Box::new(TrackingRuntimeControl {
                disconnects,
                terminal_releases,
            }) as Box<dyn SessionRuntimeControl>)
        })
    }

    fn probe(
        &self,
        _profile: ConnectionProfile,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>> {
        Box::pin(async move { Ok(()) })
    }
}

impl SessionRuntimeLauncher for InteractiveTrackingLauncher {
    fn launch(
        &self,
        _profile: ConnectionProfile,
        _session_id: Uuid,
        _attempt_id: Uuid,
        _event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn SessionRuntimeControl>>> + Send + 'static>>
    {
        let state = self.state.clone();
        Box::pin(async move {
            Ok(Box::new(InteractiveTrackingRuntimeControl { state })
                as Box<dyn SessionRuntimeControl>)
        })
    }

    fn probe(
        &self,
        _profile: ConnectionProfile,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>> {
        Box::pin(async move { Ok(()) })
    }
}

impl SessionRuntimeLauncher for DelayedTrackingLauncher {
    fn launch(
        &self,
        _profile: ConnectionProfile,
        _session_id: Uuid,
        _attempt_id: Uuid,
        _event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn SessionRuntimeControl>>> + Send + 'static>>
    {
        let disconnects = Arc::clone(&self.disconnects);
        let terminal_releases = Arc::clone(&self.terminal_releases);
        let ready_delay = self.ready_delay;
        Box::pin(async move {
            tokio::time::sleep(ready_delay).await;
            Ok(Box::new(TrackingRuntimeControl {
                disconnects,
                terminal_releases,
            }) as Box<dyn SessionRuntimeControl>)
        })
    }

    fn probe(
        &self,
        _profile: ConnectionProfile,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>> {
        Box::pin(async move { Ok(()) })
    }
}

impl SessionRuntimeLauncher for DelayedInteractiveTrackingLauncher {
    fn launch(
        &self,
        _profile: ConnectionProfile,
        _session_id: Uuid,
        _attempt_id: Uuid,
        _event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn SessionRuntimeControl>>> + Send + 'static>>
    {
        let state = self.state.clone();
        let ready_delay = self.ready_delay;
        Box::pin(async move {
            tokio::time::sleep(ready_delay).await;
            Ok(Box::new(InteractiveTrackingRuntimeControl { state })
                as Box<dyn SessionRuntimeControl>)
        })
    }

    fn probe(
        &self,
        _profile: ConnectionProfile,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>> {
        Box::pin(async move { Ok(()) })
    }
}

impl SessionRuntimeLauncher for ScrollTrackingLauncher {
    fn launch(
        &self,
        _profile: ConnectionProfile,
        session_id: Uuid,
        _attempt_id: Uuid,
        event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn SessionRuntimeControl>>> + Send + 'static>>
    {
        let state = self.state.clone();
        Box::pin(async move {
            let surface = surface_with_viewport(session_id, 1, 2, 6);
            *state.surface.lock().expect("lock scroll surface") = Some(surface.clone());
            let _ = event_tx.send(SessionRuntimeEvent::Connected);
            let _ = event_tx.send(SessionRuntimeEvent::SurfaceChanged(surface));
            Ok(Box::new(ScrollTrackingRuntimeControl { state }) as Box<dyn SessionRuntimeControl>)
        })
    }

    fn probe(
        &self,
        _profile: ConnectionProfile,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>> {
        Box::pin(async move { Ok(()) })
    }
}

impl SessionRuntimeLauncher for SurfacePullLauncher {
    fn launch(
        &self,
        _profile: ConnectionProfile,
        _session_id: Uuid,
        _attempt_id: Uuid,
        event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn SessionRuntimeControl>>> + Send + 'static>>
    {
        let surface = self.surface.clone();
        Box::pin(async move {
            let _ = event_tx.send(SessionRuntimeEvent::Connected);
            Ok(Box::new(SurfacePullRuntimeControl { surface }) as Box<dyn SessionRuntimeControl>)
        })
    }

    fn probe(
        &self,
        _profile: ConnectionProfile,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>> {
        Box::pin(async move { Ok(()) })
    }
}

impl SessionRuntimeLauncher for ThemeTrackingLauncher {
    fn launch(
        &self,
        _profile: ConnectionProfile,
        session_id: Uuid,
        _attempt_id: Uuid,
        event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn SessionRuntimeControl>>> + Send + 'static>>
    {
        Box::pin(async move {
            let terminal = Arc::new(Mutex::new(TerminalSession::new(24, 80)));
            {
                let mut terminal_guard = terminal.lock().expect("lock theme tracking terminal");
                terminal_guard.apply_remote_bytes(b"\x1b[32mready\x1b[0m");
                let _ = event_tx.send(SessionRuntimeEvent::Connected);
                let _ = event_tx.send(SessionRuntimeEvent::SurfaceChanged(
                    terminal_guard.surface_state(session_id),
                ));
            }

            Ok(Box::new(ThemeTrackingRuntimeControl {
                session_id,
                terminal,
            }) as Box<dyn SessionRuntimeControl>)
        })
    }

    fn probe(
        &self,
        _profile: ConnectionProfile,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>> {
        Box::pin(async move { Ok(()) })
    }
}

impl SessionRuntimeControl for TrackingRuntimeControl {
    fn disconnect(&self) -> Result<()> {
        self.disconnects.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    fn release_terminal_memory(&self) -> Result<()> {
        self.terminal_releases.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    fn send_text_input(&self, _text: String) -> Result<()> {
        Ok(())
    }

    fn send_key_input(&self, _event: TerminalKeyEvent) -> Result<()> {
        Ok(())
    }

    fn send_paste(&self, _text: String) -> Result<()> {
        Ok(())
    }

    fn resize(&self, _rows: u32, _cols: u32) -> Result<()> {
        Ok(())
    }

    fn send_mouse_input(&self, _event: TerminalMouseInput) -> Result<()> {
        Ok(())
    }
}

impl SessionRuntimeControl for InteractiveTrackingRuntimeControl {
    fn disconnect(&self) -> Result<()> {
        Ok(())
    }

    fn send_text_input(&self, text: String) -> Result<()> {
        self.state
            .text_inputs
            .lock()
            .expect("lock text inputs")
            .push(text);
        Ok(())
    }

    fn send_key_input(&self, event: TerminalKeyEvent) -> Result<()> {
        self.state
            .key_inputs
            .lock()
            .expect("lock key inputs")
            .push(event);
        Ok(())
    }

    fn send_paste(&self, text: String) -> Result<()> {
        self.state
            .paste_inputs
            .lock()
            .expect("lock paste inputs")
            .push(text);
        Ok(())
    }

    fn resize(&self, rows: u32, cols: u32) -> Result<()> {
        self.state
            .resizes
            .lock()
            .expect("lock resize events")
            .push((rows, cols));
        Ok(())
    }

    fn send_mouse_input(&self, event: TerminalMouseInput) -> Result<()> {
        self.state
            .mouse_inputs
            .lock()
            .expect("lock mouse events")
            .push(event);
        Ok(())
    }
}

impl SessionRuntimeControl for ScrollTrackingRuntimeControl {
    fn disconnect(&self) -> Result<()> {
        Ok(())
    }

    fn send_text_input(&self, _text: String) -> Result<()> {
        Ok(())
    }

    fn send_key_input(&self, _event: TerminalKeyEvent) -> Result<()> {
        Ok(())
    }

    fn send_paste(&self, _text: String) -> Result<()> {
        Ok(())
    }

    fn resize(&self, _rows: u32, _cols: u32) -> Result<()> {
        Ok(())
    }

    fn send_mouse_input(&self, _event: TerminalMouseInput) -> Result<()> {
        Ok(())
    }

    fn scroll_viewport_lines(&self, delta: i32) -> Result<TerminalSurfaceState> {
        self.state
            .scroll_deltas
            .lock()
            .expect("lock scroll deltas")
            .push(delta);

        let mut surface = self
            .state
            .surface
            .lock()
            .expect("lock scroll surface")
            .clone()
            .expect("current scroll surface");
        let next_offset = (surface.viewport_offset_lines as i32 + delta)
            .clamp(0, surface.viewport_max_offset_lines as i32) as u32;
        surface = surface_with_viewport(
            surface.session_id,
            surface.seqno.saturating_add(1),
            next_offset,
            surface.viewport_max_offset_lines,
        );
        *self.state.surface.lock().expect("lock scroll surface") = Some(surface.clone());
        Ok(surface)
    }
}

impl SessionRuntimeControl for SurfacePullRuntimeControl {
    fn disconnect(&self) -> Result<()> {
        Ok(())
    }

    fn send_text_input(&self, _text: String) -> Result<()> {
        Ok(())
    }

    fn send_key_input(&self, _event: TerminalKeyEvent) -> Result<()> {
        Ok(())
    }

    fn send_paste(&self, _text: String) -> Result<()> {
        Ok(())
    }

    fn resize(&self, _rows: u32, _cols: u32) -> Result<()> {
        Ok(())
    }

    fn send_mouse_input(&self, _event: TerminalMouseInput) -> Result<()> {
        Ok(())
    }

    fn terminal_surface(&self) -> Result<TerminalSurfaceState> {
        Ok(self.surface.clone())
    }
}

impl SessionRuntimeControl for ThemeTrackingRuntimeControl {
    fn disconnect(&self) -> Result<()> {
        Ok(())
    }

    fn send_text_input(&self, _text: String) -> Result<()> {
        Ok(())
    }

    fn send_key_input(&self, _event: TerminalKeyEvent) -> Result<()> {
        Ok(())
    }

    fn send_paste(&self, _text: String) -> Result<()> {
        Ok(())
    }

    fn resize(&self, _rows: u32, _cols: u32) -> Result<()> {
        Ok(())
    }

    fn send_mouse_input(&self, _event: TerminalMouseInput) -> Result<()> {
        Ok(())
    }

    fn terminal_surface(&self) -> Result<TerminalSurfaceState> {
        let terminal = self.terminal.lock().expect("lock theme tracking terminal");
        Ok(terminal.surface_state(self.session_id))
    }

    fn update_theme(
        &self,
        mode: ThemeMode,
        variant: ThemeVariant,
    ) -> Result<Option<TerminalSurfaceState>> {
        let mut terminal = self.terminal.lock().expect("lock theme tracking terminal");
        terminal.set_theme(mode, variant);
        Ok(Some(terminal.surface_state(self.session_id)))
    }
}

impl SftpBackend for NoopSftpBackend {
    fn read_dir<'a>(
        &'a self,
        _path: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<SftpDirectoryEntry>>> + Send + 'a>> {
        Box::pin(async move { Ok(Vec::new()) })
    }

    fn mkdir<'a>(
        &'a self,
        _path: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move { Ok(()) })
    }

    fn rename<'a>(
        &'a self,
        _from: &'a str,
        _to: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move { Ok(()) })
    }

    fn path_exists<'a>(
        &'a self,
        _path: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<bool>> + Send + 'a>> {
        Box::pin(async move { Ok(true) })
    }

    fn stat<'a>(
        &'a self,
        _path: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<SftpRemoteMetadata>> + Send + 'a>> {
        Box::pin(async move { Ok(SftpRemoteMetadata::default()) })
    }

    fn open_file_reader<'a>(
        &'a self,
        _path: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<BoxedSftpReader>> + Send + 'a>> {
        Box::pin(async move { Ok(Box::pin(NoopFileHandle::new()) as BoxedSftpReader) })
    }

    fn open_file_writer<'a>(
        &'a self,
        _path: &'a str,
        _mode: SftpWriteMode,
    ) -> Pin<Box<dyn Future<Output = Result<BoxedSftpWriter>> + Send + 'a>> {
        Box::pin(async move { Ok(Box::pin(NoopFileHandle::new()) as BoxedSftpWriter) })
    }

    fn upload_file<'a>(
        &'a self,
        _remote_path: &'a str,
        data: Vec<u8>,
    ) -> Pin<Box<dyn Future<Output = Result<u64>> + Send + 'a>> {
        Box::pin(async move { Ok(data.len() as u64) })
    }

    fn download_file<'a>(
        &'a self,
        _remote_path: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>>> + Send + 'a>> {
        Box::pin(async move { Ok(Vec::new()) })
    }

    fn remove_file<'a>(
        &'a self,
        _remote_path: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move { Ok(()) })
    }

    fn remove_dir<'a>(
        &'a self,
        _remote_path: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move { Ok(()) })
    }
}

impl SftpCapableLauncher {
    fn new(backend: Arc<dyn SftpBackend>) -> Self {
        Self { backend }
    }
}

impl SessionRuntimeLauncher for SftpCapableLauncher {
    fn launch(
        &self,
        _profile: ConnectionProfile,
        _session_id: Uuid,
        _attempt_id: Uuid,
        event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn SessionRuntimeControl>>> + Send + 'static>>
    {
        let backend = Arc::clone(&self.backend);
        Box::pin(async move {
            let _ = event_tx.send(SessionRuntimeEvent::Connected);
            Ok(Box::new(SftpCapableRuntimeControl {
                runtime: SftpRuntimeHandle::new(backend),
            }) as Box<dyn SessionRuntimeControl>)
        })
    }

    fn probe(
        &self,
        _profile: ConnectionProfile,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>> {
        Box::pin(async move { Ok(()) })
    }
}

impl SessionRuntimeControl for SftpCapableRuntimeControl {
    fn disconnect(&self) -> Result<()> {
        Ok(())
    }

    fn send_text_input(&self, _text: String) -> Result<()> {
        Ok(())
    }

    fn send_key_input(&self, _event: TerminalKeyEvent) -> Result<()> {
        Ok(())
    }

    fn send_mouse_input(&self, _event: TerminalMouseInput) -> Result<()> {
        Ok(())
    }

    fn send_paste(&self, _text: String) -> Result<()> {
        Ok(())
    }

    fn resize(&self, _rows: u32, _cols: u32) -> Result<()> {
        Ok(())
    }

    fn sftp_runtime(&self) -> Option<SftpRuntimeHandle> {
        Some(self.runtime.clone())
    }
}

#[derive(Clone)]
struct InteractiveTestServer {
    auth_key: russh::keys::PublicKey,
    shell_ready_delay: Duration,
    direct_tcpip_behavior: DirectTcpipBehavior,
    shell_integration_behavior: ShellIntegrationServerBehavior,
    state: InteractiveServerState,
}

#[derive(Clone)]
struct ShellIntegrationServerBehavior {
    shell_path: String,
    bootstrap_reply: BootstrapReply,
}

impl Default for ShellIntegrationServerBehavior {
    fn default() -> Self {
        Self {
            shell_path: "/bin/bash".into(),
            bootstrap_reply: BootstrapReply::Accept,
        }
    }
}

#[derive(Clone, Copy, Default)]
enum BootstrapReply {
    #[default]
    Accept,
    Reject,
}

#[derive(Clone, Default)]
struct InteractiveServerState {
    pty_terms: Arc<Mutex<Vec<String>>>,
    environment_requests: Arc<Mutex<Vec<(String, String)>>>,
    request_order: Arc<Mutex<Vec<String>>>,
    direct_tcpip_requests: Arc<Mutex<Vec<(String, u16)>>>,
    exec_requests: Arc<Mutex<Vec<String>>>,
    shell_inputs: Arc<Mutex<Vec<String>>>,
    bootstrap_attempts: Arc<AtomicUsize>,
}

impl InteractiveServerState {
    fn bootstrap_attempts(&self) -> usize {
        self.bootstrap_attempts.load(Ordering::SeqCst)
    }
}

impl server::Server for InteractiveTestServer {
    type Handler = Self;

    fn new_client(&mut self, _: Option<std::net::SocketAddr>) -> Self {
        self.clone()
    }
}

impl server::Handler for InteractiveTestServer {
    type Error = russh::Error;

    async fn auth_publickey(
        &mut self,
        _user: &str,
        public_key: &russh::keys::PublicKey,
    ) -> Result<Auth, Self::Error> {
        if public_key == &self.auth_key {
            Ok(Auth::Accept)
        } else {
            Ok(Auth::reject())
        }
    }

    async fn channel_open_session(
        &mut self,
        _channel: Channel<server::Msg>,
        _session: &mut Session,
    ) -> Result<bool, Self::Error> {
        Ok(true)
    }

    async fn pty_request(
        &mut self,
        channel: ChannelId,
        term: &str,
        _col_width: u32,
        _row_height: u32,
        _pix_width: u32,
        _pix_height: u32,
        _modes: &[(russh::Pty, u32)],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        self.state
            .pty_terms
            .lock()
            .expect("lock pty terms")
            .push(term.to_string());
        self.state
            .request_order
            .lock()
            .expect("lock request order")
            .push("pty".into());
        tokio::time::sleep(self.shell_ready_delay).await;
        let _ = session.channel_success(channel);
        Ok(())
    }

    async fn env_request(
        &mut self,
        channel: ChannelId,
        variable_name: &str,
        variable_value: &str,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        self.state
            .environment_requests
            .lock()
            .expect("lock environment requests")
            .push((variable_name.to_string(), variable_value.to_string()));
        self.state
            .request_order
            .lock()
            .expect("lock request order")
            .push(format!("env:{variable_name}"));
        let _ = session.channel_success(channel);
        Ok(())
    }

    async fn shell_request(
        &mut self,
        channel: ChannelId,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        self.state
            .request_order
            .lock()
            .expect("lock request order")
            .push("shell".into());
        tokio::time::sleep(self.shell_ready_delay).await;
        let _ = session.channel_success(channel);
        session.data(channel, b"welcome to mica-term".to_vec())?;
        Ok(())
    }

    async fn exec_request(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        self.state
            .exec_requests
            .lock()
            .expect("lock exec requests")
            .push(String::from_utf8_lossy(data).into_owned());
        let _ = session.channel_success(channel);
        session.data(
            channel,
            format!("{}\n", self.shell_integration_behavior.shell_path).into_bytes(),
        )?;
        session.eof(channel)?;
        session.close(channel)?;
        Ok(())
    }

    async fn data(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        let text = String::from_utf8_lossy(data).into_owned();
        self.state
            .shell_inputs
            .lock()
            .expect("lock shell inputs")
            .push(text.clone());

        if text.contains("MICA_TERM_ENHANCED=1") {
            self.state.bootstrap_attempts.fetch_add(1, Ordering::SeqCst);
            let ack = match self.shell_integration_behavior.bootstrap_reply {
                BootstrapReply::Accept => BOOTSTRAP_ACK_ACCEPTED,
                BootstrapReply::Reject => BOOTSTRAP_ACK_REJECTED,
            };
            session.data(channel, format!("{ack}\n").into_bytes())?;
        }

        Ok(())
    }

    async fn channel_open_direct_tcpip(
        &mut self,
        channel: Channel<server::Msg>,
        host_to_connect: &str,
        port_to_connect: u32,
        _originator_address: &str,
        _originator_port: u32,
        _session: &mut Session,
    ) -> Result<bool, Self::Error> {
        let requested_port = u16::try_from(port_to_connect).expect("direct-tcpip port fits u16");
        self.state
            .direct_tcpip_requests
            .lock()
            .expect("lock direct-tcpip requests")
            .push((host_to_connect.to_string(), requested_port));

        if self.direct_tcpip_behavior.reject_requests {
            return Ok(false);
        }

        let Some(route) = self
            .direct_tcpip_behavior
            .routes
            .iter()
            .find(|route| {
                route.requested_host == host_to_connect && route.requested_port == requested_port
            })
            .cloned()
        else {
            return Ok(false);
        };

        tokio::spawn(async move {
            let mut channel_stream = channel.into_stream();
            let mut downstream = TcpStream::connect(route.target_addr)
                .await
                .expect("connect downstream direct-tcpip target");
            if let Err(error) = copy_bidirectional(&mut channel_stream, &mut downstream).await {
                if error.kind() != std::io::ErrorKind::BrokenPipe
                    && error.kind() != std::io::ErrorKind::ConnectionReset
                    && error.kind() != std::io::ErrorKind::UnexpectedEof
                {
                    panic!("bridge direct-tcpip channel: {error}");
                }
            }
        });

        Ok(true)
    }
}

fn temp_private_key_path(label: &str) -> std::path::PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "mica-term-runtime-{}-{}-{}.key",
        label,
        std::process::id(),
        Uuid::new_v4()
    ));
    path
}

fn create_publickey_auth_material(label: &str) -> (russh::keys::PublicKey, std::path::PathBuf) {
    let client_key = PrivateKey::random(&mut OsRng, russh::keys::Algorithm::Ed25519)
        .expect("generate client key");
    let client_public = client_key.public_key().clone();
    let private_key_path = temp_private_key_path(label);
    fs::write(
        &private_key_path,
        client_key
            .to_openssh(russh::keys::ssh_key::LineEnding::LF)
            .expect("encode private key"),
    )
    .expect("write client private key");
    (client_public, private_key_path)
}

async fn spawn_publickey_server_with_auth_key(
    auth_key: russh::keys::PublicKey,
    shell_ready_delay: Duration,
    direct_tcpip_behavior: DirectTcpipBehavior,
    shell_integration_behavior: ShellIntegrationServerBehavior,
) -> (
    tokio::task::JoinHandle<()>,
    std::net::SocketAddr,
    russh::keys::PublicKey,
    InteractiveServerState,
) {
    let mut config = server::Config::default();
    config.auth_rejection_time = Duration::from_millis(5);
    config.inactivity_timeout = Some(Duration::from_secs(30));
    let server_key =
        PrivateKey::random(&mut OsRng, russh::keys::Algorithm::Ed25519).expect("server key");
    let server_public = server_key.public_key().clone();
    config.keys.push(server_key);
    let config = Arc::new(config);

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind test ssh server");
    let addr = listener.local_addr().expect("server addr");
    let state = InteractiveServerState::default();
    let server = InteractiveTestServer {
        auth_key,
        shell_ready_delay,
        direct_tcpip_behavior,
        shell_integration_behavior,
        state: state.clone(),
    };

    let join = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.expect("accept ssh client");
        server::run_stream(config, socket, server)
            .await
            .expect("run ssh server");
    });

    (join, addr, server_public, state)
}

async fn spawn_publickey_shell_server(
    shell_ready_delay: Duration,
) -> (
    tokio::task::JoinHandle<()>,
    std::net::SocketAddr,
    std::path::PathBuf,
    russh::keys::PublicKey,
    InteractiveServerState,
) {
    let (client_public, private_key_path) = create_publickey_auth_material("client");
    let (join, addr, server_public, state) = spawn_publickey_server_with_auth_key(
        client_public,
        shell_ready_delay,
        DirectTcpipBehavior::default(),
        ShellIntegrationServerBehavior::default(),
    )
    .await;
    (join, addr, private_key_path, server_public, state)
}

async fn spawn_publickey_shell_server_with_integration(
    shell_ready_delay: Duration,
    shell_path: &str,
    bootstrap_reply: BootstrapReply,
) -> (
    tokio::task::JoinHandle<()>,
    std::net::SocketAddr,
    std::path::PathBuf,
    russh::keys::PublicKey,
    InteractiveServerState,
) {
    let (client_public, private_key_path) = create_publickey_auth_material("client");
    let (join, addr, server_public, state) = spawn_publickey_server_with_auth_key(
        client_public,
        shell_ready_delay,
        DirectTcpipBehavior::default(),
        ShellIntegrationServerBehavior {
            shell_path: shell_path.into(),
            bootstrap_reply,
        },
    )
    .await;
    (join, addr, private_key_path, server_public, state)
}

fn collect_enhancement_states(
    runtime: &AppAsyncRuntime,
    event_rx: &mut mpsc::UnboundedReceiver<SessionRuntimeEvent>,
) -> Vec<EnhancedSessionState> {
    runtime.block_on(async {
        tokio::time::timeout(Duration::from_millis(150), async {
            let mut states = Vec::new();
            while let Some(event) = event_rx.recv().await {
                if let SessionRuntimeEvent::EnhancedSessionStateChanged(state) = event {
                    states.push(state);
                    break;
                }
            }
            states
        })
        .await
        .unwrap_or_default()
    })
}

fn apply_output_and_snapshot(bytes: &[u8]) -> TerminalSurfaceState {
    let session_id = Uuid::new_v4();
    let parsed = runtime_shell_events(bytes);
    let mut terminal = TerminalSession::new(24, 80);
    terminal.apply_remote_bytes(&parsed.sanitized_bytes);
    terminal.surface_state(session_id)
}

async fn spawn_fake_socks5_server(
    expected_target_host: String,
    target_addr: std::net::SocketAddr,
    auth_mode: FakeSocks5AuthMode,
) -> (tokio::task::JoinHandle<()>, std::net::SocketAddr) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind fake socks5 server");
    let addr = listener.local_addr().expect("fake socks5 addr");

    let join = tokio::spawn(async move {
        let (mut client, _) = listener.accept().await.expect("accept socks5 client");

        let greeting_version = client.read_u8().await.expect("read socks5 version");
        assert_eq!(greeting_version, 0x05, "unexpected socks5 greeting version");
        let method_count = client.read_u8().await.expect("read socks5 method count");
        let mut methods = vec![0_u8; method_count as usize];
        client
            .read_exact(&mut methods)
            .await
            .expect("read socks5 methods");

        let selected_method = match &auth_mode {
            FakeSocks5AuthMode::NoAuth => {
                assert!(
                    methods.contains(&0x00),
                    "runtime should advertise no-auth SOCKS5 support"
                );
                0x00
            }
            FakeSocks5AuthMode::UsernamePassword { .. } => {
                assert!(
                    methods.contains(&0x02),
                    "runtime should advertise username/password SOCKS5 support"
                );
                0x02
            }
            FakeSocks5AuthMode::RejectAuthentication => 0xFF,
        };

        client
            .write_all(&[0x05, selected_method])
            .await
            .expect("write socks5 method selection");

        if selected_method == 0xFF {
            return;
        }

        if let FakeSocks5AuthMode::UsernamePassword { username, password } = auth_mode {
            let auth_version = client.read_u8().await.expect("read auth version");
            assert_eq!(
                auth_version, 0x01,
                "unexpected username/password auth version"
            );
            let username_len = client.read_u8().await.expect("read username len");
            let mut username_bytes = vec![0_u8; username_len as usize];
            client
                .read_exact(&mut username_bytes)
                .await
                .expect("read username");
            let password_len = client.read_u8().await.expect("read password len");
            let mut password_bytes = vec![0_u8; password_len as usize];
            client
                .read_exact(&mut password_bytes)
                .await
                .expect("read password");

            let received_username =
                String::from_utf8(username_bytes).expect("decode socks5 username");
            let received_password =
                String::from_utf8(password_bytes).expect("decode socks5 password");
            let status =
                u8::from(!(received_username == username && received_password == password));
            client
                .write_all(&[0x01, status])
                .await
                .expect("write username/password auth reply");
            if status != 0x00 {
                return;
            }
        }

        let request_version = client.read_u8().await.expect("read connect version");
        assert_eq!(request_version, 0x05, "unexpected socks5 request version");
        let command = client.read_u8().await.expect("read connect command");
        assert_eq!(command, 0x01, "unexpected socks5 command");
        let reserved = client.read_u8().await.expect("read reserved byte");
        assert_eq!(reserved, 0x00, "unexpected socks5 reserved byte");
        let address_type = client.read_u8().await.expect("read address type");

        let requested_host = match address_type {
            0x01 => {
                let mut octets = [0_u8; 4];
                client
                    .read_exact(&mut octets)
                    .await
                    .expect("read ipv4 target");
                std::net::Ipv4Addr::from(octets).to_string()
            }
            0x03 => {
                let host_len = client.read_u8().await.expect("read domain len");
                let mut host_bytes = vec![0_u8; host_len as usize];
                client
                    .read_exact(&mut host_bytes)
                    .await
                    .expect("read domain target");
                String::from_utf8(host_bytes).expect("decode domain target")
            }
            0x04 => {
                let mut octets = [0_u8; 16];
                client
                    .read_exact(&mut octets)
                    .await
                    .expect("read ipv6 target");
                std::net::Ipv6Addr::from(octets).to_string()
            }
            other => panic!("unsupported socks5 address type: {other}"),
        };
        let requested_port = client.read_u16().await.expect("read target port");

        if requested_host != expected_target_host || requested_port != target_addr.port() {
            client
                .write_all(&[0x05, 0x04, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
                .await
                .expect("write host unreachable response");
            return;
        }

        let mut upstream = TcpStream::connect(target_addr)
            .await
            .expect("connect target ssh server");
        client
            .write_all(&[0x05, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
            .await
            .expect("write socks5 connect success");
        copy_bidirectional(&mut client, &mut upstream)
            .await
            .expect("bridge socks5 to ssh server");
    });

    (join, addr)
}

async fn spawn_fake_http_connect_proxy(
    expected_target_host: String,
    target_addr: std::net::SocketAddr,
    auth_mode: FakeHttpProxyAuthMode,
) -> (tokio::task::JoinHandle<()>, std::net::SocketAddr) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind fake http proxy");
    let addr = listener.local_addr().expect("fake http proxy addr");

    let join = tokio::spawn(async move {
        let (mut client, _) = listener.accept().await.expect("accept http proxy client");

        let mut request = Vec::new();
        loop {
            let byte = client.read_u8().await.expect("read http proxy byte");
            request.push(byte);
            if request.ends_with(b"\r\n\r\n") {
                break;
            }
        }

        let request_text = String::from_utf8(request).expect("decode http proxy request");
        let mut lines = request_text.split("\r\n");
        let request_line = lines.next().expect("http proxy request line");
        let expected_target = format!(
            "CONNECT {expected_target_host}:{} HTTP/1.1",
            target_addr.port()
        );
        assert_eq!(
            request_line, expected_target,
            "unexpected http connect request line"
        );

        let mut proxy_authorization = None;
        for line in lines {
            if line.is_empty() {
                break;
            }
            let Some((name, value)) = line.split_once(':') else {
                continue;
            };
            if name.eq_ignore_ascii_case("proxy-authorization") {
                proxy_authorization = Some(value.trim().to_string());
            }
        }

        match auth_mode {
            FakeHttpProxyAuthMode::NoAuth => {}
            FakeHttpProxyAuthMode::Basic { username, password } => {
                let expected = format!(
                    "Basic {}",
                    encode_basic_auth_header(username.as_str(), password.as_str())
                );
                assert_eq!(
                    proxy_authorization.as_deref(),
                    Some(expected.as_str()),
                    "runtime should send basic proxy authorization header"
                );
            }
            FakeHttpProxyAuthMode::RejectAuthentication => {
                client
                    .write_all(
                        b"HTTP/1.1 407 Proxy Authentication Required\r\nProxy-Authenticate: Basic realm=\"test\"\r\n\r\n",
                    )
                    .await
                    .expect("write auth rejection");
                return;
            }
        }

        let mut upstream = TcpStream::connect(target_addr)
            .await
            .expect("connect target ssh server");
        client
            .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
            .await
            .expect("write connect success");
        copy_bidirectional(&mut client, &mut upstream)
            .await
            .expect("bridge http proxy to ssh server");
    });

    (join, addr)
}

fn sample_profile(asset_id: &str) -> ConnectionProfile {
    ConnectionProfile {
        asset_id: Some(asset_id.into()),
        name: "Prod Bastion".into(),
        host: "example.com".into(),
        user: "ops".into(),
        port: 22,
        auth_method: SshAuthMethod::Password,
        credential_ref: Some("ssh/password/prod-bastion".into()),
        private_key_path: None,
        password: Some("secret".into()),
        private_key_content: None,
        passphrase: None,
        proxy: ConnectionProxyProfile::None,
        resolved_proxy_hops: Vec::new(),
        remark: "Primary entry point".into(),
    }
}

fn sample_publickey_profile(
    asset_id: &str,
    host: String,
    port: u16,
    private_key_path: String,
) -> ConnectionProfile {
    ConnectionProfile {
        asset_id: Some(asset_id.into()),
        name: "Prod Bastion".into(),
        host,
        user: "ops".into(),
        port,
        auth_method: SshAuthMethod::PrivateKeyPath,
        credential_ref: None,
        private_key_path: Some(private_key_path),
        password: None,
        private_key_content: None,
        passphrase: None,
        proxy: ConnectionProxyProfile::None,
        resolved_proxy_hops: Vec::new(),
        remark: "Primary entry point".into(),
    }
}

fn sample_publickey_profile_via_socks5(
    asset_id: &str,
    target_host: String,
    target_port: u16,
    private_key_path: String,
    proxy_host: String,
    proxy_port: u16,
    proxy_username: Option<String>,
    proxy_password: Option<String>,
) -> ConnectionProfile {
    let mut profile =
        sample_publickey_profile(asset_id, target_host, target_port, private_key_path);
    profile.proxy = ConnectionProxyProfile::Socks5 {
        host: proxy_host.clone(),
        port: proxy_port,
        username: proxy_username.clone(),
        password: proxy_password.clone(),
        credential_ref: None,
    };
    profile.resolved_proxy_hops = vec![ResolvedProxyHop::Socks5 {
        host: proxy_host,
        port: proxy_port,
        username: proxy_username,
        password: proxy_password,
    }];
    profile
}

fn sample_publickey_profile_via_http_proxy(
    asset_id: &str,
    target_host: String,
    target_port: u16,
    private_key_path: String,
    proxy_host: String,
    proxy_port: u16,
    proxy_username: Option<String>,
    proxy_password: Option<String>,
) -> ConnectionProfile {
    let mut profile =
        sample_publickey_profile(asset_id, target_host, target_port, private_key_path);
    profile.proxy = ConnectionProxyProfile::Http {
        host: proxy_host.clone(),
        port: proxy_port,
        username: proxy_username.clone(),
        password: proxy_password.clone(),
        credential_ref: None,
    };
    profile.resolved_proxy_hops = vec![ResolvedProxyHop::Http {
        host: proxy_host,
        port: proxy_port,
        username: proxy_username,
        password: proxy_password,
    }];
    profile
}

fn sample_publickey_profile_with_proxy_hops(
    asset_id: &str,
    host: String,
    port: u16,
    private_key_path: String,
    proxy: ConnectionProxyProfile,
    resolved_proxy_hops: Vec<ResolvedProxyHop>,
) -> ConnectionProfile {
    let mut profile = sample_publickey_profile(asset_id, host, port, private_key_path);
    profile.proxy = proxy;
    profile.resolved_proxy_hops = resolved_proxy_hops;
    profile
}

fn surface_with_viewport(
    session_id: Uuid,
    seqno: usize,
    offset: u32,
    max_offset: u32,
) -> TerminalSurfaceState {
    let mut surface = TerminalSurfaceState::from_visible_lines(
        session_id,
        seqno,
        24,
        80,
        vec![format!("offset {offset}")],
    );
    surface.viewport_offset_lines = offset;
    surface.viewport_max_offset_lines = max_offset;
    surface.viewport_at_bottom = offset == 0;
    surface
}

fn temp_known_hosts_path(label: &str) -> std::path::PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "mica-term-known-hosts-{}-{}-{}.txt",
        label,
        std::process::id(),
        Uuid::new_v4()
    ));
    path
}

fn completed_timeline_steps(events: &[SessionRuntimeEvent]) -> Vec<(String, String)> {
    events
        .iter()
        .filter_map(|event| match event {
            SessionRuntimeEvent::ConnectionProgress(ConnectionProgressEvent::StepUpdated {
                step,
                ..
            }) if step.state == ConnectionStepState::Done => {
                Some((step.step_kind.clone(), step.hop_label.clone()))
            }
            _ => None,
        })
        .collect()
}

fn failed_timeline_step(events: &[SessionRuntimeEvent]) -> Option<(String, String, String)> {
    events.iter().find_map(|event| match event {
        SessionRuntimeEvent::ConnectionProgress(ConnectionProgressEvent::StepUpdated {
            step,
            ..
        }) if step.state == ConnectionStepState::Failed => Some((
            step.step_kind.clone(),
            step.hop_label.clone(),
            step.detail.clone(),
        )),
        _ => None,
    })
}

#[test]
fn session_manager_creates_connecting_session_handle() {
    let runtime = AppAsyncRuntime::new().expect("create app async runtime");
    let manager = SessionManager::new_with_launcher(
        runtime.handle(),
        Arc::new(FakeLauncher::stay_connecting()),
    );

    let handle = manager
        .open_session(
            sample_profile("asset-prod"),
            OpenSessionMode::ActivateExisting,
        )
        .expect("open session");

    assert_eq!(handle.asset_id, "asset-prod");
    assert_eq!(handle.title, "Prod Bastion");
    assert_eq!(handle.subtitle, "ops@example.com:22");
    assert_eq!(handle.state, SessionState::Connecting);
}

#[test]
fn runtime_does_not_leave_private_control_sequences_in_visible_terminal_rows() {
    let surface = apply_output_and_snapshot(
        "\u{1b}]9001;mterm;open;/tmp/readme.md\u{7}\r\nprompt$ ".as_bytes(),
    );

    assert!(
        surface
            .visible_lines
            .iter()
            .all(|line| !line.contains("9001;mterm"))
    );
}

#[test]
fn session_manager_tracks_enhanced_remote_session_state_changes() {
    let runtime = AppAsyncRuntime::new().expect("create app async runtime");
    let manager =
        SessionManager::new_with_launcher(runtime.handle(), Arc::new(EnhancedStateLauncher));

    let handle = manager
        .open_session(sample_profile("asset-prod"), OpenSessionMode::ForceNewTab)
        .expect("open session");

    runtime.block_on(async {
        tokio::time::sleep(Duration::from_millis(25)).await;
    });

    let session = manager
        .session(handle.session_id)
        .expect("session should remain registered");

    assert_eq!(
        session.enhanced_session_state,
        EnhancedSessionState::Enhanced
    );
}

#[test]
fn opening_slow_ssh_session_returns_before_runtime_attaches() {
    let runtime = AppAsyncRuntime::new().expect("create app async runtime");
    let disconnects = Arc::new(AtomicUsize::new(0));
    let manager = SessionManager::new_with_launcher(
        runtime.handle(),
        Arc::new(DelayedTrackingLauncher::new(
            Arc::clone(&disconnects),
            Arc::new(AtomicUsize::new(0)),
            Duration::from_millis(250),
        )),
    );

    let started = Instant::now();
    let handle = manager
        .open_session(
            sample_profile("asset-prod"),
            OpenSessionMode::ActivateExisting,
        )
        .expect("open delayed session");

    assert!(
        started.elapsed() < Duration::from_millis(120),
        "opening a delayed SSH session should return before runtime attachment finishes"
    );
    assert_eq!(handle.state, SessionState::Connecting);
    assert_eq!(
        manager
            .session(handle.session_id)
            .expect("stored delayed session")
            .state,
        SessionState::Connecting
    );
}

#[test]
fn connection_progress_new_session_starts_with_empty_connecting_attempt() {
    let runtime = AppAsyncRuntime::new().expect("create app async runtime");
    let manager = SessionManager::new_with_launcher(
        runtime.handle(),
        Arc::new(FakeLauncher::stay_connecting()),
    );

    let handle = manager
        .open_session(
            sample_profile("asset-prod"),
            OpenSessionMode::ActivateExisting,
        )
        .expect("open session");

    let attempt = manager
        .connection_attempt(handle.session_id)
        .expect("connection progress attempt");
    assert_eq!(attempt.headline, ConnectionHeadlineState::Connecting);
    assert!(attempt.steps.is_empty());
    assert!(attempt.diagnostics.is_empty());
}

#[test]
fn test_connection_probe_does_not_register_workspace_session() {
    let runtime = AppAsyncRuntime::new().expect("create app async runtime");
    let manager = SessionManager::new_with_launcher(
        runtime.handle(),
        Arc::new(FakeLauncher::stay_connecting()),
    );

    manager
        .probe_connection(sample_profile("asset-prod"))
        .expect("probe ssh session runtime");

    assert!(manager.ordered_sessions().is_empty());
}

#[test]
fn session_manager_reuses_existing_session_for_same_asset_by_default() {
    let runtime = AppAsyncRuntime::new().expect("create app async runtime");
    let manager = SessionManager::new_with_launcher(
        runtime.handle(),
        Arc::new(FakeLauncher::stay_connecting()),
    );

    let first = manager
        .open_session(
            sample_profile("asset-prod"),
            OpenSessionMode::ActivateExisting,
        )
        .expect("open first session");
    let second = manager
        .open_session(
            sample_profile("asset-prod"),
            OpenSessionMode::ActivateExisting,
        )
        .expect("open second session");

    assert_eq!(first.session_id, second.session_id);
}

#[test]
fn reopening_same_saved_asset_activates_existing_session_by_default() {
    let runtime = AppAsyncRuntime::new().expect("create app async runtime");
    let manager = SessionManager::new_with_launcher(
        runtime.handle(),
        Arc::new(FakeLauncher::stay_connecting()),
    );

    let first = manager
        .open_session(
            sample_profile("asset-prod"),
            OpenSessionMode::ActivateExisting,
        )
        .expect("open first session");
    let reopened = manager
        .open_session(
            sample_profile("asset-prod"),
            OpenSessionMode::ActivateExisting,
        )
        .expect("reopen existing session");

    assert_eq!(reopened.session_id, first.session_id);
}

#[test]
fn session_manager_can_force_new_tab_session_for_same_asset() {
    let runtime = AppAsyncRuntime::new().expect("create app async runtime");
    let manager = SessionManager::new_with_launcher(
        runtime.handle(),
        Arc::new(FakeLauncher::stay_connecting()),
    );

    let first = manager
        .open_session(
            sample_profile("asset-prod"),
            OpenSessionMode::ActivateExisting,
        )
        .expect("open first session");
    let second = manager
        .open_session(sample_profile("asset-prod"), OpenSessionMode::ForceNewTab)
        .expect("force second session");

    assert_ne!(first.session_id, second.session_id);
}

#[test]
fn force_new_tab_duplicate_sessions_receive_incrementing_display_titles() {
    let runtime = AppAsyncRuntime::new().expect("create app async runtime");
    let manager = SessionManager::new_with_launcher(
        runtime.handle(),
        Arc::new(FakeLauncher::stay_connecting()),
    );

    let first = manager
        .open_session(
            sample_profile("asset-prod"),
            OpenSessionMode::ActivateExisting,
        )
        .expect("open first session");
    let second = manager
        .open_session(sample_profile("asset-prod"), OpenSessionMode::ForceNewTab)
        .expect("open second session");
    let third = manager
        .open_session(sample_profile("asset-prod"), OpenSessionMode::ForceNewTab)
        .expect("open third session");

    assert_eq!(first.title, "Prod Bastion");
    assert_eq!(second.title, "Prod Bastion(2)");
    assert_eq!(third.title, "Prod Bastion(3)");
}

#[test]
fn closing_a_duplicate_session_reuses_the_smallest_available_title_suffix() {
    let runtime = AppAsyncRuntime::new().expect("create app async runtime");
    let manager = SessionManager::new_with_launcher(
        runtime.handle(),
        Arc::new(FakeLauncher::stay_connecting()),
    );

    let first = manager
        .open_session(
            sample_profile("asset-prod"),
            OpenSessionMode::ActivateExisting,
        )
        .expect("open first session");
    let second = manager
        .open_session(sample_profile("asset-prod"), OpenSessionMode::ForceNewTab)
        .expect("open second session");
    let third = manager
        .open_session(sample_profile("asset-prod"), OpenSessionMode::ForceNewTab)
        .expect("open third session");

    assert_eq!(first.title, "Prod Bastion");
    assert_eq!(second.title, "Prod Bastion(2)");
    assert_eq!(third.title, "Prod Bastion(3)");

    manager
        .close_session(second.session_id)
        .expect("close second duplicate session");

    let reopened = manager
        .open_session(sample_profile("asset-prod"), OpenSessionMode::ForceNewTab)
        .expect("reopen duplicate session");

    assert_eq!(third.title, "Prod Bastion(3)");
    assert_eq!(reopened.title, "Prod Bastion(2)");
}

#[test]
fn session_manager_exposes_sftp_binding_for_runtime_ready_session() {
    let runtime = AppAsyncRuntime::new().expect("create app async runtime");
    let manager = SessionManager::new_with_launcher(
        runtime.handle(),
        Arc::new(SftpCapableLauncher::new(Arc::new(NoopSftpBackend))),
    );

    let handle = manager
        .open_session(
            sample_profile("asset-prod"),
            OpenSessionMode::ActivateExisting,
        )
        .expect("open session");

    runtime.block_on(async {
        tokio::time::sleep(Duration::from_millis(25)).await;
    });

    let binding = manager
        .sftp_binding(handle.session_id)
        .expect("sftp binding should exist for runtime-ready session");

    assert_eq!(binding.session_id(), handle.session_id);
    assert_eq!(
        binding.mode(),
        mica_term::app::sftp::SftpPanelMode::Connecting
    );
    assert!(binding.runtime().is_some());
}

#[test]
fn retry_session_replaces_sftp_binding_and_disconnect_marks_it_recoverable() {
    let runtime = AppAsyncRuntime::new().expect("create app async runtime");
    let manager = SessionManager::new_with_launcher(
        runtime.handle(),
        Arc::new(SftpCapableLauncher::new(Arc::new(NoopSftpBackend))),
    );

    let handle = manager
        .open_session(
            sample_profile("asset-prod"),
            OpenSessionMode::ActivateExisting,
        )
        .expect("open session");

    runtime.block_on(async {
        tokio::time::sleep(Duration::from_millis(25)).await;
    });

    let first_binding = manager
        .sftp_binding(handle.session_id)
        .expect("initial sftp binding");
    let first_id = first_binding.binding_id();

    manager
        .retry_session(handle.session_id)
        .expect("retry session should succeed");

    runtime.block_on(async {
        tokio::time::sleep(Duration::from_millis(25)).await;
    });

    let second_binding = manager
        .sftp_binding(handle.session_id)
        .expect("replacement sftp binding");
    assert_ne!(first_id, second_binding.binding_id());

    manager
        .disconnect_session(handle.session_id)
        .expect("disconnect session");

    let disconnected_binding = manager
        .sftp_binding(handle.session_id)
        .expect("disconnected binding snapshot");
    assert_eq!(
        disconnected_binding.mode(),
        mica_term::app::sftp::SftpPanelMode::Disconnected
    );
    assert!(disconnected_binding.runtime().is_none());
}

#[test]
fn force_new_tab_creates_parallel_session_for_same_asset() {
    let runtime = AppAsyncRuntime::new().expect("create app async runtime");
    let manager = SessionManager::new_with_launcher(
        runtime.handle(),
        Arc::new(FakeLauncher::stay_connecting()),
    );

    let first = manager
        .open_session(
            sample_profile("asset-prod"),
            OpenSessionMode::ActivateExisting,
        )
        .expect("open first session");
    let second = manager
        .open_session(sample_profile("asset-prod"), OpenSessionMode::ForceNewTab)
        .expect("force second session");

    assert_ne!(first.session_id, second.session_id);
}

#[test]
fn session_manager_marks_session_as_error_when_runtime_fails() {
    let runtime = AppAsyncRuntime::new().expect("create app async runtime");
    let manager = SessionManager::new_with_launcher(
        runtime.handle(),
        Arc::new(FakeLauncher::fail_immediately()),
    );

    let handle = manager
        .open_session(
            sample_profile("asset-prod"),
            OpenSessionMode::ActivateExisting,
        )
        .expect("open session");

    runtime.block_on(async {
        tokio::time::sleep(Duration::from_millis(50)).await;
    });

    let updated = manager
        .session(handle.session_id)
        .expect("resolve failed session");

    assert_eq!(
        updated.state,
        SessionState::Error("authentication failed".into())
    );
    assert!(updated.can_reconnect);
}

#[test]
fn session_manager_marks_connected_only_after_runtime_connected_event() {
    let _env_lock = KNOWN_HOSTS_ENV_LOCK.lock().expect("lock known_hosts env");
    let runtime = AppAsyncRuntime::new().expect("create app async runtime");
    let (server_task, addr, private_key_path, server_public_key, _server_state) =
        runtime.block_on(async { spawn_publickey_shell_server(Duration::from_millis(75)).await });
    let known_hosts_path = temp_known_hosts_path("runtime-ready");
    let known_hosts = KnownHostsService::new(&known_hosts_path);
    known_hosts
        .accept_unknown(
            addr.ip().to_string().as_str(),
            addr.port(),
            &server_public_key,
        )
        .expect("trust test server host key");
    unsafe {
        std::env::set_var("MICA_TERM_KNOWN_HOSTS_PATH", &known_hosts_path);
    }
    let manager =
        SessionManager::new_with_launcher(runtime.handle(), Arc::new(RuntimeBackedLauncher));

    let handle = manager
        .open_session(
            sample_publickey_profile(
                "asset-prod",
                addr.ip().to_string(),
                addr.port(),
                private_key_path.display().to_string(),
            ),
            OpenSessionMode::ActivateExisting,
        )
        .expect("open session");

    runtime.block_on(async {
        tokio::time::sleep(Duration::from_millis(25)).await;
    });
    let before_ready = manager
        .session(handle.session_id)
        .expect("resolve in-flight session");
    assert_eq!(before_ready.state, SessionState::Connecting);

    runtime.block_on(async {
        tokio::time::sleep(Duration::from_millis(200)).await;
    });
    let after_ready = manager
        .session(handle.session_id)
        .expect("resolve connected session");
    assert_eq!(after_ready.state, SessionState::Connected);

    runtime.block_on(async {
        server_task.abort();
    });
    let _ = fs::remove_file(private_key_path);
    let _ = fs::remove_file(known_hosts_path);
    unsafe {
        std::env::remove_var("MICA_TERM_KNOWN_HOSTS_PATH");
    }
}

#[test]
fn runtime_probe_surfaces_unknown_host_key_as_typed_error() {
    let _env_lock = KNOWN_HOSTS_ENV_LOCK.lock().expect("lock known_hosts env");
    let runtime = AppAsyncRuntime::new().expect("create app async runtime");
    let (server_task, addr, private_key_path, _server_public_key, _server_state) =
        runtime.block_on(async { spawn_publickey_shell_server(Duration::from_millis(10)).await });
    let known_hosts_path = temp_known_hosts_path("runtime-unknown");
    let _ = fs::remove_file(&known_hosts_path);
    unsafe {
        std::env::set_var("MICA_TERM_KNOWN_HOSTS_PATH", &known_hosts_path);
    }
    let manager =
        SessionManager::new_with_launcher(runtime.handle(), Arc::new(RuntimeBackedLauncher));

    let err = manager
        .probe_connection(sample_publickey_profile(
            "asset-prod",
            addr.ip().to_string(),
            addr.port(),
            private_key_path.display().to_string(),
        ))
        .expect_err("unknown host key should block probe");

    let typed = err
        .downcast_ref::<UnknownHostKeyError>()
        .expect("typed unknown host key error");
    assert_eq!(typed.host, addr.ip().to_string());
    assert_eq!(typed.port, addr.port());
    assert!(!typed.fingerprint.is_empty());

    runtime.block_on(async {
        server_task.abort();
    });
    let _ = fs::remove_file(private_key_path);
    let _ = fs::remove_file(known_hosts_path);
    unsafe {
        std::env::remove_var("MICA_TERM_KNOWN_HOSTS_PATH");
    }
}

#[test]
fn ssh_runtime_negotiates_truecolor_environment_before_requesting_shell() {
    let _env_lock = KNOWN_HOSTS_ENV_LOCK.lock().expect("lock known_hosts env");
    let runtime = AppAsyncRuntime::new().expect("create app async runtime");
    let (server_task, addr, private_key_path, server_public_key, server_state) =
        runtime.block_on(async { spawn_publickey_shell_server(Duration::from_millis(10)).await });
    let known_hosts_path = temp_known_hosts_path("truecolor-env");
    let known_hosts = KnownHostsService::new(&known_hosts_path);
    known_hosts
        .accept_unknown(
            addr.ip().to_string().as_str(),
            addr.port(),
            &server_public_key,
        )
        .expect("trust test server host key");
    unsafe {
        std::env::set_var("MICA_TERM_KNOWN_HOSTS_PATH", &known_hosts_path);
    }

    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    let runtime_handle = runtime.block_on(async {
        SshSessionRuntime::connect(
            sample_publickey_profile(
                "asset-prod",
                addr.ip().to_string(),
                addr.port(),
                private_key_path.display().to_string(),
            ),
            Uuid::new_v4(),
            Uuid::new_v4(),
            event_tx,
        )
        .await
        .expect("connect ssh runtime")
    });

    let saw_connected = runtime.block_on(async {
        tokio::time::timeout(Duration::from_secs(1), async {
            while let Some(event) = event_rx.recv().await {
                if matches!(event, SessionRuntimeEvent::Connected) {
                    return true;
                }
            }
            false
        })
        .await
        .expect("wait for connected event")
    });

    assert!(
        saw_connected,
        "runtime should emit Connected after shell bootstrap"
    );
    runtime_handle.disconnect().expect("disconnect runtime");
    runtime.block_on(async {
        tokio::time::sleep(Duration::from_millis(20)).await;
        server_task.await.expect("join test ssh server");
    });

    let expected_environment = negotiated_terminal_environment()
        .into_iter()
        .map(|(name, value)| (name.to_string(), value.to_string()))
        .collect::<Vec<_>>();
    let recorded_environment = server_state
        .environment_requests
        .lock()
        .expect("lock environment requests")
        .clone();
    let request_order = server_state
        .request_order
        .lock()
        .expect("lock request order")
        .clone();
    let pty_terms = server_state
        .pty_terms
        .lock()
        .expect("lock pty terms")
        .clone();

    assert_eq!(pty_terms, vec!["xterm-256color".to_string()]);
    assert_eq!(recorded_environment, expected_environment);
    assert_eq!(
        request_order,
        vec![
            "pty".to_string(),
            "env:COLORTERM".to_string(),
            "shell".to_string(),
        ]
    );

    unsafe {
        std::env::remove_var("MICA_TERM_KNOWN_HOSTS_PATH");
    }
    let _ = fs::remove_file(&private_key_path);
    let _ = fs::remove_file(&known_hosts_path);
}

#[test]
fn ssh_runtime_does_not_auto_bootstrap_supported_shells() {
    let _env_lock = KNOWN_HOSTS_ENV_LOCK.lock().expect("lock known_hosts env");
    let runtime = AppAsyncRuntime::new().expect("create app async runtime");
    let (server_task, addr, private_key_path, server_public_key, server_state) =
        runtime.block_on(async {
            spawn_publickey_shell_server_with_integration(
                Duration::from_millis(10),
                "/bin/bash",
                BootstrapReply::Accept,
            )
            .await
        });
    let known_hosts_path = temp_known_hosts_path("enhanced-shell-bootstrap-once");
    let known_hosts = KnownHostsService::new(&known_hosts_path);
    known_hosts
        .accept_unknown(
            addr.ip().to_string().as_str(),
            addr.port(),
            &server_public_key,
        )
        .expect("trust test server host key");
    unsafe {
        std::env::set_var("MICA_TERM_KNOWN_HOSTS_PATH", &known_hosts_path);
    }

    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    let runtime_handle = runtime.block_on(async {
        SshSessionRuntime::connect(
            sample_publickey_profile(
                "asset-prod",
                addr.ip().to_string(),
                addr.port(),
                private_key_path.display().to_string(),
            ),
            Uuid::new_v4(),
            Uuid::new_v4(),
            event_tx,
        )
        .await
        .expect("connect runtime")
    });

    let states = collect_enhancement_states(&runtime, &mut event_rx);

    assert!(
        states.is_empty(),
        "runtime should not emit enhancement state changes when auto bootstrap is disabled"
    );
    assert_eq!(server_state.bootstrap_attempts(), 0);

    runtime_handle.disconnect().expect("disconnect runtime");
    runtime.block_on(async {
        tokio::time::sleep(Duration::from_millis(20)).await;
        server_task.await.expect("join test ssh server");
    });
    unsafe {
        std::env::remove_var("MICA_TERM_KNOWN_HOSTS_PATH");
    }
    let _ = fs::remove_file(&private_key_path);
    let _ = fs::remove_file(&known_hosts_path);
}

#[test]
fn ssh_runtime_does_not_attempt_bootstrap_even_when_server_would_reject_it() {
    let _env_lock = KNOWN_HOSTS_ENV_LOCK.lock().expect("lock known_hosts env");
    let runtime = AppAsyncRuntime::new().expect("create app async runtime");
    let (server_task, addr, private_key_path, server_public_key, server_state) =
        runtime.block_on(async {
            spawn_publickey_shell_server_with_integration(
                Duration::from_millis(10),
                "/bin/bash",
                BootstrapReply::Reject,
            )
            .await
        });
    let known_hosts_path = temp_known_hosts_path("enhanced-shell-bootstrap-fallback");
    let known_hosts = KnownHostsService::new(&known_hosts_path);
    known_hosts
        .accept_unknown(
            addr.ip().to_string().as_str(),
            addr.port(),
            &server_public_key,
        )
        .expect("trust test server host key");
    unsafe {
        std::env::set_var("MICA_TERM_KNOWN_HOSTS_PATH", &known_hosts_path);
    }

    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    let runtime_handle = runtime.block_on(async {
        SshSessionRuntime::connect(
            sample_publickey_profile(
                "asset-prod",
                addr.ip().to_string(),
                addr.port(),
                private_key_path.display().to_string(),
            ),
            Uuid::new_v4(),
            Uuid::new_v4(),
            event_tx,
        )
        .await
        .expect("connect runtime")
    });

    let states = collect_enhancement_states(&runtime, &mut event_rx);

    assert!(
        states.is_empty(),
        "runtime should not emit fallback enhancement state when no bootstrap is attempted"
    );
    assert_eq!(server_state.bootstrap_attempts(), 0);

    runtime_handle.disconnect().expect("disconnect runtime");
    runtime.block_on(async {
        tokio::time::sleep(Duration::from_millis(20)).await;
        server_task.await.expect("join test ssh server");
    });
    unsafe {
        std::env::remove_var("MICA_TERM_KNOWN_HOSTS_PATH");
    }
    let _ = fs::remove_file(&private_key_path);
    let _ = fs::remove_file(&known_hosts_path);
}

#[test]
fn ssh_runtime_connects_through_unauthenticated_socks5_proxy() {
    let _env_lock = lock_known_hosts_env();
    let runtime = AppAsyncRuntime::new().expect("create app async runtime");
    let (server_task, server_addr, private_key_path, server_public_key, _server_state) =
        runtime.block_on(async { spawn_publickey_shell_server(Duration::from_millis(10)).await });
    let target_host = "ssh.internal.test".to_string();
    let (socks_task, socks_addr) = runtime.block_on(async {
        spawn_fake_socks5_server(target_host.clone(), server_addr, FakeSocks5AuthMode::NoAuth).await
    });
    let known_hosts_path = temp_known_hosts_path("socks5-no-auth");
    let known_hosts = KnownHostsService::new(&known_hosts_path);
    known_hosts
        .accept_unknown(target_host.as_str(), server_addr.port(), &server_public_key)
        .expect("trust test server host key");
    unsafe {
        std::env::set_var("MICA_TERM_KNOWN_HOSTS_PATH", &known_hosts_path);
    }

    let profile = sample_publickey_profile_via_socks5(
        "asset-prod",
        target_host.clone(),
        server_addr.port(),
        private_key_path.display().to_string(),
        socks_addr.ip().to_string(),
        socks_addr.port(),
        None,
        None,
    );
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    let runtime_handle = runtime.block_on(async {
        SshSessionRuntime::connect(profile, Uuid::new_v4(), Uuid::new_v4(), event_tx)
            .await
            .expect("connect ssh runtime through unauthenticated socks5 proxy")
    });

    let saw_connected = runtime.block_on(async {
        tokio::time::timeout(Duration::from_secs(1), async {
            while let Some(event) = event_rx.recv().await {
                if matches!(event, SessionRuntimeEvent::Connected) {
                    return true;
                }
            }
            false
        })
        .await
        .expect("wait for connected event")
    });

    assert!(
        saw_connected,
        "runtime should emit Connected after connecting through SOCKS5"
    );
    runtime_handle.disconnect().expect("disconnect runtime");
    runtime.block_on(async {
        tokio::time::sleep(Duration::from_millis(20)).await;
        socks_task.await.expect("join fake socks5 server");
        server_task.await.expect("join test ssh server");
    });

    unsafe {
        std::env::remove_var("MICA_TERM_KNOWN_HOSTS_PATH");
    }
    let _ = fs::remove_file(&private_key_path);
    let _ = fs::remove_file(&known_hosts_path);
}

#[test]
fn ssh_runtime_connects_through_username_password_socks5_proxy() {
    let _env_lock = lock_known_hosts_env();
    let runtime = AppAsyncRuntime::new().expect("create app async runtime");
    let (server_task, server_addr, private_key_path, server_public_key, _server_state) =
        runtime.block_on(async { spawn_publickey_shell_server(Duration::from_millis(10)).await });
    let target_host = "ssh.internal.test".to_string();
    let proxy_username = "relay".to_string();
    let proxy_password = "secret-pass".to_string();
    let (socks_task, socks_addr) = runtime.block_on(async {
        spawn_fake_socks5_server(
            target_host.clone(),
            server_addr,
            FakeSocks5AuthMode::UsernamePassword {
                username: proxy_username.clone(),
                password: proxy_password.clone(),
            },
        )
        .await
    });
    let known_hosts_path = temp_known_hosts_path("socks5-password");
    let known_hosts = KnownHostsService::new(&known_hosts_path);
    known_hosts
        .accept_unknown(target_host.as_str(), server_addr.port(), &server_public_key)
        .expect("trust test server host key");
    unsafe {
        std::env::set_var("MICA_TERM_KNOWN_HOSTS_PATH", &known_hosts_path);
    }

    let profile = sample_publickey_profile_via_socks5(
        "asset-prod",
        target_host.clone(),
        server_addr.port(),
        private_key_path.display().to_string(),
        socks_addr.ip().to_string(),
        socks_addr.port(),
        Some(proxy_username),
        Some(proxy_password),
    );
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    let runtime_handle = runtime.block_on(async {
        SshSessionRuntime::connect(profile, Uuid::new_v4(), Uuid::new_v4(), event_tx)
            .await
            .expect("connect ssh runtime through username/password socks5 proxy")
    });

    let saw_connected = runtime.block_on(async {
        tokio::time::timeout(Duration::from_secs(1), async {
            while let Some(event) = event_rx.recv().await {
                if matches!(event, SessionRuntimeEvent::Connected) {
                    return true;
                }
            }
            false
        })
        .await
        .expect("wait for connected event")
    });

    assert!(
        saw_connected,
        "runtime should emit Connected after username/password SOCKS5 auth"
    );
    runtime_handle.disconnect().expect("disconnect runtime");
    runtime.block_on(async {
        tokio::time::sleep(Duration::from_millis(20)).await;
        socks_task.await.expect("join fake socks5 server");
        server_task.await.expect("join test ssh server");
    });

    unsafe {
        std::env::remove_var("MICA_TERM_KNOWN_HOSTS_PATH");
    }
    let _ = fs::remove_file(&private_key_path);
    let _ = fs::remove_file(&known_hosts_path);
}

#[test]
fn ssh_runtime_connects_through_http_connect_proxy() {
    let _env_lock = lock_known_hosts_env();
    let runtime = AppAsyncRuntime::new().expect("create app async runtime");
    let (server_task, server_addr, private_key_path, server_public_key, _server_state) =
        runtime.block_on(async { spawn_publickey_shell_server(Duration::from_millis(10)).await });
    let target_host = "ssh-http.internal.test".to_string();
    let (proxy_task, proxy_addr) = runtime.block_on(async {
        spawn_fake_http_connect_proxy(
            target_host.clone(),
            server_addr,
            FakeHttpProxyAuthMode::NoAuth,
        )
        .await
    });
    let known_hosts_path = temp_known_hosts_path("http-connect-no-auth");
    let known_hosts = KnownHostsService::new(&known_hosts_path);
    known_hosts
        .accept_unknown(target_host.as_str(), server_addr.port(), &server_public_key)
        .expect("trust test server host key");
    unsafe {
        std::env::set_var("MICA_TERM_KNOWN_HOSTS_PATH", &known_hosts_path);
    }

    let profile = sample_publickey_profile_via_http_proxy(
        "asset-http",
        target_host,
        server_addr.port(),
        private_key_path.display().to_string(),
        proxy_addr.ip().to_string(),
        proxy_addr.port(),
        None,
        None,
    );
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    let runtime_handle = runtime.block_on(async {
        SshSessionRuntime::connect(profile, Uuid::new_v4(), Uuid::new_v4(), event_tx)
            .await
            .expect("connect ssh runtime through http proxy")
    });

    let saw_connected = runtime.block_on(async {
        tokio::time::timeout(Duration::from_secs(1), async {
            while let Some(event) = event_rx.recv().await {
                if matches!(event, SessionRuntimeEvent::Connected) {
                    return true;
                }
            }
            false
        })
        .await
        .expect("wait for connected event")
    });

    assert!(
        saw_connected,
        "runtime should emit Connected after connecting through HTTP proxy"
    );
    runtime_handle.disconnect().expect("disconnect runtime");
    runtime.block_on(async {
        tokio::time::sleep(Duration::from_millis(20)).await;
        proxy_task.await.expect("join fake http proxy");
        server_task.await.expect("join fake ssh server");
    });
    unsafe {
        std::env::remove_var("MICA_TERM_KNOWN_HOSTS_PATH");
    }
    let _ = fs::remove_file(&private_key_path);
    let _ = fs::remove_file(&known_hosts_path);
}

#[test]
fn ssh_runtime_connects_through_http_connect_proxy_with_basic_auth() {
    let _env_lock = lock_known_hosts_env();
    let runtime = AppAsyncRuntime::new().expect("create app async runtime");
    let (server_task, server_addr, private_key_path, server_public_key, _server_state) =
        runtime.block_on(async { spawn_publickey_shell_server(Duration::from_millis(10)).await });
    let target_host = "ssh-http.internal.test".to_string();
    let proxy_username = "ops-proxy".to_string();
    let proxy_password = "secret-pass".to_string();
    let (proxy_task, proxy_addr) = runtime.block_on(async {
        spawn_fake_http_connect_proxy(
            target_host.clone(),
            server_addr,
            FakeHttpProxyAuthMode::Basic {
                username: proxy_username.clone(),
                password: proxy_password.clone(),
            },
        )
        .await
    });
    let known_hosts_path = temp_known_hosts_path("http-connect-basic-auth");
    let known_hosts = KnownHostsService::new(&known_hosts_path);
    known_hosts
        .accept_unknown(target_host.as_str(), server_addr.port(), &server_public_key)
        .expect("trust test server host key");
    unsafe {
        std::env::set_var("MICA_TERM_KNOWN_HOSTS_PATH", &known_hosts_path);
    }

    let profile = sample_publickey_profile_via_http_proxy(
        "asset-http",
        target_host,
        server_addr.port(),
        private_key_path.display().to_string(),
        proxy_addr.ip().to_string(),
        proxy_addr.port(),
        Some(proxy_username),
        Some(proxy_password),
    );
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    let runtime_handle = runtime.block_on(async {
        SshSessionRuntime::connect(profile, Uuid::new_v4(), Uuid::new_v4(), event_tx)
            .await
            .expect("connect ssh runtime through authenticated http proxy")
    });

    let saw_connected = runtime.block_on(async {
        tokio::time::timeout(Duration::from_secs(1), async {
            while let Some(event) = event_rx.recv().await {
                if matches!(event, SessionRuntimeEvent::Connected) {
                    return true;
                }
            }
            false
        })
        .await
        .expect("wait for connected event")
    });

    assert!(
        saw_connected,
        "runtime should emit Connected after authenticated HTTP proxy connection"
    );
    runtime_handle.disconnect().expect("disconnect runtime");
    runtime.block_on(async {
        tokio::time::sleep(Duration::from_millis(20)).await;
        proxy_task.await.expect("join fake http proxy");
        server_task.await.expect("join fake ssh server");
    });
    unsafe {
        std::env::remove_var("MICA_TERM_KNOWN_HOSTS_PATH");
    }
    let _ = fs::remove_file(&private_key_path);
    let _ = fs::remove_file(&known_hosts_path);
}

#[test]
fn ssh_runtime_surfaces_http_proxy_authentication_rejection() {
    let _env_lock = lock_known_hosts_env();
    let runtime = AppAsyncRuntime::new().expect("create app async runtime");
    let (server_task, server_addr, private_key_path, server_public_key, _server_state) =
        runtime.block_on(async { spawn_publickey_shell_server(Duration::from_millis(10)).await });
    let target_host = "ssh-http.internal.test".to_string();
    let (proxy_task, proxy_addr) = runtime.block_on(async {
        spawn_fake_http_connect_proxy(
            target_host.clone(),
            server_addr,
            FakeHttpProxyAuthMode::RejectAuthentication,
        )
        .await
    });
    let known_hosts_path = temp_known_hosts_path("http-connect-reject");
    let known_hosts = KnownHostsService::new(&known_hosts_path);
    known_hosts
        .accept_unknown(target_host.as_str(), server_addr.port(), &server_public_key)
        .expect("trust test server host key");
    unsafe {
        std::env::set_var("MICA_TERM_KNOWN_HOSTS_PATH", &known_hosts_path);
    }

    let result = runtime.block_on(async {
        SshSessionRuntime::connect(
            sample_publickey_profile_via_http_proxy(
                "asset-http",
                target_host,
                server_addr.port(),
                private_key_path.display().to_string(),
                proxy_addr.ip().to_string(),
                proxy_addr.port(),
                Some("ops-proxy".into()),
                Some("bad-secret".into()),
            ),
            Uuid::new_v4(),
            Uuid::new_v4(),
            mpsc::unbounded_channel().0,
        )
        .await
    });
    let error = match result {
        Ok(_) => panic!("http proxy auth rejection should surface"),
        Err(error) => error,
    };

    assert!(format!("{error:#}").contains("HTTP CONNECT request failed with status: 407"));
    runtime.block_on(async {
        proxy_task.await.expect("join fake http proxy");
        server_task.abort();
    });
    unsafe {
        std::env::remove_var("MICA_TERM_KNOWN_HOSTS_PATH");
    }
    let _ = fs::remove_file(&private_key_path);
    let _ = fs::remove_file(&known_hosts_path);
}

#[test]
fn ssh_runtime_surfaces_socks5_authentication_rejection() {
    let _env_lock = lock_known_hosts_env();
    let runtime = AppAsyncRuntime::new().expect("create app async runtime");
    let (server_task, server_addr, private_key_path, server_public_key, _server_state) =
        runtime.block_on(async { spawn_publickey_shell_server(Duration::from_millis(10)).await });
    let target_host = "ssh.internal.test".to_string();
    let (socks_task, socks_addr) = runtime.block_on(async {
        spawn_fake_socks5_server(
            target_host.clone(),
            server_addr,
            FakeSocks5AuthMode::RejectAuthentication,
        )
        .await
    });
    let known_hosts_path = temp_known_hosts_path("socks5-reject");
    let known_hosts = KnownHostsService::new(&known_hosts_path);
    known_hosts
        .accept_unknown(target_host.as_str(), server_addr.port(), &server_public_key)
        .expect("trust test server host key");
    unsafe {
        std::env::set_var("MICA_TERM_KNOWN_HOSTS_PATH", &known_hosts_path);
    }

    let err = runtime.block_on(async {
        let (event_tx, _event_rx) = mpsc::unbounded_channel();
        match SshSessionRuntime::connect(
            sample_publickey_profile_via_socks5(
                "asset-prod",
                target_host,
                server_addr.port(),
                private_key_path.display().to_string(),
                socks_addr.ip().to_string(),
                socks_addr.port(),
                Some("relay".into()),
                Some("secret-pass".into()),
            ),
            Uuid::new_v4(),
            Uuid::new_v4(),
            event_tx,
        )
        .await
        {
            Ok(_) => panic!("SOCKS5 auth rejection should fail runtime connect"),
            Err(err) => err,
        }
    });

    let message = err.to_string();
    assert!(
        message.contains("failed to negotiate SOCKS5 proxy"),
        "expected SOCKS5 proxy error, got: {message}"
    );

    runtime.block_on(async {
        socks_task.await.expect("join fake socks5 server");
        server_task.abort();
    });

    unsafe {
        std::env::remove_var("MICA_TERM_KNOWN_HOSTS_PATH");
    }
    let _ = fs::remove_file(&private_key_path);
    let _ = fs::remove_file(&known_hosts_path);
}

#[test]
fn ssh_runtime_connects_through_single_direct_tcpip_upstream() {
    let _env_lock = lock_known_hosts_env();
    let runtime = AppAsyncRuntime::new().expect("create app async runtime");
    let (client_public, private_key_path) = create_publickey_auth_material("direct-tcpip");
    let (target_task, target_addr, target_public_key, _target_state) = runtime.block_on(async {
        spawn_publickey_server_with_auth_key(
            client_public.clone(),
            Duration::from_millis(10),
            DirectTcpipBehavior::default(),
            ShellIntegrationServerBehavior::default(),
        )
        .await
    });
    let target_host = "ssh.internal.test".to_string();
    let (upstream_task, upstream_addr, upstream_public_key, upstream_state) =
        runtime.block_on(async {
            spawn_publickey_server_with_auth_key(
                client_public.clone(),
                Duration::from_millis(10),
                DirectTcpipBehavior {
                    routes: vec![DirectTcpipRoute {
                        requested_host: target_host.clone(),
                        requested_port: target_addr.port(),
                        target_addr,
                    }],
                    reject_requests: false,
                },
                ShellIntegrationServerBehavior::default(),
            )
            .await
        });
    let upstream_host = upstream_addr.ip().to_string();
    let known_hosts_path = temp_known_hosts_path("direct-tcpip-upstream");
    let known_hosts = KnownHostsService::new(&known_hosts_path);
    known_hosts
        .accept_unknown(target_host.as_str(), target_addr.port(), &target_public_key)
        .expect("trust target host key");
    known_hosts
        .accept_unknown(
            upstream_host.as_str(),
            upstream_addr.port(),
            &upstream_public_key,
        )
        .expect("trust upstream host key");
    unsafe {
        std::env::set_var("MICA_TERM_KNOWN_HOSTS_PATH", &known_hosts_path);
    }

    let upstream_profile = sample_publickey_profile(
        "asset-upstream-a",
        upstream_host.clone(),
        upstream_addr.port(),
        private_key_path.display().to_string(),
    );
    let profile = sample_publickey_profile_with_proxy_hops(
        "asset-prod",
        target_host.clone(),
        target_addr.port(),
        private_key_path.display().to_string(),
        ConnectionProxyProfile::SshAsset {
            asset_id: "asset-upstream-a".into(),
        },
        vec![ResolvedProxyHop::Ssh(Box::new(upstream_profile))],
    );
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    let runtime_handle = runtime.block_on(async {
        SshSessionRuntime::connect(profile, Uuid::new_v4(), Uuid::new_v4(), event_tx)
            .await
            .expect("connect ssh runtime through direct-tcpip upstream")
    });

    let saw_connected = runtime.block_on(async {
        tokio::time::timeout(Duration::from_secs(1), async {
            while let Some(event) = event_rx.recv().await {
                if matches!(event, SessionRuntimeEvent::Connected) {
                    return true;
                }
            }
            false
        })
        .await
        .expect("wait for connected event")
    });

    assert!(
        saw_connected,
        "runtime should emit Connected after single direct-tcpip hop"
    );
    let direct_tcpip_requests = upstream_state
        .direct_tcpip_requests
        .lock()
        .expect("lock direct-tcpip requests")
        .clone();
    assert_eq!(
        direct_tcpip_requests,
        vec![(target_host.clone(), target_addr.port())]
    );

    runtime_handle.disconnect().expect("disconnect runtime");
    runtime.block_on(async {
        tokio::time::sleep(Duration::from_millis(20)).await;
        upstream_task.await.expect("join upstream ssh server");
        target_task.await.expect("join target ssh server");
    });

    unsafe {
        std::env::remove_var("MICA_TERM_KNOWN_HOSTS_PATH");
    }
    let _ = fs::remove_file(&private_key_path);
    let _ = fs::remove_file(&known_hosts_path);
}

#[test]
fn multi_hop_connection_emits_timeline_steps_in_order() {
    let _env_lock = lock_known_hosts_env();
    let runtime = AppAsyncRuntime::new().expect("create app async runtime");
    let (client_public, private_key_path) = create_publickey_auth_material("multi-hop");
    let (target_task, target_addr, target_public_key, _target_state) = runtime.block_on(async {
        spawn_publickey_server_with_auth_key(
            client_public.clone(),
            Duration::from_millis(10),
            DirectTcpipBehavior::default(),
            ShellIntegrationServerBehavior::default(),
        )
        .await
    });
    let target_host = "ssh.internal.test".to_string();
    let upstream_a_host = "proxy-a.internal.test".to_string();
    let upstream_b_host = "proxy-b.internal.test".to_string();

    let (upstream_a_task, upstream_a_addr, upstream_a_public_key, upstream_a_state) = runtime
        .block_on(async {
            spawn_publickey_server_with_auth_key(
                client_public.clone(),
                Duration::from_millis(10),
                DirectTcpipBehavior {
                    routes: vec![DirectTcpipRoute {
                        requested_host: target_host.clone(),
                        requested_port: target_addr.port(),
                        target_addr,
                    }],
                    reject_requests: false,
                },
                ShellIntegrationServerBehavior::default(),
            )
            .await
        });
    let (upstream_b_task, upstream_b_addr, upstream_b_public_key, upstream_b_state) = runtime
        .block_on(async {
            spawn_publickey_server_with_auth_key(
                client_public.clone(),
                Duration::from_millis(10),
                DirectTcpipBehavior {
                    routes: vec![DirectTcpipRoute {
                        requested_host: upstream_a_host.clone(),
                        requested_port: upstream_a_addr.port(),
                        target_addr: upstream_a_addr,
                    }],
                    reject_requests: false,
                },
                ShellIntegrationServerBehavior::default(),
            )
            .await
        });
    let (socks_task, socks_addr) = runtime.block_on(async {
        spawn_fake_socks5_server(
            upstream_b_host.clone(),
            upstream_b_addr,
            FakeSocks5AuthMode::NoAuth,
        )
        .await
    });

    let known_hosts_path = temp_known_hosts_path("multi-hop-chain");
    let known_hosts = KnownHostsService::new(&known_hosts_path);
    known_hosts
        .accept_unknown(target_host.as_str(), target_addr.port(), &target_public_key)
        .expect("trust target host key");
    known_hosts
        .accept_unknown(
            upstream_a_host.as_str(),
            upstream_a_addr.port(),
            &upstream_a_public_key,
        )
        .expect("trust upstream A host key");
    known_hosts
        .accept_unknown(
            upstream_b_host.as_str(),
            upstream_b_addr.port(),
            &upstream_b_public_key,
        )
        .expect("trust upstream B host key");
    unsafe {
        std::env::set_var("MICA_TERM_KNOWN_HOSTS_PATH", &known_hosts_path);
    }

    let upstream_a_profile = sample_publickey_profile(
        "asset-upstream-a",
        upstream_a_host.clone(),
        upstream_a_addr.port(),
        private_key_path.display().to_string(),
    );
    let upstream_b_profile = sample_publickey_profile(
        "asset-upstream-b",
        upstream_b_host.clone(),
        upstream_b_addr.port(),
        private_key_path.display().to_string(),
    );
    let profile = sample_publickey_profile_with_proxy_hops(
        "asset-prod",
        target_host.clone(),
        target_addr.port(),
        private_key_path.display().to_string(),
        ConnectionProxyProfile::Socks5 {
            host: socks_addr.ip().to_string(),
            port: socks_addr.port(),
            username: None,
            password: None,
            credential_ref: None,
        },
        vec![
            ResolvedProxyHop::Socks5 {
                host: socks_addr.ip().to_string(),
                port: socks_addr.port(),
                username: None,
                password: None,
            },
            ResolvedProxyHop::Ssh(Box::new(upstream_b_profile)),
            ResolvedProxyHop::Ssh(Box::new(upstream_a_profile)),
        ],
    );
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    let runtime_handle = runtime.block_on(async {
        SshSessionRuntime::connect(profile, Uuid::new_v4(), Uuid::new_v4(), event_tx)
            .await
            .expect("connect ssh runtime through multi-hop proxy chain")
    });

    let events = runtime.block_on(async {
        tokio::time::timeout(Duration::from_secs(1), async {
            let mut events = Vec::new();
            while let Some(event) = event_rx.recv().await {
                let is_connected = matches!(event, SessionRuntimeEvent::Connected);
                events.push(event);
                if is_connected {
                    return events;
                }
            }
            events
        })
        .await
        .expect("wait for connected event stream")
    });

    assert!(
        events
            .iter()
            .any(|event| matches!(event, SessionRuntimeEvent::Connected)),
        "runtime should emit Connected after SOCKS5 -> SSH -> SSH chain"
    );
    assert_eq!(
        completed_timeline_steps(&events),
        vec![
            ("resolve-profile".into(), "Target".into()),
            ("connect-proxy".into(), "Proxy".into()),
            ("proxy-negotiate".into(), "Proxy".into()),
            ("connect-jump-host".into(), "Jump Host 1".into()),
            ("verify-host-key".into(), "Jump Host 1".into()),
            ("authenticate-jump-host".into(), "Jump Host 1".into()),
            ("open-direct-tcpip".into(), "Jump Host 1".into()),
            ("connect-jump-host".into(), "Jump Host 2".into()),
            ("verify-host-key".into(), "Jump Host 2".into()),
            ("authenticate-jump-host".into(), "Jump Host 2".into()),
            ("open-direct-tcpip".into(), "Jump Host 2".into()),
            ("connect-target".into(), "Target".into()),
            ("verify-host-key".into(), "Target".into()),
            ("authenticate-target".into(), "Target".into()),
            ("open-session-channel".into(), "Target".into()),
            ("request-pty".into(), "Target".into()),
            ("request-shell".into(), "Target".into()),
        ],
        "runtime should expose ordered hop-aware completion steps for the full chain"
    );
    let upstream_b_requests = upstream_b_state
        .direct_tcpip_requests
        .lock()
        .expect("lock upstream B requests")
        .clone();
    let upstream_a_requests = upstream_a_state
        .direct_tcpip_requests
        .lock()
        .expect("lock upstream A requests")
        .clone();
    assert_eq!(
        upstream_b_requests,
        vec![(upstream_a_host.clone(), upstream_a_addr.port())]
    );
    assert_eq!(
        upstream_a_requests,
        vec![(target_host.clone(), target_addr.port())]
    );

    runtime_handle.disconnect().expect("disconnect runtime");
    runtime.block_on(async {
        tokio::time::sleep(Duration::from_millis(20)).await;
        socks_task.await.expect("join fake socks5 server");
        upstream_b_task.await.expect("join upstream B ssh server");
        upstream_a_task.await.expect("join upstream A ssh server");
        target_task.await.expect("join target ssh server");
    });

    unsafe {
        std::env::remove_var("MICA_TERM_KNOWN_HOSTS_PATH");
    }
    let _ = fs::remove_file(&private_key_path);
    let _ = fs::remove_file(&known_hosts_path);
}

#[test]
fn multi_hop_connection_failure_is_reported_on_the_failing_hop() {
    let _env_lock = lock_known_hosts_env();
    let runtime = AppAsyncRuntime::new().expect("create app async runtime");
    let (client_public, private_key_path) = create_publickey_auth_material("direct-tcpip-reject");
    let (target_task, target_addr, target_public_key, _target_state) = runtime.block_on(async {
        spawn_publickey_server_with_auth_key(
            client_public.clone(),
            Duration::from_millis(10),
            DirectTcpipBehavior::default(),
            ShellIntegrationServerBehavior::default(),
        )
        .await
    });
    let target_host = "ssh.internal.test".to_string();
    let (upstream_task, upstream_addr, upstream_public_key, _upstream_state) =
        runtime.block_on(async {
            spawn_publickey_server_with_auth_key(
                client_public.clone(),
                Duration::from_millis(10),
                DirectTcpipBehavior {
                    routes: Vec::new(),
                    reject_requests: true,
                },
                ShellIntegrationServerBehavior::default(),
            )
            .await
        });
    let upstream_host = upstream_addr.ip().to_string();
    let known_hosts_path = temp_known_hosts_path("direct-tcpip-reject");
    let known_hosts = KnownHostsService::new(&known_hosts_path);
    known_hosts
        .accept_unknown(target_host.as_str(), target_addr.port(), &target_public_key)
        .expect("trust target host key");
    known_hosts
        .accept_unknown(
            upstream_host.as_str(),
            upstream_addr.port(),
            &upstream_public_key,
        )
        .expect("trust upstream host key");
    unsafe {
        std::env::set_var("MICA_TERM_KNOWN_HOSTS_PATH", &known_hosts_path);
    }

    let mut upstream_profile = sample_publickey_profile(
        "asset-upstream-a",
        upstream_host,
        upstream_addr.port(),
        private_key_path.display().to_string(),
    );
    upstream_profile.name = "Proxy A".into();
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    let err = runtime.block_on(async {
        match SshSessionRuntime::connect(
            sample_publickey_profile_with_proxy_hops(
                "asset-prod",
                target_host,
                target_addr.port(),
                private_key_path.display().to_string(),
                ConnectionProxyProfile::SshAsset {
                    asset_id: "asset-upstream-a".into(),
                },
                vec![ResolvedProxyHop::Ssh(Box::new(upstream_profile))],
            ),
            Uuid::new_v4(),
            Uuid::new_v4(),
            event_tx,
        )
        .await
        {
            Ok(_) => panic!("direct-tcpip rejection should fail runtime connect"),
            Err(err) => err,
        }
    });

    let message = err.to_string();
    assert!(
        message.contains("SSH upstream 'Proxy A' rejected direct-tcpip forwarding"),
        "expected direct-tcpip error, got: {message}"
    );
    let events = runtime.block_on(async {
        let mut events = Vec::new();
        while let Some(event) = event_rx.recv().await {
            events.push(event);
        }
        events
    });
    assert_eq!(
        failed_timeline_step(&events),
        Some((
            "open-direct-tcpip".into(),
            "Jump Host 1".into(),
            "SSH upstream 'Proxy A' rejected direct-tcpip forwarding".into(),
        )),
        "runtime should pin the failure to the failing hop instead of collapsing it into a generic error"
    );

    runtime.block_on(async {
        upstream_task.abort();
        target_task.abort();
    });

    unsafe {
        std::env::remove_var("MICA_TERM_KNOWN_HOSTS_PATH");
    }
    let _ = fs::remove_file(&private_key_path);
    let _ = fs::remove_file(&known_hosts_path);
}

#[test]
fn runtime_error_marks_session_reconnectable() {
    let runtime = AppAsyncRuntime::new().expect("create app async runtime");
    let manager =
        SessionManager::new_with_launcher(runtime.handle(), Arc::new(RuntimeBackedLauncher));

    let handle = manager
        .open_session(
            sample_publickey_profile(
                "asset-prod",
                "127.0.0.1".into(),
                9,
                "/tmp/does-not-matter.key".into(),
            ),
            OpenSessionMode::ActivateExisting,
        )
        .expect("open session");

    let started = Instant::now();
    loop {
        let updated = manager
            .session(handle.session_id)
            .expect("resolve failed runtime session");

        if matches!(updated.state, SessionState::Error(_)) {
            assert!(updated.can_reconnect);
            break;
        }

        assert!(
            started.elapsed() < Duration::from_secs(1),
            "runtime-backed session did not transition into an error state in time"
        );

        runtime.block_on(async {
            tokio::time::sleep(Duration::from_millis(25)).await;
        });
    }
}

#[test]
fn disconnect_session_issues_runtime_disconnect() {
    let runtime = AppAsyncRuntime::new().expect("create app async runtime");
    let disconnects = Arc::new(AtomicUsize::new(0));
    let manager = SessionManager::new_with_launcher(
        runtime.handle(),
        Arc::new(TrackingLauncher::new(
            Arc::clone(&disconnects),
            Arc::new(AtomicUsize::new(0)),
        )),
    );

    let handle = manager
        .open_session(
            sample_profile("asset-prod"),
            OpenSessionMode::ActivateExisting,
        )
        .expect("open session");

    runtime.block_on(async {
        tokio::time::sleep(Duration::from_millis(25)).await;
    });

    manager
        .disconnect_session(handle.session_id)
        .expect("disconnect session");

    assert_eq!(disconnects.load(Ordering::SeqCst), 1);
}

#[test]
fn session_manager_forwards_text_input_and_resize_to_runtime_control() {
    let runtime = AppAsyncRuntime::new().expect("create app async runtime");
    let state = InteractiveTrackingState::default();
    let manager = SessionManager::new_with_launcher(
        runtime.handle(),
        Arc::new(InteractiveTrackingLauncher::new(state.clone())),
    );

    let handle = manager
        .open_session(
            sample_profile("asset-prod"),
            OpenSessionMode::ActivateExisting,
        )
        .expect("open session");

    runtime.block_on(async {
        tokio::time::sleep(Duration::from_millis(25)).await;
    });

    manager
        .send_session_text_input(handle.session_id, "pwd\n".to_string())
        .expect("forward text input");
    manager
        .resize_session(handle.session_id, 48, 132)
        .expect("forward terminal resize");

    assert_eq!(
        state
            .text_inputs
            .lock()
            .expect("lock text inputs")
            .as_slice(),
        &["pwd\n".to_string()]
    );
    assert_eq!(
        state.resizes.lock().expect("lock resize events").as_slice(),
        &[(48, 132)]
    );
}

#[test]
fn session_manager_forwards_structured_key_events_to_runtime() {
    let runtime = AppAsyncRuntime::new().expect("create app async runtime");
    let state = InteractiveTrackingState::default();
    let manager = SessionManager::new_with_launcher(
        runtime.handle(),
        Arc::new(InteractiveTrackingLauncher::new(state.clone())),
    );

    let handle = manager
        .open_session(
            sample_profile("asset-prod"),
            OpenSessionMode::ActivateExisting,
        )
        .expect("open session");

    runtime.block_on(async {
        tokio::time::sleep(Duration::from_millis(25)).await;
    });

    manager
        .send_session_key_input(
            handle.session_id,
            TerminalKeyEvent::function(5, false, false, false),
        )
        .expect("forward key input");

    assert_eq!(
        state.key_inputs.lock().expect("lock key inputs").as_slice(),
        &[TerminalKeyEvent::function(5, false, false, false)]
    );
}

#[test]
fn session_manager_forwards_mouse_input_to_runtime_control() {
    let runtime = AppAsyncRuntime::new().expect("create app async runtime");
    let state = InteractiveTrackingState::default();
    let manager = SessionManager::new_with_launcher(
        runtime.handle(),
        Arc::new(InteractiveTrackingLauncher::new(state.clone())),
    );

    let handle = manager
        .open_session(
            sample_profile("asset-prod"),
            OpenSessionMode::ActivateExisting,
        )
        .expect("open session");

    runtime.block_on(async {
        tokio::time::sleep(Duration::from_millis(25)).await;
    });

    manager
        .send_session_mouse_input(
            handle.session_id,
            TerminalMouseInput {
                kind: TerminalMouseEventKind::Move,
                button: TerminalMouseButton::None,
                row: 11,
                col: 17,
                shift: false,
                ctrl: true,
                alt: false,
            },
        )
        .expect("forward mouse input");

    assert_eq!(
        state
            .mouse_inputs
            .lock()
            .expect("lock mouse events")
            .as_slice(),
        &[TerminalMouseInput {
            kind: TerminalMouseEventKind::Move,
            button: TerminalMouseButton::None,
            row: 11,
            col: 17,
            shift: false,
            ctrl: true,
            alt: false,
        }]
    );
}

#[test]
fn session_manager_can_scroll_session_to_top_or_bottom() {
    let runtime = AppAsyncRuntime::new().expect("create app async runtime");
    let state = ScrollTrackingState::default();
    let manager = SessionManager::new_with_launcher(
        runtime.handle(),
        Arc::new(ScrollTrackingLauncher::new(state.clone())),
    );

    let handle = manager
        .open_session(
            sample_profile("asset-prod"),
            OpenSessionMode::ActivateExisting,
        )
        .expect("open session");

    runtime.block_on(async {
        tokio::time::sleep(Duration::from_millis(25)).await;
    });

    manager
        .scroll_session_to_top(handle.session_id)
        .expect("scroll session to top");
    let at_top = manager
        .terminal_surface(handle.session_id)
        .expect("surface after top scroll");
    assert_eq!(at_top.viewport_offset_lines, 6);
    assert!(!at_top.viewport_at_bottom);

    manager
        .scroll_session_to_bottom(handle.session_id)
        .expect("scroll session to bottom");
    let at_bottom = manager
        .terminal_surface(handle.session_id)
        .expect("surface after bottom scroll");
    assert_eq!(at_bottom.viewport_offset_lines, 0);
    assert!(at_bottom.viewport_at_bottom);
    assert_eq!(
        state
            .scroll_deltas
            .lock()
            .expect("lock scroll deltas")
            .as_slice(),
        &[4, -6]
    );
}

#[test]
fn resize_before_runtime_ready_replays_latest_dimensions_when_control_attaches() {
    let runtime = AppAsyncRuntime::new().expect("create app async runtime");
    let state = InteractiveTrackingState::default();
    let manager = SessionManager::new_with_launcher(
        runtime.handle(),
        Arc::new(DelayedInteractiveTrackingLauncher::new(
            state.clone(),
            Duration::from_millis(25),
        )),
    );

    let handle = manager
        .open_session(
            sample_profile("asset-prod"),
            OpenSessionMode::ActivateExisting,
        )
        .expect("open session");

    manager
        .resize_session(handle.session_id, 36, 120)
        .expect("queue initial resize before runtime ready");
    manager
        .resize_session(handle.session_id, 40, 132)
        .expect("replace pending resize before runtime ready");

    runtime.block_on(async {
        tokio::time::sleep(Duration::from_millis(60)).await;
    });

    assert_eq!(
        state.resizes.lock().expect("lock resize events").as_slice(),
        &[(40, 132)]
    );
}

#[test]
fn runtime_surface_snapshot_tracks_visible_rows_instead_of_single_placeholder_copy() {
    let session_id = Uuid::new_v4();
    let mut terminal = TerminalSession::new(4, 12);
    terminal.apply_remote_bytes(b"line 1\r\nline 2\r\nline 3");

    let surface = terminal.surface_state(session_id);

    assert_eq!(surface.session_id, session_id);
    assert_eq!(surface.rows, 4);
    assert_eq!(surface.cols, 12);
    assert_eq!(
        surface.visible_lines,
        vec![
            "line 1".to_string(),
            "line 2".to_string(),
            "line 3".to_string()
        ]
    );
    assert!(surface.seqno > 0);
}

#[test]
fn session_manager_populates_initial_surface_from_runtime_control_snapshot() {
    let runtime = AppAsyncRuntime::new().expect("create app async runtime");
    let manager = SessionManager::new_with_launcher(
        runtime.handle(),
        Arc::new(SurfacePullLauncher::new(
            TerminalSurfaceState::from_visible_lines(
                Uuid::new_v4(),
                7,
                24,
                80,
                vec!["warm".into(), "boot".into()],
            ),
        )),
    );

    let handle = manager
        .open_session(
            sample_profile("asset-prod"),
            OpenSessionMode::ActivateExisting,
        )
        .expect("open session");

    runtime.block_on(async {
        tokio::time::sleep(Duration::from_millis(25)).await;
    });

    let surface = manager
        .terminal_surface(handle.session_id)
        .expect("manager should populate initial surface from runtime snapshot");

    assert_eq!(surface.seqno, 7);
    assert_eq!(
        surface.visible_lines,
        vec!["warm".to_string(), "boot".to_string()]
    );
}

#[test]
fn session_manager_variant_switch_updates_attached_runtime_palette_projection() {
    let runtime = AppAsyncRuntime::new().expect("create app async runtime");
    let manager =
        SessionManager::new_with_launcher(runtime.handle(), Arc::new(ThemeTrackingLauncher));

    let handle = manager
        .open_session(
            sample_profile("asset-prod"),
            OpenSessionMode::ActivateExisting,
        )
        .expect("open session");

    runtime.block_on(async {
        tokio::time::sleep(Duration::from_millis(25)).await;
    });

    let premium_surface = manager
        .terminal_surface(handle.session_id)
        .expect("premium surface");
    let premium_green = premium_surface
        .cells
        .iter()
        .find(|cell| cell.col == 0)
        .expect("premium green cell");

    manager
        .set_theme(ThemeMode::Dark, ThemeVariant::LegacyHackerGreen)
        .expect("propagate legacy theme variant");

    let legacy_surface = manager
        .terminal_surface(handle.session_id)
        .expect("legacy surface");
    let legacy_green = legacy_surface
        .cells
        .iter()
        .find(|cell| cell.col == 0)
        .expect("legacy green cell");
    let legacy_preset = preset_for_theme(ThemeMode::Dark, ThemeVariant::LegacyHackerGreen);

    assert_eq!(
        legacy_surface.default_bg_rgba,
        0xff00_0000 | legacy_preset.background
    );
    assert_eq!(
        legacy_surface.cursor.bg_rgba,
        0xff00_0000 | legacy_preset.cursor_bg
    );
    assert_eq!(
        legacy_green.fg_rgba,
        0xff00_0000
            | (u32::from(legacy_preset.ansi[2].0) << 16)
            | (u32::from(legacy_preset.ansi[2].1) << 8)
            | u32::from(legacy_preset.ansi[2].2)
    );
    assert_ne!(legacy_green.fg_rgba, premium_green.fg_rgba);
}

#[test]
fn close_session_issues_runtime_disconnect_before_removal() {
    let runtime = AppAsyncRuntime::new().expect("create app async runtime");
    let disconnects = Arc::new(AtomicUsize::new(0));
    let terminal_releases = Arc::new(AtomicUsize::new(0));
    let manager = SessionManager::new_with_launcher(
        runtime.handle(),
        Arc::new(TrackingLauncher::new(
            Arc::clone(&disconnects),
            Arc::clone(&terminal_releases),
        )),
    );

    let handle = manager
        .open_session(
            sample_profile("asset-prod"),
            OpenSessionMode::ActivateExisting,
        )
        .expect("open session");

    runtime.block_on(async {
        tokio::time::sleep(Duration::from_millis(25)).await;
    });

    manager
        .close_session(handle.session_id)
        .expect("close session");

    assert_eq!(disconnects.load(Ordering::SeqCst), 1);
    assert_eq!(terminal_releases.load(Ordering::SeqCst), 1);
    assert!(manager.session(handle.session_id).is_none());
}

#[test]
fn close_session_before_runtime_ready_disconnects_when_control_arrives() {
    let runtime = AppAsyncRuntime::new().expect("create app async runtime");
    let disconnects = Arc::new(AtomicUsize::new(0));
    let terminal_releases = Arc::new(AtomicUsize::new(0));
    let manager = SessionManager::new_with_launcher(
        runtime.handle(),
        Arc::new(DelayedTrackingLauncher::new(
            Arc::clone(&disconnects),
            Arc::clone(&terminal_releases),
            Duration::from_millis(25),
        )),
    );

    let handle = manager
        .open_session(
            sample_profile("asset-prod"),
            OpenSessionMode::ActivateExisting,
        )
        .expect("open session");

    manager
        .close_session(handle.session_id)
        .expect("close session before runtime ready");

    runtime.block_on(async {
        tokio::time::sleep(Duration::from_millis(60)).await;
    });

    assert_eq!(disconnects.load(Ordering::SeqCst), 1);
    assert_eq!(terminal_releases.load(Ordering::SeqCst), 1);
    assert!(manager.session(handle.session_id).is_none());
}

#[test]
fn closing_tab_removes_session_from_registry() {
    let runtime = AppAsyncRuntime::new().expect("create app async runtime");
    let manager = SessionManager::new_with_launcher(
        runtime.handle(),
        Arc::new(FakeLauncher::stay_connecting()),
    );

    let first = manager
        .open_session(
            sample_profile("asset-prod"),
            OpenSessionMode::ActivateExisting,
        )
        .expect("open first session");
    let second = manager
        .open_session(sample_profile("asset-prod"), OpenSessionMode::ForceNewTab)
        .expect("open second session");

    let removed = manager
        .close_session(second.session_id)
        .expect("close second session");
    assert_eq!(removed.session_id, second.session_id);
    assert!(
        manager.session(second.session_id).is_none(),
        "closed session should no longer remain in the registry"
    );

    let reopened = manager
        .open_session(
            sample_profile("asset-prod"),
            OpenSessionMode::ActivateExisting,
        )
        .expect("reopen existing session");

    assert_eq!(
        reopened.session_id, first.session_id,
        "after closing the newest tab, ActivateExisting should reuse the remaining live session"
    );
}

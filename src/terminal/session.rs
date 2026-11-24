//! GPUI Entity managing a single terminal session (Local PTY or Remote SSH).

use std::sync::Arc;
use std::time::Duration;

use alacritty_terminal::grid::Scroll;
use alacritty_terminal::index::Side;
use gpui_kit::{ClipboardItem, Context, EventEmitter};
use parking_lot::Mutex;
use tokio::sync::{mpsc, oneshot};

use crate::domain::ResolvedSshConfig;
use crate::sftp::SftpService;
use crate::ssh::{KnownHostsStore, SshClientSession, SshSessionEvent};
use crate::terminal::{
    AlacrittyTerminalEngine, EngineSideEffect, LocalPtyBackend, LocalPtyEvent, MouseReportKind,
    SemanticOptions, TerminalPalette, TerminalRenderSnapshot, encode_keystroke,
    encode_mouse_report, encode_paste,
};

#[derive(Clone)]
pub enum SessionKind {
    LocalShell,
    Ssh {
        config: Box<ResolvedSshConfig>,
        known_hosts: KnownHostsStore,
    },
}

pub struct HostKeyPrompt {
    pub host: String,
    pub port: u16,
    pub fingerprint: String,
    pub openssh_key: String,
    pub decision_tx: Option<oneshot::Sender<bool>>,
}

pub enum SessionPhase {
    Connecting(String),
    AwaitingHostKey(HostKeyPrompt),
    Connected,
    Disconnected(Option<String>),
    Error(String),
}

#[derive(Clone)]
pub enum SessionEntityEvent {
    TitleChanged(String),
    RemoteConnected {
        sftp: SftpService,
        initial_dir: String,
    },
    RemoteCwdChanged(String),
    PhaseChanged,
}

enum ActiveBackend {
    None,
    Local(LocalPtyBackend),
    Ssh(SshClientSession),
}

pub struct TerminalSessionEntity {
    pub id: String,
    pub title: String,
    pub kind: SessionKind,
    pub phase: SessionPhase,
    pub font_size: f32,
    pub search_open: bool,
    pub search_query: String,
    engine: AlacrittyTerminalEngine,
    backend: ActiveBackend,
    sftp: Option<SftpService>,
    remote_cwd: Option<String>,
    tokio_handle: tokio::runtime::Handle,
}

impl EventEmitter<SessionEntityEvent> for TerminalSessionEntity {}

impl TerminalSessionEntity {
    pub fn new_local(
        title: impl Into<String>,
        font_size: f32,
        scrollback_lines: usize,
        tokio_handle: tokio::runtime::Handle,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut this = Self {
            id: uuid::Uuid::new_v4().to_string(),
            title: title.into(),
            kind: SessionKind::LocalShell,
            phase: SessionPhase::Connecting("正在启动本地终端...".into()),
            font_size,
            search_open: false,
            search_query: String::new(),
            engine: AlacrittyTerminalEngine::new(28, 100, scrollback_lines),
            backend: ActiveBackend::None,
            sftp: None,
            remote_cwd: None,
            tokio_handle,
        };
        this.start_backend(cx);
        this
    }

    pub fn new_ssh(
        config: ResolvedSshConfig,
        known_hosts: KnownHostsStore,
        font_size: f32,
        scrollback_lines: usize,
        tokio_handle: tokio::runtime::Handle,
        cx: &mut Context<Self>,
    ) -> Self {
        let title = config.name.clone();
        let mut this = Self {
            id: uuid::Uuid::new_v4().to_string(),
            title,
            kind: SessionKind::Ssh {
                config: Box::new(config),
                known_hosts,
            },
            phase: SessionPhase::Connecting("正在初始化 SSH 会话...".into()),
            font_size,
            search_open: false,
            search_query: String::new(),
            engine: AlacrittyTerminalEngine::new(28, 100, scrollback_lines),
            backend: ActiveBackend::None,
            sftp: None,
            remote_cwd: None,
            tokio_handle,
        };
        this.start_backend(cx);
        this
    }

    pub fn is_ssh(&self) -> bool {
        matches!(self.kind, SessionKind::Ssh { .. })
    }

    pub fn viewport_rows(&self) -> usize {
        self.engine.dimensions().0
    }

    pub fn sftp_service(&self) -> Option<SftpService> {
        self.sftp.clone()
    }

    pub fn remote_cwd(&self) -> Option<&str> {
        self.remote_cwd.as_deref()
    }

    pub fn endpoint_subtitle(&self) -> String {
        match &self.kind {
            SessionKind::LocalShell => "本地 Shell".to_string(),
            SessionKind::Ssh { config, .. } => {
                format!("{}@{}:{}", config.user, config.host, config.port)
            }
        }
    }

    pub fn reconnect(&mut self, scrollback_lines: usize, cx: &mut Context<Self>) {
        let (rows, cols) = self.engine.dimensions();
        self.engine = AlacrittyTerminalEngine::new(rows, cols, scrollback_lines);
        self.sftp = None;
        self.remote_cwd = None;
        self.start_backend(cx);
        cx.notify();
    }

    pub fn respond_host_key(&mut self, trust: bool, cx: &mut Context<Self>) {
        if let SessionPhase::AwaitingHostKey(prompt) = &mut self.phase
            && let Some(tx) = prompt.decision_tx.take()
        {
            let _ = tx.send(trust);
            if trust {
                self.phase = SessionPhase::Connecting("已信任主机密钥，继续认证中...".into());
            } else {
                self.phase =
                    SessionPhase::Disconnected(Some("已取消连接（拒绝未知主机密钥）".into()));
            }
            cx.emit(SessionEntityEvent::PhaseChanged);
            cx.notify();
        }
    }

    fn start_backend(&mut self, cx: &mut Context<Self>) {
        let (rows, cols) = self.engine.dimensions();
        match self.kind.clone() {
            SessionKind::LocalShell => {
                let (tx, rx) = mpsc::unbounded_channel::<LocalPtyEvent>();
                match LocalPtyBackend::spawn(rows as u16, cols as u16, None, tx) {
                    Ok(pty) => {
                        self.backend = ActiveBackend::Local(pty);
                        self.phase = SessionPhase::Connected;
                        self.spawn_local_pump(rx, cx);
                    }
                    Err(err) => {
                        self.phase = SessionPhase::Error(format!("{err:#}"));
                    }
                }
            }
            SessionKind::Ssh {
                config,
                known_hosts,
            } => {
                let (tx, rx) = mpsc::unbounded_channel::<SshSessionEvent>();
                let ssh = SshClientSession::connect_in_runtime(
                    &self.tokio_handle,
                    *config,
                    known_hosts,
                    rows as u16,
                    cols as u16,
                    tx,
                );
                self.backend = ActiveBackend::Ssh(ssh);
                self.phase = SessionPhase::Connecting("正在准备建立连接...".into());
                self.spawn_ssh_pump(rx, cx);
            }
        }
    }

    fn spawn_local_pump(
        &self,
        mut rx: mpsc::UnboundedReceiver<LocalPtyEvent>,
        cx: &mut Context<Self>,
    ) {
        let shared_rx = Arc::new(Mutex::new(Vec::<LocalPtyEvent>::new()));
        let collector = Arc::clone(&shared_rx);
        let (wake_tx, mut wake_rx) = smol_wakeup_channel();

        self.tokio_handle.spawn(async move {
            while let Some(ev) = rx.recv().await {
                collector.lock().push(ev);
                let _ = wake_tx.try_send(());
            }
            let _ = wake_tx.try_send(());
        });

        cx.spawn(async move |weak, cx| {
            while wake_rx.recv().await.is_ok() {
                cx.background_executor()
                    .timer(Duration::from_millis(4))
                    .await;
                let batch = std::mem::take(&mut *shared_rx.lock());
                if batch.is_empty() {
                    continue;
                }
                let alive = weak.update(cx, |this, cx| {
                    for ev in batch {
                        match ev {
                            LocalPtyEvent::Output(bytes) => {
                                this.apply_output_bytes(&bytes, cx);
                            }
                            LocalPtyEvent::Exited(code) => {
                                let msg = code.map(|c| format!("本地进程已退出 (code {c})"));
                                this.phase = SessionPhase::Disconnected(msg);
                                cx.emit(SessionEntityEvent::PhaseChanged);
                            }
                            LocalPtyEvent::Error(err) => {
                                this.phase = SessionPhase::Error(err);
                                cx.emit(SessionEntityEvent::PhaseChanged);
                            }
                        }
                    }
                    cx.notify();
                });
                if alive.is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    fn spawn_ssh_pump(
        &self,
        mut rx: mpsc::UnboundedReceiver<SshSessionEvent>,
        cx: &mut Context<Self>,
    ) {
        let shared_rx = Arc::new(Mutex::new(Vec::<SshSessionEvent>::new()));
        let collector = Arc::clone(&shared_rx);
        let (wake_tx, mut wake_rx) = smol_wakeup_channel();

        self.tokio_handle.spawn(async move {
            while let Some(ev) = rx.recv().await {
                collector.lock().push(ev);
                let _ = wake_tx.try_send(());
            }
            let _ = wake_tx.try_send(());
        });

        cx.spawn(async move |weak, cx| {
            while wake_rx.recv().await.is_ok() {
                cx.background_executor()
                    .timer(Duration::from_millis(4))
                    .await;
                let batch = std::mem::take(&mut *shared_rx.lock());
                if batch.is_empty() {
                    continue;
                }
                let alive = weak.update(cx, |this, cx| {
                    for ev in batch {
                        match ev {
                            SshSessionEvent::Step(step) => {
                                this.phase = SessionPhase::Connecting(step);
                                cx.emit(SessionEntityEvent::PhaseChanged);
                            }
                            SshSessionEvent::HostKeyPrompt(prompt) => {
                                this.phase = SessionPhase::AwaitingHostKey(HostKeyPrompt {
                                    host: prompt.host,
                                    port: prompt.port,
                                    fingerprint: prompt.fingerprint,
                                    openssh_key: prompt.openssh_key,
                                    decision_tx: Some(prompt.decision_tx),
                                });
                                cx.emit(SessionEntityEvent::PhaseChanged);
                            }
                            SshSessionEvent::Connected {
                                sftp,
                                initial_dir,
                                home_dir,
                            } => {
                                tracing::info!(session_id = %this.id, initial_dir = %initial_dir, "SSH 终端会话已进入 Connected 状态");
                                this.phase = SessionPhase::Connected;
                                this.engine.set_remote_home_dir(home_dir);
                                this.sftp = Some(sftp.clone());
                                this.remote_cwd = Some(initial_dir.clone());
                                cx.emit(SessionEntityEvent::RemoteConnected { sftp, initial_dir });
                                cx.emit(SessionEntityEvent::PhaseChanged);
                            }
                            SshSessionEvent::Output(bytes) => {
                                this.apply_output_bytes(&bytes, cx);
                            }
                            SshSessionEvent::Disconnected(reason) => {
                                tracing::info!(session_id = %this.id, ?reason, "SSH 终端会话已断开");
                                this.sftp = None;
                                this.phase = SessionPhase::Disconnected(reason);
                                cx.emit(SessionEntityEvent::PhaseChanged);
                            }
                            SshSessionEvent::Error(err) => {
                                tracing::warn!(session_id = %this.id, error = %err, "SSH 终端会话错误");
                                this.sftp = None;
                                this.phase = SessionPhase::Error(err);
                                cx.emit(SessionEntityEvent::PhaseChanged);
                            }
                        }
                    }
                    cx.notify();
                });
                if alive.is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    fn apply_output_bytes(&mut self, bytes: &[u8], cx: &mut Context<Self>) {
        let effects = self.engine.feed_bytes(bytes);
        for effect in effects {
            match effect {
                EngineSideEffect::PtyReply(reply) => self.write_raw(reply),
                EngineSideEffect::TitleChanged(new_title) => {
                    if !new_title.trim().is_empty() && matches!(self.kind, SessionKind::LocalShell)
                    {
                        self.title = new_title.trim().to_string();
                        cx.emit(SessionEntityEvent::TitleChanged(self.title.clone()));
                    }
                }
                EngineSideEffect::ClipboardStore(text) => {
                    cx.write_to_clipboard(ClipboardItem::new_string(text));
                }
                EngineSideEffect::CurrentDirChanged(dir) => {
                    self.remote_cwd = Some(dir.clone());
                    cx.emit(SessionEntityEvent::RemoteCwdChanged(dir));
                }
                EngineSideEffect::Bell => {}
            }
        }
    }

    pub fn write_raw(&self, bytes: Vec<u8>) {
        match &self.backend {
            ActiveBackend::Local(pty) => pty.write(bytes),
            ActiveBackend::Ssh(ssh) => ssh.write(bytes),
            ActiveBackend::None => {}
        }
    }

    pub fn send_ime_text(&mut self, text: &str, cx: &mut Context<Self>) {
        if text.is_empty() || !matches!(self.phase, SessionPhase::Connected) {
            return;
        }
        if self.engine.display_offset() > 0 {
            self.engine.scroll(Scroll::Bottom);
        }
        self.engine.clear_selection();
        self.write_raw(text.as_bytes().to_vec());
        cx.notify();
    }

    pub fn handle_keystroke(&mut self, ks: &gpui_kit::Keystroke, cx: &mut Context<Self>) -> bool {
        if !matches!(self.phase, SessionPhase::Connected) {
            return false;
        }
        let mode = self.engine.mode();
        if let Some(bytes) = encode_keystroke(ks, mode) {
            if self.engine.display_offset() > 0 {
                self.engine.scroll(Scroll::Bottom);
            }
            self.engine.clear_selection();
            self.write_raw(bytes);
            cx.notify();
            true
        } else {
            false
        }
    }

    pub fn send_paste(&mut self, text: &str, cx: &mut Context<Self>) {
        if text.is_empty() || !matches!(self.phase, SessionPhase::Connected) {
            return;
        }
        let bytes = encode_paste(text, self.engine.mode());
        if self.engine.display_offset() > 0 {
            self.engine.scroll(Scroll::Bottom);
        }
        self.write_raw(bytes);
        cx.notify();
    }

    pub fn send_command_snippet(&mut self, command: &str, auto_enter: bool, cx: &mut Context<Self>) {
        if !matches!(self.phase, SessionPhase::Connected) {
            return;
        }
        let mut payload = command.to_string();
        if auto_enter && !payload.ends_with('\n') && !payload.ends_with('\r') {
            payload.push('\r');
        }
        self.write_raw(payload.into_bytes());
        cx.notify();
    }

    pub fn clear_screen(&mut self, cx: &mut Context<Self>) {
        if self.engine.display_offset() > 0 {
            self.engine.scroll(Scroll::Bottom);
        }
        self.write_raw(vec![0x0c]);
        cx.notify();
    }

    pub fn clear_selection(&mut self, cx: &mut Context<Self>) {
        self.engine.clear_selection();
        cx.notify();
    }

    pub fn resize_grid(&mut self, rows: usize, cols: usize, cx: &mut Context<Self>) {
        if self.engine.resize(rows, cols) {
            match &self.backend {
                ActiveBackend::Local(pty) => pty.resize(rows as u16, cols as u16),
                ActiveBackend::Ssh(ssh) => ssh.resize(rows as u16, cols as u16),
                ActiveBackend::None => {}
            }
            cx.notify();
        }
    }

    pub fn scroll_lines(&mut self, delta: i32, cx: &mut Context<Self>) {
        let mode = self.engine.mode();
        if mode.contains(alacritty_terminal::term::TermMode::ALT_SCREEN)
            && mode.contains(alacritty_terminal::term::TermMode::ALTERNATE_SCROLL)
            && !mode.intersects(alacritty_terminal::term::TermMode::MOUSE_MODE)
        {
            let seq: &[u8] = if delta > 0 {
                if mode.contains(alacritty_terminal::term::TermMode::APP_CURSOR) {
                    b"\x1bOA"
                } else {
                    b"\x1b[A"
                }
            } else if mode.contains(alacritty_terminal::term::TermMode::APP_CURSOR) {
                b"\x1bOB"
            } else {
                b"\x1b[B"
            };
            let count = delta.unsigned_abs().clamp(1, 5);
            let mut out = Vec::with_capacity(seq.len() * count as usize);
            for _ in 0..count {
                out.extend_from_slice(seq);
            }
            self.write_raw(out);
            return;
        }
        self.engine.scroll(Scroll::Delta(delta));
        cx.notify();
    }

    pub fn scroll_to_ratio(&mut self, ratio: f32, cx: &mut Context<Self>) {
        self.engine.scroll_to_ratio(ratio);
        cx.notify();
    }

    pub fn jump_command_block(&mut self, previous: bool, cx: &mut Context<Self>) -> bool {
        let jumped = self.engine.jump_command_block(previous);
        if jumped {
            cx.notify();
        }
        jumped
    }

    pub fn url_at(&self, row: usize, col: usize) -> Option<String> {
        self.engine.url_at(row, col)
    }

    pub fn try_report_mouse(
        &self,
        kind: MouseReportKind,
        col: usize,
        row: usize,
        mods: gpui_kit::Modifiers,
    ) -> bool {
        if mods.shift {
            return false;
        }
        if let Some(bytes) = encode_mouse_report(kind, col, row, mods, self.engine.mode()) {
            self.write_raw(bytes);
            true
        } else {
            false
        }
    }

    pub fn begin_selection(
        &mut self,
        row: usize,
        col: usize,
        click_count: usize,
        cx: &mut Context<Self>,
    ) {
        self.engine.start_selection(row, col, click_count);
        cx.notify();
    }

    pub fn update_selection(
        &mut self,
        row: usize,
        col: usize,
        right_side: bool,
        cx: &mut Context<Self>,
    ) {
        let side = if right_side { Side::Right } else { Side::Left };
        self.engine.update_selection(row, col, side);
        cx.notify();
    }

    pub fn select_all(&mut self, cx: &mut Context<Self>) {
        self.engine.select_all();
        cx.notify();
    }

    pub fn selected_text(&self) -> Option<String> {
        self.engine.selected_text()
    }

    pub fn set_search_query(&mut self, query: String, cx: &mut Context<Self>) {
        self.engine.set_search_query(&query);
        self.search_query = query;
        cx.notify();
    }

    pub fn jump_search(&mut self, forward: bool, cx: &mut Context<Self>) {
        self.engine.jump_search(forward);
        cx.notify();
    }

    pub fn zoom_font(&mut self, delta: f32, cx: &mut Context<Self>) {
        self.font_size = (self.font_size + delta).clamp(9.0, 36.0);
        cx.notify();
    }

    pub fn reset_font(&mut self, default_size: f32, cx: &mut Context<Self>) {
        self.font_size = default_size.clamp(9.0, 36.0);
        cx.notify();
    }

    pub fn render_snapshot(
        &mut self,
        palette: &TerminalPalette,
        focused: bool,
        semantic_opts: SemanticOptions,
    ) -> TerminalRenderSnapshot {
        self.engine.snapshot(palette, focused, semantic_opts)
    }
}

fn smol_wakeup_channel() -> (tokio::sync::mpsc::Sender<()>, WakeupReceiver) {
    let (tx, rx) = tokio::sync::mpsc::channel(1);
    (tx, WakeupReceiver { rx })
}

struct WakeupReceiver {
    rx: tokio::sync::mpsc::Receiver<()>,
}

impl WakeupReceiver {
    async fn recv(&mut self) -> Result<(), ()> {
        self.rx.recv().await.ok_or(())
    }
}

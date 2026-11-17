use mica_term::app::{
    async_runtime::AppAsyncRuntime,
    ssh::{
        known_hosts::KnownHostsService,
        profile::{ConnectionProfile, ConnectionProxyProfile, SshAuthMethod},
        runtime::{
            SessionRuntimeEvent, SshSessionRuntime, TerminalKeyEvent, TerminalMouseButton,
            TerminalMouseEventKind, TerminalMouseInput,
        },
    },
};
use russh::keys::{Algorithm, PrivateKey, ssh_key::rand_core::OsRng};
use russh::{Channel, ChannelId, server};
use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::sync::mpsc;
use uuid::Uuid;

static KNOWN_HOSTS_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct TestEnvironment {
    directory: std::path::PathBuf,
    previous_known_hosts: Option<std::ffi::OsString>,
}

impl TestEnvironment {
    fn new() -> Self {
        let directory = std::env::temp_dir().join(format!("mica-ssh-stability-{}", Uuid::new_v4()));
        std::fs::create_dir(&directory).unwrap();
        let previous_known_hosts = std::env::var_os("MICA_TERM_KNOWN_HOSTS_PATH");
        unsafe {
            std::env::set_var("MICA_TERM_KNOWN_HOSTS_PATH", directory.join("known_hosts"));
        }
        Self {
            directory,
            previous_known_hosts,
        }
    }
}

impl Drop for TestEnvironment {
    fn drop(&mut self) {
        unsafe {
            match &self.previous_known_hosts {
                Some(path) => std::env::set_var("MICA_TERM_KNOWN_HOSTS_PATH", path),
                None => std::env::remove_var("MICA_TERM_KNOWN_HOSTS_PATH"),
            }
        }
        std::fs::remove_dir_all(&self.directory).expect("remove owned SSH test directory");
    }
}

struct Handler {
    opened: mpsc::UnboundedSender<(ChannelId, server::Handle)>,
    inputs: Arc<AtomicUsize>,
}
impl server::Handler for Handler {
    type Error = russh::Error;
    async fn auth_password(&mut self, _: &str, _: &str) -> Result<server::Auth, Self::Error> {
        Ok(server::Auth::Accept)
    }
    async fn channel_open_session(
        &mut self,
        _: Channel<server::Msg>,
        _: &mut server::Session,
    ) -> Result<bool, Self::Error> {
        Ok(true)
    }
    async fn pty_request(
        &mut self,
        ch: ChannelId,
        _: &str,
        _: u32,
        _: u32,
        _: u32,
        _: u32,
        _: &[(russh::Pty, u32)],
        s: &mut server::Session,
    ) -> Result<(), Self::Error> {
        s.channel_success(ch)
    }
    async fn env_request(
        &mut self,
        ch: ChannelId,
        _: &str,
        _: &str,
        s: &mut server::Session,
    ) -> Result<(), Self::Error> {
        s.channel_success(ch)
    }
    async fn shell_request(
        &mut self,
        ch: ChannelId,
        s: &mut server::Session,
    ) -> Result<(), Self::Error> {
        s.channel_success(ch)?;
        self.opened.send((ch, s.handle())).unwrap();
        Ok(())
    }
    async fn data(
        &mut self,
        ch: ChannelId,
        data: &[u8],
        s: &mut server::Session,
    ) -> Result<(), Self::Error> {
        if data.starts_with(b"probe-") {
            self.inputs.fetch_add(1, Ordering::SeqCst);
            s.data(ch, data.to_vec())?;
        }
        Ok(())
    }
}

struct Server {
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.stop.take().unwrap().send(());
        self.thread.take().unwrap().join().unwrap();
    }
}

#[test]
fn three_sessions_survive_output_rekeys_keepalive_and_sibling_close() {
    run_stability_probe(Scenario::Normal);
}

#[test]
fn blocked_terminal_output_does_not_disconnect_other_sessions() {
    run_stability_probe(Scenario::Output);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Scenario {
    Normal,
    Output,
    Key,
    Mouse,
    Paste,
    Resize,
    Backpressure,
    RemoteClose,
    IncompleteSync,
    OversizedSync,
}

#[test]
fn blocked_key_input_preserves_other_connections() {
    run_stability_probe(Scenario::Key);
}
#[test]
fn blocked_mouse_input_preserves_other_connections() {
    run_stability_probe(Scenario::Mouse);
}
#[test]
fn blocked_paste_preserves_other_connections() {
    run_stability_probe(Scenario::Paste);
}
#[test]
fn blocked_resize_preserves_other_connections() {
    run_stability_probe(Scenario::Resize);
}
#[test]
fn terminal_replies_do_not_deadlock_with_output_backpressure() {
    run_stability_probe(Scenario::Backpressure);
}
#[test]
fn remote_close_is_observed_after_output_backpressure() {
    run_stability_probe(Scenario::RemoteClose);
}
#[test]
fn incomplete_sync_frame_expires_without_disconnect() {
    run_stability_probe(Scenario::IncompleteSync);
}
#[test]
fn oversized_sync_frame_releases_buffer_without_disconnect() {
    run_stability_probe(Scenario::OversizedSync);
}

fn run_stability_probe(scenario: Scenario) {
    let _env_lock = KNOWN_HOSTS_ENV_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let environment = TestEnvironment::new();
    let (ready_tx, ready_rx) = std::sync::mpsc::channel();
    let (opened_tx, mut opened_rx) = mpsc::unbounded_channel();
    let (stop_tx, stop_rx) = tokio::sync::oneshot::channel();
    let inputs = Arc::new(AtomicUsize::new(0));
    let server_inputs = inputs.clone();
    let thread = std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async move {
            let key = PrivateKey::random(&mut OsRng, Algorithm::Ed25519).unwrap();
            let mut config = server::Config::default();
            config.auth_rejection_time = Duration::ZERO;
            config.keepalive_interval = Some(Duration::from_secs(1));
            config.keepalive_max = 3;
            config.limits.rekey_time_limit = Duration::from_secs(2);
            config.keys.push(key.clone());
            let config = Arc::new(config);
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            ready_tx.send((listener.local_addr().unwrap(), key.public_key().clone())).unwrap();
            let mut sessions = tokio::task::JoinSet::new();
            tokio::pin!(stop_rx);
            loop {
                tokio::select! {
                    _ = &mut stop_rx => break,
                    accepted = listener.accept() => {
                        let (stream, _) = accepted.unwrap();
                        let cfg = config.clone();
                        let handler = Handler { opened: opened_tx.clone(), inputs: server_inputs.clone() };
                        sessions.spawn(async move {
                            let result = server::run_stream(cfg, stream, handler).await.unwrap().await;
                            eprintln!("server connection ended: {result:?}");
                        });
                    }
                }
            }
            sessions.shutdown().await;
        });
    });
    let _server = Server {
        stop: Some(stop_tx),
        thread: Some(thread),
    };
    let (addr, key) = ready_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    eprintln!("isolated SSH listener: {addr}; scenario={scenario:?}");
    KnownHostsService::new(environment.directory.join("known_hosts"))
        .accept_unknown("127.0.0.1", addr.port(), &key)
        .unwrap();
    let runtime = AppAsyncRuntime::new().unwrap();
    runtime.block_on(async {
        tokio::time::timeout(Duration::from_secs(45), async {
            let profile = ConnectionProfile {
                asset_id: Some("same-host".into()),
                name: "same-host".into(),
                host: "127.0.0.1".into(),
                user: "test".into(),
                port: addr.port(),
                auth_method: SshAuthMethod::Password,
                credential_ref: None,
                private_key_path: None,
                password: Some("test-only".into()),
                private_key_content: None,
                passphrase: None,
                proxy: ConnectionProxyProfile::None,
                resolved_proxy_hops: vec![],
                remark: String::new(),
            };
            let mut clients = Vec::new();
            let mut receivers = Vec::new();
            let mut peers = Vec::new();
            for _ in 0..3 {
                let (tx, rx) = mpsc::unbounded_channel();
                clients.push(
                    SshSessionRuntime::connect(profile.clone(), Uuid::new_v4(), Uuid::new_v4(), tx)
                        .await
                        .unwrap(),
                );
                receivers.push(rx);
                peers.push(opened_rx.recv().await.unwrap());
            }
            let frame =
                b"\x1b[?2026h\x1b[HRunning Codex-like output\r\n\x1b[6n\x1b[?2026l".repeat(128);
            for tick in 0..if scenario == Scenario::Normal { 200 } else { 0 } {
                for (ch, handle) in &peers {
                    handle.data(*ch, frame.clone()).await.unwrap();
                }
                tokio::time::sleep(Duration::from_millis(25)).await;
                for rx in &mut receivers {
                    while let Ok(event) = rx.try_recv() {
                        assert!(
                            !matches!(
                                event,
                                SessionRuntimeEvent::Disconnected | SessionRuntimeEvent::Error(_)
                            ),
                            "tick {tick}: {event:?}"
                        );
                    }
                }
            }
            if matches!(scenario, Scenario::IncompleteSync | Scenario::OversizedSync) {
                let (ch, peer) = &peers[0];
                let before = clients[0].terminal().lock().unwrap().sequence_number();
                peer.data(*ch, b"\x1b[?2026hvisible-after-sync-timeout".to_vec())
                    .await
                    .unwrap();
                if scenario == Scenario::OversizedSync {
                    peer.data(*ch, b" ".repeat(1024 * 1024 + 1)).await.unwrap();
                }
                tokio::time::timeout(Duration::from_secs(2), async {
                    loop {
                        let terminal = clients[0].terminal();
                        let ready = {
                            let terminal = terminal.lock().unwrap();
                            if scenario == Scenario::IncompleteSync {
                                terminal
                                    .screen_text()
                                    .contains("visible-after-sync-timeout")
                            } else {
                                terminal.sequence_number() > before
                            }
                        };
                        if ready {
                            break;
                        }
                        tokio::time::sleep(Duration::from_millis(10)).await;
                    }
                })
                .await
                .expect("unfinished sync output must become visible");
                if scenario == Scenario::IncompleteSync {
                    assert!(
                        clients[0]
                            .terminal()
                            .lock()
                            .unwrap()
                            .screen_text()
                            .contains("visible-after-sync-timeout")
                    );
                }
                peer.data(*ch, b"\x1b[?2026l\r\n".to_vec()).await.unwrap();
            } else if matches!(scenario, Scenario::Backpressure | Scenario::RemoteClose) {
                let terminal = clients[0].terminal();
                let guard = terminal.lock().unwrap();
                let (ch, peer) = &peers[0];
                let frames = b"\x1b[?2026h\x1b[6n\x1b[?2026l".repeat(12);
                peer.data(*ch, frames.clone()).await.unwrap();
                tokio::time::sleep(Duration::from_millis(200)).await;
                for _ in 0..150 {
                    peer.data(*ch, frames.clone()).await.unwrap();
                }
                peer.data(*ch, b"backlog-complete".to_vec()).await.unwrap();
                if scenario == Scenario::RemoteClose {
                    peer.eof(*ch).await.unwrap();
                    peer.close(*ch).await.unwrap();
                }
                tokio::time::sleep(Duration::from_millis(200)).await;
                drop(guard);
                if scenario == Scenario::RemoteClose {
                    tokio::time::timeout(Duration::from_secs(3), async {
                        loop {
                            match receivers[0].recv().await {
                                Some(SessionRuntimeEvent::Disconnected) => break,
                                Some(SessionRuntimeEvent::Error(error)) => {
                                    panic!("unexpected output error: {error}")
                                }
                                Some(_) => {}
                                None => panic!("runtime closed without disconnect event"),
                            }
                        }
                    })
                    .await
                    .expect("remote close must not be hidden behind queued output");
                    assert!(
                        terminal
                            .lock()
                            .unwrap()
                            .screen_text()
                            .contains("backlog-complete")
                    );
                    for client in &clients[1..] {
                        client
                            .send_text_input("probe-after-peer-close".into())
                            .unwrap();
                    }
                    tokio::time::timeout(Duration::from_secs(3), async {
                        while inputs.load(Ordering::SeqCst) < 2 {
                            tokio::time::sleep(Duration::from_millis(10)).await;
                        }
                    })
                    .await
                    .expect("sibling sessions must still deliver input after remote close");
                    for client in &clients[1..] {
                        client.disconnect().unwrap();
                    }
                    return;
                }
                tokio::time::timeout(Duration::from_secs(3), async {
                    while !terminal
                        .lock()
                        .unwrap()
                        .screen_text()
                        .contains("backlog-complete")
                    {
                        tokio::time::sleep(Duration::from_millis(10)).await;
                    }
                })
                .await
                .expect("output must drain while terminal replies are sent");
            } else if scenario != Scenario::Normal {
                let first = clients[0].terminal();
                let second = clients[1].terminal();
                let first_lock = first.lock().unwrap();
                let second_lock = second.lock().unwrap();
                match scenario {
                    Scenario::Resize => {
                        for client in &clients[..2] {
                            client.resize(25, 81).unwrap();
                        }
                    }
                    Scenario::Key => {
                        for client in &clients[..2] {
                            client
                                .send_key_input(TerminalKeyEvent::named(
                                    "enter", false, false, false,
                                ))
                                .unwrap();
                        }
                    }
                    Scenario::Mouse => {
                        for client in &clients[..2] {
                            client
                                .send_mouse_input(TerminalMouseInput {
                                    kind: TerminalMouseEventKind::Down,
                                    button: TerminalMouseButton::Left,
                                    row: 0,
                                    col: 0,
                                    alt: false,
                                    ctrl: false,
                                    shift: false,
                                })
                                .unwrap();
                        }
                    }
                    Scenario::Paste => {
                        for client in &clients[..2] {
                            client.send_paste("input".into()).unwrap();
                        }
                    }
                    Scenario::Output => {
                        for (ch, handle) in &peers[..2] {
                            handle.data(*ch, b"blocked-output".to_vec()).await.unwrap();
                        }
                    }
                    _ => unreachable!(),
                }
                // This caller is outside the Tokio worker pool; terminal operations must not block its workers.
                std::thread::sleep(Duration::from_secs(6));
                drop(second_lock);
                drop(first_lock);
                tokio::time::sleep(Duration::from_secs(1)).await;
                for (i, rx) in receivers.iter_mut().enumerate().rev() {
                    while let Ok(event) = rx.try_recv() {
                        if matches!(
                            event,
                            SessionRuntimeEvent::Disconnected | SessionRuntimeEvent::Error(_)
                        ) {
                            panic!("session {i} disconnected during {scenario:?}: {event:?}");
                        }
                    }
                }
            } else {
                tokio::time::sleep(Duration::from_secs(17)).await;
            }
            for (i, client) in clients.iter().enumerate() {
                client
                    .send_text_input(format!("probe-before-close-{i}"))
                    .unwrap();
            }
            tokio::time::timeout(Duration::from_secs(3), async {
                while inputs.load(Ordering::SeqCst) < 3 {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
                loop {
                    if clients.iter().enumerate().all(|(i, client)| {
                        client
                            .terminal()
                            .lock()
                            .unwrap()
                            .screen_text()
                            .contains(&format!("probe-before-close-{i}"))
                    }) {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            })
            .await
            .expect("all three sessions must deliver and display new input after recovery");
            clients[0].disconnect().unwrap();
            tokio::time::sleep(Duration::from_millis(250)).await;
            let before = inputs.load(Ordering::SeqCst);
            for client in &clients[1..] {
                client.send_text_input("probe-still-alive".into()).unwrap();
            }
            tokio::time::timeout(Duration::from_secs(3), async {
                while inputs.load(Ordering::SeqCst) < before + 2 {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            })
            .await
            .expect("remaining sessions must still deliver input");
            for rx in &mut receivers[1..] {
                while let Ok(event) = rx.try_recv() {
                    assert!(
                        !matches!(
                            event,
                            SessionRuntimeEvent::Disconnected | SessionRuntimeEvent::Error(_)
                        ),
                        "surviving session: {event:?}"
                    );
                }
            }
            for client in &clients[1..] {
                client.disconnect().unwrap();
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        })
        .await
        .expect("SSH stability scenario exceeded its deadline");
    });
}

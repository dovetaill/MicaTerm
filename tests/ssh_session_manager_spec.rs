use std::future::Future;
use std::fs;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Result, anyhow};
use mica_term::app::async_runtime::AppAsyncRuntime;
use mica_term::app::ssh::known_hosts::KnownHostsService;
use mica_term::app::ssh::profile::{ConnectionProfile, SshAuthMethod};
use mica_term::app::ssh::runtime::{SessionRuntimeEvent, SshSessionRuntime};
use mica_term::app::ssh::session_manager::{
    OpenSessionMode, SessionManager, SessionRuntimeLauncher, SessionState,
};
use russh::keys::PrivateKey;
use russh::keys::ssh_key::rand_core::OsRng;
use russh::server::{Auth, Session};
use russh::{Channel, ChannelId, server};
use tokio::sync::mpsc;
use tokio::net::TcpListener;
use uuid::Uuid;

static KNOWN_HOSTS_ENV_LOCK: Mutex<()> = Mutex::new(());

#[derive(Clone)]
struct FakeLauncher {
    behavior: FakeLauncherBehavior,
}

#[derive(Clone, Default)]
struct RuntimeBackedLauncher;

#[derive(Clone, Copy)]
enum FakeLauncherBehavior {
    StayConnecting,
    FailImmediately,
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

impl SessionRuntimeLauncher for FakeLauncher {
    fn launch(
        &self,
        _profile: ConnectionProfile,
        _session_id: Uuid,
        event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>> {
        let behavior = self.behavior;
        Box::pin(async move {
            match behavior {
                FakeLauncherBehavior::StayConnecting => {
                    tokio::time::sleep(Duration::from_millis(25)).await;
                    Ok(())
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
        event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>> {
        Box::pin(async move {
            let _runtime = SshSessionRuntime::connect(profile, session_id, event_tx).await?;
            Ok(())
        })
    }

    fn probe(
        &self,
        profile: ConnectionProfile,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>> {
        Box::pin(async move {
            let (event_tx, _event_rx) = mpsc::unbounded_channel();
            let runtime = SshSessionRuntime::connect(profile, Uuid::new_v4(), event_tx).await?;
            runtime.disconnect()?;
            Ok(())
        })
    }
}

#[derive(Clone)]
struct InteractiveTestServer {
    auth_key: russh::keys::PublicKey,
    shell_ready_delay: Duration,
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
        _term: &str,
        _col_width: u32,
        _row_height: u32,
        _pix_width: u32,
        _pix_height: u32,
        _modes: &[(russh::Pty, u32)],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        tokio::time::sleep(self.shell_ready_delay).await;
        let _ = session.channel_success(channel);
        Ok(())
    }

    async fn shell_request(
        &mut self,
        channel: ChannelId,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        tokio::time::sleep(self.shell_ready_delay).await;
        let _ = session.channel_success(channel);
        session.data(channel, b"welcome to mica-term".to_vec())?;
        Ok(())
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

async fn spawn_publickey_shell_server(
    shell_ready_delay: Duration,
) -> (
    tokio::task::JoinHandle<()>,
    std::net::SocketAddr,
    std::path::PathBuf,
    russh::keys::PublicKey,
) {
    let client_key = PrivateKey::random(&mut OsRng, russh::keys::Algorithm::Ed25519)
        .expect("generate client key");
    let client_public = client_key.public_key().clone();
    let private_key_path = temp_private_key_path("client");
    fs::write(
        &private_key_path,
        client_key
            .to_openssh(russh::keys::ssh_key::LineEnding::LF)
            .expect("encode private key"),
    )
    .expect("write client private key");

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
    let server = InteractiveTestServer {
        auth_key: client_public,
        shell_ready_delay,
    };

    let join = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.expect("accept ssh client");
        server::run_stream(config, socket, server)
            .await
            .expect("run ssh server");
    });

    (join, addr, private_key_path, server_public)
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
        remark: "Primary entry point".into(),
    }
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
    let (server_task, addr, private_key_path, server_public_key) = runtime.block_on(async {
        spawn_publickey_shell_server(Duration::from_millis(75)).await
    });
    let known_hosts_path = temp_known_hosts_path("runtime-ready");
    let known_hosts = KnownHostsService::new(&known_hosts_path);
    known_hosts
        .accept_unknown(addr.ip().to_string().as_str(), addr.port(), &server_public_key)
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

    runtime.block_on(async {
        tokio::time::sleep(Duration::from_millis(150)).await;
    });

    let updated = manager
        .session(handle.session_id)
        .expect("resolve failed runtime session");

    assert!(matches!(updated.state, SessionState::Error(_)));
    assert!(updated.can_reconnect);
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

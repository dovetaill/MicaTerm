use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Result, anyhow};
use mica_term::app::async_runtime::AppAsyncRuntime;
use mica_term::app::ssh::profile::{ConnectionProfile, SshAuthMethod};
use mica_term::app::ssh::runtime::SessionRuntimeEvent;
use mica_term::app::ssh::session_manager::{
    OpenSessionMode, SessionManager, SessionRuntimeLauncher, SessionState,
};
use tokio::sync::mpsc;
use uuid::Uuid;

#[derive(Clone)]
struct FakeLauncher {
    behavior: FakeLauncherBehavior,
}

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
        remark: "Primary entry point".into(),
    }
}

#[test]
fn session_manager_creates_connecting_session_handle() {
    let runtime = AppAsyncRuntime::new().expect("create app async runtime");
    let manager =
        SessionManager::new_with_launcher(runtime.handle(), Arc::new(FakeLauncher::stay_connecting()));

    let handle = manager
        .open_session(sample_profile("asset-prod"), OpenSessionMode::ActivateExisting)
        .expect("open session");

    assert_eq!(handle.asset_id, "asset-prod");
    assert_eq!(handle.title, "Prod Bastion");
    assert_eq!(handle.subtitle, "ops@example.com:22");
    assert_eq!(handle.state, SessionState::Connecting);
}

#[test]
fn session_manager_reuses_existing_session_for_same_asset_by_default() {
    let runtime = AppAsyncRuntime::new().expect("create app async runtime");
    let manager =
        SessionManager::new_with_launcher(runtime.handle(), Arc::new(FakeLauncher::stay_connecting()));

    let first = manager
        .open_session(sample_profile("asset-prod"), OpenSessionMode::ActivateExisting)
        .expect("open first session");
    let second = manager
        .open_session(sample_profile("asset-prod"), OpenSessionMode::ActivateExisting)
        .expect("open second session");

    assert_eq!(first.session_id, second.session_id);
}

#[test]
fn session_manager_can_force_new_tab_session_for_same_asset() {
    let runtime = AppAsyncRuntime::new().expect("create app async runtime");
    let manager =
        SessionManager::new_with_launcher(runtime.handle(), Arc::new(FakeLauncher::stay_connecting()));

    let first = manager
        .open_session(sample_profile("asset-prod"), OpenSessionMode::ActivateExisting)
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
        .open_session(sample_profile("asset-prod"), OpenSessionMode::ActivateExisting)
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

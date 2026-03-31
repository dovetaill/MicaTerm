use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Result, anyhow};
use mica_term::app::async_runtime::AppAsyncRuntime;
use mica_term::app::ssh::connection_progress::{
    ConnectionDiagnosticLine, ConnectionHeadlineState, ConnectionProgressEvent,
    ConnectionStepState, ConnectionStepStateItem,
};
use mica_term::app::ssh::profile::{ConnectionProfile, ConnectionProxyProfile, SshAuthMethod};
use mica_term::app::ssh::runtime::{
    SessionRuntimeEvent, TerminalKeyEvent, TerminalMouseInput, UnknownHostKeyError,
};
use mica_term::app::ssh::session_manager::{
    OpenSessionMode, SessionManager, SessionRuntimeControl, SessionRuntimeLauncher, SessionState,
};
use tokio::sync::mpsc;
use uuid::Uuid;

struct NoopRuntimeControl;

impl SessionRuntimeControl for NoopRuntimeControl {
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
}

#[derive(Clone)]
enum ScriptedEvent {
    Headline(ConnectionHeadlineState),
    Step(ConnectionStepStateItem),
    Diagnostic(String),
    StaleStep(ConnectionStepStateItem),
    Connected,
}

#[derive(Clone)]
enum LaunchOutcome {
    Ready,
    Fail(&'static str),
    UnknownHostKey,
}

#[derive(Clone)]
struct ScriptedProgressLauncher {
    events: Arc<Vec<ScriptedEvent>>,
    outcome: LaunchOutcome,
}

impl ScriptedProgressLauncher {
    fn new(events: Vec<ScriptedEvent>, outcome: LaunchOutcome) -> Self {
        Self {
            events: Arc::new(events),
            outcome,
        }
    }
}

impl SessionRuntimeLauncher for ScriptedProgressLauncher {
    fn launch(
        &self,
        _profile: ConnectionProfile,
        _session_id: Uuid,
        attempt_id: Uuid,
        event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn SessionRuntimeControl>>> + Send + 'static>>
    {
        let events = Arc::clone(&self.events);
        let outcome = self.outcome.clone();
        Box::pin(async move {
            for event in events.iter() {
                match event {
                    ScriptedEvent::Headline(headline) => {
                        let _ = event_tx.send(SessionRuntimeEvent::ConnectionProgress(
                            ConnectionProgressEvent::HeadlineChanged {
                                attempt_id,
                                headline: *headline,
                            },
                        ));
                    }
                    ScriptedEvent::Step(step) => {
                        let _ = event_tx.send(SessionRuntimeEvent::ConnectionProgress(
                            ConnectionProgressEvent::StepUpdated {
                                attempt_id,
                                step: step.clone(),
                            },
                        ));
                    }
                    ScriptedEvent::Diagnostic(message) => {
                        let _ = event_tx.send(SessionRuntimeEvent::ConnectionProgress(
                            ConnectionProgressEvent::DiagnosticAppended {
                                attempt_id,
                                message: message.clone(),
                            },
                        ));
                    }
                    ScriptedEvent::StaleStep(step) => {
                        let _ = event_tx.send(SessionRuntimeEvent::ConnectionProgress(
                            ConnectionProgressEvent::StepUpdated {
                                attempt_id: Uuid::new_v4(),
                                step: step.clone(),
                            },
                        ));
                    }
                    ScriptedEvent::Connected => {
                        let _ = event_tx.send(SessionRuntimeEvent::Connected);
                    }
                }
            }

            match outcome {
                LaunchOutcome::Ready => Ok(Box::new(NoopRuntimeControl) as Box<dyn SessionRuntimeControl>),
                LaunchOutcome::Fail(message) => Err(anyhow!(message)),
                LaunchOutcome::UnknownHostKey => Err(UnknownHostKeyError {
                    host: "example.com".into(),
                    port: 22,
                    fingerprint: "SHA256:blocked-host-key".into(),
                    public_key_openssh:
                        "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAILM+rvN+ot98qgEN796jTiQfZfG1KaT0PtFDJ/XFSqti timeline-test@example.com"
                            .into(),
                }
                .into()),
            }
        })
    }

    fn probe(
        &self,
        _profile: ConnectionProfile,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>> {
        Box::pin(async move { Ok(()) })
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
        password: Some("secret".into()),
        private_key_content: None,
        passphrase: None,
        proxy: ConnectionProxyProfile::None,
        resolved_proxy_hops: Vec::new(),
        remark: "Primary entry point".into(),
    }
}

fn running_step(
    step_id: &str,
    step_kind: &str,
    hop_label: &str,
    detail: &str,
) -> ConnectionStepStateItem {
    ConnectionStepStateItem {
        step_id: step_id.into(),
        step_kind: step_kind.into(),
        title: step_kind.into(),
        detail: detail.into(),
        hop_label: hop_label.into(),
        state: ConnectionStepState::Running,
    }
}

fn done_step(
    step_id: &str,
    step_kind: &str,
    hop_label: &str,
    detail: &str,
) -> ConnectionStepStateItem {
    ConnectionStepStateItem {
        state: ConnectionStepState::Done,
        ..running_step(step_id, step_kind, hop_label, detail)
    }
}

fn failed_step(
    step_id: &str,
    step_kind: &str,
    hop_label: &str,
    detail: &str,
) -> ConnectionStepStateItem {
    ConnectionStepStateItem {
        state: ConnectionStepState::Failed,
        ..running_step(step_id, step_kind, hop_label, detail)
    }
}

fn blocked_step(
    step_id: &str,
    step_kind: &str,
    hop_label: &str,
    detail: &str,
) -> ConnectionStepStateItem {
    ConnectionStepStateItem {
        state: ConnectionStepState::Blocked,
        ..running_step(step_id, step_kind, hop_label, detail)
    }
}

#[test]
fn multi_hop_connection_session_manager_aggregates_attempt_steps_and_ignores_stale_events() {
    let runtime = AppAsyncRuntime::new().expect("create app async runtime");
    let launcher = ScriptedProgressLauncher::new(
        vec![
            ScriptedEvent::Step(running_step(
                "00-resolve-profile",
                "resolve-profile",
                "Target",
                "Resolving profile",
            )),
            ScriptedEvent::Step(done_step(
                "00-resolve-profile",
                "resolve-profile",
                "Target",
                "Resolved profile",
            )),
            ScriptedEvent::StaleStep(done_step("99-stale", "stale-step", "Stale", "stale detail")),
            ScriptedEvent::Diagnostic("Resolved profile".into()),
            ScriptedEvent::Step(running_step(
                "01-connect-jump-host",
                "connect-jump-host",
                "Jump Host 1",
                "Opening SSH transport",
            )),
            ScriptedEvent::Step(done_step(
                "01-connect-jump-host",
                "connect-jump-host",
                "Jump Host 1",
                "Connected to jump host",
            )),
            ScriptedEvent::Headline(ConnectionHeadlineState::Connected),
            ScriptedEvent::Connected,
        ],
        LaunchOutcome::Ready,
    );
    let manager = SessionManager::new_with_launcher(runtime.handle(), Arc::new(launcher));

    let handle = manager
        .open_session(sample_profile("asset-prod"), OpenSessionMode::ForceNewTab)
        .expect("open scripted session");
    runtime.block_on(async {
        tokio::time::sleep(Duration::from_millis(20)).await;
    });

    let attempt = manager
        .connection_attempt(handle.session_id)
        .expect("connection attempt should exist");
    let session = manager
        .session(handle.session_id)
        .expect("session should exist");

    assert_eq!(attempt.headline, ConnectionHeadlineState::Connected);
    assert_eq!(
        attempt
            .steps
            .iter()
            .map(|step| (
                step.step_id.clone(),
                step.step_kind.clone(),
                step.hop_label.clone(),
                step.state
            ))
            .collect::<Vec<_>>(),
        vec![
            (
                "00-resolve-profile".into(),
                "resolve-profile".into(),
                "Target".into(),
                ConnectionStepState::Done,
            ),
            (
                "01-connect-jump-host".into(),
                "connect-jump-host".into(),
                "Jump Host 1".into(),
                ConnectionStepState::Done,
            ),
        ],
        "session manager should upsert step updates in place and ignore stale attempt events"
    );
    assert_eq!(
        attempt.diagnostics,
        vec![ConnectionDiagnosticLine {
            attempt_id: attempt.attempt_id,
            message: "Resolved profile".into(),
        }]
    );
    assert_eq!(session.state, SessionState::Connected);
}

#[test]
fn multi_hop_connection_session_manager_preserves_failing_hop_label_and_message() {
    let runtime = AppAsyncRuntime::new().expect("create app async runtime");
    let launcher = ScriptedProgressLauncher::new(
        vec![
            ScriptedEvent::Step(running_step(
                "03-open-direct-tcpip",
                "open-direct-tcpip",
                "Jump Host 1",
                "Opening SSH tunnel",
            )),
            ScriptedEvent::Step(failed_step(
                "03-open-direct-tcpip",
                "open-direct-tcpip",
                "Jump Host 1",
                "SSH upstream 'Proxy A' rejected direct-tcpip forwarding",
            )),
            ScriptedEvent::Diagnostic(
                "SSH upstream 'Proxy A' rejected direct-tcpip forwarding".into(),
            ),
        ],
        LaunchOutcome::Fail("SSH upstream 'Proxy A' rejected direct-tcpip forwarding"),
    );
    let manager = SessionManager::new_with_launcher(runtime.handle(), Arc::new(launcher));

    let handle = manager
        .open_session(sample_profile("asset-prod"), OpenSessionMode::ForceNewTab)
        .expect("open scripted session");
    runtime.block_on(async {
        tokio::time::sleep(Duration::from_millis(20)).await;
    });

    let attempt = manager
        .connection_attempt(handle.session_id)
        .expect("connection attempt should exist");
    let session = manager
        .session(handle.session_id)
        .expect("session should exist");
    let failed_step = attempt
        .steps
        .iter()
        .find(|step| step.state == ConnectionStepState::Failed)
        .expect("failed step should be recorded");

    assert_eq!(attempt.headline, ConnectionHeadlineState::Error);
    assert_eq!(failed_step.step_kind, "open-direct-tcpip");
    assert_eq!(failed_step.hop_label, "Jump Host 1");
    assert_eq!(
        failed_step.detail,
        "SSH upstream 'Proxy A' rejected direct-tcpip forwarding"
    );
    assert_eq!(
        session.state,
        SessionState::Error("SSH upstream 'Proxy A' rejected direct-tcpip forwarding".into())
    );
    assert!(session.can_reconnect);
}

#[test]
fn host_key_block_keeps_connection_timeline_waiting_for_user() {
    let runtime = AppAsyncRuntime::new().expect("create app async runtime");
    let launcher = ScriptedProgressLauncher::new(
        vec![
            ScriptedEvent::Step(blocked_step(
                "02-verify-host-key",
                "verify-host-key",
                "Target",
                "Host key verification required for example.com",
            )),
            ScriptedEvent::Headline(ConnectionHeadlineState::WaitingUser),
            ScriptedEvent::Diagnostic("Host key verification required for example.com".into()),
        ],
        LaunchOutcome::UnknownHostKey,
    );
    let manager = SessionManager::new_with_launcher(runtime.handle(), Arc::new(launcher));

    let handle = manager
        .open_session(sample_profile("asset-prod"), OpenSessionMode::ForceNewTab)
        .expect("open scripted session");
    runtime.block_on(async {
        tokio::time::sleep(Duration::from_millis(20)).await;
    });

    let attempt = manager
        .connection_attempt(handle.session_id)
        .expect("connection attempt should exist");
    let session = manager
        .session(handle.session_id)
        .expect("session should exist");

    assert_eq!(
        attempt.headline,
        ConnectionHeadlineState::WaitingUser,
        "host key confirmation should keep the timeline blocked in waiting-user instead of collapsing into a generic error"
    );
    assert_eq!(
        attempt.steps,
        vec![blocked_step(
            "02-verify-host-key",
            "verify-host-key",
            "Target",
            "Host key verification required for example.com",
        )]
    );
    assert_eq!(
        session.state,
        SessionState::WaitingUser,
        "waiting for host-key confirmation should surface a dedicated waiting-user session state"
    );
}

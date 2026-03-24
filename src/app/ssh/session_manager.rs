//! Session manager for SSH tabs and runtime event projection.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::app::ssh::profile::ConnectionProfile;
use crate::app::ssh::runtime::{SessionRuntimeEvent, TerminalSurfaceState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenSessionMode {
    ActivateExisting,
    ForceNewTab,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionState {
    Connecting,
    Connected,
    Disconnected,
    Error(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionHandle {
    pub session_id: Uuid,
    pub asset_id: String,
    pub title: String,
    pub subtitle: String,
    pub state: SessionState,
    pub can_reconnect: bool,
}

pub trait SessionRuntimeLauncher: Send + Sync {
    fn launch(
        &self,
        profile: ConnectionProfile,
        session_id: Uuid,
        event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>>;

    fn probe(
        &self,
        profile: ConnectionProfile,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>>;
}

#[derive(Clone)]
pub struct SessionManager {
    runtime_handle: tokio::runtime::Handle,
    launcher: Arc<dyn SessionRuntimeLauncher>,
    registry: Arc<Mutex<SessionRegistry>>,
}

impl SessionManager {
    pub fn new_with_launcher(
        runtime_handle: tokio::runtime::Handle,
        launcher: Arc<dyn SessionRuntimeLauncher>,
    ) -> Self {
        Self {
            runtime_handle,
            launcher,
            registry: Arc::new(Mutex::new(SessionRegistry::default())),
        }
    }

    pub fn open_session(
        &self,
        profile: ConnectionProfile,
        mode: OpenSessionMode,
    ) -> Result<SessionHandle> {
        let asset_id = profile
            .asset_id
            .clone()
            .context("session profile requires asset_id")?;

        if matches!(mode, OpenSessionMode::ActivateExisting) {
            let registry = self.registry.lock().expect("lock session registry");
            if let Some(existing_id) = registry.asset_sessions.get(&asset_id)
                && let Some(existing) = registry.sessions.get(existing_id)
            {
                return Ok(existing.clone());
            }
        }

        let session_id = Uuid::new_v4();
        let handle = SessionHandle {
            session_id,
            asset_id: asset_id.clone(),
            title: profile.name.clone(),
            subtitle: format!("{}@{}:{}", profile.user, profile.host, profile.port),
            state: SessionState::Connecting,
            can_reconnect: false,
        };

        {
            let mut registry = self.registry.lock().expect("lock session registry");
            registry.asset_sessions.insert(asset_id, session_id);
            registry.sessions.insert(session_id, handle.clone());
            registry.open_order.push(session_id);
        }

        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let registry_for_events = Arc::clone(&self.registry);
        self.runtime_handle.spawn(async move {
            while let Some(event) = event_rx.recv().await {
                apply_runtime_event(&registry_for_events, session_id, event);
            }
        });

        let launcher = Arc::clone(&self.launcher);
        let registry_for_launch = Arc::clone(&self.registry);
        self.runtime_handle.spawn(async move {
            if let Err(error) = launcher.launch(profile, session_id, event_tx).await {
                update_session(
                    &registry_for_launch,
                    session_id,
                    SessionState::Error(error.to_string()),
                    true,
                );
            }
        });

        Ok(handle)
    }

    pub fn session(&self, session_id: Uuid) -> Option<SessionHandle> {
        self.registry
            .lock()
            .expect("lock session registry")
            .sessions
            .get(&session_id)
            .cloned()
    }

    pub fn ordered_sessions(&self) -> Vec<SessionHandle> {
        let registry = self.registry.lock().expect("lock session registry");
        registry
            .open_order
            .iter()
            .filter_map(|session_id| registry.sessions.get(session_id).cloned())
            .collect()
    }

    pub fn terminal_surface(&self, session_id: Uuid) -> Option<TerminalSurfaceState> {
        self.registry
            .lock()
            .expect("lock session registry")
            .terminal_surfaces
            .get(&session_id)
            .cloned()
    }

    pub fn probe_connection(&self, profile: ConnectionProfile) -> Result<()> {
        self.runtime_handle.block_on(self.launcher.probe(profile))
    }

    pub fn disconnect_session(&self, session_id: Uuid) -> Option<SessionHandle> {
        let mut registry = self.registry.lock().expect("lock session registry");
        let session = registry.sessions.get_mut(&session_id)?;
        session.state = SessionState::Disconnected;
        session.can_reconnect = true;
        Some(session.clone())
    }

    pub fn close_session(&self, session_id: Uuid) -> Option<SessionHandle> {
        let mut registry = self.registry.lock().expect("lock session registry");
        let removed = registry.sessions.remove(&session_id)?;
        registry.open_order.retain(|existing_id| *existing_id != session_id);
        registry.terminal_surfaces.remove(&session_id);
        if registry.asset_sessions.get(&removed.asset_id) == Some(&session_id) {
            let replacement = registry
                .open_order
                .iter()
                .rev()
                .copied()
                .find(|existing_id| {
                    registry
                        .sessions
                        .get(existing_id)
                        .map(|session| session.asset_id == removed.asset_id)
                        .unwrap_or(false)
                });

            if let Some(existing_id) = replacement {
                registry
                    .asset_sessions
                    .insert(removed.asset_id.clone(), existing_id);
            } else {
                registry.asset_sessions.remove(&removed.asset_id);
            }
        }
        Some(removed)
    }
}

#[derive(Debug, Default)]
struct SessionRegistry {
    sessions: HashMap<Uuid, SessionHandle>,
    asset_sessions: HashMap<String, Uuid>,
    open_order: Vec<Uuid>,
    terminal_surfaces: HashMap<Uuid, TerminalSurfaceState>,
}

fn apply_runtime_event(
    registry: &Arc<Mutex<SessionRegistry>>,
    session_id: Uuid,
    event: SessionRuntimeEvent,
) {
    match event {
        SessionRuntimeEvent::Connected => {
            update_session(registry, session_id, SessionState::Connected, false);
        }
        SessionRuntimeEvent::Disconnected => {
            update_session(registry, session_id, SessionState::Disconnected, true);
        }
        SessionRuntimeEvent::Error(message) => {
            update_session(registry, session_id, SessionState::Error(message), true);
        }
        SessionRuntimeEvent::SurfaceChanged(surface) => {
            update_terminal_surface(registry, session_id, surface);
        }
    }
}

fn update_session(
    registry: &Arc<Mutex<SessionRegistry>>,
    session_id: Uuid,
    state: SessionState,
    can_reconnect: bool,
) {
    if let Some(session) = registry
        .lock()
        .expect("lock session registry")
        .sessions
        .get_mut(&session_id)
    {
        session.state = state;
        session.can_reconnect = can_reconnect;
    }
}

fn update_terminal_surface(
    registry: &Arc<Mutex<SessionRegistry>>,
    session_id: Uuid,
    surface: TerminalSurfaceState,
) {
    let mut registry = registry.lock().expect("lock session registry");
    if registry.sessions.contains_key(&session_id) {
        registry.terminal_surfaces.insert(session_id, surface);
    }
}

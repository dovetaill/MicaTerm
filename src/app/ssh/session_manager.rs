//! Session manager for SSH tabs and runtime event projection.

use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result, anyhow};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::app::ssh::profile::ConnectionProfile;
use crate::app::ssh::runtime::{
    SessionRuntimeEvent, TerminalKeyEvent, TerminalMouseInput, TerminalSurfaceState,
};
use crate::theme::ThemeMode;

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

pub trait SessionRuntimeControl: Send {
    fn disconnect(&self) -> Result<()>;
    fn send_text_input(&self, text: String) -> Result<()>;
    fn send_key_input(&self, event: TerminalKeyEvent) -> Result<()>;
    fn send_mouse_input(&self, event: TerminalMouseInput) -> Result<()>;
    fn send_paste(&self, text: String) -> Result<()>;
    fn resize(&self, rows: u32, cols: u32) -> Result<()>;
    fn update_theme_mode(&self, _mode: ThemeMode) -> Result<Option<TerminalSurfaceState>> {
        Ok(None)
    }
    fn scroll_viewport_lines(&self, _delta: i32) -> Result<TerminalSurfaceState> {
        Err(anyhow!("session runtime does not support local scrollback"))
    }
}

type LaunchFuture =
    Pin<Box<dyn Future<Output = Result<Box<dyn SessionRuntimeControl>>> + Send + 'static>>;
type ProbeFuture = Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>>;

pub trait SessionRuntimeLauncher: Send + Sync {
    fn launch(
        &self,
        profile: ConnectionProfile,
        session_id: Uuid,
        event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    ) -> LaunchFuture;

    fn probe(&self, profile: ConnectionProfile) -> ProbeFuture;
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
            match launcher.launch(profile, session_id, event_tx).await {
                Ok(runtime_control) => {
                    attach_runtime_control(&registry_for_launch, session_id, runtime_control);
                }
                Err(error) => {
                    update_session(
                        &registry_for_launch,
                        session_id,
                        SessionState::Error(error.to_string()),
                        true,
                    );
                }
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
        let result = self.runtime_handle.block_on(self.launcher.probe(profile));
        match &result {
            Ok(()) => {}
            Err(error) => tracing::error!(
                target: "app.ssh",
                error = %error,
                "session manager probe failed"
            ),
        }
        result
    }

    pub fn disconnect_session(&self, session_id: Uuid) -> Option<SessionHandle> {
        let (updated, runtime_control) = {
            let mut registry = self.registry.lock().expect("lock session registry");
            let session = registry.sessions.get_mut(&session_id)?;
            session.state = SessionState::Disconnected;
            session.can_reconnect = true;
            let updated = session.clone();
            let runtime_control = registry.runtime_controls.remove(&session_id);
            if runtime_control.is_none() {
                registry.pending_disconnects.insert(session_id);
            }
            (updated, runtime_control)
        };

        if let Some(runtime_control) = runtime_control {
            let _ = runtime_control.disconnect();
        }

        Some(updated)
    }

    pub fn send_session_text_input(&self, session_id: Uuid, text: String) -> Result<()> {
        let registry = self.registry.lock().expect("lock session registry");
        let runtime_control = registry
            .runtime_controls
            .get(&session_id)
            .ok_or_else(|| anyhow!("session runtime is not ready for `{session_id}`"))?;
        runtime_control.send_text_input(text)
    }

    pub fn send_session_key_input(&self, session_id: Uuid, event: TerminalKeyEvent) -> Result<()> {
        let registry = self.registry.lock().expect("lock session registry");
        let runtime_control = registry
            .runtime_controls
            .get(&session_id)
            .ok_or_else(|| anyhow!("session runtime is not ready for `{session_id}`"))?;
        runtime_control.send_key_input(event)
    }

    pub fn resize_session(&self, session_id: Uuid, rows: u32, cols: u32) -> Result<()> {
        let mut registry = self.registry.lock().expect("lock session registry");
        if let Some(runtime_control) = registry.runtime_controls.get(&session_id) {
            return runtime_control.resize(rows, cols);
        }
        if registry.sessions.contains_key(&session_id) {
            registry.pending_resizes.insert(session_id, (rows, cols));
            return Ok(());
        }
        Err(anyhow!("session runtime is not ready for `{session_id}`"))
    }

    pub fn send_session_mouse_input(
        &self,
        session_id: Uuid,
        event: TerminalMouseInput,
    ) -> Result<()> {
        let registry = self.registry.lock().expect("lock session registry");
        let runtime_control = registry
            .runtime_controls
            .get(&session_id)
            .ok_or_else(|| anyhow!("session runtime is not ready for `{session_id}`"))?;
        runtime_control.send_mouse_input(event)
    }

    pub fn send_session_paste(&self, session_id: Uuid, text: String) -> Result<()> {
        let registry = self.registry.lock().expect("lock session registry");
        let runtime_control = registry
            .runtime_controls
            .get(&session_id)
            .ok_or_else(|| anyhow!("session runtime is not ready for `{session_id}`"))?;
        runtime_control.send_paste(text)
    }

    pub fn scroll_session_viewport(&self, session_id: Uuid, delta: i32) -> Result<()> {
        let surface = {
            let registry = self.registry.lock().expect("lock session registry");
            let runtime_control = registry
                .runtime_controls
                .get(&session_id)
                .ok_or_else(|| anyhow!("session runtime is not ready for `{session_id}`"))?;
            runtime_control.scroll_viewport_lines(delta)?
        };

        update_terminal_surface(&self.registry, session_id, surface);
        Ok(())
    }

    pub fn scroll_session_to_top(&self, session_id: Uuid) -> Result<()> {
        let (_, max_offset) = self.viewport_offsets(session_id)?;
        self.scroll_session_to_offset(session_id, max_offset)
    }

    pub fn scroll_session_to_bottom(&self, session_id: Uuid) -> Result<()> {
        self.scroll_session_to_offset(session_id, 0)
    }

    pub fn scroll_session_to_ratio(&self, session_id: Uuid, ratio: f32) -> Result<()> {
        let (_, max_offset) = self.viewport_offsets(session_id)?;
        let target = ((max_offset as f32) * ratio.clamp(0.0, 1.0)).round() as u32;
        self.scroll_session_to_offset(session_id, target)
    }

    fn viewport_offsets(&self, session_id: Uuid) -> Result<(u32, u32)> {
        let registry = self.registry.lock().expect("lock session registry");
        let surface = registry
            .terminal_surfaces
            .get(&session_id)
            .ok_or_else(|| anyhow!("session terminal surface is not ready for `{session_id}`"))?;
        Ok((surface.viewport_offset_lines, surface.viewport_max_offset_lines))
    }

    fn scroll_session_to_offset(&self, session_id: Uuid, target_offset: u32) -> Result<()> {
        let (current_offset, max_offset) = self.viewport_offsets(session_id)?;
        let target_offset = target_offset.min(max_offset);
        let delta = i64::from(target_offset) - i64::from(current_offset);
        let delta =
            i32::try_from(delta).context("session viewport delta exceeded supported range")?;
        if delta == 0 {
            return Ok(());
        }

        self.scroll_session_viewport(session_id, delta)
    }

    pub fn set_theme_mode(&self, mode: ThemeMode) -> Result<()> {
        let session_ids = {
            let mut registry = self.registry.lock().expect("lock session registry");
            registry.theme_mode = mode;
            registry.runtime_controls.keys().copied().collect::<Vec<_>>()
        };

        for session_id in session_ids {
            let surface = {
                let registry = self.registry.lock().expect("lock session registry");
                let Some(runtime_control) = registry.runtime_controls.get(&session_id) else {
                    continue;
                };
                runtime_control.update_theme_mode(mode)?
            };

            if let Some(surface) = surface {
                update_terminal_surface(&self.registry, session_id, surface);
            }
        }

        Ok(())
    }

    pub fn close_session(&self, session_id: Uuid) -> Option<SessionHandle> {
        let (removed, runtime_control) = {
            let mut registry = self.registry.lock().expect("lock session registry");
            let removed = registry.sessions.remove(&session_id)?;
            registry
                .open_order
                .retain(|existing_id| *existing_id != session_id);
            registry.terminal_surfaces.remove(&session_id);
            registry.pending_disconnects.remove(&session_id);
            registry.pending_resizes.remove(&session_id);
            let runtime_control = registry.runtime_controls.remove(&session_id);
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
            (removed, runtime_control)
        };

        if let Some(runtime_control) = runtime_control {
            let _ = runtime_control.disconnect();
        }

        Some(removed)
    }
}

struct SessionRegistry {
    sessions: HashMap<Uuid, SessionHandle>,
    asset_sessions: HashMap<String, Uuid>,
    open_order: Vec<Uuid>,
    terminal_surfaces: HashMap<Uuid, TerminalSurfaceState>,
    runtime_controls: HashMap<Uuid, Box<dyn SessionRuntimeControl>>,
    pending_disconnects: HashSet<Uuid>,
    pending_resizes: HashMap<Uuid, (u32, u32)>,
    theme_mode: ThemeMode,
}

impl Default for SessionRegistry {
    fn default() -> Self {
        Self {
            sessions: HashMap::new(),
            asset_sessions: HashMap::new(),
            open_order: Vec::new(),
            terminal_surfaces: HashMap::new(),
            runtime_controls: HashMap::new(),
            pending_disconnects: HashSet::new(),
            pending_resizes: HashMap::new(),
            theme_mode: ThemeMode::Dark,
        }
    }
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
            clear_runtime_control(registry, session_id);
            update_session(registry, session_id, SessionState::Disconnected, true);
        }
        SessionRuntimeEvent::Error(message) => {
            tracing::error!(
                target: "app.ssh",
                session_id = session_id.to_string(),
                error = message.as_str(),
                "session manager received runtime error event"
            );
            clear_runtime_control(registry, session_id);
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

fn attach_runtime_control(
    registry: &Arc<Mutex<SessionRegistry>>,
    session_id: Uuid,
    runtime_control: Box<dyn SessionRuntimeControl>,
) {
    let mut runtime_control = Some(runtime_control);
    let (should_disconnect, pending_resize, theme_mode) = {
        let mut registry = registry.lock().expect("lock session registry");
        if !registry.sessions.contains_key(&session_id)
            || registry.pending_disconnects.remove(&session_id)
        {
            (true, None, registry.theme_mode)
        } else {
            registry.runtime_controls.insert(
                session_id,
                runtime_control.take().expect("runtime control available"),
            );
            (
                false,
                registry.pending_resizes.remove(&session_id),
                registry.theme_mode,
            )
        }
    };

    if should_disconnect && let Some(runtime_control) = runtime_control {
        let _ = runtime_control.disconnect();
        return;
    }

    let theme_surface = {
        let registry_guard = registry.lock().expect("lock session registry");
        registry_guard
            .runtime_controls
            .get(&session_id)
            .and_then(|runtime_control| runtime_control.update_theme_mode(theme_mode).ok().flatten())
    };
    if let Some(surface) = theme_surface {
        update_terminal_surface(registry, session_id, surface);
    }

    if let Some((rows, cols)) = pending_resize {
        let registry = registry.lock().expect("lock session registry");
        if let Some(runtime_control) = registry.runtime_controls.get(&session_id) {
            let _ = runtime_control.resize(rows, cols);
        }
    }
}

fn clear_runtime_control(registry: &Arc<Mutex<SessionRegistry>>, session_id: Uuid) {
    let mut registry = registry.lock().expect("lock session registry");
    registry.runtime_controls.remove(&session_id);
    registry.pending_disconnects.remove(&session_id);
    registry.pending_resizes.remove(&session_id);
}

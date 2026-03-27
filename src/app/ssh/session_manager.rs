//! Session manager for SSH tabs and runtime event projection.

use std::collections::{HashMap, HashSet, VecDeque};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result, anyhow};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::app::ssh::profile::ConnectionProfile;
use crate::app::ssh::runtime::{
    SessionRuntimeEvent, TerminalKeyEvent, TerminalMouseInput, TerminalSurfaceSignature,
    TerminalSurfaceState,
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
    fn terminal_surface(&self) -> Result<TerminalSurfaceState> {
        Err(anyhow!("session runtime does not expose terminal surface snapshots"))
    }
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
            let mut pending_events = VecDeque::new();

            loop {
                let event = if let Some(event) = pending_events.pop_front() {
                    Some(event)
                } else {
                    event_rx.recv().await
                };
                let Some(event) = event else {
                    break;
                };

                match event {
                    SessionRuntimeEvent::SurfaceChanged(surface) => {
                        let mut backlog = VecDeque::new();
                        while let Ok(next) = event_rx.try_recv() {
                            backlog.push_back(next);
                        }
                        let (surface, remaining) = coalesce_surface_backlog(surface, backlog);
                        pending_events.extend(remaining);
                        apply_runtime_event(
                            &registry_for_events,
                            session_id,
                            SessionRuntimeEvent::SurfaceChanged(surface),
                        );
                    }
                    SessionRuntimeEvent::SurfaceDirty => {
                        let mut backlog = VecDeque::new();
                        while let Ok(next) = event_rx.try_recv() {
                            backlog.push_back(next);
                        }
                        pending_events.extend(coalesce_surface_dirty_backlog(backlog));
                        apply_runtime_event(
                            &registry_for_events,
                            session_id,
                            SessionRuntimeEvent::SurfaceDirty,
                        );
                    }
                    other => apply_runtime_event(&registry_for_events, session_id, other),
                }
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
        let should_refresh = {
            let registry = self.registry.lock().expect("lock session registry");
            terminal_surface_stale(&registry, session_id)
        };
        if should_refresh {
            refresh_runtime_surface(&self.registry, session_id);
        }

        self.registry
            .lock()
            .expect("lock session registry")
            .terminal_surfaces
            .get(&session_id)
            .cloned()
    }

    pub fn terminal_surface_signature(&self, session_id: Uuid) -> Option<TerminalSurfaceSignature> {
        let registry = self.registry.lock().expect("lock session registry");
        terminal_surface_signature_for_registry(&registry, session_id)
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
        Ok((
            surface.viewport_offset_lines,
            surface.viewport_max_offset_lines,
        ))
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
            registry
                .runtime_controls
                .keys()
                .copied()
                .collect::<Vec<_>>()
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
            registry.terminal_surface_revisions.remove(&session_id);
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
    terminal_surface_revisions: HashMap<Uuid, usize>,
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
            terminal_surface_revisions: HashMap::new(),
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
        SessionRuntimeEvent::SurfaceDirty => {
            mark_runtime_surface_dirty(registry, session_id);
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
        registry
            .terminal_surface_revisions
            .insert(session_id, surface.seqno);
        registry.terminal_surfaces.insert(session_id, surface);
    }
}

fn mark_runtime_surface_dirty(registry: &Arc<Mutex<SessionRegistry>>, session_id: Uuid) {
    let mut registry = registry.lock().expect("lock session registry");
    if !registry.sessions.contains_key(&session_id) {
        return;
    }

    let next_revision = registry
        .terminal_surface_revisions
        .get(&session_id)
        .copied()
        .or_else(|| {
            registry
                .terminal_surfaces
                .get(&session_id)
                .map(|surface| surface.seqno)
        })
        .unwrap_or(0)
        .saturating_add(1);
    registry
        .terminal_surface_revisions
        .insert(session_id, next_revision);
}

fn refresh_runtime_surface(registry: &Arc<Mutex<SessionRegistry>>, session_id: Uuid) {
    let surface = {
        let registry = registry.lock().expect("lock session registry");
        registry
            .runtime_controls
            .get(&session_id)
            .and_then(|runtime_control| runtime_control.terminal_surface().ok())
    };

    if let Some(surface) = surface {
        update_terminal_surface(registry, session_id, surface);
    }
}

fn terminal_surface_stale(registry: &SessionRegistry, session_id: Uuid) -> bool {
    let revision = registry
        .terminal_surface_revisions
        .get(&session_id)
        .copied()
        .unwrap_or(0);

    match registry.terminal_surfaces.get(&session_id) {
        Some(surface) => revision > surface.seqno,
        None => revision > 0 && registry.runtime_controls.contains_key(&session_id),
    }
}

fn terminal_surface_signature_for_registry(
    registry: &SessionRegistry,
    session_id: Uuid,
) -> Option<TerminalSurfaceSignature> {
    let mut signature = registry.terminal_surfaces.get(&session_id)?.signature();
    if let Some(revision) = registry.terminal_surface_revisions.get(&session_id).copied()
        && revision > signature.seqno
    {
        signature.seqno = revision;
    }

    Some(signature)
}

fn coalesce_surface_backlog(
    initial_surface: TerminalSurfaceState,
    mut backlog: VecDeque<SessionRuntimeEvent>,
) -> (TerminalSurfaceState, VecDeque<SessionRuntimeEvent>) {
    let mut latest_surface = initial_surface;

    while matches!(
        backlog.front(),
        Some(SessionRuntimeEvent::SurfaceChanged(_))
    ) {
        let Some(SessionRuntimeEvent::SurfaceChanged(surface)) = backlog.pop_front() else {
            break;
        };
        latest_surface = surface;
    }

    (latest_surface, backlog)
}

fn coalesce_surface_dirty_backlog(
    mut backlog: VecDeque<SessionRuntimeEvent>,
) -> VecDeque<SessionRuntimeEvent> {
    while matches!(backlog.front(), Some(SessionRuntimeEvent::SurfaceDirty)) {
        let _ = backlog.pop_front();
    }

    backlog
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
            .and_then(|runtime_control| {
                runtime_control.update_theme_mode(theme_mode).ok().flatten()
            })
    };
    if let Some(surface) = theme_surface {
        update_terminal_surface(registry, session_id, surface);
    } else {
        refresh_runtime_surface(registry, session_id);
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
    let surface_seqno = registry
        .terminal_surfaces
        .get(&session_id)
        .map(|surface| surface.seqno);
    if let Some(surface_seqno) = surface_seqno {
        registry
            .terminal_surface_revisions
            .insert(session_id, surface_seqno);
    } else {
        registry.terminal_surface_revisions.remove(&session_id);
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    use anyhow::Result;
    use uuid::Uuid;

    use super::{
        SessionHandle, SessionRegistry, SessionRuntimeControl, SessionRuntimeEvent, SessionState,
        apply_runtime_event, coalesce_surface_backlog, coalesce_surface_dirty_backlog,
        refresh_runtime_surface, terminal_surface_signature_for_registry, terminal_surface_stale,
        update_terminal_surface,
    };
    use crate::app::ssh::runtime::{
        TerminalKeyEvent, TerminalMouseInput, TerminalSurfaceState,
    };

    #[test]
    fn coalesces_consecutive_surface_updates_but_preserves_following_control_events() {
        let session_id = Uuid::new_v4();
        let initial =
            TerminalSurfaceState::from_visible_lines(session_id, 1, 24, 80, vec!["one".into()]);
        let newer =
            TerminalSurfaceState::from_visible_lines(session_id, 2, 24, 80, vec!["two".into()]);
        let latest =
            TerminalSurfaceState::from_visible_lines(session_id, 3, 24, 80, vec!["three".into()]);
        let backlog = VecDeque::from(vec![
            SessionRuntimeEvent::SurfaceChanged(newer),
            SessionRuntimeEvent::SurfaceChanged(latest.clone()),
            SessionRuntimeEvent::Error("boom".into()),
            SessionRuntimeEvent::Disconnected,
        ]);

        let (coalesced, remaining) = coalesce_surface_backlog(initial, backlog);

        assert_eq!(coalesced, latest);
        assert_eq!(remaining.len(), 2);
        assert!(matches!(
            remaining.front(),
            Some(SessionRuntimeEvent::Error(message)) if message == "boom"
        ));
        assert!(matches!(
            remaining.get(1),
            Some(SessionRuntimeEvent::Disconnected)
        ));
    }

    #[test]
    fn leaves_non_surface_prefix_events_in_place() {
        let session_id = Uuid::new_v4();
        let initial =
            TerminalSurfaceState::from_visible_lines(session_id, 1, 24, 80, vec!["one".into()]);
        let later =
            TerminalSurfaceState::from_visible_lines(session_id, 2, 24, 80, vec!["two".into()]);
        let backlog = VecDeque::from(vec![
            SessionRuntimeEvent::Connected,
            SessionRuntimeEvent::SurfaceChanged(later.clone()),
        ]);

        let (coalesced, remaining) = coalesce_surface_backlog(initial.clone(), backlog);

        assert_eq!(coalesced, initial);
        assert_eq!(remaining.len(), 2);
        assert!(matches!(
            remaining.front(),
            Some(SessionRuntimeEvent::Connected)
        ));
        assert!(matches!(
            remaining.get(1),
            Some(SessionRuntimeEvent::SurfaceChanged(surface)) if *surface == later
        ));
    }

    #[test]
    fn coalesces_consecutive_surface_dirty_events_but_preserves_following_control_events() {
        let backlog = VecDeque::from(vec![
            SessionRuntimeEvent::SurfaceDirty,
            SessionRuntimeEvent::SurfaceDirty,
            SessionRuntimeEvent::Error("boom".into()),
            SessionRuntimeEvent::Disconnected,
        ]);

        let remaining = coalesce_surface_dirty_backlog(backlog);

        assert_eq!(remaining.len(), 2);
        assert!(matches!(
            remaining.front(),
            Some(SessionRuntimeEvent::Error(message)) if message == "boom"
        ));
        assert!(matches!(
            remaining.get(1),
            Some(SessionRuntimeEvent::Disconnected)
        ));
    }

    #[test]
    fn surface_dirty_does_not_pull_runtime_snapshot_immediately() {
        let session_id = Uuid::new_v4();
        let registry = Arc::new(Mutex::new(SessionRegistry::default()));
        let initial_surface =
            TerminalSurfaceState::from_visible_lines(session_id, 1, 24, 80, vec!["one".into()]);
        let runtime_surface =
            TerminalSurfaceState::from_visible_lines(session_id, 2, 24, 80, vec!["two".into()]);
        let terminal_surface_calls = Arc::new(AtomicUsize::new(0));

        {
            let mut registry_guard = registry.lock().expect("lock session registry");
            registry_guard.sessions.insert(
                session_id,
                SessionHandle {
                    session_id,
                    asset_id: "asset-prod".into(),
                    title: "Prod Bastion".into(),
                    subtitle: "ops@example.com:22".into(),
                    state: SessionState::Connected,
                    can_reconnect: false,
                },
            );
            registry_guard.runtime_controls.insert(
                session_id,
                Box::new(CountingRuntimeControl::new(
                    runtime_surface,
                    Arc::clone(&terminal_surface_calls),
                )),
            );
        }
        update_terminal_surface(&registry, session_id, initial_surface);

        apply_runtime_event(&registry, session_id, SessionRuntimeEvent::SurfaceDirty);

        assert_eq!(terminal_surface_calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn surface_dirty_marks_snapshot_stale_until_on_demand_refresh() {
        let session_id = Uuid::new_v4();
        let registry = Arc::new(Mutex::new(SessionRegistry::default()));
        let initial_surface =
            TerminalSurfaceState::from_visible_lines(session_id, 1, 24, 80, vec!["one".into()]);
        let runtime_surface =
            TerminalSurfaceState::from_visible_lines(session_id, 2, 24, 80, vec!["two".into()]);
        let terminal_surface_calls = Arc::new(AtomicUsize::new(0));

        {
            let mut registry_guard = registry.lock().expect("lock session registry");
            registry_guard.sessions.insert(
                session_id,
                SessionHandle {
                    session_id,
                    asset_id: "asset-prod".into(),
                    title: "Prod Bastion".into(),
                    subtitle: "ops@example.com:22".into(),
                    state: SessionState::Connected,
                    can_reconnect: false,
                },
            );
            registry_guard.runtime_controls.insert(
                session_id,
                Box::new(CountingRuntimeControl::new(
                    runtime_surface.clone(),
                    Arc::clone(&terminal_surface_calls),
                )),
            );
        }
        update_terminal_surface(&registry, session_id, initial_surface);

        apply_runtime_event(&registry, session_id, SessionRuntimeEvent::SurfaceDirty);

        {
            let registry_guard = registry.lock().expect("lock session registry");
            assert!(terminal_surface_stale(&registry_guard, session_id));
            assert_eq!(
                terminal_surface_signature_for_registry(&registry_guard, session_id)
                    .expect("signature after dirty")
                    .seqno,
                2
            );
        }
        assert_eq!(terminal_surface_calls.load(Ordering::SeqCst), 0);

        refresh_runtime_surface(&registry, session_id);

        {
            let registry_guard = registry.lock().expect("lock session registry");
            assert!(!terminal_surface_stale(&registry_guard, session_id));
            let surface = registry_guard
                .terminal_surfaces
                .get(&session_id)
                .expect("refreshed runtime surface");
            assert_eq!(surface.seqno, runtime_surface.seqno);
            assert_eq!(surface.visible_lines, runtime_surface.visible_lines);
        }
        assert_eq!(terminal_surface_calls.load(Ordering::SeqCst), 1);
    }

    struct CountingRuntimeControl {
        surface: TerminalSurfaceState,
        terminal_surface_calls: Arc<AtomicUsize>,
    }

    impl CountingRuntimeControl {
        fn new(surface: TerminalSurfaceState, terminal_surface_calls: Arc<AtomicUsize>) -> Self {
            Self {
                surface,
                terminal_surface_calls,
            }
        }
    }

    impl SessionRuntimeControl for CountingRuntimeControl {
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

        fn terminal_surface(&self) -> Result<TerminalSurfaceState> {
            self.terminal_surface_calls.fetch_add(1, Ordering::SeqCst);
            Ok(self.surface.clone())
        }
    }
}

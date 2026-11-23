//! Root `MicaWorkspace`: unified sleek TitleBar + `h_resizable` 3-pane adaptive layout
//! (Single-Layer Left Sidebar | Multi-Tab & Split-Pane Terminal Workspace | Right SFTP Panel).

use std::rc::Rc;
use std::sync::Arc;

use gpui_kit::assets::IconName as LucideIcon;
use gpui_kit::component::{
    ActiveTheme as _, Icon, Selectable as _, Sizable as _, TitleBar, WindowExt as _,
    button::{Button, ButtonVariants as _},
    h_flex, h_resizable,
    input::{InputEvent, InputState},
    menu::{ContextMenuExt as _, DropdownMenu as _, PopupMenuItem},
    notification::Notification,
    resizable_panel,
    theme::{Theme, ThemeMode},
    v_flex, v_resizable,
};
use gpui_kit::{
    App, AppContext as _, Context, Entity, Focusable as _, FontWeight, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, SharedString, StatefulInteractiveElement as _,
    Styled as _, Window, div, px,
};

use crate::domain::{
    AppSettings, CredentialStore, HostNode, HostTree, KeychainEntry, ResolvedSshConfig,
    SecretPayload, SnippetItem, host_secret_key, keychain_secret_key,
};
use crate::ssh::KnownHostsStore;
use crate::storage::{AppPaths, WorkspaceStore};
use crate::terminal::{
    TerminalSessionEntity,
    session::{SessionEntityEvent, SessionPhase},
};
use crate::ui::modals::{
    open_keychain_modal, open_settings_modal, open_simple_name_dialog, open_snippet_modal,
    open_ssh_host_modal,
};
use crate::ui::sftp_panel::SftpPanelView;
use crate::ui::sidebar::{LeftSidebar, SidebarSection};
use crate::ui::terminal_view::TerminalPaneView;
use crate::ui::welcome_view::{QuickConnectItem, WelcomeView};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabSplitAxis {
    Horizontal,
    Vertical,
}

pub struct WorkspaceTab {
    pub id: String,
    pub title: String,
    pub split_axis: Option<TabSplitAxis>,
    pub panes: Vec<Entity<TerminalPaneView>>,
    pub active_pane_id: String,
}

impl WorkspaceTab {
    pub fn active_pane_in(&self, cx: &App) -> Option<Entity<TerminalPaneView>> {
        self.panes
            .iter()
            .find(|p| p.read(cx).pane_id == self.active_pane_id)
            .cloned()
            .or_else(|| self.panes.first().cloned())
    }
}

pub struct MicaWorkspace {
    pub paths: AppPaths,
    store: WorkspaceStore,
    credentials: Arc<dyn CredentialStore>,
    known_hosts: KnownHostsStore,
    tokio_handle: tokio::runtime::Handle,

    pub hosts: HostTree,
    pub keychain: Vec<KeychainEntry>,
    pub snippets: Vec<SnippetItem>,
    pub recent_host_ids: Vec<String>,
    pub settings: AppSettings,

    pub sidebar_section: SidebarSection,
    pub sidebar_query: String,
    pub selected_sidebar_id: Option<String>,
    sidebar_search_input: Entity<InputState>,

    pub tabs: Vec<WorkspaceTab>,
    pub active_tab_id: Option<String>,
    local_shell_counter: usize,

    pub sftp_panel: Entity<SftpPanelView>,
}

impl MicaWorkspace {
    pub fn new(
        paths: AppPaths,
        store: WorkspaceStore,
        credentials: Arc<dyn CredentialStore>,
        tokio_handle: tokio::runtime::Handle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let snap = store.load_snapshot().unwrap_or_default();
        let known_hosts = KnownHostsStore::new(&paths.known_hosts_path);

        Theme::change(
            if snap.settings.dark_mode {
                ThemeMode::Dark
            } else {
                ThemeMode::Light
            },
            Some(window),
            cx,
        );

        let sidebar_search_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("搜索..."));
        cx.subscribe_in(&sidebar_search_input, window, |this, state, ev, _, cx| {
            if matches!(ev, InputEvent::Change) {
                this.sidebar_query = state.read(cx).value().to_string();
                cx.notify();
            }
        })
        .detach();

        let follow_sftp = snap.settings.follow_sftp_cwd;
        let dl_dir = paths.downloads_dir.clone();
        let rt_sftp = tokio_handle.clone();
        let sftp_panel = cx.new(|_| SftpPanelView::new(follow_sftp, dl_dir, rt_sftp));
        cx.observe(&sftp_panel, |_, _, cx| cx.notify()).detach();

        Self {
            paths,
            store,
            credentials,
            known_hosts,
            tokio_handle,
            hosts: snap.hosts,
            keychain: snap.keychain,
            snippets: snap.snippets,
            recent_host_ids: snap.recent_host_ids,
            settings: snap.settings,
            sidebar_section: SidebarSection::Hosts,
            sidebar_query: String::new(),
            selected_sidebar_id: None,
            sidebar_search_input,
            tabs: Vec::new(),
            active_tab_id: None,
            local_shell_counter: 0,
            sftp_panel,
        }
    }

    pub fn set_sidebar_section(&mut self, section: SidebarSection, cx: &mut Context<Self>) {
        self.sidebar_section = section;
        cx.notify();
    }

    pub fn toggle_left_sidebar(&mut self, cx: &mut Context<Self>) {
        self.settings.show_left_sidebar = !self.settings.show_left_sidebar;
        let _ = self.store.save_settings(&self.settings);
        cx.notify();
    }

    pub fn toggle_sftp_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings.show_sftp_panel = !self.settings.show_sftp_panel;
        let _ = self.store.save_settings(&self.settings);
        if self.settings.show_sftp_panel {
            self.sync_sftp_binding(Some(window), cx);
        }
        cx.notify();
    }

    pub fn set_dark_mode(&mut self, dark: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.settings.dark_mode = dark;
        let _ = self.store.save_settings(&self.settings);
        Theme::change(
            if dark {
                ThemeMode::Dark
            } else {
                ThemeMode::Light
            },
            Some(window),
            cx,
        );
        self.propagate_pane_preferences(cx);
        cx.notify();
    }

    pub fn adjust_default_font_size(&mut self, delta: f32, cx: &mut Context<Self>) {
        self.settings.font_size = (self.settings.font_size + delta).clamp(9.0, 36.0);
        let new_size = self.settings.font_size;
        let _ = self.store.save_settings(&self.settings);
        for tab in &self.tabs {
            for pane in &tab.panes {
                let session = pane.read(cx).session.clone();
                session.update(cx, |s, cx| s.reset_font(new_size, cx));
            }
        }
        cx.notify();
    }

    pub fn set_font_family(&mut self, family: String, cx: &mut Context<Self>) {
        if !family.is_empty() {
            self.settings.font_family = family;
            let _ = self.store.save_settings(&self.settings);
            self.propagate_pane_preferences(cx);
            cx.notify();
        }
    }

    pub fn set_scrollback_lines(&mut self, lines: usize, cx: &mut Context<Self>) {
        self.settings.scrollback_lines = lines;
        let _ = self.store.save_settings(&self.settings);
        self.propagate_pane_preferences(cx);
        cx.notify();
    }

    pub fn set_follow_sftp_cwd(&mut self, follow: bool, cx: &mut Context<Self>) {
        self.settings.follow_sftp_cwd = follow;
        let _ = self.store.save_settings(&self.settings);
        self.sftp_panel.update(cx, |p, cx| {
            p.follow_cwd = follow;
            cx.notify();
        });
        cx.notify();
    }

    fn propagate_pane_preferences(&mut self, cx: &mut Context<Self>) {
        let fam = self.settings.font_family.clone();
        let dark = self.settings.dark_mode;
        let sb = self.settings.scrollback_lines;
        for tab in &self.tabs {
            for pane in &tab.panes {
                pane.update(cx, |p, cx| p.sync_preferences(&fam, dark, sb, cx));
            }
        }
    }

    fn create_pane_for_session(
        &self,
        session: Entity<TerminalSessionEntity>,
        cx: &mut Context<Self>,
    ) -> Entity<TerminalPaneView> {
        let ws_weak = cx.entity().downgrade();
        cx.subscribe(&session, move |this, session_ent, event, cx| match event {
            SessionEntityEvent::TitleChanged(new_title) => {
                for tab in &mut this.tabs {
                    if tab
                        .panes
                        .first()
                        .is_some_and(|p| p.read(cx).session == session_ent)
                    {
                        tab.title = new_title.clone();
                    }
                }
                cx.notify();
            }
            SessionEntityEvent::RemoteConnected { .. } | SessionEntityEvent::PhaseChanged => {
                this.sync_sftp_binding(None, cx);
                cx.notify();
            }
            SessionEntityEvent::RemoteCwdChanged(new_cwd) => {
                if this.is_active_session(&session_ent, cx) {
                    let cwd = new_cwd.clone();
                    this.sftp_panel
                        .update(cx, |p, cx| p.on_terminal_cwd_changed(cwd, None, cx));
                }
            }
        })
        .detach();

        let on_activate: Rc<dyn Fn(&str, &mut Window, &mut App)> =
            Rc::new(move |pane_id, window, cx| {
                if let Some(ws) = ws_weak.upgrade() {
                    ws.update(cx, |this, cx| {
                        this.activate_pane_by_id(pane_id, Some(window), cx);
                    });
                }
            });

        let fam = self.settings.font_family.clone();
        let dark = self.settings.dark_mode;
        let sb = self.settings.scrollback_lines;
        cx.new(|cx| TerminalPaneView::new(session, fam, dark, sb, Some(on_activate), cx))
    }

    fn is_active_session(&self, session: &Entity<TerminalSessionEntity>, cx: &App) -> bool {
        let Some(active_id) = &self.active_tab_id else {
            return false;
        };
        let Some(tab) = self.tabs.iter().find(|t| &t.id == active_id) else {
            return false;
        };
        let Some(pane) = tab.active_pane_in(cx) else {
            return false;
        };
        &pane.read(cx).session == session
    }

    pub fn activate_pane_by_id(
        &mut self,
        pane_id: &str,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        let mut changed = false;
        for tab in &mut self.tabs {
            if tab.panes.iter().any(|p| p.read(cx).pane_id == pane_id)
                && tab.active_pane_id != pane_id
            {
                tab.active_pane_id = pane_id.to_string();
                changed = true;
            }
        }
        if changed {
            self.sync_sftp_binding(window, cx);
            cx.notify();
        }
    }

    pub fn open_local_shell_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.local_shell_counter += 1;
        let title = format!("本地终端 {}", self.local_shell_counter);
        let fsize = self.settings.font_size;
        let sb = self.settings.scrollback_lines;
        let rt = self.tokio_handle.clone();

        let session =
            cx.new(|cx| TerminalSessionEntity::new_local(title.clone(), fsize, sb, rt, cx));
        let pane = self.create_pane_for_session(session, cx);
        let pane_id = pane.read(cx).pane_id.clone();
        let focus = pane.focus_handle(cx);

        let tab_id = uuid::Uuid::new_v4().to_string();
        self.tabs.push(WorkspaceTab {
            id: tab_id.clone(),
            title,
            split_axis: None,
            panes: vec![pane],
            active_pane_id: pane_id,
        });
        self.active_tab_id = Some(tab_id);
        self.sync_sftp_binding(Some(window), cx);
        focus.focus(window, cx);
        cx.notify();
    }

    pub fn connect_host_in_new_tab(
        &mut self,
        host_id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let resolved = match ResolvedSshConfig::resolve_from_tree(
            host_id,
            &self.hosts,
            &self.keychain,
            self.credentials.as_ref(),
        ) {
            Ok(cfg) => cfg,
            Err(err) => {
                window.push_notification(
                    Notification::error(format!("无法发起连接：{err:#}")),
                    cx,
                );
                return;
            }
        };

        self.record_recent_host(host_id);
        let title = resolved.name.clone();
        let kh = self.known_hosts.clone();
        let fsize = self.settings.font_size;
        let sb = self.settings.scrollback_lines;
        let rt = self.tokio_handle.clone();

        let session =
            cx.new(|cx| TerminalSessionEntity::new_ssh(resolved, kh, fsize, sb, rt, cx));
        let pane = self.create_pane_for_session(session, cx);
        let pane_id = pane.read(cx).pane_id.clone();
        let focus = pane.focus_handle(cx);

        let tab_id = uuid::Uuid::new_v4().to_string();
        self.tabs.push(WorkspaceTab {
            id: tab_id.clone(),
            title,
            split_axis: None,
            panes: vec![pane],
            active_pane_id: pane_id,
        });
        self.active_tab_id = Some(tab_id);
        self.sync_sftp_binding(Some(window), cx);
        focus.focus(window, cx);
        cx.notify();
    }

    pub fn connect_host_in_split(
        &mut self,
        host_id: &str,
        horizontal: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.active_tab_id.is_none() {
            self.connect_host_in_new_tab(host_id, window, cx);
            return;
        }
        let Ok(resolved) = ResolvedSshConfig::resolve_from_tree(
            host_id,
            &self.hosts,
            &self.keychain,
            self.credentials.as_ref(),
        ) else {
            return;
        };
        self.record_recent_host(host_id);
        let kh = self.known_hosts.clone();
        let fsize = self.settings.font_size;
        let sb = self.settings.scrollback_lines;
        let rt = self.tokio_handle.clone();
        let session =
            cx.new(|cx| TerminalSessionEntity::new_ssh(resolved, kh, fsize, sb, rt, cx));
        let pane = self.create_pane_for_session(session, cx);
        self.attach_split_pane(
            pane,
            if horizontal {
                TabSplitAxis::Horizontal
            } else {
                TabSplitAxis::Vertical
            },
            window,
            cx,
        );
    }

    pub fn split_active_tab(
        &mut self,
        axis: TabSplitAxis,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(active_id) = self.active_tab_id.clone() else {
            return;
        };
        let Some(tab) = self.tabs.iter().find(|t| t.id == active_id) else {
            return;
        };
        let Some(cur_pane) = tab.active_pane_in(cx) else {
            return;
        };
        let cur_kind = cur_pane.read(cx).session.read(cx).kind.clone();
        let fsize = self.settings.font_size;
        let sb = self.settings.scrollback_lines;
        let rt = self.tokio_handle.clone();

        let new_session = match cur_kind {
            crate::terminal::SessionKind::LocalShell => {
                self.local_shell_counter += 1;
                let t = format!("本地终端 {}", self.local_shell_counter);
                cx.new(|cx| TerminalSessionEntity::new_local(t, fsize, sb, rt, cx))
            }
            crate::terminal::SessionKind::Ssh {
                config,
                known_hosts,
            } => cx.new(|cx| TerminalSessionEntity::new_ssh(config, known_hosts, fsize, sb, rt, cx)),
        };

        let new_pane = self.create_pane_for_session(new_session, cx);
        self.attach_split_pane(new_pane, axis, window, cx);
    }

    fn attach_split_pane(
        &mut self,
        pane: Entity<TerminalPaneView>,
        axis: TabSplitAxis,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(active_id) = &self.active_tab_id else {
            return;
        };
        let pane_id = pane.read(cx).pane_id.clone();
        let focus = pane.focus_handle(cx);
        if let Some(tab) = self.tabs.iter_mut().find(|t| &t.id == active_id) {
            if tab.panes.len() >= 3 {
                window.push_notification(
                    Notification::info("单个标签页最多支持 3 个分屏面板"),
                    cx,
                );
                return;
            }
            tab.split_axis = Some(axis);
            tab.panes.push(pane);
            tab.active_pane_id = pane_id;
        }
        self.sync_sftp_binding(Some(window), cx);
        focus.focus(window, cx);
        cx.notify();
    }

    pub fn select_tab(&mut self, tab_id: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.active_tab_id = Some(tab_id.to_string());
        if let Some(pane) = self
            .tabs
            .iter()
            .find(|t| t.id == tab_id)
            .and_then(|t| t.active_pane_in(cx))
        {
            pane.focus_handle(cx).focus(window, cx);
        }
        self.sync_sftp_binding(Some(window), cx);
        cx.notify();
    }

    pub fn close_tab(&mut self, tab_id: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(idx) = self.tabs.iter().position(|t| t.id == tab_id) else {
            return;
        };
        self.tabs.remove(idx);
        if self.active_tab_id.as_deref() == Some(tab_id) {
            self.active_tab_id = if self.tabs.is_empty() {
                None
            } else {
                let next_idx = idx.min(self.tabs.len() - 1);
                Some(self.tabs[next_idx].id.clone())
            };
        }
        self.sync_sftp_binding(Some(window), cx);
        cx.notify();
    }

    pub fn close_other_tabs(&mut self, keep_id: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.tabs.retain(|t| t.id == keep_id);
        self.active_tab_id = self.tabs.first().map(|t| t.id.clone());
        self.sync_sftp_binding(Some(window), cx);
        cx.notify();
    }

    pub fn clone_tab(&mut self, tab_id: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tab) = self.tabs.iter().find(|t| t.id == tab_id) else {
            return;
        };
        let Some(pane) = tab.active_pane_in(cx) else {
            return;
        };
        let kind = pane.read(cx).session.read(cx).kind.clone();
        match kind {
            crate::terminal::SessionKind::LocalShell => self.open_local_shell_tab(window, cx),
            crate::terminal::SessionKind::Ssh {
                config,
                known_hosts,
            } => {
                let title = config.name.clone();
                let fsize = self.settings.font_size;
                let sb = self.settings.scrollback_lines;
                let rt = self.tokio_handle.clone();
                let session = cx.new(|cx| {
                    TerminalSessionEntity::new_ssh(config, known_hosts, fsize, sb, rt, cx)
                });
                let new_pane = self.create_pane_for_session(session, cx);
                let pid = new_pane.read(cx).pane_id.clone();
                let focus = new_pane.focus_handle(cx);
                let new_id = uuid::Uuid::new_v4().to_string();
                self.tabs.push(WorkspaceTab {
                    id: new_id.clone(),
                    title,
                    split_axis: None,
                    panes: vec![new_pane],
                    active_pane_id: pid,
                });
                self.active_tab_id = Some(new_id);
                self.sync_sftp_binding(Some(window), cx);
                focus.focus(window, cx);
                cx.notify();
            }
        }
    }

    fn sync_sftp_binding(&mut self, window: Option<&mut Window>, cx: &mut Context<Self>) {
        let active_session = self
            .active_tab_id
            .as_ref()
            .and_then(|tid| self.tabs.iter().find(|t| &t.id == tid))
            .and_then(|t| t.active_pane_in(cx))
            .map(|p| p.read(cx).session.clone());

        let (label, svc, cwd) = if let Some(sess) = active_session {
            let s = sess.read(cx);
            (
                s.title.clone(),
                s.sftp_service(),
                s.remote_cwd().map(ToString::to_string),
            )
        } else {
            (String::new(), None, None)
        };

        self.sftp_panel
            .update(cx, |panel, cx| panel.bind_session(label, svc, cwd, window, cx));
    }

    fn record_recent_host(&mut self, host_id: &str) {
        self.recent_host_ids.retain(|id| id != host_id);
        self.recent_host_ids.insert(0, host_id.to_string());
        self.recent_host_ids.truncate(12);
        let _ = self.store.save_recent_hosts(&self.recent_host_ids);
    }

    // --- Asset & Modal CRUD operations ---

    pub fn toggle_folder(&mut self, folder_id: &str, cx: &mut Context<Self>) {
        if self.hosts.toggle_folder_expanded(folder_id) {
            let _ = self.store.save_hosts(&self.hosts);
            cx.notify();
        }
    }

    pub fn open_folder_dialog(
        &mut self,
        parent_id: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let ws = cx.entity().clone();
        open_simple_name_dialog(
            "新建主机分组文件夹",
            "文件夹名称",
            "",
            window,
            cx,
            move |name, _, cx| {
                ws.update(cx, |this, cx| {
                    this.hosts
                        .upsert(HostNode::new_folder(name, parent_id.clone()));
                    let _ = this.store.save_hosts(&this.hosts);
                    cx.notify();
                });
            },
        );
    }

    pub fn open_host_dialog(
        &mut self,
        editing_id: Option<String>,
        default_parent_id: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let editing_node = editing_id.as_deref().and_then(|id| self.hosts.get(id)).cloned();
        let existing_secrets = editing_id
            .as_deref()
            .and_then(|id| self.credentials.get_payload(&host_secret_key(id)).ok().flatten())
            .unwrap_or_default();
        let folders = self.hosts.folders();
        let jump_candidates: Vec<(String, String)> = self
            .hosts
            .hosts()
            .into_iter()
            .filter(|n| editing_id.as_deref() != Some(n.id.as_str()))
            .map(|n| (n.id.clone(), n.name.clone()))
            .collect();
        let keychains = self.keychain.clone();
        let ws = cx.entity().clone();

        open_ssh_host_modal(
            ws,
            editing_node,
            existing_secrets,
            default_parent_id,
            folders,
            jump_candidates,
            keychains,
            window,
            cx,
        );
    }

    pub fn save_host_node(
        &mut self,
        node: HostNode,
        secrets: SecretPayload,
        cx: &mut Context<Self>,
    ) {
        let _ = self
            .credentials
            .put_payload(&host_secret_key(&node.id), &secrets);
        self.hosts.upsert(node);
        let _ = self.store.save_hosts(&self.hosts);
        cx.notify();
    }

    pub fn clone_host(&mut self, host_id: &str, cx: &mut Context<Self>) {
        let Some(orig) = self.hosts.get(host_id).cloned() else {
            return;
        };
        let Some(cfg) = orig.as_host().cloned() else {
            return;
        };
        let copy_node = HostNode::new_host(
            format!("{} (副本)", orig.name),
            orig.parent_id.clone(),
            cfg,
        );
        if let Ok(Some(sec)) = self.credentials.get_payload(&host_secret_key(host_id)) {
            let _ = self
                .credentials
                .put_payload(&host_secret_key(&copy_node.id), &sec);
        }
        self.hosts.upsert(copy_node);
        let _ = self.store.save_hosts(&self.hosts);
        cx.notify();
    }

    pub fn delete_host_node(&mut self, node_id: &str, cx: &mut Context<Self>) {
        let removed = self.hosts.remove_cascade(node_id);
        for rid in &removed {
            let _ = self.credentials.delete(&host_secret_key(rid));
            self.recent_host_ids.retain(|id| id != rid);
        }
        let _ = self.store.save_hosts(&self.hosts);
        let _ = self.store.save_recent_hosts(&self.recent_host_ids);
        cx.notify();
    }

    pub fn open_keychain_dialog(
        &mut self,
        editing_id: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let editing = editing_id
            .as_deref()
            .and_then(|id| self.keychain.iter().find(|k| k.id == id))
            .cloned();
        let secrets = editing_id
            .as_deref()
            .and_then(|id| {
                self.credentials
                    .get_payload(&keychain_secret_key(id))
                    .ok()
                    .flatten()
            })
            .unwrap_or_default();
        open_keychain_modal(cx.entity().clone(), editing, secrets, window, cx);
    }

    pub fn save_keychain(
        &mut self,
        entry: KeychainEntry,
        secrets: SecretPayload,
        cx: &mut Context<Self>,
    ) {
        let _ = self
            .credentials
            .put_payload(&keychain_secret_key(&entry.id), &secrets);
        if let Some(ex) = self.keychain.iter_mut().find(|k| k.id == entry.id) {
            *ex = entry;
        } else {
            self.keychain.push(entry);
        }
        let _ = self.store.save_keychain(&self.keychain);
        cx.notify();
    }

    pub fn delete_keychain(&mut self, keychain_id: &str, cx: &mut Context<Self>) {
        self.keychain.retain(|k| k.id != keychain_id);
        let _ = self.credentials.delete(&keychain_secret_key(keychain_id));
        let _ = self.store.save_keychain(&self.keychain);
        cx.notify();
    }

    pub fn open_snippet_dialog(
        &mut self,
        editing_id: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let editing = editing_id
            .as_deref()
            .and_then(|id| self.snippets.iter().find(|s| s.id == id))
            .cloned();
        open_snippet_modal(cx.entity().clone(), editing, window, cx);
    }

    pub fn save_snippet(&mut self, item: SnippetItem, cx: &mut Context<Self>) {
        if let Some(ex) = self.snippets.iter_mut().find(|s| s.id == item.id) {
            *ex = item;
        } else {
            self.snippets.push(item);
        }
        let _ = self.store.save_snippets(&self.snippets);
        cx.notify();
    }

    pub fn delete_snippet(&mut self, snippet_id: &str, cx: &mut Context<Self>) {
        self.snippets.retain(|s| s.id != snippet_id);
        let _ = self.store.save_snippets(&self.snippets);
        cx.notify();
    }

    pub fn run_snippet(&mut self, snippet_id: &str, cx: &mut Context<Self>) {
        let Some(snip) = self.snippets.iter().find(|s| s.id == snippet_id).cloned() else {
            return;
        };
        let Some(active_id) = &self.active_tab_id else {
            return;
        };
        let Some(pane) = self
            .tabs
            .iter()
            .find(|t| &t.id == active_id)
            .and_then(|t| t.active_pane_in(cx))
        else {
            return;
        };
        let session = pane.read(cx).session.clone();
        session.update(cx, |s, cx| {
            s.send_command_snippet(&snip.command, snip.auto_enter, cx);
        });
    }

    fn filtered_snippets(&self) -> Vec<SnippetItem> {
        let q = self.sidebar_query.trim().to_lowercase();
        if q.is_empty() {
            return self.snippets.clone();
        }
        self.snippets
            .iter()
            .filter(|s| {
                s.title.to_lowercase().contains(&q)
                    || s.command.to_lowercase().contains(&q)
                    || s.group.to_lowercase().contains(&q)
            })
            .cloned()
            .collect()
    }

    fn filtered_keychain(&self) -> Vec<KeychainEntry> {
        let q = self.sidebar_query.trim().to_lowercase();
        if q.is_empty() {
            return self.keychain.clone();
        }
        self.keychain
            .iter()
            .filter(|k| {
                k.name.to_lowercase().contains(&q) || k.username.to_lowercase().contains(&q)
            })
            .cloned()
            .collect()
    }

    fn build_quick_connect_items(&self) -> Vec<QuickConnectItem> {
        let mut out = Vec::new();
        for rid in &self.recent_host_ids {
            if let Some(node) = self.hosts.get(rid)
                && let Some(cfg) = node.as_host()
            {
                out.push(QuickConnectItem {
                    id: node.id.clone(),
                    name: node.name.clone(),
                    endpoint: cfg.endpoint_label(),
                    env_tag: cfg.environment_tag.clone(),
                });
            }
        }
        for node in self.hosts.hosts() {
            if out.len() >= 8 {
                break;
            }
            if out.iter().any(|i| i.id == node.id) {
                continue;
            }
            if let Some(cfg) = node.as_host() {
                out.push(QuickConnectItem {
                    id: node.id.clone(),
                    name: node.name.clone(),
                    endpoint: cfg.endpoint_label(),
                    env_tag: cfg.environment_tag.clone(),
                });
            }
        }
        out
    }

    fn render_titlebar(&self, cx: &mut Context<Self>) -> TitleBar {
        let ws_sidebar = cx.entity().clone();
        let ws_new_local = cx.entity().clone();
        let ws_menu_local = cx.entity().clone();
        let ws_menu_ssh = cx.entity().clone();
        let ws_split_h = cx.entity().clone();
        let ws_split_v = cx.entity().clone();
        let ws_sftp = cx.entity().clone();
        let ws_settings = cx.entity().clone();

        let show_sidebar = self.settings.show_left_sidebar;
        let show_sftp = self.settings.show_sftp_panel;
        let has_active_tab = self.active_tab_id.is_some();
        let sftp_busy = self.sftp_panel.read(cx).has_active_transfers();
        let data_dir_str = self.paths.root_dir.display().to_string();

        let tab_elements: Vec<gpui_kit::AnyElement> = self
            .tabs
            .iter()
            .map(|tab| {
                let tid = tab.id.clone();
                let tid_close = tid.clone();
                let tid_ctx_h = tid.clone();
                let tid_ctx_v = tid.clone();
                let tid_ctx_clone = tid.clone();
                let tid_ctx_others = tid.clone();
                let tid_ctx_close = tid.clone();

                let is_active = self.active_tab_id.as_deref() == Some(tid.as_str());
                let (is_ssh, is_connected) = tab
                    .active_pane_in(cx)
                    .map(|p| {
                        let s = p.read(cx).session.read(cx);
                        (s.is_ssh(), matches!(s.phase, SessionPhase::Connected))
                    })
                    .unwrap_or((false, false));

                let ws_sel = cx.entity().clone();
                let ws_close = cx.entity().clone();
                let ws_c_h = cx.entity().clone();
                let ws_c_v = cx.entity().clone();
                let ws_c_clone = cx.entity().clone();
                let ws_c_others = cx.entity().clone();
                let ws_c_close = cx.entity().clone();

                h_flex()
                    .id(SharedString::from(format!("ws-tab-{tid}")))
                    .max_w(px(196.0))
                    .h(px(26.0))
                    .px_2p5()
                    .gap_1p5()
                    .items_center()
                    .rounded(cx.theme().radius)
                    .cursor_pointer()
                    .bg(if is_active {
                        cx.theme().tab_active
                    } else {
                        cx.theme().transparent
                    })
                    .border_1()
                    .border_color(if is_active {
                        cx.theme().border
                    } else {
                        cx.theme().transparent
                    })
                    .hover(|s| s.bg(cx.theme().secondary_hover))
                    .on_mouse_down(gpui_kit::MouseButton::Left, |_, window, cx| {
                        window.prevent_default();
                        cx.stop_propagation();
                    })
                    .on_click(move |_, window, cx| {
                        ws_sel.update(cx, |ws, cx| ws.select_tab(&tid, window, cx));
                    })
                    .context_menu(move |menu, _, _| {
                        let wh = ws_c_h.clone();
                        let th = tid_ctx_h.clone();
                        let wv = ws_c_v.clone();
                        let tv = tid_ctx_v.clone();
                        let wc = ws_c_clone.clone();
                        let tc = tid_ctx_clone.clone();
                        let wo = ws_c_others.clone();
                        let to = tid_ctx_others.clone();
                        let wx = ws_c_close.clone();
                        let tx = tid_ctx_close.clone();

                        menu.item(
                            PopupMenuItem::new("向右水平分屏")
                                .icon(LucideIcon::Columns2)
                                .on_click(move |_, window, cx| {
                                    wh.update(cx, |ws, cx| {
                                        ws.select_tab(&th, window, cx);
                                        ws.split_active_tab(TabSplitAxis::Horizontal, window, cx);
                                    });
                                }),
                        )
                        .item(
                            PopupMenuItem::new("向下垂直分屏")
                                .icon(LucideIcon::Rows2)
                                .on_click(move |_, window, cx| {
                                    wv.update(cx, |ws, cx| {
                                        ws.select_tab(&tv, window, cx);
                                        ws.split_active_tab(TabSplitAxis::Vertical, window, cx);
                                    });
                                }),
                        )
                        .item(
                            PopupMenuItem::new("克隆会话标签")
                                .icon(LucideIcon::Copy)
                                .on_click(move |_, window, cx| {
                                    wc.update(cx, |ws, cx| ws.clone_tab(&tc, window, cx));
                                }),
                        )
                        .separator()
                        .item(PopupMenuItem::new("关闭其他标签").on_click(
                            move |_, window, cx| {
                                wo.update(cx, |ws, cx| ws.close_other_tabs(&to, window, cx));
                            },
                        ))
                        .item(PopupMenuItem::new("关闭标签").on_click(move |_, window, cx| {
                            wx.update(cx, |ws, cx| ws.close_tab(&tx, window, cx));
                        }))
                    })
                    .child(
                        Icon::new(if is_ssh {
                            LucideIcon::Server
                        } else {
                            LucideIcon::Terminal
                        })
                        .xsmall()
                        .text_color(if is_connected {
                            cx.theme().primary
                        } else {
                            cx.theme().muted_foreground
                        }),
                    )
                    .child(
                        div()
                            .truncate()
                            .text_xs()
                            .font_weight(if is_active {
                                FontWeight::MEDIUM
                            } else {
                                FontWeight::NORMAL
                            })
                            .text_color(if is_active {
                                cx.theme().foreground
                            } else {
                                cx.theme().muted_foreground
                            })
                            .child(tab.title.clone()),
                    )
                    .child(
                        Button::new(SharedString::from(format!("close-tab-{tid_close}")))
                            .ghost()
                            .xsmall()
                            .compact()
                            .icon(LucideIcon::X)
                            .on_click(move |_, window, cx| {
                                cx.stop_propagation();
                                ws_close.update(cx, |ws, cx| ws.close_tab(&tid_close, window, cx));
                            }),
                    )
                    .into_any_element()
            })
            .collect();

        TitleBar::new()
            .child(
                // Left + Integrated Tab Strip
                h_flex()
                    .min_w_0()
                    .gap_1p5()
                    .items_center()
                    .child(
                        Button::new("toggle-left-sidebar")
                            .ghost()
                            .xsmall()
                            .compact()
                            .selected(show_sidebar)
                            .icon(LucideIcon::PanelLeft)
                            .tooltip(if show_sidebar {
                                "收起左侧栏"
                            } else {
                                "展开左侧栏"
                            })
                            .on_click(move |_, _, cx| {
                                ws_sidebar.update(cx, |ws, cx| ws.toggle_left_sidebar(cx));
                            }),
                    )
                    .children(self.tabs.is_empty().then(|| {
                        div()
                            .px_1p5()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(cx.theme().foreground)
                            .child("Mica Term")
                    }))
                    .children(tab_elements)
                    .child(
                        Button::new("new-local-tab-btn")
                            .ghost()
                            .xsmall()
                            .compact()
                            .icon(LucideIcon::Plus)
                            .tooltip("打开本地终端")
                            .on_click(move |_, window, cx| {
                                ws_new_local.update(cx, |ws, cx| ws.open_local_shell_tab(window, cx));
                            }),
                    )
                    .child(
                        Button::new("new-session-more-menu")
                            .ghost()
                            .xsmall()
                            .compact()
                            .icon(LucideIcon::ChevronDown)
                            .tooltip("更多新建选项")
                            .dropdown_menu(move |menu, _, _| {
                                let wl = ws_menu_local.clone();
                                let ws = ws_menu_ssh.clone();
                                menu.item(
                                    PopupMenuItem::new("新建本地终端")
                                        .icon(LucideIcon::Terminal)
                                        .on_click(move |_, window, cx| {
                                            wl.update(cx, |w, cx| w.open_local_shell_tab(window, cx));
                                        }),
                                )
                                .item(
                                    PopupMenuItem::new("新建 SSH 主机连接")
                                        .icon(LucideIcon::Server)
                                        .on_click(move |_, window, cx| {
                                            ws.update(cx, |w, cx| {
                                                w.open_host_dialog(None, None, window, cx)
                                            });
                                        }),
                                )
                            }),
                    )
                    .children(has_active_tab.then(|| {
                        h_flex()
                            .gap_0p5()
                            .pl_1()
                            .child(
                                Button::new("title-split-h")
                                    .ghost()
                                    .xsmall()
                                    .compact()
                                    .icon(LucideIcon::Columns2)
                                    .tooltip("水平分屏")
                                    .on_click(move |_, window, cx| {
                                        ws_split_h.update(cx, |ws, cx| {
                                            ws.split_active_tab(TabSplitAxis::Horizontal, window, cx)
                                        });
                                    }),
                            )
                            .child(
                                Button::new("title-split-v")
                                    .ghost()
                                    .xsmall()
                                    .compact()
                                    .icon(LucideIcon::Rows2)
                                    .tooltip("垂直分屏")
                                    .on_click(move |_, window, cx| {
                                        ws_split_v.update(cx, |ws, cx| {
                                            ws.split_active_tab(TabSplitAxis::Vertical, window, cx)
                                        });
                                    }),
                            )
                    })),
            )
            .child(
                // Right minimal utility controls: SFTP toggle + Settings ONLY
                h_flex()
                    .pr_2()
                    .gap_1()
                    .items_center()
                    .child(
                        Button::new("toggle-sftp-panel")
                            .ghost()
                            .xsmall()
                            .compact()
                            .selected(show_sftp)
                            .loading(sftp_busy)
                            .icon(LucideIcon::PanelRight)
                            .label("SFTP")
                            .tooltip("切换右侧 SFTP 文件面板")
                            .on_click(move |_, window, cx| {
                                ws_sftp.update(cx, |ws, cx| ws.toggle_sftp_panel(window, cx));
                            }),
                    )
                    .child(
                        Button::new("open-settings-btn")
                            .ghost()
                            .xsmall()
                            .compact()
                            .icon(LucideIcon::Settings)
                            .tooltip("设置与外观")
                            .on_click(move |_, window, cx| {
                                open_settings_modal(
                                    ws_settings.clone(),
                                    data_dir_str.clone(),
                                    window,
                                    cx,
                                );
                            }),
                    ),
            )
    }

    fn render_center_workspace(&self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let active_tab = self
            .active_tab_id
            .as_ref()
            .and_then(|id| self.tabs.iter().find(|t| &t.id == id));

        let Some(tab) = active_tab else {
            let quick = self.build_quick_connect_items();
            let ws_local = cx.entity().clone();
            let ws_ssh = cx.entity().clone();
            let ws_conn = cx.entity().clone();
            return WelcomeView::new(
                quick,
                move |window, cx| {
                    ws_local.update(cx, |ws, cx| ws.open_local_shell_tab(window, cx));
                },
                move |window, cx| {
                    ws_ssh.update(cx, |ws, cx| ws.open_host_dialog(None, None, window, cx));
                },
                move |host_id, window, cx| {
                    ws_conn.update(cx, |ws, cx| ws.connect_host_in_new_tab(host_id, window, cx));
                },
            )
            .into_any_element();
        };

        if tab.panes.len() <= 1 {
            if let Some(first) = tab.panes.first() {
                return div().size_full().child(first.clone()).into_any_element();
            }
        }

        let split_id = SharedString::from(format!("tab-split-{}", tab.id));
        match tab.split_axis.unwrap_or(TabSplitAxis::Horizontal) {
            TabSplitAxis::Horizontal => {
                let mut group = h_resizable(split_id);
                for pane in &tab.panes {
                    group = group.child(resizable_panel().child(pane.clone()));
                }
                group.into_any_element()
            }
            TabSplitAxis::Vertical => {
                let mut group = v_resizable(split_id);
                for pane in &tab.panes {
                    group = group.child(resizable_panel().child(pane.clone()));
                }
                group.into_any_element()
            }
        }
    }
}

impl Render for MicaWorkspace {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let title_bar = self.render_titlebar(cx);
        let center_content = self.render_center_workspace(cx);

        let show_left = self.settings.show_left_sidebar;
        let show_right = self.settings.show_sftp_panel;

        let body = if !show_left && !show_right {
            div().flex_1().min_h_0().child(center_content).into_any_element()
        } else {
            let layout_key = match (show_left, show_right) {
                (true, true) => "main-layout-lcr",
                (true, false) => "main-layout-lc",
                (false, true) => "main-layout-cr",
                (false, false) => "main-layout-c",
            };
            let mut group = h_resizable(layout_key);

            if show_left {
                let sidebar = LeftSidebar::new(
                    cx.entity().clone(),
                    self.sidebar_section,
                    self.sidebar_search_input.clone(),
                    self.hosts.flatten_visible(&self.sidebar_query),
                    self.filtered_snippets(),
                    self.filtered_keychain(),
                    self.selected_sidebar_id.clone(),
                );
                group = group.child(
                    resizable_panel()
                        .size(px(256.0))
                        .size_range(px(196.0)..px(420.0))
                        .child(sidebar),
                );
            }

            group = group.child(resizable_panel().child(center_content));

            if show_right {
                group = group.child(
                    resizable_panel()
                        .size(px(344.0))
                        .size_range(px(260.0)..px(560.0))
                        .child(self.sftp_panel.clone()),
                );
            }

            div().flex_1().min_h_0().child(group).into_any_element()
        };

        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(title_bar)
            .child(body)
    }
}

//! Root `MicaWorkspace`: unified macOS Sonoma/Sequoia Toolbar + `h_resizable` 3-pane adaptive layout
//! (Finder Source-List Left Sidebar | Multi-Tab & Recursive Split-Tree Terminal Workspace | Right SFTP Inspector).

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
    scroll::ScrollableElement as _,
    v_flex, v_resizable,
};
use gpui_kit::{
    AnyElement, App, AppContext as _, Context, Entity, FocusHandle, Focusable as _, FontWeight,
    InteractiveElement as _, IntoElement, KeyDownEvent, MouseButton,
    ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _, Window,
    div, px,
};

use crate::domain::{
    AppSettings, CredentialStore, HostNode, HostTree, KeychainEntry, ResolvedSshConfig,
    RightClickAction, SecretPayload, ShortcutAction, ShortcutChord, ShortcutScope, SnippetItem,
    host_secret_key, keychain_secret_key, tooltip,
};
use crate::ssh::KnownHostsStore;
use crate::storage::{AppPaths, WorkspaceSnapshot, WorkspaceStore};
use crate::terminal::{
    SemanticOptions, TerminalSessionEntity,
    session::{SessionEntityEvent, SessionPhase},
};
use crate::ui::modals::{
    SettingsTab, SshHostModalInit, open_confirm_dialog, open_keychain_modal, open_settings_modal,
    open_settings_modal_at, open_simple_name_dialog, open_snippet_modal, open_ssh_host_modal,
};
use crate::ui::sftp_panel::{SftpPanelEvent, SftpPanelView};
use crate::ui::sidebar::{LeftSidebar, SidebarSection};
use crate::ui::terminal_view::{PanePreferences, TerminalPaneEvent, TerminalPaneView};
use crate::ui::theme::apply_workspace_theme;
use crate::ui::theme_catalog::ThemeRegistry;
use crate::ui::welcome_view::{QuickConnectItem, WelcomeView};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabSplitAxis {
    Horizontal,
    Vertical,
}

/// Recursive 2D split layout tree inside a single workspace tab.
#[derive(Clone)]
pub enum PaneLayoutNode {
    Leaf(Entity<TerminalPaneView>),
    Split {
        id: String,
        axis: TabSplitAxis,
        children: Vec<PaneLayoutNode>,
    },
}

impl PaneLayoutNode {
    pub fn collect_panes(&self, out: &mut Vec<Entity<TerminalPaneView>>) {
        match self {
            Self::Leaf(pane) => out.push(pane.clone()),
            Self::Split { children, .. } => {
                for c in children {
                    c.collect_panes(out);
                }
            }
        }
    }

    pub fn split_target(
        &mut self,
        target: &Entity<TerminalPaneView>,
        axis: TabSplitAxis,
        new_pane: Entity<TerminalPaneView>,
    ) -> bool {
        match self {
            Self::Leaf(existing) => {
                if existing == target {
                    let old_leaf = Self::Leaf(existing.clone());
                    let new_leaf = Self::Leaf(new_pane);
                    *self = Self::Split {
                        id: uuid::Uuid::new_v4().to_string(),
                        axis,
                        children: vec![old_leaf, new_leaf],
                    };
                    true
                } else {
                    false
                }
            }
            Self::Split {
                axis: cur_axis,
                children,
                ..
            } => {
                if *cur_axis == axis
                    && let Some(pos) = children
                        .iter()
                        .position(|child| matches!(child, Self::Leaf(p) if p == target))
                {
                    children.insert(pos + 1, Self::Leaf(new_pane));
                    return true;
                }
                for child in children.iter_mut() {
                    if child.split_target(target, axis, new_pane.clone()) {
                        return true;
                    }
                }
                false
            }
        }
    }

    /// Removes `target` from the tree, collapsing single-child splits.
    /// Returns `None` when the entire subtree becomes empty.
    pub fn remove_pane(self, target: &Entity<TerminalPaneView>) -> Option<Self> {
        match self {
            Self::Leaf(pane) => (&pane != target).then_some(Self::Leaf(pane)),
            Self::Split { id, axis, children } => {
                let mut kept: Vec<Self> = children
                    .into_iter()
                    .filter_map(|c| c.remove_pane(target))
                    .collect();
                match kept.len() {
                    0 => None,
                    1 => kept.pop(),
                    _ => Some(Self::Split {
                        id,
                        axis,
                        children: kept,
                    }),
                }
            }
        }
    }

    #[inline(never)]
    pub fn render_element(&self) -> AnyElement {
        match self {
            Self::Leaf(pane) => div().size_full().child(pane.clone()).into_any_element(),
            Self::Split { id, axis, children } => {
                let group_id = SharedString::from(format!("split-node-{id}"));
                match axis {
                    TabSplitAxis::Horizontal => {
                        let mut group = h_resizable(group_id);
                        for child in children {
                            group = group.child(resizable_panel().child(child.render_element()));
                        }
                        group.into_any_element()
                    }
                    TabSplitAxis::Vertical => {
                        let mut group = v_resizable(group_id);
                        for child in children {
                            group = group.child(resizable_panel().child(child.render_element()));
                        }
                        group.into_any_element()
                    }
                }
            }
        }
    }
}

pub struct WorkspaceTab {
    pub id: String,
    pub title: String,
    pub root: PaneLayoutNode,
    pub active_pane: Entity<TerminalPaneView>,
}

impl WorkspaceTab {
    pub fn panes(&self) -> Vec<Entity<TerminalPaneView>> {
        let mut out = Vec::new();
        self.root.collect_panes(&mut out);
        out
    }

    pub fn active_pane(&self) -> Option<Entity<TerminalPaneView>> {
        let panes = self.panes();
        if panes.contains(&self.active_pane) {
            Some(self.active_pane.clone())
        } else {
            panes.into_iter().next()
        }
    }

    pub fn refresh_split_flags(&self, cx: &mut App) {
        let panes = self.panes();
        let is_split = panes.len() > 1;
        for pane in panes {
            let active = pane == self.active_pane;
            pane.update(cx, |p, cx| {
                if p.is_split_member != is_split || p.is_active_pane != active {
                    p.is_split_member = is_split;
                    p.is_active_pane = active;
                    cx.notify();
                }
            });
        }
    }
}

pub struct MicaWorkspace {
    pub paths: AppPaths,
    store: WorkspaceStore,
    credentials: Arc<dyn CredentialStore>,
    known_hosts: KnownHostsStore,
    tokio_handle: tokio::runtime::Handle,
    focus_handle: FocusHandle,

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
        mut snap: WorkspaceSnapshot,
        credentials: Arc<dyn CredentialStore>,
        tokio_handle: tokio::runtime::Handle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let known_hosts = KnownHostsStore::new(&paths.known_hosts_path);
        crate::syntax::SyntaxRegistry::global().reload_from_dir(Some(&paths.syntaxes_dir));
        let theme_pack = ThemeRegistry::global().reload_from_dir(Some(&paths.themes_dir));
        if theme_pack.reconcile_settings(&mut snap.settings) {
            let _ = store.save_settings(&snap.settings);
        }
        let focus_handle = cx.focus_handle();
        focus_handle.focus(window, cx);

        apply_workspace_theme(&snap.settings, Some(window), cx);

        let sidebar_search_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("搜索主机、IP、片段..."));
        cx.subscribe_in(&sidebar_search_input, window, |this, state, ev, _, cx| {
            if matches!(ev, InputEvent::Change) {
                this.sidebar_query = state.read(cx).value().to_string();
                cx.notify();
            }
        })
        .detach();

        let follow_sftp = snap.settings.follow_sftp_cwd;
        let show_sftp = snap.settings.show_sftp_panel;
        let init_font_family = snap.settings.font_family.clone();
        let init_font_size = snap.settings.font_size;
        let dl_dir = paths.downloads_dir.clone();
        let rt_sftp = tokio_handle.clone();
        let sftp_panel = cx.new(|_| {
            SftpPanelView::new(
                follow_sftp,
                show_sftp,
                init_font_family,
                init_font_size,
                dl_dir,
                rt_sftp,
            )
        });
        cx.subscribe_in(
            &sftp_panel,
            window,
            |this, _, event, window, cx| match event {
                SftpPanelEvent::FollowChanged(follow) => {
                    this.settings.follow_sftp_cwd = *follow;
                    let _ = this.store.save_settings(&this.settings);
                    cx.notify();
                }
                SftpPanelEvent::CloseRequested => this.toggle_sftp_panel(window, cx),
            },
        )
        .detach();
        cx.observe(&sftp_panel, |_, _, cx| cx.notify()).detach();

        Self {
            paths,
            store,
            credentials,
            known_hosts,
            tokio_handle,
            focus_handle,
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

    fn current_pane_preferences(&self) -> PanePreferences {
        PanePreferences {
            font_family: self.settings.font_family.clone(),
            default_font_size: self.settings.font_size,
            dark_mode: self.settings.dark_mode,
            theme_id: self.settings.active_theme_id().to_string(),
            scrollback_lines: self.settings.scrollback_lines,
            right_click_action: self.settings.right_click_action,
            copy_on_select: self.settings.copy_on_select,
            shortcuts: self.settings.shortcuts.clone(),
            semantic: SemanticOptions {
                command_blocks: self.settings.command_blocks,
                semantic_highlighting: self.settings.semantic_highlighting,
                code_highlighting: self.settings.code_highlighting,
                overview_markers: self.settings.overview_markers,
            },
        }
    }

    pub fn set_sidebar_section(&mut self, section: SidebarSection, cx: &mut Context<Self>) {
        self.sidebar_section = section;
        cx.notify();
    }

    pub fn toggle_left_sidebar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings.show_left_sidebar = !self.settings.show_left_sidebar;
        let _ = self.store.save_settings(&self.settings);
        self.focus_active_pane(window, cx);
        cx.notify();
    }

    pub fn toggle_sftp_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings.show_sftp_panel = !self.settings.show_sftp_panel;
        let visible = self.settings.show_sftp_panel;
        let _ = self.store.save_settings(&self.settings);
        self.sftp_panel
            .update(cx, |p, cx| p.set_panel_visible(visible, Some(window), cx));
        if visible {
            self.sync_sftp_binding(Some(window), cx);
        }
        self.focus_active_pane(window, cx);
        cx.notify();
    }

    pub fn set_dark_mode(&mut self, dark: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.settings.dark_mode = dark;
        let _ = self.store.save_settings(&self.settings);
        apply_workspace_theme(&self.settings, Some(window), cx);
        self.propagate_pane_preferences(cx);
        if !window.has_active_dialog(cx) {
            self.focus_active_pane(window, cx);
        }
        cx.notify();
    }

    pub fn select_theme(&mut self, theme_id: &str, window: &mut Window, cx: &mut Context<Self>) {
        let pack = ThemeRegistry::global().current();
        let Some(spec) = pack.get(theme_id).cloned() else {
            return;
        };
        if spec.dark {
            self.settings.dark_theme = spec.id;
            self.settings.dark_mode = true;
        } else {
            self.settings.light_theme = spec.id;
            self.settings.dark_mode = false;
        }
        let _ = self.store.save_settings(&self.settings);
        apply_workspace_theme(&self.settings, Some(window), cx);
        self.propagate_pane_preferences(cx);
        if !window.has_active_dialog(cx) {
            self.focus_active_pane(window, cx);
        }
        cx.notify();
    }

    pub fn set_tint_ui_with_theme(
        &mut self,
        enabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.settings.tint_ui_with_theme = enabled;
        let _ = self.store.save_settings(&self.settings);
        apply_workspace_theme(&self.settings, Some(window), cx);
        cx.notify();
    }

    pub fn reload_custom_themes(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let pack = ThemeRegistry::global().reload_from_dir(Some(&self.paths.themes_dir));
        if pack.reconcile_settings(&mut self.settings) {
            let _ = self.store.save_settings(&self.settings);
        }
        let loaded = pack.loaded_custom_count();
        let failed = pack.custom_entries.len().saturating_sub(loaded);
        apply_workspace_theme(&self.settings, Some(window), cx);
        self.propagate_pane_preferences(cx);
        let msg = if failed > 0 {
            format!("主题库已重载：成功加载 {loaded} 个外部主题，{failed} 个条目存在读取或格式错误")
        } else {
            format!(
                "主题库已重载：内置 {} 款主题，已加载 {loaded} 个自定义主题",
                pack.builtin_count
            )
        };
        window.push_notification(Notification::info(msg), cx);
        window.refresh();
        cx.notify();
    }

    pub fn import_custom_themes(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let themes_dir = self.paths.themes_dir.clone();
        let win_handle = window.window_handle();

        cx.spawn(async move |weak, cx| {
            let picked = rfd::AsyncFileDialog::new()
                .set_title("选择要导入的终端主题文件 (Ghostty / iTerm2 .itermcolors)")
                .add_filter(
                    "主题配色文件",
                    &["itermcolors", "ghostty", "conf", "theme", "txt", "toml"],
                )
                .add_filter("所有文件 (含无后缀 Ghostty 主题)", &["*"])
                .pick_files()
                .await;
            let Some(handles) = picked else {
                return;
            };
            let source_paths: Vec<std::path::PathBuf> =
                handles.into_iter().map(|h| h.path().to_path_buf()).collect();
            let selected_count = source_paths.len();
            let first_fname = source_paths
                .first()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().into_owned());
            let import_res = cx
                .background_executor()
                .spawn(async move {
                    ThemeRegistry::global().import_files(&themes_dir, &source_paths)
                })
                .await;

            let _ = cx.update_window(win_handle, |_, window, cx| {
                if let Some(ws_ent) = weak.upgrade() {
                    ws_ent.update(cx, |ws, cx| {
                        match import_res {
                            Ok(pack) => {
                                if let Some(fname) = first_fname
                                    && let Some(entry) =
                                        pack.custom_entries.iter().find(|e| e.file_name == fname)
                                    && let Some(ref tid) = entry.theme_id
                                {
                                    ws.select_theme(tid, window, cx);
                                } else {
                                    if pack.reconcile_settings(&mut ws.settings) {
                                        let _ = ws.store.save_settings(&ws.settings);
                                    }
                                    apply_workspace_theme(&ws.settings, Some(window), cx);
                                    ws.propagate_pane_preferences(cx);
                                }
                                let loaded = pack.loaded_custom_count();
                                window.push_notification(
                                    Notification::success(format!(
                                        "已导入 {selected_count} 个主题文件（当前生效自定义主题 {loaded} 个）",
                                    )),
                                    cx,
                                );
                            }
                            Err(err) => {
                                window.push_notification(
                                    Notification::error(format!("导入主题文件失败：{err:#}")),
                                    cx,
                                );
                            }
                        }
                        window.refresh();
                        cx.notify();
                    });
                }
            });
        })
        .detach();
    }

    pub fn remove_custom_theme(
        &mut self,
        file_name: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match ThemeRegistry::global().remove_custom_file(&self.paths.themes_dir, file_name) {
            Ok(pack) => {
                if pack.reconcile_settings(&mut self.settings) {
                    let _ = self.store.save_settings(&self.settings);
                }
                apply_workspace_theme(&self.settings, Some(window), cx);
                self.propagate_pane_preferences(cx);
                window.refresh();
                cx.notify();
            }
            Err(err) => {
                window.push_notification(
                    Notification::error(format!("移除主题文件失败：{err:#}")),
                    cx,
                );
            }
        }
    }

    pub fn adjust_default_font_size(&mut self, delta: f32, cx: &mut Context<Self>) {
        self.settings.font_size = (self.settings.font_size + delta).clamp(9.0, 36.0);
        let new_size = self.settings.font_size;
        let _ = self.store.save_settings(&self.settings);
        for tab in &self.tabs {
            for pane in tab.panes() {
                let session = pane.read(cx).session.clone();
                session.update(cx, |s, cx| s.reset_font(new_size, cx));
            }
        }
        self.propagate_pane_preferences(cx);
        cx.notify();
    }

    pub fn set_font_family(&mut self, family: String, cx: &mut Context<Self>) {
        let normalized = if family.trim().is_empty() {
            crate::domain::default_terminal_font_family()
        } else {
            family.trim().to_owned()
        };
        if self.settings.font_family != normalized {
            self.settings.font_family = normalized;
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

    pub fn set_command_blocks(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.settings.command_blocks = enabled;
        let _ = self.store.save_settings(&self.settings);
        self.propagate_pane_preferences(cx);
        cx.notify();
    }

    pub fn set_semantic_highlighting(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.settings.semantic_highlighting = enabled;
        let _ = self.store.save_settings(&self.settings);
        self.propagate_pane_preferences(cx);
        cx.notify();
    }

    pub fn set_code_highlighting(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.settings.code_highlighting = enabled;
        let _ = self.store.save_settings(&self.settings);
        self.propagate_pane_preferences(cx);
        cx.notify();
    }

    pub fn reload_syntax_extensions(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let pack = crate::syntax::SyntaxRegistry::global()
            .reload_from_dir(Some(&self.paths.syntaxes_dir));
        let loaded = pack.loaded_custom_count();
        let failed = pack.custom_entries.len().saturating_sub(loaded);
        self.propagate_pane_preferences(cx);
        let msg = if failed > 0 {
            format!("语法库已重载：成功加载 {loaded} 个拓展，{failed} 个文件存在错误")
        } else {
            format!(
                "语法库已重载：内置 {} 种语言，已加载 {loaded} 个自定义语法",
                pack.builtin_count
            )
        };
        window.push_notification(Notification::info(msg), cx);
        window.refresh();
        cx.notify();
    }

    pub fn import_syntax_extensions(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let syntaxes_dir = self.paths.syntaxes_dir.clone();
        let win_handle = window.window_handle();

        cx.spawn(async move |weak, cx| {
            let picked = rfd::AsyncFileDialog::new()
                .set_title("选择要导入的语法定义文件 (.sublime-syntax / .tmLanguage.json)")
                .add_filter("语法定义文件", &["sublime-syntax", "json", "tmLanguage"])
                .pick_files()
                .await;
            let Some(handles) = picked else {
                return;
            };
            let source_paths: Vec<std::path::PathBuf> =
                handles.into_iter().map(|h| h.path().to_path_buf()).collect();
            let import_res = crate::syntax::SyntaxRegistry::global()
                .import_files(&syntaxes_dir, &source_paths);

            let _ = cx.update_window(win_handle, |_, window, cx| {
                if let Some(ws_ent) = weak.upgrade() {
                    ws_ent.update(cx, |ws, cx| {
                        ws.propagate_pane_preferences(cx);
                        match import_res {
                            Ok(pack) => {
                                let loaded = pack.loaded_custom_count();
                                window.push_notification(
                                    Notification::success(format!(
                                        "已导入 {} 个文件（当前生效自定义语法 {loaded} 个）",
                                        source_paths.len()
                                    )),
                                    cx,
                                );
                            }
                            Err(err) => {
                                window.push_notification(
                                    Notification::error(format!("导入语法文件失败：{err:#}")),
                                    cx,
                                );
                            }
                        }
                        window.refresh();
                        cx.notify();
                    });
                }
            });
        })
        .detach();
    }

    pub fn remove_syntax_extension(
        &mut self,
        file_name: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match crate::syntax::SyntaxRegistry::global()
            .remove_custom_file(&self.paths.syntaxes_dir, file_name)
        {
            Ok(_) => {
                self.propagate_pane_preferences(cx);
                window.refresh();
                cx.notify();
            }
            Err(err) => {
                window.push_notification(
                    Notification::error(format!("移除语法文件失败：{err:#}")),
                    cx,
                );
            }
        }
    }

    pub fn set_overview_markers(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.settings.overview_markers = enabled;
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

    pub fn set_right_click_action(&mut self, action: RightClickAction, cx: &mut Context<Self>) {
        self.settings.right_click_action = action;
        let _ = self.store.save_settings(&self.settings);
        self.propagate_pane_preferences(cx);
        cx.notify();
    }

    pub fn set_copy_on_select(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.settings.copy_on_select = enabled;
        let _ = self.store.save_settings(&self.settings);
        self.propagate_pane_preferences(cx);
        cx.notify();
    }

    pub fn set_shortcut(
        &mut self,
        action: ShortcutAction,
        chord: Option<ShortcutChord>,
        cx: &mut Context<Self>,
    ) -> Option<ShortcutAction> {
        let displaced = self.settings.shortcuts.set(action, chord);
        let _ = self.store.save_settings(&self.settings);
        self.propagate_pane_preferences(cx);
        cx.notify();
        displaced
    }

    pub fn reset_shortcut(&mut self, action: ShortcutAction, cx: &mut Context<Self>) {
        self.settings.shortcuts.reset_one(action);
        let _ = self.store.save_settings(&self.settings);
        self.propagate_pane_preferences(cx);
        cx.notify();
    }

    pub fn reset_all_shortcuts(&mut self, cx: &mut Context<Self>) {
        self.settings.shortcuts.reset_all();
        let _ = self.store.save_settings(&self.settings);
        self.propagate_pane_preferences(cx);
        cx.notify();
    }

    fn propagate_pane_preferences(&mut self, cx: &mut Context<Self>) {
        let prefs = self.current_pane_preferences();
        let fam = self.settings.font_family.clone();
        let fsize = self.settings.font_size;
        self.sftp_panel.update(cx, |p, _| {
            p.set_editor_typography(fam, fsize);
        });
        for tab in &self.tabs {
            for pane in tab.panes() {
                pane.update(cx, |p, cx| p.sync_preferences(&prefs, cx));
            }
        }
    }

    fn create_pane_for_session(
        &self,
        session: Entity<TerminalSessionEntity>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<TerminalPaneView> {
        cx.subscribe(&session, move |this, session_ent, event, cx| match event {
            SessionEntityEvent::TitleChanged(new_title) => {
                for tab in &mut this.tabs {
                    if tab
                        .active_pane()
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

        let prefs = self.current_pane_preferences();
        let pane = cx.new(|cx| TerminalPaneView::new(session, prefs, window, cx));
        cx.subscribe_in(&pane, window, |this, pane, event, window, cx| match event {
            TerminalPaneEvent::Activate => this.activate_pane(pane, Some(window), cx),
            TerminalPaneEvent::CloseRequested => this.close_pane(pane, window, cx),
        })
        .detach();
        pane
    }

    fn is_active_session(&self, session: &Entity<TerminalSessionEntity>, cx: &App) -> bool {
        let Some(active_id) = &self.active_tab_id else {
            return false;
        };
        let Some(tab) = self.tabs.iter().find(|t| &t.id == active_id) else {
            return false;
        };
        let Some(pane) = tab.active_pane() else {
            return false;
        };
        &pane.read(cx).session == session
    }

    pub fn activate_pane(
        &mut self,
        pane: &Entity<TerminalPaneView>,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        if let Some(tab) = self
            .tabs
            .iter_mut()
            .find(|tab| tab.active_pane != *pane && tab.panes().contains(pane))
        {
            tab.active_pane = pane.clone();
            tab.refresh_split_flags(cx);
            self.sync_sftp_binding(window, cx);
            cx.notify();
        }
    }

    pub fn close_pane(
        &mut self,
        pane: &Entity<TerminalPaneView>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(tab) = self.tabs.iter_mut().find(|t| t.panes().contains(pane)) else {
            return;
        };

        let Some(updated_root) = tab.root.clone().remove_pane(pane) else {
            let tab_id = tab.id.clone();
            self.close_tab(&tab_id, window, cx);
            return;
        };

        tab.root = updated_root;
        if tab.active_pane == *pane
            && let Some(next_pane) = tab.panes().pop()
        {
            next_pane.focus_handle(cx).focus(window, cx);
            tab.active_pane = next_pane;
        }
        tab.refresh_split_flags(cx);
        self.sync_sftp_binding(Some(window), cx);
        cx.notify();
    }

    pub fn open_local_shell_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.local_shell_counter += 1;
        let title = format!("本地终端 {}", self.local_shell_counter);
        tracing::info!(title = %title, "打开新建本地终端标签页");
        let fsize = self.settings.font_size;
        let sb = self.settings.scrollback_lines;
        let rt = self.tokio_handle.clone();

        let session =
            cx.new(|cx| TerminalSessionEntity::new_local(title.clone(), fsize, sb, rt, cx));
        let pane = self.create_pane_for_session(session, window, cx);
        let focus = pane.focus_handle(cx);

        let tab_id = uuid::Uuid::new_v4().to_string();
        self.tabs.push(WorkspaceTab {
            id: tab_id.clone(),
            title,
            root: PaneLayoutNode::Leaf(pane.clone()),
            active_pane: pane,
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
                tracing::warn!(host_id = %host_id, error = ?err, "解析 SSH 主机连接配置失败");
                window.push_notification(
                    Notification::error(format!("无法发起连接：{err:#}")),
                    cx,
                );
                return;
            }
        };

        tracing::info!(
            host_id = %host_id,
            name = %resolved.name,
            endpoint = %format!("{}:{}", resolved.host, resolved.port),
            user = %resolved.user,
            "发起新建 SSH 标签页连接"
        );
        self.record_recent_host(host_id);
        let title = resolved.name.clone();
        let kh = self.known_hosts.clone();
        let fsize = self.settings.font_size;
        let sb = self.settings.scrollback_lines;
        let rt = self.tokio_handle.clone();

        let session =
            cx.new(|cx| TerminalSessionEntity::new_ssh(resolved, kh, fsize, sb, rt, cx));
        let pane = self.create_pane_for_session(session, window, cx);
        let focus = pane.focus_handle(cx);

        let tab_id = uuid::Uuid::new_v4().to_string();
        self.tabs.push(WorkspaceTab {
            id: tab_id.clone(),
            title,
            root: PaneLayoutNode::Leaf(pane.clone()),
            active_pane: pane,
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
        let resolved = match ResolvedSshConfig::resolve_from_tree(
            host_id,
            &self.hosts,
            &self.keychain,
            self.credentials.as_ref(),
        ) {
            Ok(cfg) => cfg,
            Err(err) => {
                window.push_notification(
                    Notification::error(format!("无法在分屏中发起连接：{err:#}")),
                    cx,
                );
                return;
            }
        };
        self.record_recent_host(host_id);
        let kh = self.known_hosts.clone();
        let fsize = self.settings.font_size;
        let sb = self.settings.scrollback_lines;
        let rt = self.tokio_handle.clone();
        let session =
            cx.new(|cx| TerminalSessionEntity::new_ssh(resolved, kh, fsize, sb, rt, cx));
        let pane = self.create_pane_for_session(session, window, cx);
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
        let Some(cur_pane) = tab.active_pane() else {
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
            } => {
                cx.new(|cx| TerminalSessionEntity::new_ssh(*config, known_hosts, fsize, sb, rt, cx))
            }
        };

        let new_pane = self.create_pane_for_session(new_session, window, cx);
        self.attach_split_pane(new_pane, axis, window, cx);
    }

    fn attach_split_pane(
        &mut self,
        pane: Entity<TerminalPaneView>,
        axis: TabSplitAxis,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(active_id) = self.active_tab_id.clone() else {
            return;
        };
        let focus = pane.focus_handle(cx);
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == active_id) {
            if tab.panes().len() >= 6 {
                window.push_notification(
                    Notification::info("单个标签页最多支持 6 个分屏面板"),
                    cx,
                );
                return;
            }
            if let Some(target) = tab.active_pane()
                && tab.root.split_target(&target, axis, pane.clone())
            {
                tab.active_pane = pane;
                tab.refresh_split_flags(cx);
            }
        }
        self.sync_sftp_binding(Some(window), cx);
        focus.focus(window, cx);
        cx.notify();
    }

    fn active_tab(&self) -> Option<&WorkspaceTab> {
        let id = self.active_tab_id.as_ref()?;
        self.tabs.iter().find(|tab| &tab.id == id)
    }

    fn focus_active_pane(&self, window: &mut Window, cx: &mut App) {
        if let Some(pane) = self.active_tab().and_then(WorkspaceTab::active_pane) {
            pane.focus_handle(cx).focus(window, cx);
        } else {
            self.focus_handle.focus(window, cx);
        }
    }

    pub fn select_tab(&mut self, tab_id: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.active_tab_id = Some(tab_id.to_string());
        self.focus_active_pane(window, cx);
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
                self.focus_handle.focus(window, cx);
                None
            } else {
                let next_idx = idx.min(self.tabs.len() - 1);
                Some(self.tabs[next_idx].id.clone())
            };
        }
        self.focus_active_pane(window, cx);
        self.sync_sftp_binding(Some(window), cx);
        cx.notify();
    }

    pub fn close_other_tabs(&mut self, keep_id: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.tabs.retain(|t| t.id == keep_id);
        self.active_tab_id = self.tabs.first().map(|t| t.id.clone());
        self.focus_active_pane(window, cx);
        self.sync_sftp_binding(Some(window), cx);
        cx.notify();
    }

    pub fn clone_tab(&mut self, tab_id: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tab) = self.tabs.iter().find(|t| t.id == tab_id) else {
            return;
        };
        let Some(pane) = tab.active_pane() else {
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
                    TerminalSessionEntity::new_ssh(*config, known_hosts, fsize, sb, rt, cx)
                });
                let new_pane = self.create_pane_for_session(session, window, cx);
                let focus = new_pane.focus_handle(cx);
                let new_id = uuid::Uuid::new_v4().to_string();
                self.tabs.push(WorkspaceTab {
                    id: new_id.clone(),
                    title,
                    root: PaneLayoutNode::Leaf(new_pane.clone()),
                    active_pane: new_pane,
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
            .active_tab()
            .and_then(WorkspaceTab::active_pane)
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
            SshHostModalInit {
                editing_node,
                existing_secrets,
                default_parent_id,
                folders,
                jump_candidates,
                keychains,
            },
            window,
            cx,
        );
    }

    pub fn save_host_node(
        &mut self,
        node: HostNode,
        secrets: SecretPayload,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Err(err) = self
            .credentials
            .put_payload(&host_secret_key(&node.id), &secrets)
        {
            window.push_notification(
                Notification::error(format!("保存安全凭据失败：{err:#}")),
                cx,
            );
        }
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

    pub fn confirm_delete_host_node(
        &mut self,
        node_id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(node) = self.hosts.get(node_id) else {
            return;
        };
        let desc = self.hosts.descendant_count(node_id);
        let (title, msg) = if node.is_folder() {
            (
                "删除文件夹",
                if desc > 0 {
                    format!(
                        "确定删除文件夹 `{}` 及其包含的 {} 个子项（含保存的凭据）吗？",
                        node.name, desc
                    )
                } else {
                    format!("确定删除空文件夹 `{}` 吗？", node.name)
                },
            )
        } else {
            (
                "删除主机",
                format!("确定永久删除 SSH 主机 `{}` 及其保存的凭据吗？", node.name),
            )
        };
        let ws = cx.entity().clone();
        let target_id = node_id.to_string();
        open_confirm_dialog(title, msg, "确认删除", window, cx, move |_, cx| {
            ws.update(cx, |this, cx| this.delete_host_node(&target_id, cx));
        });
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
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Err(err) = self
            .credentials
            .put_payload(&keychain_secret_key(&entry.id), &secrets)
        {
            window.push_notification(
                Notification::error(format!("保存密钥凭据失败：{err:#}")),
                cx,
            );
        }
        if let Some(ex) = self.keychain.iter_mut().find(|k| k.id == entry.id) {
            *ex = entry;
        } else {
            self.keychain.push(entry);
        }
        let _ = self.store.save_keychain(&self.keychain);
        cx.notify();
    }

    pub fn confirm_delete_keychain(
        &mut self,
        keychain_id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(entry) = self.keychain.iter().find(|k| k.id == keychain_id) else {
            return;
        };
        let msg = format!(
            "确定删除密钥库凭据 `{}` 吗？引用该凭据的主机将需要重新配置认证。",
            entry.name
        );
        let ws = cx.entity().clone();
        let kid = keychain_id.to_string();
        open_confirm_dialog("删除密钥凭据", msg, "确认删除", window, cx, move |_, cx| {
            ws.update(cx, |this, cx| this.delete_keychain(&kid, cx));
        });
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

    pub fn confirm_delete_snippet(
        &mut self,
        snippet_id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(snip) = self.snippets.iter().find(|s| s.id == snippet_id) else {
            return;
        };
        let msg = format!("确定删除命令片段 `{}` 吗？", snip.title);
        let ws = cx.entity().clone();
        let sid = snippet_id.to_string();
        open_confirm_dialog("删除命令片段", msg, "确认删除", window, cx, move |_, cx| {
            ws.update(cx, |this, cx| this.delete_snippet(&sid, cx));
        });
    }

    pub fn delete_snippet(&mut self, snippet_id: &str, cx: &mut Context<Self>) {
        self.snippets.retain(|s| s.id != snippet_id);
        let _ = self.store.save_snippets(&self.snippets);
        cx.notify();
    }

    pub fn run_snippet(&mut self, snippet_id: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(snip) = self.snippets.iter().find(|s| s.id == snippet_id).cloned() else {
            return;
        };
        let Some(pane) = self.active_tab().and_then(WorkspaceTab::active_pane) else {
            return;
        };
        let session = pane.read(cx).session.clone();
        session.update(cx, |s, cx| {
            s.send_command_snippet(&snip.command, snip.auto_enter, cx);
        });
        self.focus_active_pane(window, cx);
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
                k.name.to_lowercase().contains(&q)
                    || k.username.to_lowercase().contains(&q)
                    || k.remark.to_lowercase().contains(&q)
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

    fn handle_capture_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if window.has_active_dialog(cx)
            || self
                .active_tab()
                .is_some_and(|tab| tab.active_pane.read(cx).has_marked_text())
        {
            return;
        }
        let Some(action) = self
            .settings
            .shortcuts
            .match_action(&event.keystroke, ShortcutScope::Workspace)
        else {
            return;
        };
        cx.stop_propagation();
        match action {
            ShortcutAction::NewLocalTab => self.open_local_shell_tab(window, cx),
            ShortcutAction::NewSshHost => self.open_host_dialog(None, None, window, cx),
            ShortcutAction::ClosePaneOrTab => {
                if let Some(pane) = self.active_tab().and_then(WorkspaceTab::active_pane) {
                    self.close_pane(&pane, window, cx);
                }
            }
            ShortcutAction::SplitHorizontal => {
                self.split_active_tab(TabSplitAxis::Horizontal, window, cx)
            }
            ShortcutAction::SplitVertical => {
                self.split_active_tab(TabSplitAxis::Vertical, window, cx)
            }
            ShortcutAction::NextTab | ShortcutAction::PrevTab => {
                let count = self.tabs.len();
                if count < 2 {
                    return;
                }
                let Some(index) = self
                    .tabs
                    .iter()
                    .position(|t| Some(&t.id) == self.active_tab_id.as_ref())
                else {
                    tracing::error!("切换标签页失败，工作区没有有效的激活标签页");
                    return;
                };
                let next = if action == ShortcutAction::PrevTab {
                    (index + count - 1) % count
                } else {
                    (index + 1) % count
                };
                let id = self.tabs[next].id.clone();
                self.select_tab(&id, window, cx);
            }
            ShortcutAction::ToggleLeftSidebar => self.toggle_left_sidebar(window, cx),
            ShortcutAction::ToggleSftpPanel => self.toggle_sftp_panel(window, cx),
            ShortcutAction::OpenSettings => {
                let workspace = cx.entity();
                let paths = self.paths.clone();
                // Opening preferences reads the workspace after this listener releases its update.
                window.defer(cx, move |window, cx| {
                    open_settings_modal(workspace, paths, window, cx)
                });
            }
            tab => {
                let index = tab.tab_index().expect("工作区快捷键缺少分发操作");
                if let Some(tab) = self.tabs.get(index) {
                    let id = tab.id.clone();
                    self.select_tab(&id, window, cx);
                }
            }
        }
    }

    #[inline(never)]
    fn render_single_tab_pill(
        &self,
        tab: &WorkspaceTab,
        workspace: Entity<MicaWorkspace>,
        cx: &App,
    ) -> AnyElement {
        let tid = tab.id.clone();
        let tid_close = tid.clone();
        let tid_ctx_h = tid.clone();
        let tid_ctx_v = tid.clone();
        let tid_ctx_clone = tid.clone();
        let tid_ctx_others = tid.clone();
        let tid_ctx_close = tid.clone();

        let is_active = self.active_tab_id.as_deref() == Some(tid.as_str());
        let (is_ssh, dot_color) = tab
            .active_pane()
            .map(|p| {
                let s = p.read(cx).session.read(cx);
                let c = match s.phase {
                    SessionPhase::Connected => cx.theme().success,
                    SessionPhase::Connecting(_) | SessionPhase::AwaitingHostKey(_) => {
                        cx.theme().warning
                    }
                    SessionPhase::Error(_) => cx.theme().danger,
                    SessionPhase::Disconnected(_) => cx.theme().muted_foreground.opacity(0.55),
                };
                (s.is_ssh(), c)
            })
            .unwrap_or((false, cx.theme().muted_foreground));

        let ws_sel = workspace.clone();
        let ws_close = workspace.clone();
        let ws_c_h = workspace.clone();
        let ws_c_v = workspace.clone();
        let ws_c_clone = workspace.clone();
        let ws_c_others = workspace.clone();
        let ws_c_close = workspace;

        h_flex()
            .id(SharedString::from(format!("ws-tab-{tid}")))
            .flex_shrink_0()
            .max_w(px(210.0))
            .h(px(27.0))
            .pl_2p5()
            .pr_1p5()
            .gap_1p5()
            .items_center()
            .rounded(px(7.0))
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
            .hover(|s| {
                if is_active {
                    s
                } else {
                    s.bg(cx.theme().list_hover)
                }
            })
            .on_mouse_down(MouseButton::Left, |_, window, _| {
                window.prevent_default();
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
                .item(
                    PopupMenuItem::new("关闭其他标签").on_click(move |_, window, cx| {
                        wo.update(cx, |ws, cx| ws.close_other_tabs(&to, window, cx));
                    }),
                )
                .item(
                    PopupMenuItem::new("关闭标签").on_click(move |_, window, cx| {
                        wx.update(cx, |ws, cx| ws.close_tab(&tx, window, cx));
                    }),
                )
            })
            .child(
                div()
                    .flex_shrink_0()
                    .w(px(6.5))
                    .h(px(6.5))
                    .rounded_full()
                    .bg(dot_color),
            )
            .child(
                Icon::new(if is_ssh {
                    LucideIcon::Server
                } else {
                    LucideIcon::Terminal
                })
                .xsmall()
                .text_color(if is_active {
                    cx.theme().tab_active_foreground
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
                        cx.theme().tab_active_foreground
                    } else {
                        cx.theme().muted_foreground
                    })
                    .child(tab.title.clone()),
            )
            .child(
                Button::new(SharedString::from(format!("close-tab-{tid_close}")))
                    .tab_stop(false)
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
    }

    #[inline(never)]
    fn render_titlebar_element(&self, cx: &mut Context<Self>) -> AnyElement {
        let ws_ent = cx.entity().clone();
        let ws_sidebar = ws_ent.clone();
        let ws_new_local = ws_ent.clone();
        let ws_menu_local = ws_ent.clone();
        let ws_menu_ssh = ws_ent.clone();
        let ws_split_h = ws_ent.clone();
        let ws_split_v = ws_ent.clone();
        let ws_sftp = ws_ent.clone();
        let ws_palette = ws_ent.clone();
        let ws_theme = ws_ent.clone();
        let ws_settings = ws_ent.clone();

        let show_sidebar = self.settings.show_left_sidebar;
        let show_sftp = self.settings.show_sftp_panel;
        let dark_mode = self.settings.dark_mode;
        let theme_pack = ThemeRegistry::global().current();
        let active_theme_name = theme_pack
            .resolve(self.settings.active_theme_id(), dark_mode)
            .name;
        let alt_theme_name = if dark_mode {
            theme_pack.resolve(&self.settings.light_theme, false).name
        } else {
            theme_pack.resolve(&self.settings.dark_theme, true).name
        };
        let has_active_tab = self.active_tab_id.is_some();
        let sftp_busy = self.sftp_panel.read(cx).has_active_transfers();
        let shortcut_tooltip = |label: &str, action| tooltip(label, self.settings.shortcuts.get(action).as_ref());
        let app_paths = self.paths.clone();
        let app_paths_palette = self.paths.clone();
        let paths_for_logs = self.paths.clone();

        let tab_elements: Vec<AnyElement> = self
            .tabs
            .iter()
            .map(|tab| self.render_single_tab_pill(tab, ws_ent.clone(), cx))
            .collect();

        TitleBar::new()
            .h(px(38.0))
            .bg(cx.theme().title_bar)
            .child(
                h_flex()
                    .h_full()
                    .min_w_0()
                    .flex_1()
                    .gap_1p5()
                    .items_center()
                    .child(
                        div()
                            .on_mouse_down(MouseButton::Left, |_, window, _| {
                                window.prevent_default();
                            })
                            .child(
                                Button::new("toggle-left-sidebar")
                                    .tab_stop(false)
                                    .ghost()
                                    .xsmall()
                                    .compact()
                                    .selected(show_sidebar)
                                    .icon(if show_sidebar {
                                        LucideIcon::PanelLeftClose
                                    } else {
                                        LucideIcon::PanelLeftOpen
                                    })
                                    .tooltip(shortcut_tooltip(
                                        if show_sidebar { "隐藏左侧边栏" } else { "展开左侧边栏" },
                                        ShortcutAction::ToggleLeftSidebar,
                                    ))
                                    .on_click(move |_, window, cx| {
                                        ws_sidebar.update(cx, |ws, cx| ws.toggle_left_sidebar(window, cx));
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .w(px(1.0))
                            .h(px(14.0))
                            .bg(cx.theme().border),
                    )
                    .children(self.tabs.is_empty().then(|| {
                        h_flex()
                            .px_1p5()
                            .gap_1p5()
                            .items_center()
                            .child(crate::ui::brand::brand_mark_icon_element(16.0))
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(cx.theme().foreground)
                                    .child("Mica Term"),
                            )
                    }))
                    .children((!tab_elements.is_empty()).then(|| {
                        h_flex()
                            .min_w_0()
                            .overflow_x_scrollbar()
                            .gap_1()
                            .items_center()
                            .children(tab_elements)
                    }))
                    .child(
                        h_flex()
                            .p(px(1.5))
                            .gap_0p5()
                            .items_center()
                            .rounded(px(6.5))
                            .bg(cx.theme().secondary.opacity(0.65))
                            .on_mouse_down(MouseButton::Left, |_, window, _| {
                                window.prevent_default();
                            })
                            .child(
                                Button::new("new-local-tab-btn")
                                    .tab_stop(false)
                                    .ghost()
                                    .xsmall()
                                    .compact()
                                    .icon(LucideIcon::Plus)
                                    .tooltip(shortcut_tooltip(ShortcutAction::NewLocalTab.label(), ShortcutAction::NewLocalTab))
                                    .on_click(move |_, window, cx| {
                                        ws_new_local.update(cx, |ws, cx| {
                                            ws.open_local_shell_tab(window, cx)
                                        });
                                    }),
                            )
                            .child(
                                Button::new("new-session-more-menu")
                                    .tab_stop(false)
                                    .ghost()
                                    .xsmall()
                                    .compact()
                                    .icon(LucideIcon::ChevronDown)
                                    .tooltip("新建 SSH 或本地会话")
                                    .dropdown_menu(move |menu, _, _| {
                                        let wl = ws_menu_local.clone();
                                        let ws = ws_menu_ssh.clone();
                                        menu.item(
                                            PopupMenuItem::new("新建本地终端")
                                                .icon(LucideIcon::Terminal)
                                                .on_click(move |_, window, cx| {
                                                    wl.update(cx, |w, cx| {
                                                        w.open_local_shell_tab(window, cx)
                                                    });
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
                            ),
                    )
                    .child(div().flex_1().min_w(px(24.0)).h_full()),
            )
            .child(
                h_flex()
                    .h_full()
                    .pr_2()
                    .gap_1()
                    .items_center()
                    .on_mouse_down(MouseButton::Left, |_, window, _| {
                        window.prevent_default();
                    })
                    .children(has_active_tab.then(|| {
                        h_flex()
                            .gap_0p5()
                            .pr_1()
                            .items_center()
                            .child(
                                Button::new("title-split-h")
                                    .tab_stop(false)
                                    .ghost()
                                    .xsmall()
                                    .compact()
                                    .icon(LucideIcon::Columns2)
                                    .tooltip(shortcut_tooltip(ShortcutAction::SplitHorizontal.label(), ShortcutAction::SplitHorizontal))
                                    .on_click(move |_, window, cx| {
                                        ws_split_h.update(cx, |ws, cx| {
                                            ws.split_active_tab(
                                                TabSplitAxis::Horizontal,
                                                window,
                                                cx,
                                            )
                                        });
                                    }),
                            )
                            .child(
                                Button::new("title-split-v")
                                    .tab_stop(false)
                                    .ghost()
                                    .xsmall()
                                    .compact()
                                    .icon(LucideIcon::Rows2)
                                    .tooltip(shortcut_tooltip(ShortcutAction::SplitVertical.label(), ShortcutAction::SplitVertical))
                                    .on_click(move |_, window, cx| {
                                        ws_split_v.update(cx, |ws, cx| {
                                            ws.split_active_tab(TabSplitAxis::Vertical, window, cx)
                                        });
                                    }),
                            )
                    }))
                    .child(
                        Button::new("toggle-sftp-panel")
                            .tab_stop(false)
                            .ghost()
                            .xsmall()
                            .compact()
                            .selected(show_sftp)
                            .loading(sftp_busy)
                            .icon(if show_sftp {
                                LucideIcon::PanelRightClose
                            } else {
                                LucideIcon::PanelRightOpen
                            })
                            .label("SFTP")
                            .tooltip(shortcut_tooltip("切换右侧 SFTP 检查器", ShortcutAction::ToggleSftpPanel))
                            .on_click(move |_, window, cx| {
                                ws_sftp.update(cx, |ws, cx| ws.toggle_sftp_panel(window, cx));
                            }),
                    )
                    .child(
                        Button::new("open-theme-gallery-btn")
                            .tab_stop(false)
                            .ghost()
                            .xsmall()
                            .compact()
                            .icon(LucideIcon::Palette)
                            .tooltip(format!(
                                "当前主题：{active_theme_name}（点击切换 {} 款精选配色）",
                                theme_pack.themes.len()
                            ))
                            .on_click(move |_, window, cx| {
                                open_settings_modal_at(
                                    ws_palette.clone(),
                                    app_paths_palette.clone(),
                                    SettingsTab::Themes,
                                    window,
                                    cx,
                                );
                            }),
                    )
                    .child(
                        Button::new("quick-theme-toggle-btn")
                            .tab_stop(false)
                            .ghost()
                            .xsmall()
                            .compact()
                            .icon(if dark_mode {
                                LucideIcon::Sun
                            } else {
                                LucideIcon::Moon
                            })
                            .tooltip(if dark_mode {
                                format!("切换为浅色模式 ({alt_theme_name})")
                            } else {
                                format!("切换为深色模式 ({alt_theme_name})")
                            })
                            .on_click(move |_, window, cx| {
                                ws_theme.update(cx, |ws, cx| {
                                    ws.set_dark_mode(!dark_mode, window, cx)
                                });
                            }),
                    )
                    .child(
                        Button::new("open-logs-btn")
                            .tab_stop(false)
                            .ghost()
                            .xsmall()
                            .compact()
                            .icon(LucideIcon::Terminal)
                            .label("日志")
                            .tooltip("查看运行与崩溃诊断日志")
                            .on_click(move |_, window, cx| {
                                crate::ui::modals::open_logs_modal(
                                    paths_for_logs.clone(),
                                    window,
                                    cx,
                                );
                            }),
                    )
                    .child(
                        Button::new("open-settings-btn")
                            .tab_stop(false)
                            .ghost()
                            .xsmall()
                            .compact()
                            .icon(LucideIcon::SlidersHorizontal)
                            .tooltip(shortcut_tooltip("偏好设置", ShortcutAction::OpenSettings))
                            .on_click(move |_, window, cx| {
                                open_settings_modal(
                                    ws_settings.clone(),
                                    app_paths.clone(),
                                    window,
                                    cx,
                                );
                            }),
                    ),
            )
            .into_any_element()
    }

    #[inline(never)]
    fn render_center_workspace(&self, cx: &mut Context<Self>) -> AnyElement {
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

        tab.root.render_element()
    }
}

impl Render for MicaWorkspace {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let title_bar = self.render_titlebar_element(cx);
        let center_content = self.render_center_workspace(cx);

        let show_left = self.settings.show_left_sidebar;
        let show_right = self.settings.show_sftp_panel;

        let body = if !show_left && !show_right {
            div()
                .flex_1()
                .min_h_0()
                .child(center_content)
                .into_any_element()
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
                        .size(px(260.0))
                        .size_range(px(200.0)..px(420.0))
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
            .id("mica-workspace-root")
            .on_modifiers_changed(cx.listener(
                |this, ev: &gpui_kit::ModifiersChangedEvent, _, cx| {
                    if let Some(tab) = this.active_tab() {
                        for pane in tab.panes() {
                            pane.update(cx, |p, cx| p.sync_modifiers(ev.modifiers, cx));
                        }
                    }
                },
            ))
            .track_focus(&self.focus_handle)
            .capture_key_down(cx.listener(Self::handle_capture_key_down))
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(title_bar)
            .child(body)
    }
}

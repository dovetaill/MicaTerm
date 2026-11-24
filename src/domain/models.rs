//! Core domain models for Mica Term GPUI: Hosts, Keychain, Snippets, and Settings.

use std::collections::HashSet;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::credentials::{CredentialStore, host_secret_key, keychain_secret_key};
use crate::domain::shortcuts::{DoubleClickAction, ShortcutConfig};

pub const BUNDLED_SARASA_FONT_FAMILY: &str = "Sarasa Term SC Nerd";
pub const BUNDLED_MAPLE_FONT_FAMILY: &str = "JetBrains Maple Mono";

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppSettings {
    pub dark_mode: bool,
    pub font_family: String,
    pub font_size: f32,
    pub scrollback_lines: usize,
    pub follow_sftp_cwd: bool,
    pub show_left_sidebar: bool,
    pub show_sftp_panel: bool,
    #[serde(default = "default_true")]
    pub command_blocks: bool,
    #[serde(default = "default_true")]
    pub semantic_highlighting: bool,
    #[serde(default = "default_true")]
    pub code_highlighting: bool,
    #[serde(default = "default_true")]
    pub overview_markers: bool,
    #[serde(default)]
    pub double_click_action: DoubleClickAction,
    #[serde(default)]
    pub shortcuts: ShortcutConfig,
    #[serde(default)]
    pub font_migrated_sarasa: bool,
    #[serde(default)]
    pub typography_v2_migrated: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            dark_mode: true,
            font_family: default_terminal_font_family(),
            font_size: default_terminal_font_size(),
            scrollback_lines: 5_000,
            follow_sftp_cwd: true,
            show_left_sidebar: true,
            show_sftp_panel: false,
            command_blocks: true,
            semantic_highlighting: true,
            code_highlighting: true,
            overview_markers: true,
            double_click_action: DoubleClickAction::SelectWord,
            shortcuts: ShortcutConfig::default(),
            font_migrated_sarasa: true,
            typography_v2_migrated: true,
        }
    }
}

pub fn default_terminal_font_family() -> String {
    BUNDLED_SARASA_FONT_FAMILY.to_string()
}

pub fn default_terminal_font_size() -> f32 {
    if cfg!(target_os = "windows") {
        16.0
    } else {
        14.0
    }
}

pub fn system_monospace_font_family() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        "Menlo"
    }
    #[cfg(target_os = "windows")]
    {
        "Cascadia Mono"
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        "DejaVu Sans Mono"
    }
}

pub fn is_legacy_system_default_font(family: &str) -> bool {
    let trimmed = family.trim();
    trimmed.is_empty()
        || matches!(
            trimmed,
            "DejaVu Sans Mono" | "Cascadia Mono" | "Menlo"
        )
}

/// Expands leading `~` or `~/...` (`~\...` on Windows) to the current user's home directory.
pub fn expand_tilde_path(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed == "~" {
        if let Some(home) = resolve_user_home_dir() {
            return home.to_string_lossy().into_owned();
        }
    } else if let Some(rest) = trimmed
        .strip_prefix("~/")
        .or_else(|| trimmed.strip_prefix("~\\"))
        && let Some(home) = resolve_user_home_dir()
    {
        return home.join(rest).to_string_lossy().into_owned();
    }
    trimmed.to_string()
}

fn resolve_user_home_dir() -> Option<PathBuf> {
    directories::UserDirs::new()
        .map(|u| u.home_dir().to_path_buf())
        .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
        .or_else(|| std::env::var_os("USERPROFILE").map(PathBuf::from))
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum SshAuthConfig {
    #[default]
    Password,
    PrivateKeyPath {
        path: String,
    },
    PrivateKeyInline,
    Keychain {
        keychain_id: String,
    },
}

impl SshAuthConfig {
    pub fn summary_label(&self) -> &'static str {
        match self {
            Self::Password => "密码认证",
            Self::PrivateKeyPath { .. } => "私钥文件",
            Self::PrivateKeyInline => "内联私钥",
            Self::Keychain { .. } => "密钥库凭据",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum SshProxyConfig {
    #[default]
    None,
    Socks5 {
        host: String,
        port: u16,
        username: Option<String>,
    },
    Http {
        host: String,
        port: u16,
        username: Option<String>,
    },
    JumpHost {
        host_id: String,
    },
}

impl SshProxyConfig {
    pub fn summary_label(&self) -> &'static str {
        match self {
            Self::None => "直连",
            Self::Socks5 { .. } => "SOCKS5 代理",
            Self::Http { .. } => "HTTP 代理",
            Self::JumpHost { .. } => "SSH 跳板机",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SshHostConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub auth: SshAuthConfig,
    pub proxy: SshProxyConfig,
    pub default_remote_dir: String,
    pub environment_tag: String,
    pub remark: String,
}

impl Default for SshHostConfig {
    fn default() -> Self {
        Self {
            host: String::new(),
            port: 22,
            user: "root".to_string(),
            auth: SshAuthConfig::Password,
            proxy: SshProxyConfig::None,
            default_remote_dir: String::new(),
            environment_tag: String::new(),
            remark: String::new(),
        }
    }
}

impl SshHostConfig {
    pub fn endpoint_label(&self) -> String {
        if self.port == 22 {
            format!("{}@{}", self.user, self.host)
        } else {
            format!("{}@{}:{}", self.user, self.host, self.port)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HostFolder {
    pub expanded: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum HostNodeKind {
    Folder(HostFolder),
    Host(Box<SshHostConfig>),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HostNode {
    pub id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub kind: HostNodeKind,
}

impl HostNode {
    pub fn new_folder(name: impl Into<String>, parent_id: Option<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            parent_id,
            name: name.into(),
            kind: HostNodeKind::Folder(HostFolder { expanded: true }),
        }
    }

    pub fn new_host(
        name: impl Into<String>,
        parent_id: Option<String>,
        config: SshHostConfig,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            parent_id,
            name: name.into(),
            kind: HostNodeKind::Host(Box::new(config)),
        }
    }

    pub fn is_folder(&self) -> bool {
        matches!(self.kind, HostNodeKind::Folder(_))
    }

    pub fn as_host(&self) -> Option<&SshHostConfig> {
        match &self.kind {
            HostNodeKind::Host(cfg) => Some(cfg.as_ref()),
            HostNodeKind::Folder(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlatHostRow {
    pub id: String,
    pub name: String,
    pub subtitle: String,
    pub env_badge: String,
    pub depth: usize,
    pub is_folder: bool,
    pub expanded: bool,
    pub child_count: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct HostTree {
    pub nodes: Vec<HostNode>,
}

impl HostTree {
    pub fn get(&self, id: &str) -> Option<&HostNode> {
        self.nodes.iter().find(|n| n.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut HostNode> {
        self.nodes.iter_mut().find(|n| n.id == id)
    }

    pub fn folders(&self) -> Vec<(String, String)> {
        self.nodes
            .iter()
            .filter(|n| n.is_folder())
            .map(|n| (n.id.clone(), self.folder_path_label(&n.id)))
            .collect()
    }

    pub fn hosts(&self) -> Vec<&HostNode> {
        self.nodes.iter().filter(|n| !n.is_folder()).collect()
    }

    pub fn descendant_count(&self, id: &str) -> usize {
        let mut set = HashSet::new();
        set.insert(id.to_string());
        loop {
            let mut grew = false;
            for node in &self.nodes {
                if let Some(pid) = &node.parent_id
                    && set.contains(pid)
                    && !set.contains(&node.id)
                {
                    set.insert(node.id.clone());
                    grew = true;
                }
            }
            if !grew {
                break;
            }
        }
        set.len().saturating_sub(1)
    }

    pub fn folder_path_label(&self, id: &str) -> String {
        let mut parts = Vec::new();
        let mut current = self.get(id);
        let mut guard = 0;
        while let Some(node) = current {
            parts.push(node.name.clone());
            guard += 1;
            if guard > 16 {
                break;
            }
            current = node.parent_id.as_deref().and_then(|pid| self.get(pid));
        }
        parts.reverse();
        parts.join(" / ")
    }

    pub fn upsert(&mut self, node: HostNode) {
        if let Some(existing) = self.get_mut(&node.id) {
            *existing = node;
        } else {
            self.nodes.push(node);
        }
    }

    pub fn toggle_folder_expanded(&mut self, id: &str) -> bool {
        if let Some(node) = self.get_mut(id)
            && let HostNodeKind::Folder(folder) = &mut node.kind
        {
            folder.expanded = !folder.expanded;
            return true;
        }
        false
    }

    /// Removes `id` and all of its descendants, returning all removed node IDs.
    pub fn remove_cascade(&mut self, id: &str) -> Vec<String> {
        let mut to_remove = HashSet::new();
        to_remove.insert(id.to_string());
        loop {
            let mut grew = false;
            for node in &self.nodes {
                if let Some(pid) = &node.parent_id
                    && to_remove.contains(pid)
                    && !to_remove.contains(&node.id)
                {
                    to_remove.insert(node.id.clone());
                    grew = true;
                }
            }
            if !grew {
                break;
            }
        }
        let removed: Vec<String> = self
            .nodes
            .iter()
            .filter(|n| to_remove.contains(&n.id))
            .map(|n| n.id.clone())
            .collect();
        self.nodes.retain(|n| !to_remove.contains(&n.id));
        removed
    }

    pub fn flatten_visible(&self, query: &str) -> Vec<FlatHostRow> {
        let trimmed = query.trim().to_lowercase();
        if !trimmed.is_empty() {
            return self
                .nodes
                .iter()
                .filter_map(|node| {
                    let cfg = node.as_host()?;
                    let hay = format!(
                        "{} {} {} {} {}",
                        node.name, cfg.host, cfg.user, cfg.environment_tag, cfg.remark
                    )
                    .to_lowercase();
                    if hay.contains(&trimmed) {
                        Some(FlatHostRow {
                            id: node.id.clone(),
                            name: node.name.clone(),
                            subtitle: cfg.endpoint_label(),
                            env_badge: cfg.environment_tag.clone(),
                            depth: 0,
                            is_folder: false,
                            expanded: false,
                            child_count: 0,
                        })
                    } else {
                        None
                    }
                })
                .collect();
        }

        let mut out = Vec::new();
        self.collect_children(None, 0, &mut out);
        out
    }

    fn collect_children(&self, parent_id: Option<&str>, depth: usize, out: &mut Vec<FlatHostRow>) {
        let mut children: Vec<&HostNode> = self
            .nodes
            .iter()
            .filter(|n| n.parent_id.as_deref() == parent_id)
            .collect();
        // Folders first, then alphabetical by name
        children.sort_by(|a, b| {
            b.is_folder()
                .cmp(&a.is_folder())
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });

        for node in children {
            match &node.kind {
                HostNodeKind::Folder(folder) => {
                    let child_count = self
                        .nodes
                        .iter()
                        .filter(|n| n.parent_id.as_deref() == Some(node.id.as_str()))
                        .count();
                    out.push(FlatHostRow {
                        id: node.id.clone(),
                        name: node.name.clone(),
                        subtitle: format!("{child_count} 项"),
                        env_badge: String::new(),
                        depth,
                        is_folder: true,
                        expanded: folder.expanded,
                        child_count,
                    });
                    if folder.expanded {
                        self.collect_children(Some(&node.id), depth + 1, out);
                    }
                }
                HostNodeKind::Host(cfg) => {
                    out.push(FlatHostRow {
                        id: node.id.clone(),
                        name: node.name.clone(),
                        subtitle: cfg.endpoint_label(),
                        env_badge: cfg.environment_tag.clone(),
                        depth,
                        is_folder: false,
                        expanded: false,
                        child_count: 0,
                    });
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum KeychainKind {
    PasswordIdentity,
    SshKey,
}

impl KeychainKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::PasswordIdentity => "账号密码",
            Self::SshKey => "SSH 私钥",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KeychainEntry {
    pub id: String,
    pub name: String,
    pub kind: KeychainKind,
    pub username: String,
    pub remark: String,
}

impl KeychainEntry {
    pub fn new(
        name: impl Into<String>,
        kind: KeychainKind,
        username: impl Into<String>,
        remark: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            kind,
            username: username.into(),
            remark: remark.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SnippetItem {
    pub id: String,
    pub title: String,
    pub group: String,
    pub command: String,
    pub auto_enter: bool,
}

impl SnippetItem {
    pub fn new(
        title: impl Into<String>,
        group: impl Into<String>,
        command: impl Into<String>,
        auto_enter: bool,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            title: title.into(),
            group: group.into(),
            command: command.into(),
            auto_enter,
        }
    }
}

/// Fully-resolved runtime SSH configuration including secrets and recursive jump-host chain.
#[derive(Debug, Clone)]
pub enum ResolvedProxyHop {
    Socks5 {
        host: String,
        port: u16,
        username: Option<String>,
        password: Option<String>,
    },
    Http {
        host: String,
        port: u16,
        username: Option<String>,
        password: Option<String>,
    },
    JumpHost(Box<ResolvedSshConfig>),
}

#[derive(Debug, Clone)]
pub enum ResolvedAuthMaterial {
    Password(String),
    PrivateKeyPath {
        path: String,
        passphrase: Option<String>,
    },
    PrivateKeyInline {
        private_key_pem: String,
        passphrase: Option<String>,
    },
}

#[derive(Debug, Clone)]
pub struct ResolvedSshConfig {
    pub host_id: Option<String>,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub user: String,
    pub auth: ResolvedAuthMaterial,
    pub proxy_hops: Vec<ResolvedProxyHop>,
    pub default_remote_dir: String,
}

impl ResolvedSshConfig {
    pub fn resolve_from_tree(
        host_id: &str,
        tree: &HostTree,
        keychain: &[KeychainEntry],
        credentials: &dyn CredentialStore,
    ) -> Result<Self> {
        let mut visited = HashSet::new();
        Self::resolve_inner(host_id, tree, keychain, credentials, &mut visited, 0)
    }

    fn resolve_inner(
        host_id: &str,
        tree: &HostTree,
        keychain: &[KeychainEntry],
        credentials: &dyn CredentialStore,
        visited: &mut HashSet<String>,
        depth: usize,
    ) -> Result<Self> {
        if depth > 6 {
            bail!("SSH 跳板机链路超过最大深度 (6)");
        }
        if !visited.insert(host_id.to_string()) {
            bail!("SSH 跳板机配置存在循环引用");
        }

        let node = tree
            .get(host_id)
            .with_context(|| format!("未找到主机节点 `{host_id}`"))?;
        let cfg = node
            .as_host()
            .with_context(|| format!("节点 `{}` 不是 SSH 主机", node.name))?;

        let host_secrets = credentials
            .get_payload(&host_secret_key(host_id))?
            .unwrap_or_default();

        let mut effective_user = cfg.user.trim().to_string();
        let auth = match &cfg.auth {
            SshAuthConfig::Password => {
                let pwd = host_secrets
                    .password
                    .clone()
                    .filter(|s| !s.is_empty())
                    .with_context(|| format!("主机 `{}` 缺少保存的密码", node.name))?;
                ResolvedAuthMaterial::Password(pwd)
            }
            SshAuthConfig::PrivateKeyPath { path } => {
                let expanded = expand_tilde_path(path);
                if expanded.is_empty() {
                    bail!("主机 `{}` 未配置私钥文件路径", node.name);
                }
                ResolvedAuthMaterial::PrivateKeyPath {
                    path: expanded,
                    passphrase: host_secrets.passphrase.clone().filter(|s| !s.is_empty()),
                }
            }
            SshAuthConfig::PrivateKeyInline => {
                let pem = host_secrets
                    .private_key_pem
                    .clone()
                    .filter(|s| !s.trim().is_empty())
                    .with_context(|| format!("主机 `{}` 缺少内联私钥内容", node.name))?;
                ResolvedAuthMaterial::PrivateKeyInline {
                    private_key_pem: pem,
                    passphrase: host_secrets.passphrase.clone().filter(|s| !s.is_empty()),
                }
            }
            SshAuthConfig::Keychain { keychain_id } => {
                let entry = keychain
                    .iter()
                    .find(|k| &k.id == keychain_id)
                    .with_context(|| format!("主机 `{}` 引用的密钥库凭据不存在", node.name))?;
                if effective_user.is_empty() && !entry.username.trim().is_empty() {
                    effective_user = entry.username.trim().to_string();
                }
                let kc_secrets = credentials
                    .get_payload(&keychain_secret_key(&entry.id))?
                    .unwrap_or_default();
                match entry.kind {
                    KeychainKind::PasswordIdentity => {
                        let pwd = kc_secrets
                            .password
                            .clone()
                            .filter(|s| !s.is_empty())
                            .with_context(|| format!("密钥库凭据 `{}` 缺少密码", entry.name))?;
                        ResolvedAuthMaterial::Password(pwd)
                    }
                    KeychainKind::SshKey => {
                        let pem = kc_secrets
                            .private_key_pem
                            .clone()
                            .filter(|s| !s.trim().is_empty())
                            .with_context(|| format!("密钥库 `{}` 缺少私钥内容", entry.name))?;
                        ResolvedAuthMaterial::PrivateKeyInline {
                            private_key_pem: pem,
                            passphrase: kc_secrets.passphrase.clone().filter(|s| !s.is_empty()),
                        }
                    }
                }
            }
        };

        if effective_user.is_empty() {
            effective_user = "root".to_string();
        }

        let mut proxy_hops = Vec::new();
        match &cfg.proxy {
            SshProxyConfig::None => {}
            SshProxyConfig::Socks5 {
                host,
                port,
                username,
            } => {
                if host.trim().is_empty() {
                    bail!("主机 `{}` 的 SOCKS5 代理地址为空", node.name);
                }
                proxy_hops.push(ResolvedProxyHop::Socks5 {
                    host: host.trim().to_string(),
                    port: *port,
                    username: username.clone().filter(|s| !s.trim().is_empty()),
                    password: host_secrets.proxy_password.clone().filter(|s| !s.is_empty()),
                });
            }
            SshProxyConfig::Http {
                host,
                port,
                username,
            } => {
                if host.trim().is_empty() {
                    bail!("主机 `{}` 的 HTTP 代理地址为空", node.name);
                }
                proxy_hops.push(ResolvedProxyHop::Http {
                    host: host.trim().to_string(),
                    port: *port,
                    username: username.clone().filter(|s| !s.trim().is_empty()),
                    password: host_secrets.proxy_password.clone().filter(|s| !s.is_empty()),
                });
            }
            SshProxyConfig::JumpHost {
                host_id: upstream_id,
            } => {
                let mut upstream = Self::resolve_inner(
                    upstream_id,
                    tree,
                    keychain,
                    credentials,
                    visited,
                    depth + 1,
                )?;
                let upstream_hops = std::mem::take(&mut upstream.proxy_hops);
                proxy_hops.extend(upstream_hops);
                proxy_hops.push(ResolvedProxyHop::JumpHost(Box::new(upstream)));
            }
        }

        visited.remove(host_id);

        Ok(Self {
            host_id: Some(host_id.to_string()),
            name: node.name.clone(),
            host: cfg.host.trim().to_string(),
            port: cfg.port,
            user: effective_user,
            auth,
            proxy_hops,
            default_remote_dir: cfg.default_remote_dir.trim().to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::credentials::{MemoryCredentialStore, SecretPayload};

    #[test]
    fn host_tree_flattens_and_cascades_removal() {
        let mut tree = HostTree::default();
        let folder = HostNode::new_folder("Production", None);
        let folder_id = folder.id.clone();
        tree.upsert(folder);

        let cfg = SshHostConfig {
            host: "10.0.0.12".to_string(),
            user: "ubuntu".to_string(),
            environment_tag: "PROD".to_string(),
            ..Default::default()
        };
        let host = HostNode::new_host("Web-01", Some(folder_id.clone()), cfg);
        let host_id = host.id.clone();
        tree.upsert(host);

        assert_eq!(tree.descendant_count(&folder_id), 1);

        let rows = tree.flatten_visible("");
        assert_eq!(rows.len(), 2);
        assert!(rows[0].is_folder);
        assert_eq!(rows[1].id, host_id);
        assert_eq!(rows[1].depth, 1);

        let filtered = tree.flatten_visible("10.0.0.12");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, host_id);

        let removed = tree.remove_cascade(&folder_id);
        assert_eq!(removed.len(), 2);
        assert!(tree.nodes.is_empty());
    }

    #[test]
    fn resolves_jump_host_and_keychain_credentials() {
        let mut tree = HostTree::default();
        let store = MemoryCredentialStore::default();

        let kc = KeychainEntry::new("Ops Key", KeychainKind::PasswordIdentity, "ops", "");
        store
            .put_payload(
                &keychain_secret_key(&kc.id),
                &SecretPayload {
                    password: Some("top-secret".to_string()),
                    private_key_pem: None,
                    passphrase: None,
                    proxy_password: None,
                },
            )
            .unwrap();

        let bastion_cfg = SshHostConfig {
            host: "bastion.example.com".to_string(),
            user: String::new(),
            auth: SshAuthConfig::Keychain {
                keychain_id: kc.id.clone(),
            },
            ..Default::default()
        };
        let bastion = HostNode::new_host("Bastion", None, bastion_cfg);
        let bastion_id = bastion.id.clone();
        tree.upsert(bastion);

        let app_cfg = SshHostConfig {
            host: "10.10.1.5".to_string(),
            user: "deploy".to_string(),
            auth: SshAuthConfig::Password,
            proxy: SshProxyConfig::JumpHost {
                host_id: bastion_id,
            },
            ..Default::default()
        };
        let app_node = HostNode::new_host("App-Server", None, app_cfg);
        let app_id = app_node.id.clone();
        store
            .put_payload(
                &host_secret_key(&app_id),
                &SecretPayload {
                    password: Some("app-pass".to_string()),
                    private_key_pem: None,
                    passphrase: None,
                    proxy_password: None,
                },
            )
            .unwrap();
        tree.upsert(app_node);

        let resolved =
            ResolvedSshConfig::resolve_from_tree(&app_id, &tree, &[kc], &store).unwrap();
        assert_eq!(resolved.host, "10.10.1.5");
        assert_eq!(resolved.proxy_hops.len(), 1);
        match &resolved.proxy_hops[0] {
            ResolvedProxyHop::JumpHost(jump) => {
                assert_eq!(jump.host, "bastion.example.com");
                assert_eq!(jump.user, "ops");
            }
            _ => panic!("expected jump host hop"),
        }
    }

    #[test]
    fn expands_tilde_in_private_key_path() {
        let expanded = expand_tilde_path("~/.ssh/id_ed25519");
        assert!(!expanded.starts_with("~/"));
        assert!(expanded.ends_with(".ssh/id_ed25519"));
    }
}

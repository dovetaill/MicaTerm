//! Asset sidebar identifiers, tree projection helpers, and inline rename utilities.

use std::collections::{BTreeSet, HashMap};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetViewMode {
    Tree,
    Flat,
}

impl AssetViewMode {
    pub fn id(self) -> &'static str {
        match self {
            Self::Tree => "tree",
            Self::Flat => "flat",
        }
    }

    pub fn toggle(self) -> Self {
        match self {
            Self::Tree => Self::Flat,
            Self::Flat => Self::Tree,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetDomain {
    Console,
    Snippets,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetCreateAction {
    NewFolder,
    NewSshConnection,
}

impl AssetCreateAction {
    pub fn id(self) -> &'static str {
        match self {
            Self::NewFolder => "new-folder",
            Self::NewSshConnection => "new-ssh-connection",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsoleAssetKind {
    SshConnection,
    Folder,
    SnippetPackage,
    Snippet,
}

impl ConsoleAssetKind {
    pub fn id(self) -> &'static str {
        match self {
            Self::SshConnection => "ssh",
            Self::Folder => "folder",
            Self::SnippetPackage => "snippet-package",
            Self::Snippet => "snippet",
        }
    }

    pub fn from_id(value: &str) -> Option<Self> {
        match value {
            "ssh" => Some(Self::SshConnection),
            "folder" => Some(Self::Folder),
            "snippet-package" => Some(Self::SnippetPackage),
            "snippet" => Some(Self::Snippet),
            _ => None,
        }
    }

    pub fn from_create_action_id(value: &str) -> Option<Self> {
        match value {
            "new-folder" => Some(Self::Folder),
            "new-ssh-connection" => Some(Self::SshConnection),
            _ => None,
        }
    }

    pub fn placeholder_label(self) -> &'static str {
        match self {
            Self::Folder => "New Folder",
            Self::SshConnection => "New SSH Connection",
            Self::SnippetPackage => "New Package",
            Self::Snippet => "New Snippet",
        }
    }

    pub fn default_name_prefix(self) -> &'static str {
        match self {
            Self::Folder => "Folder",
            Self::SshConnection => "SSH Connection",
            Self::SnippetPackage => "Package",
            Self::Snippet => "Snippet",
        }
    }

    pub fn domain(self) -> AssetDomain {
        match self {
            Self::Folder | Self::SshConnection => AssetDomain::Console,
            Self::SnippetPackage | Self::Snippet => AssetDomain::Snippets,
        }
    }

    fn can_accept_children(self) -> bool {
        matches!(self, Self::Folder | Self::SnippetPackage)
    }

    fn allows_child(self, child: Self) -> bool {
        match self {
            Self::Folder => matches!(child, Self::Folder | Self::SshConnection),
            Self::SnippetPackage => matches!(child, Self::Snippet),
            Self::SshConnection | Self::Snippet => false,
        }
    }

    fn should_appear_in_flat_view(self) -> bool {
        matches!(self, Self::SshConnection | Self::Snippet)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MockConsoleAssetItem {
    pub id: String,
    pub kind: ConsoleAssetKind,
    pub label: String,
}

impl MockConsoleAssetItem {
    pub fn new(id: impl Into<String>, kind: ConsoleAssetKind, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            kind,
            label: label.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetDisclosureState {
    None,
    Collapsed,
    Expanded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetNameValidation {
    Valid,
    Empty,
    Duplicate,
    Invalid,
    Unchanged,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AssetSocks5ProxySpec {
    pub host: String,
    pub port: String,
    pub username: String,
    pub password_credential_ref: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum AssetSshProxySpec {
    #[default]
    None,
    Socks5(AssetSocks5ProxySpec),
    Http(AssetSocks5ProxySpec),
    SshAsset {
        asset_id: String,
    },
}

pub const SSH_AUTH_SOURCE_MANUAL: &str = "manual";
pub const SSH_AUTH_SOURCE_KEYCHAIN_IDENTITY: &str = "keychain-identity";

pub fn normalized_ssh_auth_source(raw: &str) -> &str {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        SSH_AUTH_SOURCE_MANUAL
    } else {
        trimmed
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetSshConnectionSpec {
    pub host: String,
    pub user: String,
    pub port: String,
    pub auth_method: String,
    pub auth_source: String,
    pub keychain_identity_id: Option<String>,
    pub private_key_source: String,
    pub private_key_path: String,
    pub environment: String,
    pub proxy: AssetSshProxySpec,
    // Transitional compatibility for pre-proxy-chain UI code.
    pub proxy_method: String,
    pub remark: String,
    pub credential_ref: Option<String>,
}

impl Default for AssetSshConnectionSpec {
    fn default() -> Self {
        Self {
            host: String::new(),
            user: String::new(),
            port: String::new(),
            auth_method: String::new(),
            auth_source: SSH_AUTH_SOURCE_MANUAL.into(),
            keychain_identity_id: None,
            private_key_source: String::new(),
            private_key_path: String::new(),
            environment: String::new(),
            proxy: AssetSshProxySpec::None,
            proxy_method: String::new(),
            remark: String::new(),
            credential_ref: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AssetSnippetSpec {
    pub script: String,
    pub package_id: Option<String>,
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssetNodePayload {
    Folder,
    SshConnection(AssetSshConnectionSpec),
    SnippetPackage,
    Snippet(AssetSnippetSpec),
}

impl AssetNodePayload {
    fn for_kind(kind: ConsoleAssetKind) -> Self {
        match kind {
            ConsoleAssetKind::Folder => Self::Folder,
            ConsoleAssetKind::SshConnection => {
                Self::SshConnection(AssetSshConnectionSpec::default())
            }
            ConsoleAssetKind::SnippetPackage => Self::SnippetPackage,
            ConsoleAssetKind::Snippet => Self::Snippet(AssetSnippetSpec::default()),
        }
    }

    fn matches_kind(&self, kind: ConsoleAssetKind) -> bool {
        matches!(
            (kind, self),
            (ConsoleAssetKind::Folder, Self::Folder)
                | (ConsoleAssetKind::SshConnection, Self::SshConnection(_))
                | (ConsoleAssetKind::SnippetPackage, Self::SnippetPackage)
                | (ConsoleAssetKind::Snippet, Self::Snippet(_))
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetNode {
    pub id: String,
    pub kind: ConsoleAssetKind,
    pub title: String,
    pub parent_id: Option<String>,
    pub children: Vec<String>,
    pub expanded: bool,
    pub payload: AssetNodePayload,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisibleAssetRow {
    pub id: String,
    pub kind: ConsoleAssetKind,
    pub label: String,
    pub depth: usize,
    pub has_children: bool,
    pub expanded: bool,
    pub disclosure_state: AssetDisclosureState,
    pub path_hint: Option<String>,
    pub show_disclosure: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemovedAssetSummary {
    pub removed_ids: Vec<String>,
    pub descendant_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AssetTree {
    nodes: HashMap<String, AssetNode>,
    root_ids: Vec<String>,
    next_serial: u64,
}

impl AssetTree {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert_root(&mut self, kind: ConsoleAssetKind, title: impl Into<String>) -> String {
        self.insert_root_with_payload(kind, title, AssetNodePayload::for_kind(kind))
    }

    pub fn insert_root_with_payload(
        &mut self,
        kind: ConsoleAssetKind,
        title: impl Into<String>,
        payload: AssetNodePayload,
    ) -> String {
        assert!(
            payload.matches_kind(kind),
            "payload kind must match runtime node kind"
        );
        if let AssetNodePayload::Snippet(spec) = &payload {
            assert!(
                spec.package_id.is_none(),
                "root snippets must not carry package references"
            );
        }

        let id = self.next_id();
        let node = AssetNode {
            id: id.clone(),
            kind,
            title: title.into(),
            parent_id: None,
            children: Vec::new(),
            expanded: false,
            payload,
        };
        self.root_ids.push(id.clone());
        self.nodes.insert(id.clone(), node);
        id
    }

    pub fn insert_child(
        &mut self,
        parent_id: &str,
        kind: ConsoleAssetKind,
        title: impl Into<String>,
    ) -> String {
        self.insert_child_with_payload(parent_id, kind, title, AssetNodePayload::for_kind(kind))
    }

    pub fn insert_child_with_payload(
        &mut self,
        parent_id: &str,
        kind: ConsoleAssetKind,
        title: impl Into<String>,
        payload: AssetNodePayload,
    ) -> String {
        assert!(
            self.nodes.contains_key(parent_id),
            "parent node `{parent_id}` must exist before inserting a child"
        );
        assert!(
            payload.matches_kind(kind),
            "payload kind must match runtime node kind"
        );
        let parent_kind = self
            .nodes
            .get(parent_id)
            .expect("parent existence checked above")
            .kind;
        assert!(
            parent_kind.can_accept_children(),
            "parent kind `{}` cannot accept children",
            parent_kind.id()
        );
        assert!(
            parent_kind.allows_child(kind),
            "parent kind `{}` cannot contain child kind `{}`",
            parent_kind.id(),
            kind.id()
        );
        if let AssetNodePayload::Snippet(spec) = &payload {
            match kind {
                ConsoleAssetKind::Snippet if parent_kind == ConsoleAssetKind::SnippetPackage => {
                    assert_eq!(
                        spec.package_id.as_deref(),
                        Some(parent_id),
                        "package snippet payload must reference its parent package"
                    );
                }
                ConsoleAssetKind::Snippet => {
                    assert!(
                        spec.package_id.is_none(),
                        "non-package snippet children must not carry package references"
                    );
                }
                _ => {}
            }
        }

        let id = self.next_id();
        let node = AssetNode {
            id: id.clone(),
            kind,
            title: title.into(),
            parent_id: Some(parent_id.to_string()),
            children: Vec::new(),
            expanded: false,
            payload,
        };
        self.nodes.insert(id.clone(), node);
        self.nodes
            .get_mut(parent_id)
            .expect("parent existence checked above")
            .children
            .push(id.clone());
        id
    }

    pub fn from_parts(root_ids: Vec<String>, nodes: HashMap<String, AssetNode>) -> Self {
        Self {
            next_serial: next_serial_from_nodes(&nodes),
            nodes,
            root_ids,
        }
    }

    pub fn set_expanded(&mut self, node_id: &str, expanded: bool) {
        if let Some(node) = self.nodes.get_mut(node_id) {
            node.expanded = expanded;
        }
    }

    pub fn set_all_expanded(&mut self, expanded: bool) {
        for node in self.nodes.values_mut() {
            if node.kind.can_accept_children() {
                node.expanded = expanded;
            }
        }
    }

    pub fn title(&self, node_id: &str) -> Option<&str> {
        self.nodes.get(node_id).map(|node| node.title.as_str())
    }

    pub fn node(&self, node_id: &str) -> Option<&AssetNode> {
        self.nodes.get(node_id)
    }

    pub fn root_ids(&self) -> &[String] {
        &self.root_ids
    }

    pub fn kind(&self, node_id: &str) -> Option<ConsoleAssetKind> {
        self.nodes.get(node_id).map(|node| node.kind)
    }

    pub fn parent_id(&self, node_id: &str) -> Option<Option<&str>> {
        self.nodes
            .get(node_id)
            .map(|node| node.parent_id.as_deref())
    }

    pub fn set_title(&mut self, node_id: &str, title: impl Into<String>) {
        if let Some(node) = self.nodes.get_mut(node_id) {
            node.title = title.into();
        }
    }

    pub fn contains(&self, node_id: &str) -> bool {
        self.nodes.contains_key(node_id)
    }

    pub fn is_expanded(&self, node_id: &str) -> Option<bool> {
        self.nodes.get(node_id).map(|node| node.expanded)
    }

    pub fn ssh_connection_spec(&self, node_id: &str) -> Option<&AssetSshConnectionSpec> {
        match &self.nodes.get(node_id)?.payload {
            AssetNodePayload::Folder => None,
            AssetNodePayload::SshConnection(spec) => Some(spec),
            AssetNodePayload::SnippetPackage => None,
            AssetNodePayload::Snippet(_) => None,
        }
    }

    pub fn set_ssh_connection_spec(&mut self, node_id: &str, spec: AssetSshConnectionSpec) -> bool {
        let Some(node) = self.nodes.get_mut(node_id) else {
            return false;
        };

        match &mut node.payload {
            AssetNodePayload::Folder => false,
            AssetNodePayload::SshConnection(current) => {
                *current = spec;
                true
            }
            AssetNodePayload::SnippetPackage | AssetNodePayload::Snippet(_) => false,
        }
    }

    pub fn snippet_spec(&self, node_id: &str) -> Option<&AssetSnippetSpec> {
        match &self.nodes.get(node_id)?.payload {
            AssetNodePayload::Snippet(spec) => Some(spec),
            AssetNodePayload::Folder
            | AssetNodePayload::SshConnection(_)
            | AssetNodePayload::SnippetPackage => None,
        }
    }

    pub fn set_snippet_spec(&mut self, node_id: &str, spec: AssetSnippetSpec) -> bool {
        let Some(node) = self.nodes.get_mut(node_id) else {
            return false;
        };

        match &mut node.payload {
            AssetNodePayload::Snippet(current) => {
                *current = spec;
                true
            }
            AssetNodePayload::Folder
            | AssetNodePayload::SshConnection(_)
            | AssetNodePayload::SnippetPackage => false,
        }
    }

    pub fn sibling_items_for_parent(
        &self,
        parent_id: Option<&str>,
        exclude_id: Option<&str>,
    ) -> Vec<MockConsoleAssetItem> {
        let sibling_ids: &[String] = match parent_id {
            Some(parent_id) => self
                .nodes
                .get(parent_id)
                .map(|node| node.children.as_slice())
                .unwrap_or(&[]),
            None => &self.root_ids,
        };

        sibling_ids
            .iter()
            .filter(|id| Some(id.as_str()) != exclude_id)
            .filter_map(|id| self.nodes.get(id))
            .map(|node| MockConsoleAssetItem::new(&node.id, node.kind, &node.title))
            .collect()
    }

    pub fn project_visible_rows(
        &self,
        view_mode: AssetViewMode,
        search_query: &str,
    ) -> Vec<VisibleAssetRow> {
        let normalized_query = search_query.trim().to_ascii_lowercase();
        let mut rows = Vec::new();

        if normalized_query.is_empty() {
            match view_mode {
                AssetViewMode::Tree => self.collect_tree_rows(&self.root_ids, 0, &mut rows),
                AssetViewMode::Flat => self.collect_flat_rows(&self.root_ids, &mut rows),
            }
        } else {
            match view_mode {
                AssetViewMode::Tree => {
                    self.collect_search_rows(&self.root_ids, 0, &normalized_query, &mut rows);
                }
                AssetViewMode::Flat => {
                    self.collect_flat_search_rows(&self.root_ids, &normalized_query, &mut rows);
                }
            }
        }

        rows
    }

    pub fn next_default_name_for_parent(
        &self,
        parent_id: Option<&str>,
        kind: ConsoleAssetKind,
    ) -> String {
        let sibling_items = self.sibling_items_for_parent(parent_id, None);
        next_default_name(kind, &sibling_items)
    }

    pub fn validate_name_in_parent(
        &self,
        parent_id: Option<&str>,
        candidate: &str,
        exclude_id: Option<&str>,
    ) -> AssetNameValidation {
        let trimmed = candidate.trim();
        if trimmed.is_empty() {
            return AssetNameValidation::Empty;
        }

        let sibling_items = self.sibling_items_for_parent(parent_id, exclude_id);
        if sibling_items
            .iter()
            .any(|item| item.label.trim() == trimmed)
        {
            AssetNameValidation::Duplicate
        } else {
            AssetNameValidation::Valid
        }
    }

    pub fn descendant_count(&self, node_id: &str) -> Option<usize> {
        let mut subtree_ids = Vec::new();
        self.collect_subtree_ids(node_id, &mut subtree_ids)?;
        Some(subtree_ids.len().saturating_sub(1))
    }

    pub fn remove_subtree(&mut self, node_id: &str) -> Option<RemovedAssetSummary> {
        let parent_id = self.nodes.get(node_id)?.parent_id.clone();
        let mut removed_ids = Vec::new();
        self.collect_subtree_ids(node_id, &mut removed_ids)?;

        match parent_id {
            Some(parent_id) => {
                if let Some(parent) = self.nodes.get_mut(&parent_id) {
                    parent.children.retain(|child_id| child_id != node_id);
                }
            }
            None => {
                self.root_ids.retain(|root_id| root_id != node_id);
            }
        }

        for removed_id in &removed_ids {
            self.nodes.remove(removed_id);
        }

        Some(RemovedAssetSummary {
            descendant_count: removed_ids.len().saturating_sub(1),
            removed_ids,
        })
    }

    fn next_id(&mut self) -> String {
        loop {
            let id = format!("asset-{}", self.next_serial);
            self.next_serial += 1;
            if !self.nodes.contains_key(&id) {
                return id;
            }
        }
    }

    fn collect_tree_rows(
        &self,
        node_ids: &[String],
        depth: usize,
        rows: &mut Vec<VisibleAssetRow>,
    ) {
        for node_id in node_ids {
            let Some(node) = self.nodes.get(node_id) else {
                continue;
            };

            rows.push(self.row_from_node(node, depth));
            if node.expanded {
                self.collect_tree_rows(&node.children, depth + 1, rows);
            }
        }
    }

    fn collect_flat_rows(&self, node_ids: &[String], rows: &mut Vec<VisibleAssetRow>) {
        for node_id in node_ids {
            let Some(node) = self.nodes.get(node_id) else {
                continue;
            };

            if node.kind.should_appear_in_flat_view() {
                rows.push(self.flat_row_from_node(node));
            }
            self.collect_flat_rows(&node.children, rows);
        }
    }

    fn collect_flat_search_rows(
        &self,
        node_ids: &[String],
        search_query: &str,
        rows: &mut Vec<VisibleAssetRow>,
    ) {
        for node_id in node_ids {
            let Some(node) = self.nodes.get(node_id) else {
                continue;
            };

            if node.kind.should_appear_in_flat_view() {
                let row = self.flat_row_from_node(node);
                let label_match = row.label.to_ascii_lowercase().contains(search_query);
                let path_hint_match = row
                    .path_hint
                    .as_deref()
                    .unwrap_or_default()
                    .to_ascii_lowercase()
                    .contains(search_query);
                if label_match || path_hint_match {
                    rows.push(row);
                }
            }

            self.collect_flat_search_rows(&node.children, search_query, rows);
        }
    }

    fn collect_search_rows(
        &self,
        node_ids: &[String],
        depth: usize,
        search_query: &str,
        rows: &mut Vec<VisibleAssetRow>,
    ) -> bool {
        let mut found_match = false;

        for node_id in node_ids {
            let Some(node) = self.nodes.get(node_id) else {
                continue;
            };

            let mut child_rows = Vec::new();
            let descendant_match =
                self.collect_search_rows(&node.children, depth + 1, search_query, &mut child_rows);
            let node_match = node.title.to_ascii_lowercase().contains(search_query);

            if node_match || descendant_match {
                rows.push(self.row_from_node(node, depth));
                rows.extend(child_rows);
                found_match = true;
            }
        }

        found_match
    }

    fn row_from_node(&self, node: &AssetNode, depth: usize) -> VisibleAssetRow {
        let disclosure_state = match (node.kind, node.children.is_empty(), node.expanded) {
            (kind, false, false) if kind.can_accept_children() => AssetDisclosureState::Collapsed,
            (kind, false, true) if kind.can_accept_children() => AssetDisclosureState::Expanded,
            _ => AssetDisclosureState::None,
        };

        VisibleAssetRow {
            id: node.id.clone(),
            kind: node.kind,
            label: node.title.clone(),
            depth,
            has_children: !node.children.is_empty(),
            expanded: node.expanded,
            disclosure_state,
            path_hint: None,
            show_disclosure: node.kind.can_accept_children() && !node.children.is_empty(),
        }
    }

    fn flat_row_from_node(&self, node: &AssetNode) -> VisibleAssetRow {
        VisibleAssetRow {
            id: node.id.clone(),
            kind: node.kind,
            label: node.title.clone(),
            depth: 0,
            has_children: false,
            expanded: false,
            disclosure_state: AssetDisclosureState::None,
            path_hint: self.path_hint_for_node(node),
            show_disclosure: false,
        }
    }

    fn path_hint_for_node(&self, node: &AssetNode) -> Option<String> {
        let mut ancestors = Vec::new();
        let mut cursor = node.parent_id.as_deref();

        while let Some(parent_id) = cursor {
            let Some(parent) = self.nodes.get(parent_id) else {
                break;
            };
            if parent.kind.can_accept_children() {
                ancestors.push(parent.title.clone());
            }
            cursor = parent.parent_id.as_deref();
        }

        ancestors.reverse();
        (!ancestors.is_empty()).then(|| ancestors.join(" / "))
    }

    fn collect_subtree_ids(&self, node_id: &str, output: &mut Vec<String>) -> Option<()> {
        let node = self.nodes.get(node_id)?;
        output.push(node.id.clone());
        for child_id in &node.children {
            self.collect_subtree_ids(child_id, output)?;
        }
        Some(())
    }
}

fn next_serial_from_nodes(nodes: &HashMap<String, AssetNode>) -> u64 {
    nodes
        .keys()
        .filter_map(|id| id.strip_prefix("asset-"))
        .filter_map(|suffix| suffix.parse::<u64>().ok())
        .max()
        .map(|serial| serial + 1)
        .unwrap_or(0)
}

fn parse_default_name_index(kind: ConsoleAssetKind, label: &str) -> Option<u32> {
    let trimmed = label.trim();
    let prefix = kind.default_name_prefix();
    let suffix = trimmed.strip_prefix(prefix)?.strip_prefix(' ')?;
    if suffix.len() > 1 && suffix.starts_with('0') {
        return None;
    }

    let index = suffix.parse::<u32>().ok()?;
    (index > 0).then_some(index)
}

fn parse_custom_name_suffix(base: &str, label: &str) -> Option<u32> {
    let trimmed = label.trim();
    if trimmed == base {
        return Some(0);
    }

    let suffix = trimmed.strip_prefix(base)?.strip_prefix(' ')?;
    if suffix.len() > 1 && suffix.starts_with('0') {
        return None;
    }

    let index = suffix.parse::<u32>().ok()?;
    (index > 0).then_some(index)
}

fn parse_dashed_name_suffix(base: &str, label: &str) -> Option<u32> {
    let trimmed = label.trim();
    if trimmed == base {
        return Some(0);
    }

    let suffix = trimmed.strip_prefix(base)?.strip_prefix('-')?;
    if suffix.len() > 1 && suffix.starts_with('0') {
        return None;
    }

    let index = suffix.parse::<u32>().ok()?;
    (index > 0).then_some(index)
}

pub fn next_default_name_from_base(base: &str, siblings: &[MockConsoleAssetItem]) -> String {
    let used = siblings
        .iter()
        .filter_map(|item| parse_dashed_name_suffix(base, &item.label))
        .collect::<BTreeSet<_>>();

    if !used.contains(&0) {
        return base.to_string();
    }

    let next_suffix = (1..)
        .find(|suffix| !used.contains(suffix))
        .expect("positive integers are unbounded");
    format!("{base}-{next_suffix}")
}

pub fn next_default_name(kind: ConsoleAssetKind, items: &[MockConsoleAssetItem]) -> String {
    let base = format!("{} 1", kind.default_name_prefix());
    next_default_name_from_base(&base, items)
}

pub fn resolve_committed_name(
    kind: ConsoleAssetKind,
    draft: &str,
    items: &[MockConsoleAssetItem],
) -> String {
    let trimmed = draft.trim();
    if trimmed.is_empty() {
        return next_default_name(kind, items);
    }

    let conflicts_existing_name = items.iter().any(|item| item.label.trim() == trimmed);
    if conflicts_existing_name && parse_default_name_index(kind, trimmed).is_some() {
        return next_default_name(kind, items);
    }
    if conflicts_existing_name {
        let used = items
            .iter()
            .filter_map(|item| parse_custom_name_suffix(trimmed, &item.label))
            .collect::<BTreeSet<_>>();
        let next_index = (1..)
            .find(|index| !used.contains(index))
            .expect("positive integers are unbounded");
        return format!("{trimmed} {next_index}");
    }

    trimmed.to_string()
}

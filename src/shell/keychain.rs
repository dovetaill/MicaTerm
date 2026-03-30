//! Keychain explorer projection and mutation helpers for the shell workspace.

use std::collections::{BTreeSet, HashSet};

use crate::app::keychain::{
    KeychainCatalog, KeychainIdentityAuthKind, KeychainIdentitySpec, KeychainNode,
    KeychainNodeKind, KeychainNodePayload, KeychainSshKeySpec,
};
use crate::shell::assets::{
    AssetDisclosureState, AssetTree, SSH_AUTH_SOURCE_KEYCHAIN_IDENTITY, normalized_ssh_auth_source,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeychainItemKind {
    Folder,
    Identity,
    SshKey,
}

impl KeychainItemKind {
    pub fn id(self) -> &'static str {
        match self {
            Self::Folder => "folder",
            Self::Identity => "identity",
            Self::SshKey => "ssh-key",
        }
    }

    pub fn default_name_prefix(self) -> &'static str {
        match self {
            Self::Folder => "Folder",
            Self::Identity => "Identity",
            Self::SshKey => "SSH Key",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeychainCreateAction {
    NewFolder,
    NewIdentity,
    NewSshKey,
}

impl KeychainCreateAction {
    pub fn id(self) -> &'static str {
        match self {
            Self::NewFolder => "new-folder",
            Self::NewIdentity => "new-identity",
            Self::NewSshKey => "new-ssh-key",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::NewFolder => "New Folder",
            Self::NewIdentity => "New Identity",
            Self::NewSshKey => "New SSH Key",
        }
    }

    pub fn kind(self) -> KeychainItemKind {
        match self {
            Self::NewFolder => KeychainItemKind::Folder,
            Self::NewIdentity => KeychainItemKind::Identity,
            Self::NewSshKey => KeychainItemKind::SshKey,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisibleKeychainRow {
    pub id: String,
    pub kind: KeychainItemKind,
    pub label: String,
    pub depth: usize,
    pub has_children: bool,
    pub expanded: bool,
    pub disclosure_state: AssetDisclosureState,
    pub path_hint: Option<String>,
    pub show_disclosure: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemovedKeychainSummary {
    pub removed_ids: Vec<String>,
    pub descendant_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeychainDeleteError {
    FolderNotEmpty { child_count: usize },
    ReferencedByHosts { reference_count: usize },
    ReferencedByIdentities { reference_count: usize },
}

impl std::fmt::Display for KeychainDeleteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FolderNotEmpty { child_count } => {
                write!(f, "keychain folder is not empty ({child_count} child items)")
            }
            Self::ReferencedByHosts { reference_count } => {
                write!(f, "keychain identity is still referenced by {reference_count} SSH hosts")
            }
            Self::ReferencedByIdentities { reference_count } => write!(
                f,
                "keychain SSH key is still referenced by {reference_count} identities"
            ),
        }
    }
}

impl std::error::Error for KeychainDeleteError {}

pub fn create_keychain_node(
    catalog: &mut KeychainCatalog,
    parent_id: Option<&str>,
    kind: KeychainItemKind,
    title: Option<&str>,
) -> String {
    let parent_id = normalize_folder_parent_id(catalog, parent_id).map(ToOwned::to_owned);
    let sibling_titles = sibling_titles_for_parent(catalog, parent_id.as_deref(), None);
    let next_title = resolve_committed_name(kind, title.unwrap_or_default(), &sibling_titles);
    let next_id = next_keychain_id(catalog, kind);
    let payload = match kind {
        KeychainItemKind::Folder => KeychainNodePayload::Folder,
        KeychainItemKind::Identity => KeychainNodePayload::Identity(KeychainIdentitySpec::default()),
        KeychainItemKind::SshKey => KeychainNodePayload::SshKey(KeychainSshKeySpec::default()),
    };

    let node = KeychainNode {
        id: next_id.clone(),
        parent_id: parent_id.clone(),
        title: next_title,
        kind: node_kind(kind),
        child_ids: Vec::new(),
        payload,
    };

    if let Some(parent_id) = parent_id.as_deref() {
        if let Some(parent) = catalog.nodes.get_mut(parent_id) {
            parent.child_ids.push(next_id.clone());
        }
    } else {
        catalog.root_ids.push(next_id.clone());
    }
    catalog.nodes.insert(next_id.clone(), node);

    next_id
}

pub fn rename_keychain_node(
    catalog: &mut KeychainCatalog,
    node_id: &str,
    title: &str,
) -> anyhow::Result<()> {
    let Some(node) = catalog.nodes.get(node_id).cloned() else {
        anyhow::bail!("keychain node `{node_id}` was not found");
    };
    let kind = runtime_kind(node.kind);
    let sibling_titles = sibling_titles_for_parent(catalog, node.parent_id.as_deref(), Some(node_id));
    let next_title = resolve_committed_name(kind, title, &sibling_titles);
    let Some(current) = catalog.nodes.get_mut(node_id) else {
        anyhow::bail!("keychain node `{node_id}` was not found");
    };
    current.title = next_title;
    Ok(())
}

pub fn delete_keychain_node(
    catalog: &mut KeychainCatalog,
    node_id: &str,
    asset_tree: &AssetTree,
) -> Result<RemovedKeychainSummary, KeychainDeleteError> {
    let Some(node) = catalog.nodes.get(node_id).cloned() else {
        return Ok(RemovedKeychainSummary {
            removed_ids: Vec::new(),
            descendant_count: 0,
        });
    };

    match &node.payload {
        KeychainNodePayload::Folder => {
            if !node.child_ids.is_empty() {
                return Err(KeychainDeleteError::FolderNotEmpty {
                    child_count: node.child_ids.len(),
                });
            }
        }
        KeychainNodePayload::Identity(_) => {
            let reference_count = count_host_references(asset_tree, node_id);
            if reference_count > 0 {
                return Err(KeychainDeleteError::ReferencedByHosts { reference_count });
            }
        }
        KeychainNodePayload::SshKey(_) => {
            let reference_count = count_identity_references(catalog, node_id);
            if reference_count > 0 {
                return Err(KeychainDeleteError::ReferencedByIdentities { reference_count });
            }
        }
    }

    let parent_id = node.parent_id.clone();
    let mut removed_ids = Vec::new();
    collect_subtree_ids(catalog, node_id, &mut removed_ids);
    if let Some(parent_id) = parent_id {
        if let Some(parent) = catalog.nodes.get_mut(&parent_id) {
            parent.child_ids.retain(|child_id| child_id != node_id);
        }
    } else {
        catalog.root_ids.retain(|root_id| root_id != node_id);
    }
    for removed_id in &removed_ids {
        catalog.nodes.remove(removed_id);
    }

    Ok(RemovedKeychainSummary {
        descendant_count: removed_ids.len().saturating_sub(1),
        removed_ids,
    })
}

pub fn next_default_name_for_parent(
    catalog: &KeychainCatalog,
    parent_id: Option<&str>,
    kind: KeychainItemKind,
) -> String {
    let sibling_titles = sibling_titles_for_parent(catalog, parent_id, None);
    next_default_name(kind, &sibling_titles)
}

pub fn project_keychain_rows(
    catalog: &KeychainCatalog,
    expanded_ids: &BTreeSet<String>,
    search_query: &str,
) -> Vec<VisibleKeychainRow> {
    let normalized_query = search_query.trim().to_ascii_lowercase();
    let mut rows = Vec::new();
    if normalized_query.is_empty() {
        collect_tree_rows(catalog, &catalog.root_ids, 0, expanded_ids, &mut rows);
    } else {
        collect_search_rows(
            catalog,
            &catalog.root_ids,
            0,
            &normalized_query,
            expanded_ids,
            &mut rows,
        );
    }
    rows
}

fn collect_tree_rows(
    catalog: &KeychainCatalog,
    node_ids: &[String],
    depth: usize,
    expanded_ids: &BTreeSet<String>,
    rows: &mut Vec<VisibleKeychainRow>,
) {
    for node_id in node_ids {
        let Some(node) = catalog.nodes.get(node_id) else {
            continue;
        };

        let expanded = expanded_ids.contains(node_id);
        rows.push(row_from_node(node, depth, expanded));
        if expanded {
            collect_tree_rows(catalog, &node.child_ids, depth + 1, expanded_ids, rows);
        }
    }
}

fn collect_search_rows(
    catalog: &KeychainCatalog,
    node_ids: &[String],
    depth: usize,
    query: &str,
    expanded_ids: &BTreeSet<String>,
    rows: &mut Vec<VisibleKeychainRow>,
) -> bool {
    let mut found_match = false;

    for node_id in node_ids {
        let Some(node) = catalog.nodes.get(node_id) else {
            continue;
        };

        let mut child_rows = Vec::new();
        let descendant_match =
            collect_search_rows(catalog, &node.child_ids, depth + 1, query, expanded_ids, &mut child_rows);
        let node_match = node_matches_search(node, query);

        if node_match || descendant_match {
            rows.push(row_from_node(node, depth, descendant_match || expanded_ids.contains(node_id)));
            rows.extend(child_rows);
            found_match = true;
        }
    }

    found_match
}

fn row_from_node(node: &KeychainNode, depth: usize, expanded: bool) -> VisibleKeychainRow {
    let has_children = !node.child_ids.is_empty();
    let disclosure_state = if has_children {
        if expanded {
            AssetDisclosureState::Expanded
        } else {
            AssetDisclosureState::Collapsed
        }
    } else {
        AssetDisclosureState::None
    };

    VisibleKeychainRow {
        id: node.id.clone(),
        kind: runtime_kind(node.kind),
        label: node.title.clone(),
        depth,
        has_children,
        expanded,
        disclosure_state,
        path_hint: None,
        show_disclosure: has_children,
    }
}

fn node_matches_search(node: &KeychainNode, query: &str) -> bool {
    if node.title.to_ascii_lowercase().contains(query) {
        return true;
    }

    match &node.payload {
        KeychainNodePayload::Folder => false,
        KeychainNodePayload::Identity(identity) => identity.username.to_ascii_lowercase().contains(query),
        KeychainNodePayload::SshKey(ssh_key) => {
            ssh_key.fingerprint.to_ascii_lowercase().contains(query)
                || ssh_key.comment.to_ascii_lowercase().contains(query)
                || ssh_key.public_key.to_ascii_lowercase().contains(query)
        }
    }
}

fn sibling_titles_for_parent(
    catalog: &KeychainCatalog,
    parent_id: Option<&str>,
    exclude_id: Option<&str>,
) -> Vec<String> {
    let sibling_ids: &[String] = match parent_id {
        Some(parent_id) => catalog
            .nodes
            .get(parent_id)
            .map(|node| node.child_ids.as_slice())
            .unwrap_or(&[]),
        None => &catalog.root_ids,
    };

    sibling_ids
        .iter()
        .filter(|node_id| Some(node_id.as_str()) != exclude_id)
        .filter_map(|node_id| catalog.nodes.get(node_id))
        .map(|node| node.title.clone())
        .collect()
}

fn collect_subtree_ids(catalog: &KeychainCatalog, node_id: &str, output: &mut Vec<String>) {
    let Some(node) = catalog.nodes.get(node_id) else {
        return;
    };

    output.push(node_id.to_string());
    for child_id in &node.child_ids {
        collect_subtree_ids(catalog, child_id, output);
    }
}

fn count_identity_references(catalog: &KeychainCatalog, key_id: &str) -> usize {
    catalog
        .nodes
        .values()
        .filter_map(|node| match &node.payload {
            KeychainNodePayload::Identity(identity)
                if identity.auth_kind == KeychainIdentityAuthKind::SshKey
                    && identity.ssh_key_id.as_deref() == Some(key_id) =>
            {
                Some(())
            }
            _ => None,
        })
        .count()
}

fn count_host_references(asset_tree: &AssetTree, identity_id: &str) -> usize {
    let mut pending = asset_tree.root_ids().to_vec();
    let mut visited = HashSet::new();
    let mut count = 0;

    while let Some(node_id) = pending.pop() {
        if !visited.insert(node_id.clone()) {
            continue;
        }
        if let Some(node) = asset_tree.node(&node_id) {
            pending.extend(node.children.iter().cloned());
        }
        if asset_tree
            .ssh_connection_spec(&node_id)
            .is_some_and(|spec| {
                normalized_ssh_auth_source(&spec.auth_source) == SSH_AUTH_SOURCE_KEYCHAIN_IDENTITY
                    && spec.keychain_identity_id.as_deref() == Some(identity_id)
            })
        {
            count += 1;
        }
    }

    count
}

fn normalize_folder_parent_id<'a>(
    catalog: &'a KeychainCatalog,
    parent_id: Option<&'a str>,
) -> Option<&'a str> {
    parent_id.filter(|parent_id| {
        catalog
            .nodes
            .get(*parent_id)
            .is_some_and(|node| node.kind == KeychainNodeKind::Folder)
    })
}

fn next_keychain_id(catalog: &KeychainCatalog, kind: KeychainItemKind) -> String {
    let prefix = match kind {
        KeychainItemKind::Folder => "folder",
        KeychainItemKind::Identity => "identity",
        KeychainItemKind::SshKey => "key",
    };

    let mut next_index = 1u32;
    loop {
        let candidate = format!("{prefix}-{next_index}");
        if !catalog.nodes.contains_key(&candidate) {
            return candidate;
        }
        next_index += 1;
    }
}

fn node_kind(kind: KeychainItemKind) -> KeychainNodeKind {
    match kind {
        KeychainItemKind::Folder => KeychainNodeKind::Folder,
        KeychainItemKind::Identity => KeychainNodeKind::Identity,
        KeychainItemKind::SshKey => KeychainNodeKind::SshKey,
    }
}

fn runtime_kind(kind: KeychainNodeKind) -> KeychainItemKind {
    match kind {
        KeychainNodeKind::Folder => KeychainItemKind::Folder,
        KeychainNodeKind::Identity => KeychainItemKind::Identity,
        KeychainNodeKind::SshKey => KeychainItemKind::SshKey,
    }
}

fn next_default_name(kind: KeychainItemKind, sibling_titles: &[String]) -> String {
    let base = format!("{} 1", kind.default_name_prefix());
    next_default_name_from_base(&base, sibling_titles)
}

fn resolve_committed_name(kind: KeychainItemKind, draft: &str, sibling_titles: &[String]) -> String {
    let trimmed = draft.trim();
    if trimmed.is_empty() {
        return next_default_name(kind, sibling_titles);
    }

    let conflicts_existing_name = sibling_titles.iter().any(|title| title.trim() == trimmed);
    if conflicts_existing_name && parse_default_name_index(kind, trimmed).is_some() {
        return next_default_name(kind, sibling_titles);
    }
    if conflicts_existing_name {
        let used = sibling_titles
            .iter()
            .filter_map(|title| parse_custom_name_suffix(trimmed, title))
            .collect::<BTreeSet<_>>();
        let next_index = (1..)
            .find(|index| !used.contains(index))
            .expect("positive integers are unbounded");
        return format!("{trimmed} {next_index}");
    }

    trimmed.to_string()
}

fn next_default_name_from_base(base: &str, sibling_titles: &[String]) -> String {
    let used = sibling_titles
        .iter()
        .filter_map(|title| parse_dashed_name_suffix(base, title))
        .collect::<BTreeSet<_>>();

    if !used.contains(&0) {
        return base.to_string();
    }

    let next_suffix = (1..)
        .find(|suffix| !used.contains(suffix))
        .expect("positive integers are unbounded");
    format!("{base}-{next_suffix}")
}

fn parse_default_name_index(kind: KeychainItemKind, label: &str) -> Option<u32> {
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

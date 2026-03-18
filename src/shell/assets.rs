//! Asset sidebar view-mode identifiers shared between Rust state and Slint properties.

use std::collections::BTreeSet;

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
pub enum ConsoleAssetKind {
    SshConnection,
    Folder,
}

impl ConsoleAssetKind {
    pub fn id(self) -> &'static str {
        match self {
            Self::SshConnection => "ssh",
            Self::Folder => "folder",
        }
    }

    pub fn from_id(value: &str) -> Option<Self> {
        match value {
            "ssh" => Some(Self::SshConnection),
            "folder" => Some(Self::Folder),
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
        }
    }

    pub fn default_name_prefix(self) -> &'static str {
        match self {
            Self::Folder => "Folder",
            Self::SshConnection => "SSH Connection",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MockConsoleAssetItem {
    pub id: String,
    pub kind: ConsoleAssetKind,
    pub label: String,
}

impl MockConsoleAssetItem {
    pub fn new(
        id: impl Into<String>,
        kind: ConsoleAssetKind,
        label: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            kind,
            label: label.into(),
        }
    }
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

pub fn next_default_name(kind: ConsoleAssetKind, items: &[MockConsoleAssetItem]) -> String {
    let used = items
        .iter()
        .filter(|item| item.kind == kind)
        .filter_map(|item| parse_default_name_index(kind, &item.label))
        .collect::<BTreeSet<_>>();

    let next_index = (1..)
        .find(|index| !used.contains(index))
        .expect("positive integers are unbounded");
    format!("{} {}", kind.default_name_prefix(), next_index)
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

    let conflicts_existing_name = items
        .iter()
        .filter(|item| item.kind == kind)
        .any(|item| item.label.trim() == trimmed);
    if conflicts_existing_name && parse_default_name_index(kind, trimmed).is_some() {
        return next_default_name(kind, items);
    }
    if conflicts_existing_name {
        let used = items
            .iter()
            .filter(|item| item.kind == kind)
            .filter_map(|item| parse_custom_name_suffix(trimmed, &item.label))
            .collect::<BTreeSet<_>>();
        let next_index = (1..)
            .find(|index| !used.contains(index))
            .expect("positive integers are unbounded");
        return format!("{trimmed} {next_index}");
    }

    trimmed.to_string()
}

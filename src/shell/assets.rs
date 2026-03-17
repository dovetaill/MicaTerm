//! Asset sidebar view-mode identifiers shared between Rust state and Slint properties.

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

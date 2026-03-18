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
}

impl ConsoleAssetKind {
    pub fn id(self) -> &'static str {
        match self {
            Self::SshConnection => "ssh",
            Self::Folder => "folder",
        }
    }

    pub fn from_create_action_id(value: &str) -> Option<Self> {
        match value {
            "new-folder" => Some(Self::Folder),
            "new-ssh-connection" => Some(Self::SshConnection),
            _ => None,
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
pub struct AssetNode {
    pub id: String,
    pub kind: ConsoleAssetKind,
    pub title: String,
    pub parent_id: Option<String>,
    pub children: Vec<String>,
    pub expanded: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisibleAssetRow {
    pub id: String,
    pub kind: ConsoleAssetKind,
    pub label: String,
    pub depth: usize,
    pub has_children: bool,
    pub expanded: bool,
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

    pub fn insert_root(
        &mut self,
        kind: ConsoleAssetKind,
        title: impl Into<String>,
    ) -> String {
        let id = self.next_id();
        let node = AssetNode {
            id: id.clone(),
            kind,
            title: title.into(),
            parent_id: None,
            children: Vec::new(),
            expanded: false,
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
        assert!(
            self.nodes.contains_key(parent_id),
            "parent node `{parent_id}` must exist before inserting a child"
        );

        let id = self.next_id();
        let node = AssetNode {
            id: id.clone(),
            kind,
            title: title.into(),
            parent_id: Some(parent_id.to_string()),
            children: Vec::new(),
            expanded: false,
        };
        self.nodes.insert(id.clone(), node);
        self.nodes
            .get_mut(parent_id)
            .expect("parent existence checked above")
            .children
            .push(id.clone());
        id
    }

    pub fn set_expanded(&mut self, node_id: &str, expanded: bool) {
        if let Some(node) = self.nodes.get_mut(node_id) {
            node.expanded = expanded;
        }
    }

    pub fn title(&self, node_id: &str) -> Option<&str> {
        self.nodes.get(node_id).map(|node| node.title.as_str())
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

    pub fn project_visible_rows(
        &self,
        view_mode: AssetViewMode,
        search_query: &str,
    ) -> Vec<VisibleAssetRow> {
        let normalized_query = search_query.trim().to_ascii_lowercase();
        let mut rows = Vec::new();

        if normalized_query.is_empty() {
            match view_mode {
                AssetViewMode::Tree => {
                    self.collect_tree_rows(&self.root_ids, 0, &mut rows);
                }
                AssetViewMode::Flat => {
                    self.collect_flat_rows(&self.root_ids, 0, &mut rows);
                }
            }
        } else {
            self.collect_search_rows(&self.root_ids, 0, &normalized_query, &mut rows);
        }

        rows
    }

    pub fn next_default_name_for_parent(
        &self,
        parent_id: Option<&str>,
        kind: ConsoleAssetKind,
    ) -> String {
        let sibling_ids: &[String] = match parent_id {
            Some(parent_id) => self
                .nodes
                .get(parent_id)
                .map(|node| node.children.as_slice())
                .unwrap_or(&[]),
            None => &self.root_ids,
        };

        let used = sibling_ids
            .iter()
            .filter_map(|id| self.nodes.get(id))
            .filter(|node| node.kind == kind)
            .filter_map(|node| parse_default_name_index(kind, &node.title))
            .collect::<BTreeSet<_>>();

        let next_index = (1..)
            .find(|index| !used.contains(index))
            .expect("positive integers are unbounded");
        format!("{} {}", kind.default_name_prefix(), next_index)
    }

    fn next_id(&mut self) -> String {
        let id = format!("asset-{}", self.next_serial);
        self.next_serial += 1;
        id
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

    fn collect_flat_rows(
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
            self.collect_flat_rows(&node.children, depth + 1, rows);
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
        VisibleAssetRow {
            id: node.id.clone(),
            kind: node.kind,
            label: node.title.clone(),
            depth,
            has_children: !node.children.is_empty(),
            expanded: node.expanded,
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

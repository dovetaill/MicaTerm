#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextTargetKind {
    BlankArea,
    SshConnection,
    Folder,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextMenuActionNode {
    pub id: &'static str,
    pub label: &'static str,
    pub children: Vec<ContextMenuActionNode>,
}

pub fn resolve_action_tree(target: ContextTargetKind) -> Vec<ContextMenuActionNode> {
    match target {
        ContextTargetKind::BlankArea | ContextTargetKind::Folder => vec![
            leaf("new-folder", "New Folder"),
            leaf("new-ssh-connection", "New SSH Connection"),
        ],
        ContextTargetKind::SshConnection => Vec::new(),
    }
}

fn leaf(id: &'static str, label: &'static str) -> ContextMenuActionNode {
    ContextMenuActionNode {
        id,
        label,
        children: Vec::new(),
    }
}

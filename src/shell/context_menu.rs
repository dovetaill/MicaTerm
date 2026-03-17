//! Context menu domain state for the assets sidebar.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextTargetKind {
    BlankArea,
    SshConnection,
    Folder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextMenuActionState {
    Enabled,
    Disabled,
    Planned,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextMenuActionNode {
    pub id: &'static str,
    pub title: &'static str,
    pub state: ContextMenuActionState,
    pub children: Vec<ContextMenuActionNode>,
    pub divider_before: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SelectionContext {
    pub selected_ids: Vec<String>,
    pub clipboard_has_asset_payload: bool,
    pub target_mutable: bool,
    pub target_has_active_connection: bool,
}

impl SelectionContext {
    fn has_selection(&self) -> bool {
        !self.selected_ids.is_empty()
    }
}

pub const CONTEXT_MENU_COLUMN_WIDTH: f32 = 224.0;
pub const CONTEXT_MENU_COLUMN_GAP: f32 = 8.0;
pub const CONTEXT_MENU_HEIGHT: f32 = 320.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MenuPlacementInput {
    pub host_width: f32,
    pub host_height: f32,
    pub anchor_x: f32,
    pub anchor_y: f32,
    pub root_width: f32,
    pub root_height: f32,
    pub child_width: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    fn left(self) -> f32 {
        self.x
    }

    fn right(self) -> f32 {
        self.x + self.width
    }

    fn top(self) -> f32 {
        self.y
    }

    fn bottom(self) -> f32 {
        self.y + self.height
    }

    fn contains(self, pointer: (f32, f32)) -> bool {
        pointer.0 >= self.left()
            && pointer.0 <= self.right()
            && pointer.1 >= self.top()
            && pointer.1 <= self.bottom()
    }
}

pub fn resolve_action_tree(
    target: ContextTargetKind,
    selection: &SelectionContext,
) -> Vec<ContextMenuActionNode> {
    match target {
        ContextTargetKind::BlankArea => resolve_blank_area_actions(selection),
        ContextTargetKind::SshConnection => resolve_ssh_actions(selection),
        ContextTargetKind::Folder => resolve_folder_actions(selection),
    }
}

pub fn visible_columns_for_path(
    roots: &[ContextMenuActionNode],
    open_path: &[usize],
) -> [Vec<ContextMenuActionNode>; 3] {
    let mut columns = [roots.to_vec(), Vec::new(), Vec::new()];
    let mut current = roots;

    for (depth, index) in open_path.iter().copied().take(2).enumerate() {
        let Some(node) = current.get(index) else {
            break;
        };

        columns[depth + 1] = node.children.clone();
        current = &node.children;
    }

    columns
}

pub fn resolve_root_menu_origin(input: MenuPlacementInput) -> (f32, f32, bool) {
    let total_width = (input.root_width + input.child_width).max(input.root_width);
    let child_flows_left =
        input.child_width > 0.0 && input.anchor_x + input.root_width + input.child_width > input.host_width;

    let unclamped_x = if child_flows_left {
        input.anchor_x - input.child_width
    } else if input.anchor_x + total_width > input.host_width {
        input.anchor_x - total_width
    } else {
        input.anchor_x
    };

    let max_x = (input.host_width - total_width).max(0.0);
    let origin_x = unclamped_x.clamp(0.0, max_x);

    let unclamped_y = if input.anchor_y + input.root_height > input.host_height {
        input.anchor_y - input.root_height
    } else {
        input.anchor_y
    };
    let max_y = (input.host_height - input.root_height).max(0.0);
    let origin_y = unclamped_y.clamp(0.0, max_y);

    (origin_x, origin_y, child_flows_left)
}

pub fn should_keep_corridor_open(
    pointer: (f32, f32),
    parent_rect: Rect,
    child_rect: Rect,
) -> bool {
    if parent_rect.contains(pointer) || child_rect.contains(pointer) {
        return true;
    }

    let corridor_margin = 12.0;
    let child_is_right = child_rect.left() >= parent_rect.right();
    let (start_x, end_x) = if child_is_right {
        (parent_rect.right(), child_rect.left())
    } else {
        (child_rect.right(), parent_rect.left())
    };

    if pointer.0 < start_x.min(end_x) || pointer.0 > start_x.max(end_x) {
        return false;
    }

    let denominator = (end_x - start_x).abs();
    let progress = if denominator <= f32::EPSILON {
        1.0
    } else if child_is_right {
        ((pointer.0 - start_x) / (end_x - start_x)).clamp(0.0, 1.0)
    } else {
        ((start_x - pointer.0) / (start_x - end_x)).clamp(0.0, 1.0)
    };

    let top = lerp(
        parent_rect.top() - corridor_margin,
        child_rect.top() - corridor_margin,
        progress,
    );
    let bottom = lerp(
        parent_rect.bottom() + corridor_margin,
        child_rect.bottom() + corridor_margin,
        progress,
    );

    pointer.1 >= top.min(bottom) && pointer.1 <= top.max(bottom)
}

fn lerp(start: f32, end: f32, progress: f32) -> f32 {
    start + (end - start) * progress
}

fn resolve_blank_area_actions(selection: &SelectionContext) -> Vec<ContextMenuActionNode> {
    let has_selection = selection.has_selection();

    vec![
        action("new-folder", "New Folder"),
        new_connection_submenu(false),
        action_with_state(
            "batch-open",
            "Batch Open",
            if has_selection {
                ContextMenuActionState::Enabled
            } else {
                ContextMenuActionState::Disabled
            },
            false,
        ),
        action_with_state(
            "delete-asset",
            "Delete",
            mutable_selection_state(selection),
            true,
        ),
        action_with_state(
            "rename-asset",
            "Rename",
            mutable_selection_state(selection),
            false,
        ),
        action_with_state(
            "copy-asset",
            "Copy",
            selection_state(selection),
            false,
        ),
        action_with_state(
            "cut-asset",
            "Cut",
            mutable_selection_state(selection),
            false,
        ),
        action_with_state(
            "paste-asset",
            "Paste",
            if selection.clipboard_has_asset_payload {
                ContextMenuActionState::Enabled
            } else {
                ContextMenuActionState::Disabled
            },
            false,
        ),
        action_with_state("refresh-assets", "Refresh", ContextMenuActionState::Enabled, true),
        action_with_state("import-assets", "Import", ContextMenuActionState::Enabled, false),
        action_with_state("export-assets", "Export", ContextMenuActionState::Enabled, false),
    ]
}

fn resolve_ssh_actions(selection: &SelectionContext) -> Vec<ContextMenuActionNode> {
    vec![
        action_with_state(
            "close-connection",
            "Close",
            if selection.target_has_active_connection {
                ContextMenuActionState::Enabled
            } else {
                ContextMenuActionState::Disabled
            },
            false,
        ),
        action_with_state(
            "open-in-new-tab",
            "Open in New Tab",
            selection_state(selection),
            false,
        ),
        action_with_state(
            "new-folder",
            "New Folder",
            if selection.target_mutable {
                ContextMenuActionState::Enabled
            } else {
                ContextMenuActionState::Disabled
            },
            true,
        ),
        new_connection_submenu(false),
        action_with_state("edit-connection", "Edit", selection_state(selection), true),
        action_with_state("batch-edit", "Batch Edit", selection_state(selection), false),
        action_with_state("clone-connection", "Clone", selection_state(selection), false),
        action_with_state("copy-host", "Copy Host", selection_state(selection), false),
        action_with_state(
            "proxy-chrome-via-server",
            "Proxy Chrome via Server",
            ContextMenuActionState::Planned,
            true,
        ),
        action_with_state(
            "upload-ssh-public-key",
            "Upload SSH Public Key (ssh-copy-id)",
            ContextMenuActionState::Planned,
            false,
        ),
        action_with_state(
            "delete-asset",
            "Delete",
            mutable_selection_state(selection),
            true,
        ),
        action_with_state(
            "rename-asset",
            "Rename",
            mutable_selection_state(selection),
            false,
        ),
        action_with_state("refresh-assets", "Refresh", ContextMenuActionState::Enabled, true),
        action_with_state("import-assets", "Import", ContextMenuActionState::Enabled, false),
        action_with_state("export-assets", "Export", ContextMenuActionState::Enabled, false),
    ]
}

fn resolve_folder_actions(selection: &SelectionContext) -> Vec<ContextMenuActionNode> {
    let has_selection = selection.has_selection();

    vec![
        action_with_state(
            "new-folder",
            "New Folder",
            if selection.target_mutable {
                ContextMenuActionState::Enabled
            } else {
                ContextMenuActionState::Disabled
            },
            false,
        ),
        new_connection_submenu(false),
        action_with_state(
            "batch-open",
            "Batch Open",
            if has_selection {
                ContextMenuActionState::Enabled
            } else {
                ContextMenuActionState::Disabled
            },
            false,
        ),
        action_with_state(
            "delete-asset",
            "Delete",
            mutable_selection_state(selection),
            true,
        ),
        action_with_state(
            "rename-asset",
            "Rename",
            mutable_selection_state(selection),
            false,
        ),
        action_with_state("copy-asset", "Copy", selection_state(selection), false),
        action_with_state("cut-asset", "Cut", mutable_selection_state(selection), false),
        action_with_state(
            "paste-asset",
            "Paste",
            if selection.clipboard_has_asset_payload {
                ContextMenuActionState::Enabled
            } else {
                ContextMenuActionState::Disabled
            },
            false,
        ),
        action_with_state("refresh-assets", "Refresh", ContextMenuActionState::Enabled, true),
        action_with_state("import-assets", "Import", ContextMenuActionState::Enabled, false),
        action_with_state("export-assets", "Export", ContextMenuActionState::Enabled, false),
    ]
}

fn new_connection_submenu(divider_before: bool) -> ContextMenuActionNode {
    submenu(
        "new-connection",
        "New Connection",
        vec![
            action("ssh", "SSH"),
            action("local-terminal", "Local Terminal"),
            action("serial", "Serial"),
            action("telnet", "Telnet"),
            action("ssh-tunnel", "SSH Tunnel"),
        ],
        divider_before,
    )
}

fn selection_state(selection: &SelectionContext) -> ContextMenuActionState {
    if selection.has_selection() {
        ContextMenuActionState::Enabled
    } else {
        ContextMenuActionState::Disabled
    }
}

fn mutable_selection_state(selection: &SelectionContext) -> ContextMenuActionState {
    if selection.has_selection() && selection.target_mutable {
        ContextMenuActionState::Enabled
    } else {
        ContextMenuActionState::Disabled
    }
}

fn action(id: &'static str, title: &'static str) -> ContextMenuActionNode {
    action_with_state(id, title, ContextMenuActionState::Enabled, false)
}

fn submenu(
    id: &'static str,
    title: &'static str,
    children: Vec<ContextMenuActionNode>,
    divider_before: bool,
) -> ContextMenuActionNode {
    ContextMenuActionNode {
        id,
        title,
        state: ContextMenuActionState::Enabled,
        children,
        divider_before,
    }
}

fn action_with_state(
    id: &'static str,
    title: &'static str,
    state: ContextMenuActionState,
    divider_before: bool,
) -> ContextMenuActionNode {
    ContextMenuActionNode {
        id,
        title,
        state,
        children: Vec::new(),
        divider_before,
    }
}

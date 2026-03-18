use mica_term::shell::assets::{AssetTree, AssetViewMode, ConsoleAssetKind};

#[test]
fn tree_projection_hides_children_until_folder_is_expanded() {
    let mut tree = AssetTree::new();
    let folder_id = tree.insert_root(ConsoleAssetKind::Folder, "Folder 1");
    tree.insert_child(&folder_id, ConsoleAssetKind::SshConnection, "SSH Connection 1");

    let collapsed = tree.project_visible_rows(AssetViewMode::Tree, "");
    assert_eq!(collapsed.len(), 1);
    assert_eq!(collapsed[0].depth, 0);

    tree.set_expanded(&folder_id, true);
    let expanded = tree.project_visible_rows(AssetViewMode::Tree, "");
    assert_eq!(expanded.len(), 2);
    assert_eq!(expanded[1].depth, 1);
}

#[test]
fn default_names_are_unique_within_parent_scope() {
    let mut tree = AssetTree::new();
    let root_folder = tree.insert_root(ConsoleAssetKind::Folder, "Folder 1");
    tree.insert_root(ConsoleAssetKind::Folder, "Folder 2");
    tree.insert_child(&root_folder, ConsoleAssetKind::Folder, "Folder 1");

    assert_eq!(
        tree.next_default_name_for_parent(None, ConsoleAssetKind::Folder),
        "Folder 3"
    );
    assert_eq!(
        tree.next_default_name_for_parent(Some(&root_folder), ConsoleAssetKind::Folder),
        "Folder 2"
    );
}

#[test]
fn flat_projection_keeps_all_nodes_visible_regardless_of_expanded_state() {
    let mut tree = AssetTree::new();
    let folder_id = tree.insert_root(ConsoleAssetKind::Folder, "Folder 1");
    tree.insert_child(&folder_id, ConsoleAssetKind::SshConnection, "SSH Connection 1");

    let rows = tree.project_visible_rows(AssetViewMode::Flat, "");

    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].depth, 0);
    assert_eq!(rows[1].depth, 1);
}

#[test]
fn search_filters_visible_rows_without_destroying_tree_state() {
    let mut tree = AssetTree::new();
    let folder_id = tree.insert_root(ConsoleAssetKind::Folder, "Folder 1");
    tree.insert_child(&folder_id, ConsoleAssetKind::SshConnection, "SSH Connection 1");

    let collapsed = tree.project_visible_rows(AssetViewMode::Tree, "");
    assert_eq!(collapsed.len(), 1);
    assert_eq!(tree.is_expanded(&folder_id), Some(false));

    let filtered = tree.project_visible_rows(AssetViewMode::Tree, "SSH Connection 1");
    assert_eq!(filtered.len(), 2);
    assert_eq!(filtered[0].id, folder_id);
    assert_eq!(filtered[1].depth, 1);

    let cleared = tree.project_visible_rows(AssetViewMode::Tree, "");
    assert_eq!(cleared.len(), 1);
    assert_eq!(tree.is_expanded(&folder_id), Some(false));
}

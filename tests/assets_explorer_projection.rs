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
fn flat_projection_only_returns_ssh_rows_and_adds_path_hints() {
    let mut tree = AssetTree::new();
    let folder_id = tree.insert_root(ConsoleAssetKind::Folder, "Prod");
    tree.insert_child(&folder_id, ConsoleAssetKind::SshConnection, "Bastion");

    let rows = tree.project_visible_rows(AssetViewMode::Flat, "");

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].label, "Bastion");
    assert_eq!(rows[0].path_hint.as_deref(), Some("Prod"));
    assert!(!rows[0].show_disclosure);
}

#[test]
fn flat_search_matches_path_hint_without_surfacing_folder_rows() {
    let mut tree = AssetTree::new();
    let folder_id = tree.insert_root(ConsoleAssetKind::Folder, "Prod");
    tree.insert_child(&folder_id, ConsoleAssetKind::SshConnection, "Bastion");
    tree.insert_root(ConsoleAssetKind::Folder, "Archive");

    let rows = tree.project_visible_rows(AssetViewMode::Flat, "Prod");

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].kind, ConsoleAssetKind::SshConnection);
    assert_eq!(rows[0].label, "Bastion");
    assert_eq!(rows[0].path_hint.as_deref(), Some("Prod"));
    assert_eq!(rows[0].depth, 0);
}

#[test]
fn flat_search_matches_joined_nested_path_hint_without_surfacing_folder_rows() {
    let mut tree = AssetTree::new();
    let team_id = tree.insert_root(ConsoleAssetKind::Folder, "Team");
    let prod_id = tree.insert_child(&team_id, ConsoleAssetKind::Folder, "Prod");
    tree.insert_child(&prod_id, ConsoleAssetKind::SshConnection, "Bastion");
    tree.insert_root(ConsoleAssetKind::Folder, "Archive");

    let rows = tree.project_visible_rows(AssetViewMode::Flat, "Team / Prod");

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].kind, ConsoleAssetKind::SshConnection);
    assert_eq!(rows[0].label, "Bastion");
    assert_eq!(rows[0].path_hint.as_deref(), Some("Team / Prod"));
    assert_eq!(rows[0].depth, 0);
}

#[test]
fn tree_search_surfaces_folder_ancestors_for_nested_ssh_match() {
    let mut tree = AssetTree::new();
    let team_id = tree.insert_root(ConsoleAssetKind::Folder, "Team");
    let prod_id = tree.insert_child(&team_id, ConsoleAssetKind::Folder, "Prod");
    tree.insert_child(&prod_id, ConsoleAssetKind::SshConnection, "Bastion");

    let rows = tree.project_visible_rows(AssetViewMode::Tree, "Bastion");

    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].kind, ConsoleAssetKind::Folder);
    assert_eq!(rows[0].label, "Team");
    assert_eq!(rows[0].depth, 0);
    assert_eq!(rows[1].kind, ConsoleAssetKind::Folder);
    assert_eq!(rows[1].label, "Prod");
    assert_eq!(rows[1].depth, 1);
    assert_eq!(rows[2].kind, ConsoleAssetKind::SshConnection);
    assert_eq!(rows[2].label, "Bastion");
    assert_eq!(rows[2].depth, 2);
}

#[test]
fn nested_tree_search_clears_back_to_original_collapsed_state() {
    let mut tree = AssetTree::new();
    let team_id = tree.insert_root(ConsoleAssetKind::Folder, "Team");
    let prod_id = tree.insert_child(&team_id, ConsoleAssetKind::Folder, "Prod");
    tree.insert_child(&prod_id, ConsoleAssetKind::SshConnection, "Bastion");

    let collapsed = tree.project_visible_rows(AssetViewMode::Tree, "");
    assert_eq!(collapsed.len(), 1);
    assert_eq!(tree.is_expanded(&team_id), Some(false));
    assert_eq!(tree.is_expanded(&prod_id), Some(false));

    let filtered = tree.project_visible_rows(AssetViewMode::Tree, "Bastion");
    assert_eq!(filtered.len(), 3);
    assert_eq!(filtered[0].label, "Team");
    assert_eq!(filtered[1].label, "Prod");
    assert_eq!(filtered[2].label, "Bastion");
    assert_eq!(tree.is_expanded(&team_id), Some(false));
    assert_eq!(tree.is_expanded(&prod_id), Some(false));

    let cleared = tree.project_visible_rows(AssetViewMode::Tree, "");
    assert_eq!(cleared.len(), 1);
    assert_eq!(cleared[0].label, "Team");
    assert_eq!(tree.is_expanded(&team_id), Some(false));
    assert_eq!(tree.is_expanded(&prod_id), Some(false));
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

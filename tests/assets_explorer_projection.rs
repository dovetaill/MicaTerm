use mica_term::app::assets_catalog::{asset_tree_to_catalog, catalog_to_asset_tree};
use mica_term::shell::assets::{
    AssetDisclosureState, AssetNameValidation, AssetNodePayload, AssetSshConnectionSpec, AssetTree,
    AssetViewMode, ConsoleAssetKind, next_default_name_from_base,
};

#[test]
fn tree_projection_hides_children_until_folder_is_expanded() {
    let mut tree = AssetTree::new();
    let folder_id = tree.insert_root(ConsoleAssetKind::Folder, "Folder 1");
    tree.insert_child(
        &folder_id,
        ConsoleAssetKind::SshConnection,
        "SSH Connection 1",
    );

    let collapsed = tree.project_visible_rows(AssetViewMode::Tree, "");
    assert_eq!(collapsed.len(), 1);
    assert_eq!(collapsed[0].depth, 0);

    tree.set_expanded(&folder_id, true);
    let expanded = tree.project_visible_rows(AssetViewMode::Tree, "");
    assert_eq!(expanded.len(), 2);
    assert_eq!(expanded[1].depth, 1);
}

#[test]
fn tree_projection_exposes_disclosure_state_for_folder_rows() {
    let mut tree = AssetTree::new();
    let collapsed_folder_id = tree.insert_root(ConsoleAssetKind::Folder, "Collapsed");
    tree.insert_child(
        &collapsed_folder_id,
        ConsoleAssetKind::SshConnection,
        "Collapsed Bastion",
    );
    let expanded_folder_id = tree.insert_root(ConsoleAssetKind::Folder, "Expanded");
    tree.insert_child(
        &expanded_folder_id,
        ConsoleAssetKind::SshConnection,
        "Expanded Bastion",
    );
    let ssh_id = tree.insert_root(ConsoleAssetKind::SshConnection, "Leaf SSH");
    tree.set_expanded(&expanded_folder_id, true);

    let rows = tree.project_visible_rows(AssetViewMode::Tree, "");

    let collapsed_folder = rows
        .iter()
        .find(|row| row.id == collapsed_folder_id)
        .expect("collapsed folder row should be projected");
    let expanded_folder = rows
        .iter()
        .find(|row| row.id == expanded_folder_id)
        .expect("expanded folder row should be projected");
    let ssh_row = rows
        .iter()
        .find(|row| row.id == ssh_id)
        .expect("ssh row should be projected");

    assert_eq!(
        collapsed_folder.disclosure_state,
        AssetDisclosureState::Collapsed
    );
    assert_eq!(
        expanded_folder.disclosure_state,
        AssetDisclosureState::Expanded
    );
    assert_eq!(ssh_row.disclosure_state, AssetDisclosureState::None);
}

#[test]
fn default_names_are_unique_within_parent_scope() {
    let mut tree = AssetTree::new();
    let root_folder = tree.insert_root(ConsoleAssetKind::Folder, "Folder 1");
    tree.insert_root(ConsoleAssetKind::Folder, "Folder 2");
    tree.insert_child(&root_folder, ConsoleAssetKind::Folder, "Folder 1");

    assert_eq!(
        tree.next_default_name_for_parent(None, ConsoleAssetKind::Folder),
        "Folder 1-1"
    );
    assert_eq!(
        tree.next_default_name_for_parent(Some(&root_folder), ConsoleAssetKind::Folder),
        "Folder 1-1"
    );
}

#[test]
fn parent_scope_uniqueness_blocks_cross_kind_duplicates() {
    let mut tree = AssetTree::new();
    tree.insert_root(ConsoleAssetKind::Folder, "Prod");

    assert_eq!(
        tree.validate_name_in_parent(None, "Prod", None),
        AssetNameValidation::Duplicate
    );
}

#[test]
fn next_default_folder_name_uses_dash_suffix_after_base_collision() {
    let siblings = [mica_term::shell::assets::MockConsoleAssetItem::new(
        "folder-1",
        ConsoleAssetKind::Folder,
        "Folder 1",
    )];

    assert_eq!(
        next_default_name_from_base("Folder 1", &siblings),
        "Folder 1-1"
    );
}

#[test]
fn removing_folder_subtree_removes_all_descendants() {
    let mut tree = AssetTree::new();
    let root_id = tree.insert_root(ConsoleAssetKind::Folder, "Team");
    let child_folder_id = tree.insert_child(&root_id, ConsoleAssetKind::Folder, "Prod");
    let nested_ssh_id =
        tree.insert_child(&child_folder_id, ConsoleAssetKind::SshConnection, "Bastion");
    let sibling_id = tree.insert_root(ConsoleAssetKind::SshConnection, "Standalone");

    let removed = tree
        .remove_subtree(&root_id)
        .expect("folder subtree should be removed");

    assert_eq!(removed.descendant_count, 2);
    assert_eq!(
        removed.removed_ids,
        vec![
            root_id.clone(),
            child_folder_id.clone(),
            nested_ssh_id.clone()
        ]
    );
    assert!(!tree.contains(&root_id));
    assert!(!tree.contains(&child_folder_id));
    assert!(!tree.contains(&nested_ssh_id));

    let rows = tree.project_visible_rows(AssetViewMode::Tree, "");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, sibling_id);
}

#[test]
fn descendant_count_reports_nested_item_total() {
    let mut tree = AssetTree::new();
    let root_id = tree.insert_root(ConsoleAssetKind::Folder, "Team");
    let child_folder_id = tree.insert_child(&root_id, ConsoleAssetKind::Folder, "Prod");
    let nested_ssh_id =
        tree.insert_child(&child_folder_id, ConsoleAssetKind::SshConnection, "Bastion");
    tree.insert_child(&root_id, ConsoleAssetKind::SshConnection, "Ops");

    assert_eq!(tree.descendant_count(&root_id), Some(3));
    assert_eq!(tree.descendant_count(&child_folder_id), Some(1));
    assert_eq!(tree.descendant_count(&nested_ssh_id), Some(0));
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
    tree.insert_child(
        &folder_id,
        ConsoleAssetKind::SshConnection,
        "SSH Connection 1",
    );

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

#[test]
fn expanded_state_remains_runtime_only_after_catalog_mapping() {
    let mut tree = AssetTree::new();
    let folder_id = tree.insert_root(ConsoleAssetKind::Folder, "Team");
    tree.insert_child_with_payload(
        &folder_id,
        ConsoleAssetKind::SshConnection,
        "Bastion",
        AssetNodePayload::SshConnection(AssetSshConnectionSpec {
            host: "10.0.0.12".into(),
            user: "ops".into(),
            port: "22".into(),
            auth_method: "password".into(),
            private_key_source: "content".into(),
            private_key_path: String::new(),
            environment: "".into(),
            proxy_method: "".into(),
            remark: String::new(),
            credential_ref: None,
        }),
    );
    tree.set_expanded(&folder_id, true);

    let catalog = asset_tree_to_catalog(&tree);
    let round_tripped = catalog_to_asset_tree(&catalog);

    assert_eq!(tree.is_expanded(&folder_id), Some(true));
    assert_eq!(round_tripped.is_expanded(&folder_id), Some(false));
}

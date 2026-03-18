use mica_term::shell::assets::{AssetCreateAction, AssetViewMode};
use mica_term::shell::sidebar::{SidebarDestination, toolbar_descriptor_for};
use mica_term::shell::view_model::ShellViewModel;

#[test]
fn asset_view_mode_defaults_to_tree() {
    let view_model = ShellViewModel::default();

    assert_eq!(view_model.asset_view_mode, AssetViewMode::Tree);
    assert!(!view_model.asset_search_expanded);
    assert!(view_model.asset_search_query.is_empty());
    assert!(!view_model.asset_create_menu_open);
    assert!(!view_model.asset_tree_fully_expanded);
}

#[test]
fn toggling_asset_view_mode_flips_between_tree_and_flat() {
    let mut view_model = ShellViewModel::default();

    view_model.toggle_asset_view_mode();
    assert_eq!(view_model.asset_view_mode, AssetViewMode::Flat);

    view_model.toggle_asset_view_mode();
    assert_eq!(view_model.asset_view_mode, AssetViewMode::Tree);
}

#[test]
fn collapsing_empty_search_hides_search_row() {
    let mut view_model = ShellViewModel::default();

    view_model.toggle_asset_search();
    assert!(view_model.asset_search_expanded);

    view_model.collapse_asset_search_if_empty();
    assert!(!view_model.asset_search_expanded);
}

#[test]
fn non_empty_search_stays_open_when_focus_leaves() {
    let mut view_model = ShellViewModel::default();

    view_model.toggle_asset_search();
    view_model.set_asset_search_query("prod".into());
    view_model.collapse_asset_search_if_empty();

    assert!(view_model.asset_search_expanded);
    assert_eq!(view_model.asset_search_query, "prod");
}

#[test]
fn force_closing_search_hides_it_even_with_query() {
    let mut view_model = ShellViewModel::default();

    view_model.toggle_asset_search();
    view_model.set_asset_search_query("prod".into());
    view_model.close_asset_search();

    assert!(!view_model.asset_search_expanded);
    assert_eq!(view_model.asset_search_query, "prod");
}

#[test]
fn activating_search_closes_create_menu() {
    let mut view_model = ShellViewModel::default();

    view_model.toggle_asset_create_menu();
    assert!(view_model.asset_create_menu_open);

    view_model.activate_asset_search();

    assert!(view_model.asset_search_expanded);
    assert!(!view_model.asset_create_menu_open);
}

#[test]
fn opening_create_menu_closes_search_even_when_query_exists() {
    let mut view_model = ShellViewModel::default();

    view_model.activate_asset_search();
    view_model.set_asset_search_query("prod".into());
    view_model.toggle_asset_create_menu();

    assert!(view_model.asset_create_menu_open);
    assert!(!view_model.asset_search_expanded);
    assert_eq!(view_model.asset_search_query, "prod");
}

#[test]
fn flat_mode_disables_tree_expansion_toggle() {
    let mut view_model = ShellViewModel::default();

    view_model.toggle_asset_view_mode();
    assert_eq!(view_model.asset_view_mode, AssetViewMode::Flat);

    view_model.toggle_asset_tree_expansion();
    assert!(!view_model.asset_tree_fully_expanded);
}

#[test]
fn create_menu_toggles_and_actions_are_named() {
    let mut view_model = ShellViewModel::default();

    view_model.toggle_asset_create_menu();
    assert!(view_model.asset_create_menu_open);

    assert_eq!(AssetCreateAction::NewFolder.id(), "new-folder");
    assert_eq!(
        AssetCreateAction::NewSshConnection.id(),
        "new-ssh-connection"
    );
}

#[test]
fn console_destination_uses_create_popover_instead_of_single_direct_action() {
    let view_model = ShellViewModel::default();
    let descriptor = toolbar_descriptor_for(view_model.active_sidebar_destination, &view_model);

    assert!(descriptor.uses_create_popover);
    assert_eq!(descriptor.primary_create_action_id, None);
    assert_eq!(descriptor.primary_create_tooltip, "Create Asset");
}

#[test]
fn snippets_destination_keeps_direct_create_action() {
    let mut view_model = ShellViewModel::default();
    view_model.select_sidebar_destination(SidebarDestination::Snippets);

    let descriptor = toolbar_descriptor_for(view_model.active_sidebar_destination, &view_model);

    assert!(!descriptor.uses_create_popover);
    assert_eq!(descriptor.primary_create_action_id, Some("new-snippet"));
}

//! View-model contract tests for asset toolbar toggles and default modes.

use mica_term::shell::assets::AssetViewMode;
use mica_term::shell::sidebar::{SidebarDestination, toolbar_descriptor_for};
use mica_term::shell::view_model::ShellViewModel;

#[test]
fn asset_view_mode_defaults_to_tree() {
    let view_model = ShellViewModel::default();

    assert_eq!(view_model.asset_view_mode, AssetViewMode::Tree);
    assert!(!view_model.asset_search_expanded);
    assert!(view_model.asset_search_query.is_empty());
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
fn flat_mode_disables_tree_expansion_toggle() {
    let mut view_model = ShellViewModel::default();

    view_model.toggle_asset_view_mode();
    assert_eq!(view_model.asset_view_mode, AssetViewMode::Flat);

    view_model.toggle_asset_tree_expansion();
    assert!(!view_model.asset_tree_fully_expanded);
}

#[test]
fn console_destination_exposes_new_ssh_as_primary_create_action() {
    let view_model = ShellViewModel::default();

    let descriptor = toolbar_descriptor_for(view_model.active_sidebar_destination, &view_model);
    assert_eq!(descriptor.primary_create_action_id, Some("new-ssh-connection"));
    assert_eq!(descriptor.primary_create_tooltip, "New SSH Connection");
}

#[test]
fn switching_sidebar_destination_updates_primary_create_descriptor() {
    let mut view_model = ShellViewModel::default();
    view_model.select_sidebar_destination(SidebarDestination::Snippets);

    let descriptor = toolbar_descriptor_for(view_model.active_sidebar_destination, &view_model);
    assert_eq!(descriptor.primary_create_action_id, Some("new-snippet"));
    assert_eq!(descriptor.primary_create_tooltip, "New Snippet");
}

#[test]
fn dismissing_empty_search_on_shell_interaction_only_closes_blank_queries() {
    let mut view_model = ShellViewModel::default();

    view_model.activate_asset_search();
    assert!(view_model.dismiss_empty_asset_search_on_shell_interaction());
    assert!(!view_model.asset_search_expanded);

    view_model.activate_asset_search();
    view_model.set_asset_search_query("prod".into());
    assert!(!view_model.dismiss_empty_asset_search_on_shell_interaction());
    assert!(view_model.asset_search_expanded);
}

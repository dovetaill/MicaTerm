//! Sidebar destination ordering and activation contracts.

use mica_term::shell::sidebar::{
    SidebarDestination, create_popover_actions_for, sidebar_destinations, toolbar_descriptor_for,
};
use mica_term::shell::metrics::ShellMetrics;
use mica_term::shell::view_model::ShellViewModel;

#[test]
fn sidebar_destinations_match_the_approved_order() {
    assert_eq!(
        sidebar_destinations(),
        &[
            SidebarDestination::Console,
            SidebarDestination::Snippets,
            SidebarDestination::Keychain,
        ]
    );
}

#[test]
fn shell_view_model_starts_with_console_selected_and_assets_sidebar_open() {
    let view_model = ShellViewModel::default();

    assert!(view_model.show_assets_sidebar);
    assert_eq!(
        view_model.active_sidebar_destination,
        SidebarDestination::Console
    );
}

#[test]
fn toggling_assets_sidebar_keeps_current_destination() {
    let mut view_model = ShellViewModel::default();

    view_model.select_sidebar_destination(SidebarDestination::Snippets);
    view_model.toggle_assets_sidebar();

    assert!(!view_model.show_assets_sidebar);
    assert_eq!(
        view_model.active_sidebar_destination,
        SidebarDestination::Snippets
    );
}

#[test]
fn selecting_sidebar_destination_auto_expands_assets_sidebar() {
    let mut view_model = ShellViewModel::default();

    view_model.toggle_assets_sidebar();
    assert!(!view_model.show_assets_sidebar);

    view_model.select_sidebar_destination(SidebarDestination::Keychain);

    assert!(view_model.show_assets_sidebar);
    assert_eq!(
        view_model.active_sidebar_destination,
        SidebarDestination::Keychain
    );
}

#[test]
fn selecting_active_destination_collapses_assets_sidebar() {
    let mut view_model = ShellViewModel::default();

    view_model.select_sidebar_destination(SidebarDestination::Console);

    assert!(!view_model.show_assets_sidebar);
    assert_eq!(
        view_model.active_sidebar_destination,
        SidebarDestination::Console
    );
}

#[test]
fn selecting_active_destination_again_reopens_assets_sidebar() {
    let mut view_model = ShellViewModel::default();

    view_model.select_sidebar_destination(SidebarDestination::Console);
    assert!(!view_model.show_assets_sidebar);

    view_model.select_sidebar_destination(SidebarDestination::Console);

    assert!(view_model.show_assets_sidebar);
    assert_eq!(
        view_model.active_sidebar_destination,
        SidebarDestination::Console
    );
}

#[test]
fn selecting_active_destination_collapse_hides_sidebar_search() {
    let mut view_model = ShellViewModel::default();

    view_model.activate_asset_search();
    view_model.set_asset_search_query("prod".into());

    view_model.select_sidebar_destination(SidebarDestination::Console);

    assert!(!view_model.show_assets_sidebar);
    assert!(!view_model.asset_search_expanded);
    assert_eq!(view_model.asset_search_query, "prod");
}

#[test]
fn reopening_assets_sidebar_restores_last_resized_width() {
    let mut view_model = ShellViewModel::default();

    assert!(view_model.set_assets_sidebar_expanded_width(360.0));

    view_model.toggle_assets_sidebar();
    assert!(!view_model.show_assets_sidebar);

    view_model.toggle_assets_sidebar();

    assert!(view_model.show_assets_sidebar);
    assert_eq!(view_model.assets_sidebar_expanded_width_px() as u32, 360);
}

#[test]
fn reopening_right_panel_restores_last_resized_width() {
    let mut view_model = ShellViewModel::default();

    view_model.toggle_right_panel();
    assert!(view_model.set_right_panel_expanded_width(448.0));

    view_model.toggle_right_panel();
    assert!(!view_model.show_right_panel);

    view_model.toggle_right_panel();

    assert!(view_model.show_right_panel);
    assert_eq!(view_model.right_panel_expanded_width_px() as u32, 448);
}

#[test]
fn dragging_below_collapse_threshold_hides_panels_without_forgetting_widths() {
    let mut view_model = ShellViewModel::default();

    assert!(view_model.set_assets_sidebar_expanded_width(364.0));
    assert!(view_model.apply_assets_sidebar_resize(
        (ShellMetrics::ASSETS_SIDEBAR_COLLAPSE_THRESHOLD - 1) as f32,
    ));
    assert!(!view_model.show_assets_sidebar);
    assert_eq!(view_model.assets_sidebar_expanded_width_px() as u32, 364);

    view_model.toggle_right_panel();
    assert!(view_model.set_right_panel_expanded_width(456.0));
    assert!(view_model.apply_right_panel_resize(
        (ShellMetrics::RIGHT_PANEL_COLLAPSE_THRESHOLD - 1) as f32,
    ));
    assert!(!view_model.show_right_panel);
    assert_eq!(view_model.right_panel_expanded_width_px() as u32, 456);
}

#[test]
fn keychain_toolbar_uses_create_popover_with_keychain_specific_actions() {
    let mut view_model = ShellViewModel::default();
    view_model.select_sidebar_destination(SidebarDestination::Keychain);

    let descriptor = toolbar_descriptor_for(view_model.active_sidebar_destination, &view_model);
    let actions = create_popover_actions_for(view_model.active_sidebar_destination);

    assert!(descriptor.uses_create_popover);
    assert_eq!(descriptor.primary_create_action_id, None);
    assert_eq!(descriptor.primary_create_tooltip, "Create Keychain Item");
    assert_eq!(descriptor.search_tooltip, "Search Keychain");
    assert_eq!(
        actions.iter().map(|action| action.id).collect::<Vec<_>>(),
        vec!["new-folder", "new-identity", "new-ssh-key"]
    );
}

//! Sidebar destination ordering and activation contracts.

use mica_term::shell::sidebar::{
    SidebarDestination, create_popover_actions_for, sidebar_destinations, toolbar_descriptor_for,
};
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

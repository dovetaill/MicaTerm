//! Sidebar destination identifiers, navigation items, and toolbar descriptors.

use slint::SharedString;

use crate::SidebarNavItem;
use crate::shell::assets::AssetViewMode;
use crate::shell::view_model::ShellViewModel;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarDestination {
    Console,
    Snippets,
    Keychain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssetsToolbarDescriptor {
    pub uses_create_popover: bool,
    pub primary_create_action_id: Option<&'static str>,
    pub primary_create_tooltip: &'static str,
    pub search_tooltip: &'static str,
    pub view_mode_tooltip: &'static str,
    pub tree_expansion_tooltip: &'static str,
    pub show_tree_controls: bool,
}

impl SidebarDestination {
    pub fn id(self) -> &'static str {
        match self {
            Self::Console => "console",
            Self::Snippets => "snippets",
            Self::Keychain => "keychain",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::Console => "Window Console",
            Self::Snippets => "Snippets",
            Self::Keychain => "Keychain",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "console" => Some(Self::Console),
            "snippets" => Some(Self::Snippets),
            "keychain" => Some(Self::Keychain),
            _ => None,
        }
    }
}

pub fn sidebar_destinations() -> &'static [SidebarDestination] {
    &[
        SidebarDestination::Console,
        SidebarDestination::Snippets,
        SidebarDestination::Keychain,
    ]
}

pub fn sidebar_items_for(state: &ShellViewModel) -> Vec<SidebarNavItem> {
    sidebar_destinations()
        .iter()
        .map(|destination| SidebarNavItem {
            id: SharedString::from(destination.id()),
            label: SharedString::from(destination.title()),
            active: *destination == state.active_sidebar_destination,
        })
        .collect()
}

pub fn toolbar_descriptor_for(
    destination: SidebarDestination,
    view_model: &ShellViewModel,
) -> AssetsToolbarDescriptor {
    let (
        uses_create_popover,
        primary_create_action_id,
        primary_create_tooltip,
        search_tooltip,
        show_tree_controls,
    ) = match destination {
        SidebarDestination::Console => (
            true,
            None,
            "Create Asset",
            "Search Console Assets",
            true,
        ),
        SidebarDestination::Snippets => (
            false,
            Some("new-snippet"),
            "New Snippet",
            "Search Snippets",
            false,
        ),
        SidebarDestination::Keychain => (
            false,
            Some("new-keychain"),
            "New Keychain",
            "Search Keychain",
            false,
        ),
    };

    let view_mode_tooltip = match view_model.asset_view_mode {
        AssetViewMode::Tree => "Switch to Flat List",
        AssetViewMode::Flat => "Switch to Tree View",
    };
    let tree_expansion_tooltip = if view_model.asset_tree_fully_expanded {
        "Collapse Tree"
    } else {
        "Expand Tree"
    };

    AssetsToolbarDescriptor {
        uses_create_popover,
        primary_create_action_id,
        primary_create_tooltip,
        search_tooltip,
        view_mode_tooltip,
        tree_expansion_tooltip,
        show_tree_controls,
    }
}

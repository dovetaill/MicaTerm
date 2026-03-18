use slint::SharedString;

use crate::SidebarNavItem;
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
    _view_model: &ShellViewModel,
) -> AssetsToolbarDescriptor {
    match destination {
        SidebarDestination::Console => AssetsToolbarDescriptor {
            uses_create_popover: true,
            primary_create_action_id: None,
            primary_create_tooltip: "Create Asset",
        },
        SidebarDestination::Snippets => AssetsToolbarDescriptor {
            uses_create_popover: false,
            primary_create_action_id: Some("new-snippet"),
            primary_create_tooltip: "New Snippet",
        },
        SidebarDestination::Keychain => AssetsToolbarDescriptor {
            uses_create_popover: false,
            primary_create_action_id: Some("new-keychain"),
            primary_create_tooltip: "New Keychain",
        },
    }
}

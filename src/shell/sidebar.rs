#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarDestination {
    Console,
    Snippets,
    Keychain,
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
use slint::SharedString;

use crate::SidebarNavItem;
use crate::shell::view_model::ShellViewModel;

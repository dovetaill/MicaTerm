//! Asset sidebar view-mode identifiers shared between Rust state and Slint properties.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetViewMode {
    Tree,
    Flat,
}

impl AssetViewMode {
    pub fn id(self) -> &'static str {
        match self {
            Self::Tree => "tree",
            Self::Flat => "flat",
        }
    }

    pub fn toggle(self) -> Self {
        match self {
            Self::Tree => Self::Flat,
            Self::Flat => Self::Tree,
        }
    }
}

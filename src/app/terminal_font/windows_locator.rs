//! Windows font-family discovery helper backed by the local system font database.

use std::collections::{BTreeMap, BTreeSet};

use fontdb::Database;

pub struct WindowsFontLocator {
    families_by_lowercase: BTreeMap<String, String>,
    family_scan_order: Vec<String>,
}

impl WindowsFontLocator {
    pub fn new() -> Self {
        let mut database = Database::new();
        database.load_system_fonts();

        let mut families_by_lowercase = BTreeMap::new();
        let mut family_scan_order = Vec::new();

        for face in database.faces() {
            for (family_name, _) in &face.families {
                let family_key = family_name.to_ascii_lowercase();
                if families_by_lowercase
                    .insert(family_key, family_name.clone())
                    .is_none()
                {
                    family_scan_order.push(family_name.clone());
                }
            }
        }

        Self {
            families_by_lowercase,
            family_scan_order,
        }
    }

    pub fn resolve_family(&self, candidates: &[&str]) -> Option<String> {
        candidates
            .iter()
            .find_map(|candidate| self.resolve_installed_family(candidate))
    }

    pub fn first_distinct_family(&self, excluded: &[String]) -> Option<String> {
        let excluded = excluded
            .iter()
            .map(|family_name| family_name.to_ascii_lowercase())
            .collect::<BTreeSet<_>>();

        self.family_scan_order
            .iter()
            .find(|family_name| !excluded.contains(&family_name.to_ascii_lowercase()))
            .cloned()
    }

    fn resolve_installed_family(&self, family_name: &str) -> Option<String> {
        self.families_by_lowercase
            .get(&family_name.to_ascii_lowercase())
            .cloned()
    }
}

impl Default for WindowsFontLocator {
    fn default() -> Self {
        Self::new()
    }
}

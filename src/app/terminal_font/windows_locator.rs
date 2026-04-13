//! Windows font-family discovery helper backed by the local system font database.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use fontdb::{Database, Family, Query, Stretch, Style, Weight};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResolvedFontFaceSource {
    Bundled,
    System,
}

#[derive(Clone, Debug)]
pub struct ResolvedFontFaceData {
    pub family_name: String,
    pub post_script_name: String,
    pub face_index: u32,
    pub font_data: Vec<u8>,
    pub source: ResolvedFontFaceSource,
}

pub struct WindowsFontLocator {
    database: Arc<Database>,
    families_by_lowercase: BTreeMap<String, String>,
    family_scan_order: Vec<String>,
}

impl WindowsFontLocator {
    pub fn from_database(database: Arc<Database>) -> Self {
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
            database,
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

    pub fn resolve_face_data(&self, family_name: &str) -> Option<ResolvedFontFaceData> {
        let family_name = self.resolve_installed_family(family_name)?;
        let families = [Family::Name(family_name.as_str())];
        let face_id = self.database.query(&Query {
            families: &families,
            weight: Weight::NORMAL,
            stretch: Stretch::Normal,
            style: Style::Normal,
        })?;
        let face = self.database.face(face_id)?;
        let resolved_family = face
            .families
            .first()
            .map(|(name, _)| name.clone())
            .unwrap_or_else(|| family_name.clone());
        let post_script_name = face.post_script_name.clone();

        self.database
            .with_face_data(face_id, |font_data, face_index| ResolvedFontFaceData {
                family_name: resolved_family,
                post_script_name,
                face_index,
                font_data: font_data.to_vec(),
                source: ResolvedFontFaceSource::System,
            })
    }

    fn resolve_installed_family(&self, family_name: &str) -> Option<String> {
        self.families_by_lowercase
            .get(&family_name.to_ascii_lowercase())
            .cloned()
    }
}

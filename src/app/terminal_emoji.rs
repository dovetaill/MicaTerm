//! Classifies terminal clusters that need color emoji rendering and resolves preferred emoji fonts.

use fontdb::{Database, ID};
use unicode_properties::UnicodeEmoji;

const VISIBLE_EMOJI_FALLBACK_TEXT: &str = "\u{fffd}";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClusterRenderKind {
    Mono,
    Emoji,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EmojiFallbackReason {
    MissingPreferredFont,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedEmojiFont {
    pub face_id: ID,
    pub family_name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EmojiFontResolution {
    Resolved(ResolvedEmojiFont),
    VisibleFallback {
        replacement_text: String,
        reason: EmojiFallbackReason,
    },
}

pub struct TerminalEmojiResolver {
    database: Database,
}

impl TerminalEmojiResolver {
    pub fn new() -> Self {
        let mut database = Database::new();
        database.load_system_fonts();
        Self { database }
    }

    pub fn from_database(database: Database) -> Self {
        Self { database }
    }

    pub fn resolve_preferred_font(&self) -> EmojiFontResolution {
        for preferred_family in preferred_emoji_families() {
            if let Some(face) = self.database.faces().find(|face| {
                face.families
                    .iter()
                    .any(|family| family.0.eq_ignore_ascii_case(preferred_family))
            }) {
                return EmojiFontResolution::Resolved(ResolvedEmojiFont {
                    face_id: face.id,
                    family_name: preferred_family.to_string(),
                });
            }
        }

        EmojiFontResolution::VisibleFallback {
            replacement_text: VISIBLE_EMOJI_FALLBACK_TEXT.to_string(),
            reason: EmojiFallbackReason::MissingPreferredFont,
        }
    }
}

pub fn classify_cluster_render_kind(text: &str) -> ClusterRenderKind {
    if text.is_empty() || text.chars().all(char::is_whitespace) || contains_private_use(text) {
        return ClusterRenderKind::Mono;
    }

    let saw_emoji = text.chars().any(|ch| ch.is_emoji_char());
    let has_emoji_presentation_markers =
        text.contains('\u{fe0f}') || text.contains('\u{200d}') || text.contains('\u{20e3}');

    if saw_emoji || has_emoji_presentation_markers {
        ClusterRenderKind::Emoji
    } else {
        ClusterRenderKind::Mono
    }
}

pub fn preferred_emoji_families() -> &'static [&'static str] {
    if cfg!(target_os = "windows") {
        &["Segoe UI Emoji"]
    } else if cfg!(target_os = "linux") {
        &[
            "Noto Color Emoji",
            "Twitter Color Emoji",
            "EmojiOne Color",
            "JoyPixels",
        ]
    } else if cfg!(target_os = "macos") {
        &["Apple Color Emoji"]
    } else {
        &["Noto Color Emoji", "Segoe UI Emoji"]
    }
}

fn contains_private_use(text: &str) -> bool {
    text.chars().any(|ch| {
        matches!(
            ch as u32,
            0xE000..=0xF8FF | 0xF0000..=0xFFFFD | 0x100000..=0x10FFFD
        )
    })
}

//! Semantic overlay descriptors derived from terminal model frames.

mod command_blocks;
mod input_line;
mod output_blocks;
mod output_rules;

use crate::app::terminal_model::TerminalModelFrame;
use crate::theme::SemanticStyleRole;

pub use command_blocks::{CommandBlock, CommandBlockStatus, OverviewMarker, OverviewMarkerKind};
pub use input_line::{
    SemanticInputOverlay, SemanticInputSpanKind, detect_input_line_overlays,
    detect_input_semantic_spans,
};
pub use output_blocks::{
    SemanticOutputBlockKind, SemanticOutputOverlay, SemanticOverlayRowRange,
    detect_output_block_overlays,
};
pub use output_rules::{
    count_search_query_matches_in_lines, detect_output_rule_spans, detect_search_match_spans,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum OutputRuleProfile {
    #[default]
    Default,
    Focused,
}

impl OutputRuleProfile {
    pub fn id(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Focused => "focused",
        }
    }

    pub fn from_id(value: &str) -> Self {
        match value {
            "focused" => Self::Focused,
            _ => Self::Default,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SemanticSpan {
    pub row: u32,
    pub start_col: u32,
    pub end_col: u32,
    pub role: SemanticStyleRole,
    pub text: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SemanticAnnotationSet {
    pub spans: Vec<SemanticSpan>,
    pub command_blocks: Vec<CommandBlock>,
    pub overview_markers: Vec<OverviewMarker>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerminalSemanticSettings {
    pub input_highlighting_enabled: bool,
    pub output_rule_highlighting_enabled: bool,
    pub output_rule_profile: OutputRuleProfile,
    pub command_decorations_enabled: bool,
    pub overview_markers_enabled: bool,
    pub search_query: Option<String>,
}

impl Default for TerminalSemanticSettings {
    fn default() -> Self {
        Self {
            input_highlighting_enabled: true,
            output_rule_highlighting_enabled: true,
            output_rule_profile: OutputRuleProfile::Default,
            command_decorations_enabled: true,
            overview_markers_enabled: true,
            search_query: None,
        }
    }
}

pub fn analyze_semantic_annotations(frame: &TerminalModelFrame) -> SemanticAnnotationSet {
    analyze_semantic_annotations_with_settings(frame, TerminalSemanticSettings::default())
}

pub fn analyze_semantic_annotations_with_settings(
    frame: &TerminalModelFrame,
    settings: TerminalSemanticSettings,
) -> SemanticAnnotationSet {
    let mut spans = detect_input_semantic_spans(frame);
    if !settings.input_highlighting_enabled {
        spans.clear();
    }
    if settings.output_rule_highlighting_enabled {
        spans.extend(output_rules::detect_output_rule_spans(
            frame,
            settings.output_rule_profile,
        ));
    }
    if let Some(query) = settings.search_query.as_deref() {
        spans.extend(output_rules::detect_search_match_spans(frame, query));
    }
    spans.sort_by(|left, right| {
        left.row
            .cmp(&right.row)
            .then(left.start_col.cmp(&right.start_col))
            .then(left.end_col.cmp(&right.end_col))
            .then(left.role.cmp(&right.role))
            .then(left.text.cmp(&right.text))
    });
    spans.dedup();

    let command_blocks = if settings.command_decorations_enabled {
        command_blocks::detect_command_blocks(frame, &spans)
    } else {
        Vec::new()
    };
    let overview_markers =
        if settings.command_decorations_enabled && settings.overview_markers_enabled {
            command_blocks::overview_markers_for(&command_blocks)
        } else {
            Vec::new()
        };

    SemanticAnnotationSet {
        spans,
        command_blocks,
        overview_markers,
    }
}

pub(crate) fn push_unique_span(spans: &mut Vec<SemanticSpan>, span: SemanticSpan) {
    if !spans.contains(&span) {
        spans.push(span);
    }
}

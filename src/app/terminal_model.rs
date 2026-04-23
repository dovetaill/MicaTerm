//! Renderer-facing terminal frame model derived from runtime surface snapshots.

use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::app::ssh::runtime::{TerminalCellState, TerminalCursorShape, TerminalSurfaceState};
use crate::app::terminal_semantic::SemanticSpan;
use crate::theme::{AppThemeSpec, SearchMatchHighlightStrength, SemanticStyleRole};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerminalModelFrame {
    pub session_id: Uuid,
    pub seqno: usize,
    pub grid_rows: u32,
    pub grid_cols: u32,
    pub rows: Vec<TerminalModelRow>,
    pub cursor: TerminalCursorModel,
    pub selection: Option<TerminalSelectionModel>,
    pub palette: TerminalPaletteModel,
    pub viewport_offset_lines: u32,
    pub viewport_max_offset_lines: u32,
    pub viewport_at_bottom: bool,
    pub alternate_screen_active: bool,
    pub mouse_grabbed: bool,
    pub bracketed_paste_enabled: bool,
    pub dirty_rows: Vec<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerminalModelRow {
    pub row_index: u32,
    pub text: String,
    pub wrapped: bool,
    pub cells: Vec<TerminalModelCell>,
    pub content_hash: u64,
    pub row_hash: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerminalModelCell {
    pub row: u32,
    pub col: u32,
    pub width: u32,
    pub text: String,
    pub bold: bool,
    pub underline: bool,
    pub fg_rgba: u32,
    pub bg_rgba: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TerminalCursorModel {
    pub row: u32,
    pub col: u32,
    pub visible: bool,
    pub blinking: bool,
    pub shape: TerminalCursorShape,
    pub fg_rgba: u32,
    pub bg_rgba: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TerminalSelectionModel {
    pub start_row: u32,
    pub start_col: u32,
    pub end_row: u32,
    pub end_col: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TerminalPaletteModel {
    pub default_fg_rgba: u32,
    pub default_bg_rgba: u32,
    pub row_bg_even_rgba: u32,
    pub row_bg_odd_rgba: u32,
}

impl TerminalModelFrame {
    pub fn from_surface(
        surface: &TerminalSurfaceState,
        previous: Option<&TerminalModelFrame>,
    ) -> Self {
        let row_meta = surface
            .visible_rows
            .iter()
            .map(|row| (row.index, (row.text.clone(), row.wrapped)))
            .collect::<HashMap<_, _>>();
        let mut row_cells = HashMap::<u32, Vec<TerminalModelCell>>::new();
        for cell in &surface.cells {
            row_cells
                .entry(cell.row)
                .or_default()
                .push(TerminalModelCell::from_cell(cell));
        }

        let palette = TerminalPaletteModel {
            default_fg_rgba: surface.default_fg_rgba,
            default_bg_rgba: surface.default_bg_rgba,
            row_bg_even_rgba: surface.row_bg_even_rgba,
            row_bg_odd_rgba: surface.row_bg_odd_rgba,
        };

        let mut rows = Vec::with_capacity(surface.rows as usize);
        for row_index in 0..surface.rows {
            let (text, wrapped) = row_meta
                .get(&row_index)
                .cloned()
                .or_else(|| {
                    surface
                        .visible_lines
                        .get(row_index as usize)
                        .cloned()
                        .map(|text| (text, false))
                })
                .unwrap_or_else(|| (String::new(), false));
            let cells = row_cells.remove(&row_index).unwrap_or_default();
            let content_hash = hash_row_content(&text, wrapped, &cells);
            let row_hash = hash_row(row_index, &text, wrapped, &cells, palette);
            rows.push(TerminalModelRow {
                row_index,
                text,
                wrapped,
                cells,
                content_hash,
                row_hash,
            });
        }

        let dirty_rows = rows
            .iter()
            .filter_map(|row| {
                let previous_hash = previous
                    .and_then(|frame| frame.rows.get(row.row_index as usize))
                    .map(|value| value.row_hash);
                if previous_hash == Some(row.row_hash) {
                    None
                } else {
                    Some(row.row_index)
                }
            })
            .collect();

        Self {
            session_id: surface.session_id,
            seqno: surface.seqno,
            grid_rows: surface.rows,
            grid_cols: surface.cols,
            rows,
            cursor: TerminalCursorModel {
                row: surface.cursor.row,
                col: surface.cursor.col,
                visible: surface.cursor.visible,
                blinking: surface.cursor.blinking,
                shape: surface.cursor.shape,
                fg_rgba: surface.cursor.fg_rgba,
                bg_rgba: surface.cursor.bg_rgba,
            },
            selection: None,
            palette,
            viewport_offset_lines: surface.viewport_offset_lines,
            viewport_max_offset_lines: surface.viewport_max_offset_lines,
            viewport_at_bottom: surface.viewport_at_bottom,
            alternate_screen_active: surface.alternate_screen_active,
            mouse_grabbed: surface.mouse_grabbed,
            bracketed_paste_enabled: surface.bracketed_paste_enabled,
            dirty_rows,
        }
    }

    pub fn apply_semantic_style_overlays(
        &mut self,
        previous: Option<&TerminalModelFrame>,
        theme: AppThemeSpec,
        spans: &[SemanticSpan],
        search_match_highlight: SearchMatchHighlightStrength,
    ) {
        let mut spans_by_row = HashMap::<u32, Vec<&SemanticSpan>>::new();
        for span in spans {
            spans_by_row.entry(span.row).or_default().push(span);
        }

        for row in &mut self.rows {
            let Some(row_spans) = spans_by_row.get(&row.row_index) else {
                continue;
            };

            for cell in &mut row.cells {
                if cell.width == 0 || cell.text.is_empty() {
                    continue;
                }

                let mut desired_fg = None::<(u8, u32)>;
                let mut desired_bg = None::<(u8, u32)>;
                let mut underline = cell.underline;
                let mut bold = cell.bold;
                let mut search_background = None::<u32>;

                for span in row_spans {
                    if !cell_overlaps_semantic_span(cell, span) {
                        continue;
                    }

                    if span.role == SemanticStyleRole::OutputGrepMatch {
                        search_background = Some(blend_search_match_background(
                            cell.bg_rgba,
                            theme.terminal.search_match.rgb,
                            search_match_alpha(theme, search_match_highlight),
                        ));
                        continue;
                    }

                    let style = theme.semantic_style(span.role);
                    let priority = semantic_role_priority(span.role);
                    if style.underline {
                        underline = true;
                    }
                    if style.bold {
                        bold = true;
                    }

                    if cell.fg_rgba == self.palette.default_fg_rgba
                        && desired_fg
                            .as_ref()
                            .is_none_or(|(current_priority, _)| priority >= *current_priority)
                    {
                        desired_fg = Some((priority, 0xff00_0000 | style.foreground));
                    }
                    if let Some(background) = style.background
                        && cell.bg_rgba == self.palette.default_bg_rgba
                        && desired_bg
                            .as_ref()
                            .is_none_or(|(current_priority, _)| priority >= *current_priority)
                    {
                        desired_bg = Some((priority, 0xff00_0000 | background));
                    }
                }

                if let Some((_, foreground)) = desired_fg {
                    cell.fg_rgba = foreground;
                }
                if let Some((_, background)) = desired_bg {
                    cell.bg_rgba = background;
                }
                if let Some(background) = search_background {
                    cell.bg_rgba = background;
                }
                cell.underline = underline;
                cell.bold = bold;
            }
        }

        self.recompute_row_hashes(previous);
    }

    fn recompute_row_hashes(&mut self, previous: Option<&TerminalModelFrame>) {
        self.dirty_rows = self
            .rows
            .iter_mut()
            .filter_map(|row| {
                row.content_hash = hash_row_content(&row.text, row.wrapped, &row.cells);
                row.row_hash = hash_row(
                    row.row_index,
                    &row.text,
                    row.wrapped,
                    &row.cells,
                    self.palette,
                );
                let previous_hash = previous
                    .and_then(|frame| frame.rows.get(row.row_index as usize))
                    .map(|value| value.row_hash);
                if previous_hash == Some(row.row_hash) {
                    None
                } else {
                    Some(row.row_index)
                }
            })
            .collect();
    }
}

impl TerminalModelCell {
    fn from_cell(cell: &TerminalCellState) -> Self {
        Self {
            row: cell.row,
            col: cell.col,
            width: cell.width,
            text: cell.text.clone(),
            bold: cell.bold,
            underline: cell.underline,
            fg_rgba: cell.fg_rgba,
            bg_rgba: cell.bg_rgba,
        }
    }
}

fn hash_row(
    row_index: u32,
    text: &str,
    wrapped: bool,
    cells: &[TerminalModelCell],
    palette: TerminalPaletteModel,
) -> u64 {
    let mut hasher = DefaultHasher::new();
    row_index.hash(&mut hasher);
    palette.default_fg_rgba.hash(&mut hasher);
    palette.default_bg_rgba.hash(&mut hasher);
    palette.row_bg_even_rgba.hash(&mut hasher);
    palette.row_bg_odd_rgba.hash(&mut hasher);
    hash_row_content_into(&mut hasher, text, wrapped, cells);
    hasher.finish()
}

fn hash_row_content(text: &str, wrapped: bool, cells: &[TerminalModelCell]) -> u64 {
    let mut hasher = DefaultHasher::new();
    hash_row_content_into(&mut hasher, text, wrapped, cells);
    hasher.finish()
}

fn hash_row_content_into(
    hasher: &mut DefaultHasher,
    text: &str,
    wrapped: bool,
    cells: &[TerminalModelCell],
) {
    text.hash(hasher);
    wrapped.hash(hasher);
    for cell in cells {
        cell.col.hash(hasher);
        cell.width.hash(hasher);
        cell.text.hash(hasher);
        cell.bold.hash(hasher);
        cell.underline.hash(hasher);
        cell.fg_rgba.hash(hasher);
        cell.bg_rgba.hash(hasher);
    }
}

fn cell_overlaps_semantic_span(cell: &TerminalModelCell, span: &SemanticSpan) -> bool {
    let cell_end = cell.col.saturating_add(cell.width.saturating_sub(1));
    cell.col <= span.end_col && cell_end >= span.start_col
}

fn semantic_role_priority(role: SemanticStyleRole) -> u8 {
    match role {
        SemanticStyleRole::OutputDiffAdded
        | SemanticStyleRole::OutputDiffRemoved
        | SemanticStyleRole::OutputDiffHunk => 10,
        SemanticStyleRole::InputArgument
        | SemanticStyleRole::InputOption
        | SemanticStyleRole::InputPath
        | SemanticStyleRole::InputVariable
        | SemanticStyleRole::InputOperator
        | SemanticStyleRole::OutputUnixPath
        | SemanticStyleRole::OutputWindowsPath
        | SemanticStyleRole::OutputNetworkEndpoint
        | SemanticStyleRole::OutputTimestamp
        | SemanticStyleRole::OutputJsonKey
        | SemanticStyleRole::OutputJsonString
        | SemanticStyleRole::OutputJsonNumber
        | SemanticStyleRole::OutputJsonBoolean => 20,
        SemanticStyleRole::InputPrompt
        | SemanticStyleRole::InputCommand
        | SemanticStyleRole::InputSubcommand
        | SemanticStyleRole::InputString
        | SemanticStyleRole::InputInvalidCommand
        | SemanticStyleRole::OutputSeverityError
        | SemanticStyleRole::OutputSeverityWarning
        | SemanticStyleRole::OutputSeverityInfo
        | SemanticStyleRole::OutputSeverityDebug
        | SemanticStyleRole::OutputSuccessKeyword
        | SemanticStyleRole::OutputFailureKeyword
        | SemanticStyleRole::OutputGrepMatch
        | SemanticStyleRole::CommandStatusRunning
        | SemanticStyleRole::CommandStatusSuccess
        | SemanticStyleRole::CommandStatusFailure => 30,
        SemanticStyleRole::OutputUrl | SemanticStyleRole::OutputLineReference => 40,
    }
}

fn search_match_alpha(theme: AppThemeSpec, strength: SearchMatchHighlightStrength) -> f32 {
    let base = theme.terminal.search_match.alpha;
    match strength {
        SearchMatchHighlightStrength::Subtle => (base * 0.72).clamp(0.08, 0.22),
        SearchMatchHighlightStrength::Balanced => base.clamp(0.12, 0.34),
        SearchMatchHighlightStrength::Strong => (base * 1.35).clamp(0.18, 0.46),
    }
}

fn blend_search_match_background(base_rgba: u32, overlay_rgb: u32, alpha: f32) -> u32 {
    let alpha = alpha.clamp(0.0, 1.0);
    let base_red = ((base_rgba >> 16) & 0xff) as f32;
    let base_green = ((base_rgba >> 8) & 0xff) as f32;
    let base_blue = (base_rgba & 0xff) as f32;
    let overlay_red = ((overlay_rgb >> 16) & 0xff) as f32;
    let overlay_green = ((overlay_rgb >> 8) & 0xff) as f32;
    let overlay_blue = (overlay_rgb & 0xff) as f32;

    let mix = |base: f32, overlay: f32| -> u32 {
        ((base * (1.0 - alpha)) + (overlay * alpha)).round() as u32
    };

    0xff00_0000
        | (mix(base_red, overlay_red) << 16)
        | (mix(base_green, overlay_green) << 8)
        | mix(base_blue, overlay_blue)
}

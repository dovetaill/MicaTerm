//! Renderer-facing terminal frame model derived from runtime surface snapshots.

use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::app::ssh::runtime::{TerminalCellState, TerminalCursorShape, TerminalSurfaceState};
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

    pub fn refresh_row_hashes(&mut self) {
        let palette = self.palette;
        for row in &mut self.rows {
            row.content_hash = hash_row_content(&row.text, row.wrapped, &row.cells);
            row.row_hash = hash_row(row.row_index, &row.text, row.wrapped, &row.cells, palette);
        }
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

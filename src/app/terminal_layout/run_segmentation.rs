//! Run segmentation from terminal cells into shapeable text runs.

use std::ops::Range;

use unicode_segmentation::UnicodeSegmentation;

use crate::app::terminal_model::{TerminalModelCell, TerminalModelRow};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TextStyleKey {
    pub fg_rgba: u32,
    pub bg_rgba: u32,
    pub bold: bool,
    pub underline: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunCluster {
    pub text: String,
    pub cell_range: Range<u32>,
    pub byte_range: Range<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SegmentedRun {
    pub row: u32,
    pub cell_range: Range<u32>,
    pub text: String,
    pub style: TextStyleKey,
    pub clusters: Vec<RunCluster>,
}

pub fn segment_row(row: &TerminalModelRow) -> Vec<SegmentedRun> {
    if row.cells.is_empty() {
        return Vec::new();
    }

    let mut ordered_cells = row.cells.clone();
    ordered_cells.sort_by_key(|cell| cell.col);

    let mut runs = Vec::new();
    let mut current_cells = Vec::<TerminalModelCell>::new();
    let mut current_style = TextStyleKey {
        fg_rgba: ordered_cells[0].fg_rgba,
        bg_rgba: ordered_cells[0].bg_rgba,
        bold: ordered_cells[0].bold,
        underline: ordered_cells[0].underline,
    };
    let mut expected_next_col = ordered_cells[0].col;
    for cell in ordered_cells {
        let next_style = TextStyleKey {
            fg_rgba: cell.fg_rgba,
            bg_rgba: cell.bg_rgba,
            bold: cell.bold,
            underline: cell.underline,
        };
        let style_changed = next_style != current_style;
        let gap_detected = !current_cells.is_empty() && cell.col != expected_next_col;
        if (style_changed || gap_detected) && !current_cells.is_empty() {
            runs.push(build_run(row.row_index, current_style, &current_cells));
            current_cells.clear();
        }
        current_style = next_style;
        expected_next_col = cell.col.saturating_add(cell.width);
        current_cells.push(cell);
    }

    if !current_cells.is_empty() {
        runs.push(build_run(row.row_index, current_style, &current_cells));
    }

    runs
}

fn build_run(row_index: u32, style: TextStyleKey, cells: &[TerminalModelCell]) -> SegmentedRun {
    let start_col = cells.first().map(|cell| cell.col).unwrap_or(0);
    let end_col = cells
        .last()
        .map(|cell| cell.col.saturating_add(cell.width))
        .unwrap_or(start_col);
    let mut text = String::new();
    let mut clusters = Vec::new();

    for cell in cells {
        let cluster_start = text.len();
        let mut last_end = cluster_start;
        for (relative_start, grapheme) in cell.text.grapheme_indices(true) {
            let absolute_start = cluster_start + relative_start;
            let absolute_end = absolute_start + grapheme.len();
            clusters.push(RunCluster {
                text: grapheme.to_string(),
                cell_range: cell.col..cell.col.saturating_add(cell.width),
                byte_range: absolute_start..absolute_end,
            });
            last_end = absolute_end;
        }
        if cluster_start == last_end {
            clusters.push(RunCluster {
                text: cell.text.clone(),
                cell_range: cell.col..cell.col.saturating_add(cell.width),
                byte_range: cluster_start..cluster_start + cell.text.len(),
            });
        }
        text.push_str(&cell.text);
    }

    SegmentedRun {
        row: row_index,
        cell_range: start_col..end_col,
        text,
        style,
        clusters,
    }
}

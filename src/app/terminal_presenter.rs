//! Terminal presentation seam that decouples bootstrap/UI projection from concrete renderers.

use anyhow::Result;

use crate::app::ssh::runtime::{TerminalCursorShape, TerminalSurfaceState};
use crate::app::terminal_atlas::TerminalAtlasSelection;
#[cfg(feature = "terminal-native-renderer")]
use crate::app::terminal_font::{DirectWriteFontSystem, FontRequest, FontSystem, LoadedFont};
#[cfg(feature = "terminal-native-renderer")]
use crate::app::terminal_layout::{TerminalTextShaper, TextShaper};
use crate::app::terminal_model::TerminalModelFrame;
#[cfg(feature = "terminal-native-renderer")]
use crate::app::terminal_renderer::wgpu_renderer::{
    PreparedBackgroundRun, PreparedColorGlyphDraw, PreparedMonochromeGlyphDraw,
    PreparedUnderlineRun,
};
#[cfg(feature = "terminal-native-renderer")]
use crate::app::terminal_renderer::{ShapedTerminalFrame, WgpuTerminalRenderer};
use crate::app::terminal_semantic::{
    SemanticInputOverlay, SemanticOutputOverlay, detect_input_line_overlays,
    detect_output_block_overlays,
};

#[derive(Clone, Debug)]
pub enum PresentedTerminalFrame {
    Native(Box<NativeTerminalFrame>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeCursorFrameState {
    pub row: u32,
    pub col: u32,
    pub visible: bool,
    pub blinking: bool,
    pub shape: TerminalCursorShape,
    pub fg_rgba: u32,
    pub bg_rgba: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NativeSelectionFrameState {
    pub active: bool,
    pub start_row: u32,
    pub start_col: u32,
    pub end_row: u32,
    pub end_col: u32,
    pub overlay_rgba: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeCursorOverlay {
    pub visible: bool,
    pub row: u32,
    pub col: u32,
    pub cell_width_px: u32,
    pub cell_height_px: u32,
    pub shape: TerminalCursorShape,
    pub fg_rgba: u32,
    pub bg_rgba: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeSelectionRect {
    pub row: u32,
    pub start_col: u32,
    pub end_col: u32,
    pub overlay_rgba: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NativeSelectionOverlay {
    pub active: bool,
    pub rect_count: usize,
    pub rects: Vec<NativeSelectionRect>,
    pub start_row: u32,
    pub start_col: u32,
    pub end_row: u32,
    pub end_col: u32,
    pub overlay_rgba: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeUnderlineRun {
    pub row: u32,
    pub start_col: u32,
    pub end_col: u32,
    pub fg_rgba: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NativeUnderlineOverlay {
    pub visible: bool,
    pub run_count: usize,
    pub runs: Vec<NativeUnderlineRun>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NativeImePreviewOverlay {
    pub active: bool,
    pub row: u32,
    pub start_col: u32,
    pub end_col: u32,
    pub cursor_col: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeRendererFrameStats {
    pub glyph_cache_entries: usize,
    pub mono_glyph_cache_entries: usize,
    pub color_glyph_cache_entries: usize,
    pub monochrome_glyphs_prepared: usize,
    pub color_glyphs_prepared: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PresentableNativeFrame {
    pub seqno: u64,
    pub shaped_row_count: usize,
    pub glyph_run_count: usize,
    pub glyph_count: usize,
    pub dirty_row_count: usize,
    pub default_fg_rgba: u32,
    pub default_bg_rgba: u32,
    pub row_bg_even_rgba: u32,
    pub row_bg_odd_rgba: u32,
    pub grid_rows: u32,
    pub grid_cols: u32,
    pub background_runs: Vec<PreparedBackgroundRun>,
    pub monochrome_glyph_draws: Vec<PreparedMonochromeGlyphDraw>,
    pub color_glyph_draws: Vec<PreparedColorGlyphDraw>,
    pub underline_run_count: usize,
    pub cursor: NativeCursorFrameState,
    pub cursor_overlay: NativeCursorOverlay,
    pub selection: NativeSelectionFrameState,
    pub selection_overlay: NativeSelectionOverlay,
    pub underline_overlay: NativeUnderlineOverlay,
    pub semantic_overlays: Vec<SemanticOutputOverlay>,
    pub semantic_input_overlays: Vec<SemanticInputOverlay>,
    pub ime_preview_overlay: NativeImePreviewOverlay,
    pub renderer_stats: NativeRendererFrameStats,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Retained display-list payload consumed by platform surface backends.
pub struct NativeTerminalFrame {
    pub frame_token: u64,
    pub cell_width_px: u32,
    pub cell_height_px: u32,
    pub presentable_frame: PresentableNativeFrame,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TerminalPresentationOptions {
    pub selection: Option<TerminalAtlasSelection>,
    pub selection_overlay_rgba: u32,
    pub ime_preview_overlay: NativeImePreviewOverlay,
}

pub trait TerminalPresenter {
    fn present(
        &mut self,
        surface: &TerminalSurfaceState,
        options: TerminalPresentationOptions,
    ) -> Result<PresentedTerminalFrame>;

    fn default_cell_size(&self) -> (u32, u32);
}

#[cfg(feature = "terminal-native-renderer")]
pub struct WindowsNativePresenter {
    font_system: DirectWriteFontSystem,
    shaper: TerminalTextShaper,
    renderer: WgpuTerminalRenderer,
    loaded_font: LoadedFont,
    previous_frame: Option<TerminalModelFrame>,
}

#[cfg(feature = "terminal-native-renderer")]
impl WindowsNativePresenter {
    pub fn new() -> Result<Self> {
        let request = FontRequest::default();
        let mut font_system = DirectWriteFontSystem::new()?;
        let loaded_font = font_system.load_font(&request)?;

        Ok(Self {
            font_system,
            shaper: TerminalTextShaper,
            renderer: WgpuTerminalRenderer::new(),
            loaded_font,
            previous_frame: None,
        })
    }
}

#[cfg(feature = "terminal-native-renderer")]
impl TerminalPresenter for WindowsNativePresenter {
    fn present(
        &mut self,
        surface: &TerminalSurfaceState,
        options: TerminalPresentationOptions,
    ) -> Result<PresentedTerminalFrame> {
        let frame_model = TerminalModelFrame::from_surface(surface, self.previous_frame.as_ref());
        let rows = frame_model
            .rows
            .iter()
            .map(|row| {
                self.shaper
                    .shape_row(row, &self.loaded_font, &mut self.font_system)
            })
            .collect::<Result<Vec<_>>>()?;
        let prepared = self.renderer.prepare(
            &ShapedTerminalFrame {
                seqno: frame_model.seqno as u64,
                font: self.loaded_font.clone(),
                rows,
            },
            &mut self.font_system,
        )?;
        let selection = options.selection;
        let selection_state = match selection {
            Some(selection) => NativeSelectionFrameState {
                active: true,
                start_row: selection.start_row,
                start_col: selection.start_col,
                end_row: selection.end_row,
                end_col: selection.end_col,
                overlay_rgba: options.selection_overlay_rgba,
            },
            None => NativeSelectionFrameState::default(),
        };
        let selection_overlay = match selection {
            Some(selection) => {
                let rects = selection_overlay_rects(
                    selection,
                    frame_model.grid_cols,
                    options.selection_overlay_rgba,
                );
                NativeSelectionOverlay {
                    active: true,
                    rect_count: rects.len(),
                    rects,
                    start_row: selection.start_row,
                    start_col: selection.start_col,
                    end_row: selection.end_row,
                    end_col: selection.end_col,
                    overlay_rgba: options.selection_overlay_rgba,
                }
            }
            None => NativeSelectionOverlay::default(),
        };
        let semantic_overlays = detect_output_block_overlays(&frame_model);
        let semantic_input_overlays = detect_input_line_overlays(&frame_model);
        let cursor = NativeCursorFrameState {
            row: frame_model.cursor.row,
            col: frame_model.cursor.col,
            visible: frame_model.cursor.visible,
            blinking: frame_model.cursor.blinking,
            shape: frame_model.cursor.shape,
            fg_rgba: frame_model.cursor.fg_rgba,
            bg_rgba: frame_model.cursor.bg_rgba,
        };
        let cursor_overlay = NativeCursorOverlay {
            visible: cursor.visible,
            row: cursor.row,
            col: cursor.col,
            cell_width_px: prepared.cell_width_px,
            cell_height_px: prepared.cell_height_px,
            shape: cursor.shape,
            fg_rgba: cursor.fg_rgba,
            bg_rgba: cursor.bg_rgba,
        };
        let presentable_frame = PresentableNativeFrame {
            seqno: frame_model.seqno as u64,
            shaped_row_count: prepared.shaped_row_count,
            glyph_run_count: prepared.glyph_run_count,
            glyph_count: prepared.glyph_count,
            dirty_row_count: frame_model.dirty_rows.len(),
            default_fg_rgba: frame_model.palette.default_fg_rgba,
            default_bg_rgba: frame_model.palette.default_bg_rgba,
            row_bg_even_rgba: frame_model.palette.row_bg_even_rgba,
            row_bg_odd_rgba: frame_model.palette.row_bg_odd_rgba,
            grid_rows: frame_model.grid_rows,
            grid_cols: frame_model.grid_cols,
            background_runs: prepared.background_runs.clone(),
            monochrome_glyph_draws: prepared.monochrome_glyph_draws.clone(),
            color_glyph_draws: prepared.color_glyph_draws.clone(),
            underline_run_count: prepared.underline_run_count,
            cursor,
            cursor_overlay,
            selection: selection_state,
            selection_overlay,
            underline_overlay: NativeUnderlineOverlay {
                visible: prepared.underline_overlay.visible,
                run_count: prepared.underline_overlay.run_count,
                runs: prepared
                    .underline_overlay
                    .runs
                    .iter()
                    .copied()
                    .map(NativeUnderlineRun::from)
                    .collect(),
            },
            semantic_overlays,
            semantic_input_overlays,
            ime_preview_overlay: options.ime_preview_overlay,
            renderer_stats: NativeRendererFrameStats {
                glyph_cache_entries: prepared.renderer_stats.glyph_cache_entries,
                mono_glyph_cache_entries: prepared.renderer_stats.mono_glyph_cache_entries,
                color_glyph_cache_entries: prepared.renderer_stats.color_glyph_cache_entries,
                monochrome_glyphs_prepared: prepared.renderer_stats.monochrome_glyphs_prepared,
                color_glyphs_prepared: prepared.renderer_stats.color_glyphs_prepared,
            },
        };
        self.previous_frame = Some(frame_model);

        Ok(PresentedTerminalFrame::Native(Box::new(
            NativeTerminalFrame {
                frame_token: prepared.frame_token,
                cell_width_px: prepared.cell_width_px,
                cell_height_px: prepared.cell_height_px,
                presentable_frame,
            },
        )))
    }

    fn default_cell_size(&self) -> (u32, u32) {
        self.loaded_font.cell_size_px()
    }
}

#[cfg(feature = "terminal-native-renderer")]
impl From<PreparedUnderlineRun> for NativeUnderlineRun {
    fn from(run: PreparedUnderlineRun) -> Self {
        Self {
            row: run.row,
            start_col: run.start_col,
            end_col: run.end_col,
            fg_rgba: run.fg_rgba,
        }
    }
}

fn selection_overlay_rects(
    selection: TerminalAtlasSelection,
    cols: u32,
    overlay_rgba: u32,
) -> Vec<NativeSelectionRect> {
    if cols == 0 {
        return Vec::new();
    }

    (selection.start_row..=selection.end_row)
        .map(|row| {
            let start_col = if row == selection.start_row {
                selection.start_col
            } else {
                0
            };
            let end_col = if row == selection.end_row {
                selection.end_col
            } else {
                cols.saturating_sub(1)
            };

            NativeSelectionRect {
                row,
                start_col: start_col.min(cols.saturating_sub(1)),
                end_col: end_col.min(cols.saturating_sub(1)),
                overlay_rgba,
            }
        })
        .collect()
}

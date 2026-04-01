//! Terminal presentation seam that decouples bootstrap/UI projection from concrete renderers.

use anyhow::Result;
use slint::Image;

use crate::app::ssh::runtime::{TerminalCursorShape, TerminalSurfaceState};
use crate::app::terminal_atlas::{TerminalAtlasRenderer, TerminalAtlasSelection};
#[cfg(feature = "terminal-native-renderer")]
use crate::app::terminal_font::{DirectWriteFontSystem, FontRequest, FontSystem, LoadedFont};
#[cfg(feature = "terminal-native-renderer")]
use crate::app::terminal_layout::{TerminalTextShaper, TextShaper};
use crate::app::terminal_model::TerminalModelFrame;
#[cfg(feature = "terminal-native-renderer")]
use crate::app::terminal_renderer::{ShapedTerminalFrame, WgpuTerminalRenderer};

#[derive(Clone, Debug)]
pub enum PresentedTerminalFrame {
    Bitmap(BitmapTerminalFrame),
    Native(NativeTerminalFrame),
}

#[derive(Clone, Debug)]
pub struct BitmapTerminalFrame {
    pub image: Image,
    pub cell_width_px: u32,
    pub cell_height_px: u32,
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NativeSelectionOverlay {
    pub active: bool,
    pub rect_count: usize,
    pub start_row: u32,
    pub start_col: u32,
    pub end_row: u32,
    pub end_col: u32,
    pub overlay_rgba: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NativeUnderlineOverlay {
    pub visible: bool,
    pub run_count: usize,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PresentableNativeFrame {
    pub seqno: u64,
    pub shaped_row_count: usize,
    pub glyph_run_count: usize,
    pub glyph_count: usize,
    pub dirty_row_count: usize,
    pub underline_run_count: usize,
    pub cursor: NativeCursorFrameState,
    pub selection: NativeSelectionFrameState,
    pub selection_overlay: NativeSelectionOverlay,
    pub underline_overlay: NativeUnderlineOverlay,
    pub ime_preview_overlay: NativeImePreviewOverlay,
    pub renderer_stats: NativeRendererFrameStats,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
    fn set_raster_scale(&mut self, _scale_factor: f32) {}

    fn present(
        &mut self,
        surface: &TerminalSurfaceState,
        options: TerminalPresentationOptions,
    ) -> Result<PresentedTerminalFrame>;

    fn default_cell_size(&self) -> (u32, u32);
}

pub struct BitmapAtlasPresenter {
    renderer: TerminalAtlasRenderer,
    previous_frame: Option<TerminalModelFrame>,
}

impl BitmapAtlasPresenter {
    pub fn new() -> Result<Self> {
        Ok(Self {
            renderer: TerminalAtlasRenderer::new()?,
            previous_frame: None,
        })
    }
}

impl TerminalPresenter for BitmapAtlasPresenter {
    fn set_raster_scale(&mut self, scale_factor: f32) {
        self.renderer.set_raster_scale(scale_factor);
    }

    fn present(
        &mut self,
        surface: &TerminalSurfaceState,
        options: TerminalPresentationOptions,
    ) -> Result<PresentedTerminalFrame> {
        let frame_model = TerminalModelFrame::from_surface(surface, self.previous_frame.as_ref());
        let atlas_surface = model_frame_to_surface(&frame_model);
        let frame = self.renderer.render_with_selection(
            &atlas_surface,
            options.selection,
            options.selection_overlay_rgba,
        )?;
        self.previous_frame = Some(frame_model);

        Ok(PresentedTerminalFrame::Bitmap(BitmapTerminalFrame {
            image: frame.image,
            cell_width_px: frame.metrics.cell_width,
            cell_height_px: frame.metrics.cell_height,
        }))
    }

    fn default_cell_size(&self) -> (u32, u32) {
        let metrics = self.renderer.metrics();
        (metrics.cell_width, metrics.cell_height)
    }
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
            Some(selection) => NativeSelectionOverlay {
                active: true,
                rect_count: selection_overlay_rect_count(selection),
                start_row: selection.start_row,
                start_col: selection.start_col,
                end_row: selection.end_row,
                end_col: selection.end_col,
                overlay_rgba: options.selection_overlay_rgba,
            },
            None => NativeSelectionOverlay::default(),
        };
        let presentable_frame = PresentableNativeFrame {
            seqno: frame_model.seqno as u64,
            shaped_row_count: prepared.shaped_row_count,
            glyph_run_count: prepared.glyph_run_count,
            glyph_count: prepared.glyph_count,
            dirty_row_count: frame_model.dirty_rows.len(),
            underline_run_count: prepared.underline_run_count,
            cursor: NativeCursorFrameState {
                row: frame_model.cursor.row,
                col: frame_model.cursor.col,
                visible: frame_model.cursor.visible,
                blinking: frame_model.cursor.blinking,
                shape: frame_model.cursor.shape,
                fg_rgba: frame_model.cursor.fg_rgba,
                bg_rgba: frame_model.cursor.bg_rgba,
            },
            selection: selection_state,
            selection_overlay,
            underline_overlay: NativeUnderlineOverlay {
                visible: prepared.underline_overlay.visible,
                run_count: prepared.underline_overlay.run_count,
            },
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

        Ok(PresentedTerminalFrame::Native(NativeTerminalFrame {
            frame_token: prepared.frame_token,
            cell_width_px: prepared.cell_width_px,
            cell_height_px: prepared.cell_height_px,
            presentable_frame,
        }))
    }

    fn default_cell_size(&self) -> (u32, u32) {
        self.loaded_font.cell_size_px()
    }
}

#[cfg(feature = "terminal-native-renderer")]
fn selection_overlay_rect_count(selection: TerminalAtlasSelection) -> usize {
    usize::try_from(selection.end_row.saturating_sub(selection.start_row))
        .unwrap_or(usize::MAX)
        .saturating_add(1)
}

fn model_frame_to_surface(model: &TerminalModelFrame) -> TerminalSurfaceState {
    TerminalSurfaceState {
        session_id: model.session_id,
        seqno: model.seqno,
        rows: model.grid_rows,
        cols: model.grid_cols,
        default_fg_rgba: model.palette.default_fg_rgba,
        default_bg_rgba: model.palette.default_bg_rgba,
        row_bg_even_rgba: model.palette.row_bg_even_rgba,
        row_bg_odd_rgba: model.palette.row_bg_odd_rgba,
        viewport_offset_lines: model.viewport_offset_lines,
        viewport_max_offset_lines: model.viewport_max_offset_lines,
        viewport_at_bottom: model.viewport_at_bottom,
        visible_rows: model
            .rows
            .iter()
            .map(|row| crate::app::ssh::runtime::TerminalRowState {
                index: row.row_index,
                text: row.text.clone(),
                wrapped: row.wrapped,
            })
            .collect(),
        visible_lines: model.rows.iter().map(|row| row.text.clone()).collect(),
        cells: model
            .rows
            .iter()
            .flat_map(|row| {
                row.cells
                    .iter()
                    .map(|cell| crate::app::ssh::runtime::TerminalCellState {
                        row: cell.row,
                        col: cell.col,
                        width: cell.width,
                        text: cell.text.clone(),
                        bold: cell.bold,
                        underline: cell.underline,
                        fg_rgba: cell.fg_rgba,
                        bg_rgba: cell.bg_rgba,
                    })
            })
            .collect(),
        cursor: crate::app::ssh::runtime::TerminalCursorState {
            row: model.cursor.row,
            col: model.cursor.col,
            visible: model.cursor.visible,
            blinking: model.cursor.blinking,
            shape: model.cursor.shape,
            fg_rgba: model.cursor.fg_rgba,
            bg_rgba: model.cursor.bg_rgba,
        },
        mouse_grabbed: model.mouse_grabbed,
        bracketed_paste_enabled: model.bracketed_paste_enabled,
    }
}

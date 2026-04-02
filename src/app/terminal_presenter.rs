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
use crate::app::terminal_scene_image::SceneImageTerminalRenderer;
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
    Bitmap(BitmapTerminalFrame),
    Native(Box<NativeTerminalFrame>),
}

#[derive(Clone, Debug)]
pub struct BitmapTerminalFrame {
    pub image: Image,
    pub grid_rows: u32,
    pub grid_cols: u32,
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
        let grid_rows = frame_model.grid_rows;
        let grid_cols = frame_model.grid_cols;
        let atlas_surface = model_frame_to_surface(&frame_model);
        let frame = self.renderer.render_with_selection(
            &atlas_surface,
            options.selection,
            options.selection_overlay_rgba,
        )?;
        self.previous_frame = Some(frame_model);

        Ok(PresentedTerminalFrame::Bitmap(BitmapTerminalFrame {
            image: frame.image,
            grid_rows,
            grid_cols,
            cell_width_px: frame.raster_metrics.cell_width,
            cell_height_px: frame.raster_metrics.cell_height,
        }))
    }

    fn default_cell_size(&self) -> (u32, u32) {
        let metrics = self.renderer.raster_metrics();
        (metrics.cell_width, metrics.cell_height)
    }
}

#[cfg(feature = "terminal-native-renderer")]
pub struct WindowsNativePresenter {
    font_system: DirectWriteFontSystem,
    shaper: TerminalTextShaper,
    renderer: WgpuTerminalRenderer,
    base_font_request: FontRequest,
    loaded_font: LoadedFont,
    raster_scale: f32,
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
            base_font_request: request,
            loaded_font,
            raster_scale: 1.0,
            previous_frame: None,
        })
    }

    fn scaled_font_request(&self, scale_factor: f32) -> FontRequest {
        let mut request = self.base_font_request.clone();
        request.px_size = (self.base_font_request.px_size * scale_factor.max(1.0)).max(1.0);
        request
    }

    fn reload_loaded_font_for_scale(&mut self, scale_factor: f32) -> Result<()> {
        let request = self.scaled_font_request(scale_factor);
        self.loaded_font = self.font_system.load_font(&request)?;
        self.previous_frame = None;
        Ok(())
    }
}

#[cfg(feature = "terminal-native-renderer")]
impl TerminalPresenter for WindowsNativePresenter {
    fn set_raster_scale(&mut self, scale_factor: f32) {
        let next_scale = scale_factor.max(1.0);
        if (next_scale - self.raster_scale).abs() < 0.01 {
            return;
        }

        if let Err(err) = self.reload_loaded_font_for_scale(next_scale) {
            tracing::warn!(
                target: "app.terminal",
                error = %err,
                scale_factor = next_scale,
                "failed to reload native terminal font metrics for raster scale change"
            );
            return;
        }

        self.raster_scale = next_scale;
    }

    fn present(
        &mut self,
        surface: &TerminalSurfaceState,
        options: TerminalPresentationOptions,
    ) -> Result<PresentedTerminalFrame> {
        let frame = prepare_native_terminal_frame(
            &mut self.font_system,
            &mut self.shaper,
            &mut self.renderer,
            &self.loaded_font,
            &mut self.previous_frame,
            surface,
            options,
        )?;
        Ok(PresentedTerminalFrame::Native(Box::new(frame)))
    }

    fn default_cell_size(&self) -> (u32, u32) {
        self.loaded_font.cell_size_px()
    }
}

#[cfg(feature = "terminal-native-renderer")]
pub struct WindowsSceneImagePresenter {
    font_system: DirectWriteFontSystem,
    shaper: TerminalTextShaper,
    renderer: WgpuTerminalRenderer,
    scene_renderer: SceneImageTerminalRenderer,
    base_font_request: FontRequest,
    loaded_font: LoadedFont,
    raster_scale: f32,
    previous_frame: Option<TerminalModelFrame>,
}

#[cfg(feature = "terminal-native-renderer")]
impl WindowsSceneImagePresenter {
    pub fn new() -> Result<Self> {
        let request = FontRequest::default();
        let mut font_system = DirectWriteFontSystem::new()?;
        let loaded_font = font_system.load_scene_image_font(&request)?;

        Ok(Self {
            font_system,
            shaper: TerminalTextShaper,
            renderer: WgpuTerminalRenderer::new(),
            scene_renderer: SceneImageTerminalRenderer::default(),
            base_font_request: request,
            loaded_font,
            raster_scale: 1.0,
            previous_frame: None,
        })
    }

    fn scaled_font_request(&self, scale_factor: f32) -> FontRequest {
        let mut request = self.base_font_request.clone();
        request.px_size = (self.base_font_request.px_size * scale_factor.max(1.0)).max(1.0);
        request
    }

    fn reload_loaded_font_for_scale(&mut self, scale_factor: f32) -> Result<()> {
        let request = self.scaled_font_request(scale_factor);
        self.loaded_font = self.font_system.load_scene_image_font(&request)?;
        self.previous_frame = None;
        self.scene_renderer.clear();
        Ok(())
    }
}

#[cfg(feature = "terminal-native-renderer")]
impl TerminalPresenter for WindowsSceneImagePresenter {
    fn set_raster_scale(&mut self, scale_factor: f32) {
        let next_scale = scale_factor.max(1.0);
        if (next_scale - self.raster_scale).abs() < 0.01 {
            return;
        }

        if let Err(err) = self.reload_loaded_font_for_scale(next_scale) {
            tracing::warn!(
                target: "app.terminal",
                error = %err,
                scale_factor = next_scale,
                "failed to reload scene-image terminal font metrics for raster scale change"
            );
            return;
        }

        self.raster_scale = next_scale;
    }

    fn present(
        &mut self,
        surface: &TerminalSurfaceState,
        options: TerminalPresentationOptions,
    ) -> Result<PresentedTerminalFrame> {
        // software 包必须把终端像素放回 Slint scene，否则 overlay 一定会被整窗 post-pass 盖掉。
        let frame = prepare_native_terminal_frame(
            &mut self.font_system,
            &mut self.shaper,
            &mut self.renderer,
            &self.loaded_font,
            &mut self.previous_frame,
            surface,
            options,
        )?;
        Ok(PresentedTerminalFrame::Bitmap(
            self.scene_renderer.render(&frame)?,
        ))
    }

    fn default_cell_size(&self) -> (u32, u32) {
        self.loaded_font.cell_size_px()
    }
}

#[cfg(feature = "terminal-native-renderer")]
fn prepare_native_terminal_frame(
    font_system: &mut DirectWriteFontSystem,
    shaper: &mut TerminalTextShaper,
    renderer: &mut WgpuTerminalRenderer,
    loaded_font: &LoadedFont,
    previous_frame: &mut Option<TerminalModelFrame>,
    surface: &TerminalSurfaceState,
    options: TerminalPresentationOptions,
) -> Result<NativeTerminalFrame> {
    let frame_model = TerminalModelFrame::from_surface(surface, previous_frame.as_ref());
    let rows = frame_model
        .rows
        .iter()
        .map(|row| shaper.shape_row(row, loaded_font, font_system))
        .collect::<Result<Vec<_>>>()?;
    let prepared = renderer.prepare(
        &ShapedTerminalFrame {
            seqno: frame_model.seqno as u64,
            font: loaded_font.clone(),
            rows,
        },
        font_system,
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
            let rects =
                selection_overlay_rects(selection, frame_model.grid_cols, options.selection_overlay_rgba);
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
    *previous_frame = Some(frame_model);

    Ok(NativeTerminalFrame {
        frame_token: prepared.frame_token,
        cell_width_px: prepared.cell_width_px,
        cell_height_px: prepared.cell_height_px,
        presentable_frame,
    })
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
        alternate_screen_active: model.alternate_screen_active,
        mouse_grabbed: model.mouse_grabbed,
        bracketed_paste_enabled: model.bracketed_paste_enabled,
    }
}

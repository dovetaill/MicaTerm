//! Terminal presentation seam consumed through TerminalRendererHost.

use anyhow::Result;
use slint::Image;

#[cfg(feature = "terminal-native-renderer")]
use std::collections::HashMap;

use crate::app::ssh::runtime::{SurfaceState, TerminalCursorShape};
use crate::app::terminal_atlas::{TerminalAtlasRenderer, TerminalAtlasSelection};
use crate::app::terminal_core::TerminalFrameSnapshot;
#[cfg(feature = "terminal-native-renderer")]
use crate::app::terminal_font::{DirectWriteFontSystem, FontRequest, FontSystem, LoadedFont};
#[cfg(feature = "terminal-native-renderer")]
use crate::app::terminal_layout::{TerminalTextShaper, TextShaper};
#[cfg(feature = "terminal-native-renderer")]
use crate::app::terminal_model::TerminalModelFrame;
#[cfg(feature = "terminal-native-renderer")]
use crate::app::terminal_renderer::wgpu_renderer::{
    PreparedBackgroundRun, PreparedColorGlyphDraw, PreparedMonochromeGlyphDraw,
    PreparedUnderlineRun,
};
#[cfg(feature = "terminal-native-renderer")]
use crate::app::terminal_renderer::{ShapedTerminalFrame, WgpuTerminalRenderer};
#[cfg(feature = "terminal-native-renderer")]
use crate::app::terminal_scene_image::SceneImageTerminalRenderer;
#[cfg(feature = "terminal-native-renderer")]
use crate::app::terminal_semantic::{detect_input_line_overlays, detect_output_block_overlays};
use crate::app::terminal_semantic::{SemanticInputOverlay, SemanticOutputOverlay};

#[allow(dead_code)]
type PresenterFrameSnapshot = TerminalFrameSnapshot;

#[cfg(not(feature = "terminal-native-renderer"))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PreparedBackgroundRun;

#[cfg(not(feature = "terminal-native-renderer"))]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PreparedMonochromeGlyphDraw;

#[cfg(not(feature = "terminal-native-renderer"))]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PreparedColorGlyphDraw;

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
        surface: &SurfaceState,
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
        surface: &SurfaceState,
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
    previous_shaped_rows: Option<Vec<crate::app::terminal_layout::ShapedRow>>,
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
            previous_shaped_rows: None,
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
        self.previous_shaped_rows = None;
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
        surface: &SurfaceState,
        options: TerminalPresentationOptions,
    ) -> Result<PresentedTerminalFrame> {
        let frame = prepare_native_terminal_frame(
            &mut self.font_system,
            &mut self.shaper,
            &mut self.renderer,
            &self.loaded_font,
            &mut self.previous_frame,
            &mut self.previous_shaped_rows,
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
/// Scene-image presenter for Windows software compatibility and the default
/// packaged Windows mainline path while `MICA_TERM_TERMINAL_SUBSYSTEM=retained-native-surface`
/// remains the explicit bring-up switch.
pub struct WindowsSceneImagePresenter {
    font_system: DirectWriteFontSystem,
    shaper: TerminalTextShaper,
    renderer: WgpuTerminalRenderer,
    scene_renderer: SceneImageTerminalRenderer,
    base_font_request: FontRequest,
    loaded_font: LoadedFont,
    raster_scale: f32,
    previous_frame: Option<TerminalModelFrame>,
    previous_shaped_rows: Option<Vec<crate::app::terminal_layout::ShapedRow>>,
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
            previous_shaped_rows: None,
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
        self.previous_shaped_rows = None;
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
        surface: &SurfaceState,
        options: TerminalPresentationOptions,
    ) -> Result<PresentedTerminalFrame> {
        // software 包必须把终端像素放回 Slint scene，否则 overlay 一定会被整窗 post-pass 盖掉。
        let frame = prepare_native_terminal_frame(
            &mut self.font_system,
            &mut self.shaper,
            &mut self.renderer,
            &self.loaded_font,
            &mut self.previous_frame,
            &mut self.previous_shaped_rows,
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
    font_system: &mut dyn FontSystem,
    shaper: &mut TerminalTextShaper,
    renderer: &mut WgpuTerminalRenderer,
    loaded_font: &LoadedFont,
    previous_frame: &mut Option<TerminalModelFrame>,
    previous_shaped_rows: &mut Option<Vec<crate::app::terminal_layout::ShapedRow>>,
    surface: &SurfaceState,
    options: TerminalPresentationOptions,
) -> Result<NativeTerminalFrame> {
    let frame_model = TerminalModelFrame::from_surface(surface, previous_frame.as_ref());
    let rows = shape_rows_with_previous_cache(
        &frame_model,
        previous_frame.as_ref(),
        previous_shaped_rows.as_ref(),
        shaper,
        loaded_font,
        font_system,
    )?;
    let prepared = renderer.prepare(
        &ShapedTerminalFrame {
            seqno: frame_model.seqno as u64,
            font: loaded_font.clone(),
            rows: rows.clone(),
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
    *previous_shaped_rows = Some(rows);
    *previous_frame = Some(frame_model);

    Ok(NativeTerminalFrame {
        frame_token: prepared.frame_token,
        cell_width_px: prepared.cell_width_px,
        cell_height_px: prepared.cell_height_px,
        presentable_frame,
    })
}

#[cfg(feature = "terminal-native-renderer")]
fn shape_rows_with_previous_cache(
    frame_model: &TerminalModelFrame,
    previous_frame: Option<&TerminalModelFrame>,
    previous_shaped_rows: Option<&Vec<crate::app::terminal_layout::ShapedRow>>,
    shaper: &mut TerminalTextShaper,
    loaded_font: &LoadedFont,
    font_system: &mut dyn FontSystem,
) -> Result<Vec<crate::app::terminal_layout::ShapedRow>> {
    let mut previous_row_cache = HashMap::new();
    if let (Some(previous_frame), Some(previous_shaped_rows)) =
        (previous_frame, previous_shaped_rows)
    {
        for (model_row, shaped_row) in previous_frame.rows.iter().zip(previous_shaped_rows.iter()) {
            previous_row_cache
                .entry(model_row.content_hash)
                .or_insert_with(|| shaped_row.clone());
        }
    }

    frame_model
        .rows
        .iter()
        .map(|row| {
            if let Some(cached) = previous_row_cache.get(&row.content_hash) {
                Ok(rebased_shaped_row(cached, row.row_index))
            } else {
                shaper.shape_row(row, loaded_font, font_system)
            }
        })
        .collect()
}

#[cfg(feature = "terminal-native-renderer")]
fn rebased_shaped_row(
    shaped_row: &crate::app::terminal_layout::ShapedRow,
    row_index: u32,
) -> crate::app::terminal_layout::ShapedRow {
    let mut reused = shaped_row.clone();
    reused.row = row_index;
    for run in &mut reused.runs {
        run.row = row_index;
    }
    reused
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

#[cfg(feature = "terminal-native-renderer")]
fn selection_overlay_rects(
    selection: TerminalAtlasSelection,
    cols: u32,
    overlay_rgba: u32,
) -> Vec<NativeSelectionRect> {
    if cols == 0 {
        return Vec::new();
    }

    (selection.start_row..=selection.end_row)
        .filter_map(|row| {
            let start_col = if row == selection.start_row {
                selection.start_col.min(cols)
            } else {
                0
            };
            let end_col_exclusive = if row == selection.end_row {
                selection.end_col.min(cols)
            } else {
                cols
            };
            if start_col >= end_col_exclusive {
                return None;
            }

            Some(NativeSelectionRect {
                row,
                start_col: start_col.min(cols.saturating_sub(1)),
                end_col: end_col_exclusive
                    .saturating_sub(1)
                    .min(cols.saturating_sub(1)),
                overlay_rgba,
            })
        })
        .collect()
}

#[cfg(all(test, feature = "terminal-native-renderer"))]
mod tests {
    use super::*;

    use anyhow::Result;
    use uuid::Uuid;

    use crate::app::ssh::runtime::{TerminalCellState, TerminalRowState};
    use crate::app::terminal_font::mock::MockFontSystem;
    use crate::app::terminal_font::{
        FontFallbackFace, GlyphRasterRequest, RasterizedGlyph, ShapedGlyphRun, TextShapingRequest,
    };

    struct CountingFontSystem {
        inner: MockFontSystem,
        shape_text_runs_calls: usize,
    }

    impl CountingFontSystem {
        fn new() -> Result<Self> {
            Ok(Self {
                inner: MockFontSystem::new()?,
                shape_text_runs_calls: 0,
            })
        }

        fn shape_text_runs_calls(&self) -> usize {
            self.shape_text_runs_calls
        }
    }

    impl FontSystem for CountingFontSystem {
        fn load_font(&mut self, request: &FontRequest) -> Result<LoadedFont> {
            self.inner.load_font(request)
        }

        fn shape_text(
            &mut self,
            font: &LoadedFont,
            text: &str,
        ) -> Result<Vec<crate::app::terminal_font::ShapedGlyph>> {
            self.inner.shape_text(font, text)
        }

        fn rasterize_glyph(
            &mut self,
            font: &LoadedFont,
            request: GlyphRasterRequest,
        ) -> Result<RasterizedGlyph> {
            self.inner.rasterize_glyph(font, request)
        }

        fn discover_fallback_faces(
            &mut self,
            font: &LoadedFont,
            text: &str,
        ) -> Result<Vec<FontFallbackFace>> {
            self.inner.discover_fallback_faces(font, text)
        }

        fn shape_text_runs(
            &mut self,
            font: &LoadedFont,
            request: &TextShapingRequest,
        ) -> Result<Vec<ShapedGlyphRun>> {
            self.shape_text_runs_calls = self.shape_text_runs_calls.saturating_add(1);
            self.inner.shape_text_runs(font, request)
        }
    }

    fn scroll_perf_surface(
        session_id: Uuid,
        seqno: usize,
        viewport_offset_lines: u32,
        lines: [&str; 3],
    ) -> SurfaceState {
        let mut surface = SurfaceState::from_visible_lines(
            session_id,
            seqno,
            3,
            8,
            lines.iter().map(|line| (*line).to_string()).collect(),
        );
        surface.viewport_offset_lines = viewport_offset_lines;
        surface.viewport_max_offset_lines = 12;
        surface.viewport_at_bottom = viewport_offset_lines == 0;
        surface.visible_rows = lines
            .iter()
            .enumerate()
            .map(|(index, text)| TerminalRowState {
                index: index as u32,
                text: (*text).into(),
                wrapped: false,
            })
            .collect();
        surface.cells = lines
            .iter()
            .enumerate()
            .map(|(row, text)| TerminalCellState {
                row: row as u32,
                col: 0,
                width: 1,
                text: (*text).into(),
                bold: false,
                underline: false,
                fg_rgba: match *text {
                    "one" => 0xff11_1111,
                    "two" => 0xff22_2222,
                    "three" => 0xff33_3333,
                    "zero" => 0xff44_4444,
                    _ => 0xff55_5555,
                },
                bg_rgba: 0xff00_0000,
            })
            .collect();
        surface
    }

    #[test]
    fn prepare_native_terminal_frame_reuses_shaped_rows_for_overlapping_scrollback_rows(
    ) -> Result<()> {
        let session_id = Uuid::new_v4();
        let first_surface = scroll_perf_surface(session_id, 1, 0, ["one", "two", "three"]);
        let second_surface = scroll_perf_surface(session_id, 2, 1, ["zero", "one", "two"]);
        let mut font_system = CountingFontSystem::new()?;
        let loaded_font = font_system.load_font(&FontRequest::default())?;
        let mut shaper = TerminalTextShaper;
        let mut renderer = WgpuTerminalRenderer::new_for_test()?;
        let mut previous_frame = None;
        let mut previous_shaped_rows = None;

        prepare_native_terminal_frame(
            &mut font_system,
            &mut shaper,
            &mut renderer,
            &loaded_font,
            &mut previous_frame,
            &mut previous_shaped_rows,
            &first_surface,
            TerminalPresentationOptions::default(),
        )?;
        assert_eq!(
            font_system.shape_text_runs_calls(),
            3,
            "the first viewport should shape all visible rows once"
        );

        prepare_native_terminal_frame(
            &mut font_system,
            &mut shaper,
            &mut renderer,
            &loaded_font,
            &mut previous_frame,
            &mut previous_shaped_rows,
            &second_surface,
            TerminalPresentationOptions::default(),
        )?;
        assert_eq!(
            font_system.shape_text_runs_calls(),
            4,
            "scrolling the viewport by one line should only shape the newly exposed row instead of reshaping all three visible rows"
        );

        Ok(())
    }
}

fn model_frame_to_surface(model: &TerminalModelFrame) -> SurfaceState {
    SurfaceState {
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

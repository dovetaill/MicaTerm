//! Terminal presentation seam that decouples bootstrap/UI projection from concrete renderers.

use anyhow::Result;
use slint::Image;

use crate::app::ssh::runtime::TerminalSurfaceState;
use crate::app::terminal_model::TerminalModelFrame;
use crate::app::terminal_atlas::{TerminalAtlasRenderer, TerminalAtlasSelection};
#[cfg(feature = "terminal-native-renderer")]
use crate::app::terminal_font::{
    DirectWriteFontSystem, FontFaceKey, FontMetrics, FontRequest, FontSystem,
};
#[cfg(feature = "terminal-native-renderer")]
use crate::app::terminal_layout::{HarfBuzzTextShaper, TextShaper};
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
pub struct NativeTerminalFrame {
    pub frame_token: u64,
    pub cell_width_px: u32,
    pub cell_height_px: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TerminalPresentationOptions {
    pub selection: Option<TerminalAtlasSelection>,
    pub selection_overlay_rgba: u32,
}

pub trait TerminalPresenter {
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
    shaper: HarfBuzzTextShaper,
    renderer: WgpuTerminalRenderer,
    request: FontRequest,
    face: FontFaceKey,
    metrics: FontMetrics,
    previous_frame: Option<TerminalModelFrame>,
}

#[cfg(feature = "terminal-native-renderer")]
impl WindowsNativePresenter {
    pub fn new() -> Result<Self> {
        let request = FontRequest::default();
        let mut font_system = DirectWriteFontSystem::new()?;
        let face = font_system.resolve_face(&request)?;
        let metrics = font_system.metrics(face, request.px_size)?;

        Ok(Self {
            font_system,
            shaper: HarfBuzzTextShaper::default(),
            renderer: WgpuTerminalRenderer::new(),
            request,
            face,
            metrics,
            previous_frame: None,
        })
    }
}

#[cfg(feature = "terminal-native-renderer")]
impl TerminalPresenter for WindowsNativePresenter {
    fn present(
        &mut self,
        surface: &TerminalSurfaceState,
        _options: TerminalPresentationOptions,
    ) -> Result<PresentedTerminalFrame> {
        let frame_model = TerminalModelFrame::from_surface(surface, self.previous_frame.as_ref());
        let rows = frame_model
            .rows
            .iter()
            .map(|row| self.shaper.shape_row(row, &mut self.font_system))
            .collect::<Result<Vec<_>>>()?;
        let prepared = self.renderer.prepare(
            &ShapedTerminalFrame {
                seqno: frame_model.seqno as u64,
                face: self.face,
                px_size: self.request.px_size,
                metrics: self.metrics,
                rows,
            },
            &mut self.font_system,
        )?;
        self.previous_frame = Some(frame_model);

        Ok(PresentedTerminalFrame::Native(NativeTerminalFrame {
            frame_token: prepared.frame_token,
            cell_width_px: prepared.cell_width_px,
            cell_height_px: prepared.cell_height_px,
        }))
    }

    fn default_cell_size(&self) -> (u32, u32) {
        (
            self.metrics.cell_width_px.ceil() as u32,
            self.metrics.cell_height_px.ceil() as u32,
        )
    }
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
                row.cells.iter().map(|cell| crate::app::ssh::runtime::TerminalCellState {
                    row: cell.row,
                    col: cell.col,
                    width: cell.width,
                    text: cell.text.clone(),
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

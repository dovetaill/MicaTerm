use anyhow::{Result, bail};
use uuid::Uuid;

use crate::app::ssh::runtime::TerminalSurfaceState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ClipboardInlineImageRequest {
    pub request_id: Uuid,
    pub session_id: Uuid,
    pub active_session_generation: u64,
}

#[derive(Debug, Default)]
pub(crate) struct ClipboardInlineImageController {
    pending: Option<ClipboardInlineImageRequest>,
}

impl ClipboardInlineImageController {
    pub(crate) fn begin(
        &mut self,
        session_id: Uuid,
        active_session_generation: u64,
    ) -> ClipboardInlineImageRequest {
        let request = ClipboardInlineImageRequest {
            request_id: Uuid::new_v4(),
            session_id,
            active_session_generation,
        };
        self.pending = Some(request);
        request
    }

    pub(crate) fn is_current(
        &self,
        request: ClipboardInlineImageRequest,
        active_session_id: Option<Uuid>,
        active_session_generation: u64,
    ) -> bool {
        self.pending == Some(request)
            && active_session_id == Some(request.session_id)
            && active_session_generation == request.active_session_generation
    }

    pub(crate) fn finish_if_current(
        &mut self,
        request: ClipboardInlineImageRequest,
        active_session_id: Option<Uuid>,
        active_session_generation: u64,
    ) -> Option<ClipboardInlineImageRequest> {
        self.is_current(request, active_session_id, active_session_generation)
            .then(|| self.pending.take())
            .flatten()
    }

    pub(crate) fn is_pending(&self, request: ClipboardInlineImageRequest) -> bool {
        self.pending == Some(request)
    }

    pub(crate) fn discard_if_pending(&mut self, request: ClipboardInlineImageRequest) -> bool {
        if !self.is_pending(request) {
            return false;
        }
        self.pending.take();
        true
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LocalInlineImageCellSize {
    pub columns: u32,
    pub rows: u32,
}

pub(crate) fn inline_image_cell_size(
    source_width: u32,
    source_height: u32,
    surface: &TerminalSurfaceState,
) -> Result<LocalInlineImageCellSize> {
    if source_width == 0 || source_height == 0 {
        bail!("clipboard image dimensions must be non-zero");
    }
    if surface.rows == 0 || surface.cols == 0 {
        bail!("terminal grid dimensions must be non-zero");
    }

    let rows = u64::from(surface.rows);
    let cols = u64::from(surface.cols);
    let pixel_width = u64::from(surface.viewport_metrics.pixel_width);
    let pixel_height = u64::from(surface.viewport_metrics.pixel_height);
    let cell_width = (pixel_width / cols).max(1);
    let cell_height = (pixel_height / rows).max(1);
    let cursor_col = u64::from(surface.cursor.col).min(cols.saturating_sub(1));
    let available_columns = cols.saturating_sub(cursor_col).max(1);
    let max_width_px = available_columns
        .checked_mul(cell_width)
        .ok_or_else(|| anyhow::anyhow!("clipboard image width capacity overflowed"))?;
    let max_height_px = (pixel_height / 2).max(1);

    let scale = 1.0_f64
        .min(max_width_px as f64 / f64::from(source_width))
        .min(max_height_px as f64 / f64::from(source_height));
    let scaled_width_px = (f64::from(source_width) * scale).floor().max(1.0) as u64;
    let scaled_height_px = (f64::from(source_height) * scale).floor().max(1.0) as u64;
    let max_rows = ceil_div(max_height_px, cell_height)?.max(1);
    let columns = ceil_div(scaled_width_px, cell_width)?.clamp(1, available_columns);
    let rows = ceil_div(scaled_height_px, cell_height)?.clamp(1, max_rows);

    Ok(LocalInlineImageCellSize {
        columns: u32::try_from(columns)
            .map_err(|_| anyhow::anyhow!("clipboard image column span overflowed"))?,
        rows: u32::try_from(rows)
            .map_err(|_| anyhow::anyhow!("clipboard image row span overflowed"))?,
    })
}

pub(crate) fn surface_allows_inline_image(surface: &TerminalSurfaceState) -> bool {
    !surface.alternate_screen_active && !surface.mouse_grabbed && !surface.application_cursor_keys
}

fn ceil_div(value: u64, divisor: u64) -> Result<u64> {
    if divisor == 0 {
        bail!("clipboard image cell size must be non-zero");
    }
    value
        .checked_add(divisor - 1)
        .map(|adjusted| adjusted / divisor)
        .ok_or_else(|| anyhow::anyhow!("clipboard image cell span overflowed"))
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::{
        ClipboardInlineImageController, inline_image_cell_size, surface_allows_inline_image,
    };
    use crate::app::ssh::runtime::TerminalSurfaceState;
    use crate::app::terminal_core::TerminalViewportMetrics;

    fn sizing_surface(
        rows: u32,
        cols: u32,
        pixel_width: u32,
        pixel_height: u32,
        cursor_col: u32,
    ) -> TerminalSurfaceState {
        let mut surface =
            TerminalSurfaceState::from_visible_lines(Uuid::new_v4(), 1, rows, cols, Vec::new());
        surface.viewport_metrics = TerminalViewportMetrics {
            pixel_width,
            pixel_height,
            dpi: 96,
        };
        surface.cursor.col = cursor_col;
        surface
    }

    #[test]
    fn active_session_generation_change_invalidates_pending_inline_result() {
        let session_a = Uuid::new_v4();
        let mut controller = ClipboardInlineImageController::default();
        let request = controller.begin(session_a, 7);

        assert!(!controller.is_current(request, Some(session_a), 9));
    }

    #[test]
    fn newer_inline_request_invalidates_older_result() {
        let session_a = Uuid::new_v4();
        let mut controller = ClipboardInlineImageController::default();
        let first = controller.begin(session_a, 7);
        let second = controller.begin(session_a, 7);

        assert!(!controller.is_current(first, Some(session_a), 7));
        assert!(controller.is_current(second, Some(session_a), 7));
        assert_eq!(
            controller.finish_if_current(second, Some(session_a), 7),
            Some(second)
        );
        assert!(!controller.is_current(second, Some(session_a), 7));
    }

    #[test]
    fn inline_image_sizing_does_not_upscale_and_rounds_to_cells() {
        let surface = sizing_surface(10, 10, 100, 100, 0);

        let size = inline_image_cell_size(40, 20, &surface).expect("size image");

        assert_eq!(size.columns, 4);
        assert_eq!(size.rows, 2);
    }

    #[test]
    fn inline_image_sizing_uses_cursor_right_width_and_half_viewport_height() {
        let near_edge = sizing_surface(10, 10, 100, 100, 8);
        assert_eq!(
            inline_image_cell_size(40, 20, &near_edge).expect("fit near edge"),
            super::LocalInlineImageCellSize {
                columns: 2,
                rows: 1,
            }
        );

        let height_limited = sizing_surface(10, 10, 100, 100, 0);
        assert_eq!(
            inline_image_cell_size(100, 200, &height_limited).expect("fit height"),
            super::LocalInlineImageCellSize {
                columns: 3,
                rows: 5,
            }
        );
    }

    #[test]
    fn inline_image_sizing_rejects_invalid_dimensions_and_handles_extremes() {
        let surface = sizing_surface(10, 10, 100, 100, 9);
        assert!(inline_image_cell_size(0, 20, &surface).is_err());
        assert!(inline_image_cell_size(20, 0, &surface).is_err());

        let empty_grid = sizing_surface(0, 10, 100, 100, 0);
        assert!(inline_image_cell_size(20, 20, &empty_grid).is_err());

        let extreme = sizing_surface(1, u32::MAX, u32::MAX, u32::MAX, 0);
        let size = inline_image_cell_size(u32::MAX, u32::MAX, &extreme)
            .expect("extreme dimensions remain bounded");
        assert!(size.columns >= 1);
        assert_eq!(size.rows, 1);
    }

    #[test]
    fn inline_image_guard_rejects_all_interactive_tui_modes() {
        let mut surface = sizing_surface(10, 10, 100, 100, 0);
        assert!(surface_allows_inline_image(&surface));

        surface.alternate_screen_active = true;
        assert!(!surface_allows_inline_image(&surface));
        surface.alternate_screen_active = false;
        surface.mouse_grabbed = true;
        assert!(!surface_allows_inline_image(&surface));
        surface.mouse_grabbed = false;
        surface.application_cursor_keys = true;
        assert!(!surface_allows_inline_image(&surface));
    }
}

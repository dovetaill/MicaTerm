use mica_term::app::terminal_presenter::{
    NativeCursorFrameState, NativeCursorOverlay, NativeImePreviewOverlay, NativeRendererFrameStats,
    NativeSelectionFrameState, NativeSelectionOverlay, NativeTerminalFrame,
    NativeUnderlineOverlay, PresentableNativeFrame,
};
use mica_term::app::terminal_renderer::{
    NativeFrameDamageTracker, NativeSurfaceDamageKind, NativeTerminalSurfaceRect,
    RetainedNativeTerminalSurfaceFrame,
};
use mica_term::app::ssh::runtime::TerminalCursorShape;

fn frame_with_cursor(frame_token: u64, cursor_col: Option<u32>) -> NativeTerminalFrame {
    let cursor_overlay = cursor_col.map_or(
        NativeCursorOverlay {
            visible: false,
            row: 0,
            col: 0,
            cell_width_px: 8,
            cell_height_px: 16,
            shape: TerminalCursorShape::Block,
            fg_rgba: 0,
            bg_rgba: 0,
        },
        |col| NativeCursorOverlay {
            visible: true,
            row: 0,
            col,
            cell_width_px: 8,
            cell_height_px: 16,
            shape: TerminalCursorShape::Block,
            fg_rgba: 0xffff_ffff,
            bg_rgba: 0xff33_99ff,
        },
    );

    NativeTerminalFrame {
        frame_token,
        cell_width_px: 8,
        cell_height_px: 16,
        presentable_frame: PresentableNativeFrame {
            seqno: frame_token,
            shaped_row_count: 0,
            glyph_run_count: 0,
            glyph_count: 0,
            dirty_row_count: 1,
            default_fg_rgba: 0xffff_ffff,
            default_bg_rgba: 0xff11_2233,
            row_bg_even_rgba: 0xff11_2233,
            row_bg_odd_rgba: 0xff11_2233,
            grid_rows: 2,
            grid_cols: 6,
            background_runs: vec![],
            monochrome_glyph_draws: vec![],
            color_glyph_draws: vec![],
            underline_run_count: 0,
            cursor: NativeCursorFrameState {
                row: 0,
                col: cursor_col.unwrap_or(0),
                visible: cursor_col.is_some(),
                blinking: true,
                shape: TerminalCursorShape::Block,
                fg_rgba: 0xffff_ffff,
                bg_rgba: 0xff33_99ff,
            },
            cursor_overlay,
            selection: NativeSelectionFrameState::default(),
            selection_overlay: NativeSelectionOverlay::default(),
            underline_overlay: NativeUnderlineOverlay::default(),
            semantic_overlays: vec![],
            semantic_input_overlays: vec![],
            ime_preview_overlay: NativeImePreviewOverlay::default(),
            renderer_stats: NativeRendererFrameStats {
                glyph_cache_entries: 0,
                mono_glyph_cache_entries: 0,
                color_glyph_cache_entries: 0,
                monochrome_glyphs_prepared: 0,
                color_glyphs_prepared: 0,
            },
        },
    }
}

fn retained_frame(frame: NativeTerminalFrame) -> RetainedNativeTerminalSurfaceFrame {
    RetainedNativeTerminalSurfaceFrame {
        frame,
        rect: NativeTerminalSurfaceRect {
            x: 10,
            y: 20,
            width: 96,
            height: 32,
        },
    }
}

#[test]
fn overlay_only_damage_uses_union_of_previous_and_next_cursor_regions() {
    let previous = retained_frame(frame_with_cursor(7, Some(0)));
    let next = retained_frame(frame_with_cursor(7, Some(2)));
    let mut tracker = NativeFrameDamageTracker::default();

    tracker.track_frame_damage(Some(&previous), Some(&next));
    let damage = tracker.take_damage().expect("overlay-only damage");

    assert_eq!(damage.kind, NativeSurfaceDamageKind::OverlayOnly);
    assert_eq!(
        damage.rect,
        NativeTerminalSurfaceRect {
            x: 10,
            y: 20,
            width: 24,
            height: 16,
        },
        "overlay-only damage should shrink to the union of the old and new cursor cells instead of repainting the entire terminal surface"
    );
}

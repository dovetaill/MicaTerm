use mica_term::app::ssh::runtime::TerminalCursorShape;
use mica_term::app::terminal_presenter::{
    NativeCursorFrameState, NativeCursorOverlay, NativeImePreviewOverlay, NativeRendererFrameStats,
    NativeSelectionFrameState, NativeSelectionOverlay, NativeSelectionRect, NativeTerminalFrame,
    NativeUnderlineOverlay, NativeUnderlineRun, PresentableNativeFrame,
};
use mica_term::app::terminal_renderer::{
    NativeFrameDamageTracker, NativeSurfaceDamageKind, NativeTerminalSurfaceRect,
    RetainedNativeTerminalSurfaceFrame,
};

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
            viewport_offset_lines: 0,
            row_content_hashes: vec![0, 0],
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
            semantic_spans: vec![],
            command_blocks: vec![],
            overview_markers: vec![],
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

fn frame_with_underline(
    frame_token: u64,
    underline_run: Option<NativeUnderlineRun>,
) -> NativeTerminalFrame {
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
            viewport_offset_lines: 0,
            row_content_hashes: vec![0, 0],
            default_fg_rgba: 0xffff_ffff,
            default_bg_rgba: 0xff11_2233,
            row_bg_even_rgba: 0xff11_2233,
            row_bg_odd_rgba: 0xff11_2233,
            grid_rows: 2,
            grid_cols: 6,
            background_runs: vec![],
            monochrome_glyph_draws: vec![],
            color_glyph_draws: vec![],
            underline_run_count: underline_run.iter().count(),
            cursor: NativeCursorFrameState {
                row: 0,
                col: 0,
                visible: false,
                blinking: false,
                shape: TerminalCursorShape::Block,
                fg_rgba: 0,
                bg_rgba: 0,
            },
            cursor_overlay: NativeCursorOverlay {
                visible: false,
                row: 0,
                col: 0,
                cell_width_px: 8,
                cell_height_px: 16,
                shape: TerminalCursorShape::Block,
                fg_rgba: 0,
                bg_rgba: 0,
            },
            selection: NativeSelectionFrameState::default(),
            selection_overlay: NativeSelectionOverlay::default(),
            underline_overlay: NativeUnderlineOverlay {
                visible: underline_run.is_some(),
                run_count: underline_run.iter().count(),
                runs: underline_run.into_iter().collect(),
            },
            semantic_overlays: vec![],
            semantic_input_overlays: vec![],
            semantic_spans: vec![],
            command_blocks: vec![],
            overview_markers: vec![],
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

fn frame_with_selection(
    frame_token: u64,
    selection_rects: Vec<NativeSelectionRect>,
) -> NativeTerminalFrame {
    let active = !selection_rects.is_empty();
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
            viewport_offset_lines: 0,
            row_content_hashes: vec![0, 0],
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
                col: 0,
                visible: false,
                blinking: false,
                shape: TerminalCursorShape::Block,
                fg_rgba: 0,
                bg_rgba: 0,
            },
            cursor_overlay: NativeCursorOverlay {
                visible: false,
                row: 0,
                col: 0,
                cell_width_px: 8,
                cell_height_px: 16,
                shape: TerminalCursorShape::Block,
                fg_rgba: 0,
                bg_rgba: 0,
            },
            selection: NativeSelectionFrameState::default(),
            selection_overlay: NativeSelectionOverlay {
                active,
                rect_count: selection_rects.len(),
                rects: selection_rects,
                start_row: 0,
                start_col: 0,
                end_row: 0,
                end_col: 0,
                overlay_rgba: 0x6622_aaff,
            },
            underline_overlay: NativeUnderlineOverlay::default(),
            semantic_overlays: vec![],
            semantic_input_overlays: vec![],
            semantic_spans: vec![],
            command_blocks: vec![],
            overview_markers: vec![],
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

fn frame_with_ime_preview(
    frame_token: u64,
    ime_preview_overlay: NativeImePreviewOverlay,
) -> NativeTerminalFrame {
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
            viewport_offset_lines: 0,
            row_content_hashes: vec![0, 0],
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
                col: 0,
                visible: false,
                blinking: false,
                shape: TerminalCursorShape::Block,
                fg_rgba: 0,
                bg_rgba: 0,
            },
            cursor_overlay: NativeCursorOverlay {
                visible: false,
                row: 0,
                col: 0,
                cell_width_px: 8,
                cell_height_px: 16,
                shape: TerminalCursorShape::Block,
                fg_rgba: 0,
                bg_rgba: 0,
            },
            selection: NativeSelectionFrameState::default(),
            selection_overlay: NativeSelectionOverlay::default(),
            underline_overlay: NativeUnderlineOverlay::default(),
            semantic_overlays: vec![],
            semantic_input_overlays: vec![],
            semantic_spans: vec![],
            command_blocks: vec![],
            overview_markers: vec![],
            ime_preview_overlay,
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

#[test]
fn underline_overlay_changes_stay_on_overlay_only_damage_path() {
    let previous = retained_frame(frame_with_underline(11, None));
    let next = retained_frame(frame_with_underline(
        11,
        Some(NativeUnderlineRun {
            row: 1,
            start_col: 1,
            end_col: 3,
            fg_rgba: 0xff44_ccff,
        }),
    ));
    let mut tracker = NativeFrameDamageTracker::default();

    tracker.track_frame_damage(Some(&previous), Some(&next));
    let damage = tracker.take_damage().expect("underline overlay damage");

    assert_eq!(
        damage.kind,
        NativeSurfaceDamageKind::OverlayOnly,
        "stable prepared-frame tokens with only underline overlay changes should stay on the overlay-only damage path"
    );
    assert_eq!(
        damage.rect,
        NativeTerminalSurfaceRect {
            x: 18,
            y: 36,
            width: 24,
            height: 16,
        },
        "underline overlay damage should shrink to the underline run's cell span instead of repainting the entire terminal surface"
    );
}

#[test]
fn selection_overlay_damage_shrinks_to_changed_cells_only() {
    let previous = retained_frame(frame_with_selection(
        15,
        vec![NativeSelectionRect {
            row: 0,
            start_col: 1,
            end_col: 3,
            overlay_rgba: 0x6622_aaff,
        }],
    ));
    let next = retained_frame(frame_with_selection(
        15,
        vec![NativeSelectionRect {
            row: 0,
            start_col: 1,
            end_col: 4,
            overlay_rgba: 0x6622_aaff,
        }],
    ));
    let mut tracker = NativeFrameDamageTracker::default();

    tracker.track_frame_damage(Some(&previous), Some(&next));
    let damage = tracker.take_damage().expect("selection overlay damage");

    assert_eq!(damage.kind, NativeSurfaceDamageKind::OverlayOnly);
    assert_eq!(
        damage.rect,
        NativeTerminalSurfaceRect {
            x: 42,
            y: 20,
            width: 8,
            height: 16,
        },
        "selection overlay damage should collapse to only the newly changed cell instead of repainting the full previous-plus-next selection union"
    );
}

#[test]
fn ime_preview_damage_shrinks_to_changed_cells_only() {
    let previous = retained_frame(frame_with_ime_preview(
        21,
        NativeImePreviewOverlay {
            active: true,
            row: 1,
            start_col: 1,
            end_col: 3,
            cursor_col: 2,
        },
    ));
    let next = retained_frame(frame_with_ime_preview(
        21,
        NativeImePreviewOverlay {
            active: true,
            row: 1,
            start_col: 1,
            end_col: 4,
            cursor_col: 3,
        },
    ));
    let mut tracker = NativeFrameDamageTracker::default();

    tracker.track_frame_damage(Some(&previous), Some(&next));
    let damage = tracker.take_damage().expect("ime preview damage");

    assert_eq!(damage.kind, NativeSurfaceDamageKind::OverlayOnly);
    assert_eq!(
        damage.rect,
        NativeTerminalSurfaceRect {
            x: 26,
            y: 36,
            width: 24,
            height: 16,
        },
        "IME preview damage should collapse to the old cursor cell, new cursor cell, and newly changed tail cell instead of repainting the full previous-plus-next preview span"
    );
}

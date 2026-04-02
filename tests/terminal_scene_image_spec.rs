//! Scene-image terminal renderer coverage for software composition metrics contracts.

use mica_term::app::terminal_presenter::{
    NativeCursorFrameState, NativeCursorOverlay, NativeImePreviewOverlay, NativeRendererFrameStats,
    NativeSelectionFrameState, NativeSelectionOverlay, NativeTerminalFrame,
    NativeUnderlineOverlay, PresentableNativeFrame,
};
use mica_term::app::terminal_renderer::atlas::{GlyphAtlasEntry, GlyphCacheKind};
use mica_term::app::terminal_renderer::wgpu_renderer::{
    PreparedBackgroundRun, PreparedMonochromeGlyphDraw, PreparedMonochromeGlyphUploadPayload,
};
use mica_term::app::terminal_scene_image::SceneImageTerminalRenderer;
use mica_term::app::ssh::runtime::TerminalCursorShape;

fn pixel_argb(buffer: &slint::SharedPixelBuffer<slint::Rgba8Pixel>, x: u32, y: u32) -> u32 {
    let pixel = buffer.as_slice()[(y * buffer.width() + x) as usize];
    ((pixel.a as u32) << 24) | ((pixel.r as u32) << 16) | ((pixel.g as u32) << 8) | (pixel.b as u32)
}

fn sample_frame_with_wide_glyph() -> NativeTerminalFrame {
    NativeTerminalFrame {
        frame_token: 1,
        cell_width_px: 2,
        cell_height_px: 2,
        presentable_frame: PresentableNativeFrame {
            seqno: 1,
            shaped_row_count: 1,
            glyph_run_count: 1,
            glyph_count: 1,
            dirty_row_count: 1,
            default_fg_rgba: 0xffff_0000,
            default_bg_rgba: 0xff00_0000,
            row_bg_even_rgba: 0xff00_0000,
            row_bg_odd_rgba: 0xff00_0000,
            grid_rows: 1,
            grid_cols: 2,
            background_runs: vec![PreparedBackgroundRun {
                row: 0,
                start_col: 0,
                end_col: 1,
                bg_rgba: 0xff00_0000,
            }],
            monochrome_glyph_draws: vec![PreparedMonochromeGlyphDraw {
                row: 0,
                start_col: 0,
                end_col: 0,
                glyph_id: 42,
                atlas_entry: GlyphAtlasEntry {
                    slot: 7,
                    width_px: 4,
                    height_px: 1,
                    cache_kind: GlyphCacheKind::Monochrome,
                },
                upload: Some(PreparedMonochromeGlyphUploadPayload {
                    width_px: 4,
                    height_px: 1,
                    bearing_x_px: 0,
                    bearing_y_px: 0,
                    advance_px: 4,
                    coverage: vec![255, 255, 255, 255],
                }),
                x_offset_px: 0,
                y_offset_px: 0,
                dest_x_px: 0,
                dest_y_px: 0,
                fg_rgba: 0xffff_0000,
            }],
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
                cell_width_px: 2,
                cell_height_px: 2,
                shape: TerminalCursorShape::Block,
                fg_rgba: 0,
                bg_rgba: 0,
            },
            selection: NativeSelectionFrameState::default(),
            selection_overlay: NativeSelectionOverlay::default(),
            underline_overlay: NativeUnderlineOverlay::default(),
            semantic_overlays: vec![],
            semantic_input_overlays: vec![],
            ime_preview_overlay: NativeImePreviewOverlay::default(),
            renderer_stats: NativeRendererFrameStats {
                glyph_cache_entries: 1,
                mono_glyph_cache_entries: 1,
                color_glyph_cache_entries: 0,
                monochrome_glyphs_prepared: 1,
                color_glyphs_prepared: 0,
            },
        },
    }
}

fn sample_frame_with_right_shifted_glyph() -> NativeTerminalFrame {
    NativeTerminalFrame {
        frame_token: 2,
        cell_width_px: 4,
        cell_height_px: 2,
        presentable_frame: PresentableNativeFrame {
            seqno: 2,
            shaped_row_count: 1,
            glyph_run_count: 1,
            glyph_count: 1,
            dirty_row_count: 1,
            default_fg_rgba: 0xffff_0000,
            default_bg_rgba: 0xff00_0000,
            row_bg_even_rgba: 0xff00_0000,
            row_bg_odd_rgba: 0xff00_0000,
            grid_rows: 1,
            grid_cols: 1,
            background_runs: vec![PreparedBackgroundRun {
                row: 0,
                start_col: 0,
                end_col: 0,
                bg_rgba: 0xff00_0000,
            }],
            monochrome_glyph_draws: vec![PreparedMonochromeGlyphDraw {
                row: 0,
                start_col: 0,
                end_col: 0,
                glyph_id: 99,
                atlas_entry: GlyphAtlasEntry {
                    slot: 8,
                    width_px: 3,
                    height_px: 1,
                    cache_kind: GlyphCacheKind::Monochrome,
                },
                upload: Some(PreparedMonochromeGlyphUploadPayload {
                    width_px: 3,
                    height_px: 1,
                    bearing_x_px: 0,
                    bearing_y_px: 0,
                    advance_px: 3,
                    coverage: vec![255, 255, 255],
                }),
                x_offset_px: 0,
                y_offset_px: 0,
                dest_x_px: 3,
                dest_y_px: 0,
                fg_rgba: 0xffff_0000,
            }],
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
                cell_width_px: 4,
                cell_height_px: 2,
                shape: TerminalCursorShape::Block,
                fg_rgba: 0,
                bg_rgba: 0,
            },
            selection: NativeSelectionFrameState::default(),
            selection_overlay: NativeSelectionOverlay::default(),
            underline_overlay: NativeUnderlineOverlay::default(),
            semantic_overlays: vec![],
            semantic_input_overlays: vec![],
            ime_preview_overlay: NativeImePreviewOverlay::default(),
            renderer_stats: NativeRendererFrameStats {
                glyph_cache_entries: 1,
                mono_glyph_cache_entries: 1,
                color_glyph_cache_entries: 0,
                monochrome_glyphs_prepared: 1,
                color_glyphs_prepared: 0,
            },
        },
    }
}

fn sample_frame_with_cursor_only() -> NativeTerminalFrame {
    NativeTerminalFrame {
        frame_token: 3,
        cell_width_px: 4,
        cell_height_px: 2,
        presentable_frame: PresentableNativeFrame {
            seqno: 3,
            shaped_row_count: 0,
            glyph_run_count: 0,
            glyph_count: 0,
            dirty_row_count: 1,
            default_fg_rgba: 0xffff_ffff,
            default_bg_rgba: 0xff11_2233,
            row_bg_even_rgba: 0xff11_2233,
            row_bg_odd_rgba: 0xff11_2233,
            grid_rows: 1,
            grid_cols: 1,
            background_runs: vec![],
            monochrome_glyph_draws: vec![],
            color_glyph_draws: vec![],
            underline_run_count: 0,
            cursor: NativeCursorFrameState {
                row: 0,
                col: 0,
                visible: true,
                blinking: true,
                shape: TerminalCursorShape::Block,
                fg_rgba: 0xffff_ffff,
                bg_rgba: 0xffff_ff00,
            },
            cursor_overlay: NativeCursorOverlay {
                visible: true,
                row: 0,
                col: 0,
                cell_width_px: 4,
                cell_height_px: 2,
                shape: TerminalCursorShape::Block,
                fg_rgba: 0xffff_ffff,
                bg_rgba: 0xffff_ff00,
            },
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

fn sample_frame_with_vertical_overhang_glyph() -> NativeTerminalFrame {
    NativeTerminalFrame {
        frame_token: 4,
        cell_width_px: 2,
        cell_height_px: 2,
        presentable_frame: PresentableNativeFrame {
            seqno: 4,
            shaped_row_count: 1,
            glyph_run_count: 1,
            glyph_count: 1,
            dirty_row_count: 1,
            default_fg_rgba: 0xffff_0000,
            default_bg_rgba: 0xff00_0000,
            row_bg_even_rgba: 0xff00_0000,
            row_bg_odd_rgba: 0xff00_0000,
            grid_rows: 1,
            grid_cols: 1,
            background_runs: vec![PreparedBackgroundRun {
                row: 0,
                start_col: 0,
                end_col: 0,
                bg_rgba: 0xff00_0000,
            }],
            monochrome_glyph_draws: vec![PreparedMonochromeGlyphDraw {
                row: 0,
                start_col: 0,
                end_col: 0,
                glyph_id: 123,
                atlas_entry: GlyphAtlasEntry {
                    slot: 9,
                    width_px: 1,
                    height_px: 3,
                    cache_kind: GlyphCacheKind::Monochrome,
                },
                upload: Some(PreparedMonochromeGlyphUploadPayload {
                    width_px: 1,
                    height_px: 3,
                    bearing_x_px: 0,
                    bearing_y_px: 0,
                    advance_px: 1,
                    coverage: vec![255, 255, 255],
                }),
                x_offset_px: 0,
                y_offset_px: 0,
                dest_x_px: 0,
                dest_y_px: 1,
                fg_rgba: 0xffff_0000,
            }],
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
                cell_width_px: 2,
                cell_height_px: 2,
                shape: TerminalCursorShape::Block,
                fg_rgba: 0,
                bg_rgba: 0,
            },
            selection: NativeSelectionFrameState::default(),
            selection_overlay: NativeSelectionOverlay::default(),
            underline_overlay: NativeUnderlineOverlay::default(),
            semantic_overlays: vec![],
            semantic_input_overlays: vec![],
            ime_preview_overlay: NativeImePreviewOverlay::default(),
            renderer_stats: NativeRendererFrameStats {
                glyph_cache_entries: 1,
                mono_glyph_cache_entries: 1,
                color_glyph_cache_entries: 0,
                monochrome_glyphs_prepared: 1,
                color_glyphs_prepared: 0,
            },
        },
    }
}

#[test]
fn scene_image_renderer_clips_monochrome_glyphs_to_declared_cell_span() {
    let mut renderer = SceneImageTerminalRenderer::default();
    let frame = renderer
        .render(&sample_frame_with_wide_glyph())
        .expect("render frame");
    let buffer = frame.image.to_rgba8().expect("rgba image");

    assert_eq!(buffer.width(), 4);
    assert_eq!(buffer.height(), 2);
    assert_eq!(pixel_argb(&buffer, 0, 0), 0xffff_0000);
    assert_eq!(pixel_argb(&buffer, 1, 0), 0xffff_0000);
    assert_eq!(pixel_argb(&buffer, 2, 0), 0xff00_0000);
    assert_eq!(pixel_argb(&buffer, 3, 0), 0xff00_0000);
}

#[test]
fn scene_image_renderer_clamps_glyph_origin_back_inside_its_cell_span() {
    let mut renderer = SceneImageTerminalRenderer::default();
    let frame = renderer
        .render(&sample_frame_with_right_shifted_glyph())
        .expect("render frame");
    let buffer = frame.image.to_rgba8().expect("rgba image");

    assert_eq!(buffer.width(), 4);
    assert_eq!(pixel_argb(&buffer, 0, 0), 0xff00_0000);
    assert_eq!(pixel_argb(&buffer, 1, 0), 0xffff_0000);
    assert_eq!(pixel_argb(&buffer, 2, 0), 0xffff_0000);
    assert_eq!(pixel_argb(&buffer, 3, 0), 0xffff_0000);
}

#[test]
fn scene_image_renderer_leaves_cursor_blink_to_the_slint_host_overlay() {
    let mut renderer = SceneImageTerminalRenderer::default();
    let frame = renderer
        .render(&sample_frame_with_cursor_only())
        .expect("render frame");
    let buffer = frame.image.to_rgba8().expect("rgba image");

    assert_eq!(buffer.width(), 4);
    assert_eq!(buffer.height(), 2);
    assert!(
        buffer
            .as_slice()
            .iter()
            .all(|pixel| ((pixel.a as u32) << 24)
                | ((pixel.r as u32) << 16)
                | ((pixel.g as u32) << 8)
                | (pixel.b as u32)
                == 0xff11_2233),
        "scene-image output should keep the bitmap free of cursor ink so the Slint host can own blink timing without double-drawing a block cursor"
    );
}

#[test]
fn scene_image_renderer_clips_vertical_overhang_without_reanchoring_glyph_origin() {
    let mut renderer = SceneImageTerminalRenderer::default();
    let frame = renderer
        .render(&sample_frame_with_vertical_overhang_glyph())
        .expect("render frame");
    let buffer = frame.image.to_rgba8().expect("rgba image");

    assert_eq!(buffer.width(), 2);
    assert_eq!(buffer.height(), 2);
    assert_eq!(
        pixel_argb(&buffer, 0, 0),
        0xff00_0000,
        "scene-image blit should leave the first row untouched when a monochrome glyph starts lower in the cell and only overhangs downward"
    );
    assert_eq!(
        pixel_argb(&buffer, 0, 1),
        0xffff_0000,
        "scene-image blit should preserve the glyph's incoming y origin and let clip rects drop the overflow below the row"
    );
}

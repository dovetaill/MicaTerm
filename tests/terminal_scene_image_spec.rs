//! Scene-image terminal renderer coverage for software composition metrics contracts.

use std::fs;

use mica_term::app::terminal_font::FontFaceKey;
use mica_term::app::terminal_presenter::{
    NativeCursorFrameState, NativeCursorOverlay, NativeImePreviewOverlay, NativeRendererFrameStats,
    NativeSelectionFrameState, NativeSelectionOverlay, NativeSelectionRect, NativeTerminalFrame,
    NativeUnderlineOverlay, NativeUnderlineRun, PresentableNativeFrame,
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
                face_key: FontFaceKey(1),
                font_family_name: "Test Terminal".into(),
                font_em_size_px: 14,
                atlas_entry: GlyphAtlasEntry {
                    slot: 7,
                    width_px: 4,
                    height_px: 1,
                    padding_left_px: 0,
                    padding_right_px: 0,
                    cache_kind: GlyphCacheKind::Monochrome,
                },
                upload: Some(PreparedMonochromeGlyphUploadPayload {
                    width_px: 4,
                    height_px: 1,
                    padding_left_px: 0,
                    padding_right_px: 0,
                    bearing_x_px: 0,
                    bearing_y_px: 0,
                    advance_px: 4,
                    coverage: vec![255, 255, 255, 255],
                }),
                advance_px: 4,
                visible_left_px: 0,
                visible_top_px: 0,
                visible_width_px: 4,
                visible_height_px: 1,
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
                face_key: FontFaceKey(1),
                font_family_name: "Test Terminal".into(),
                font_em_size_px: 14,
                atlas_entry: GlyphAtlasEntry {
                    slot: 8,
                    width_px: 3,
                    height_px: 1,
                    padding_left_px: 0,
                    padding_right_px: 0,
                    cache_kind: GlyphCacheKind::Monochrome,
                },
                upload: Some(PreparedMonochromeGlyphUploadPayload {
                    width_px: 3,
                    height_px: 1,
                    padding_left_px: 0,
                    padding_right_px: 0,
                    bearing_x_px: 0,
                    bearing_y_px: 0,
                    advance_px: 3,
                    coverage: vec![255, 255, 255],
                }),
                advance_px: 3,
                visible_left_px: 0,
                visible_top_px: 0,
                visible_width_px: 3,
                visible_height_px: 1,
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
                face_key: FontFaceKey(1),
                font_family_name: "Test Terminal".into(),
                font_em_size_px: 14,
                atlas_entry: GlyphAtlasEntry {
                    slot: 9,
                    width_px: 1,
                    height_px: 3,
                    padding_left_px: 0,
                    padding_right_px: 0,
                    cache_kind: GlyphCacheKind::Monochrome,
                },
                upload: Some(PreparedMonochromeGlyphUploadPayload {
                    width_px: 1,
                    height_px: 3,
                    padding_left_px: 0,
                    padding_right_px: 0,
                    bearing_x_px: 0,
                    bearing_y_px: 0,
                    advance_px: 1,
                    coverage: vec![255, 255, 255],
                }),
                advance_px: 1,
                visible_left_px: 0,
                visible_top_px: 0,
                visible_width_px: 1,
                visible_height_px: 3,
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
fn scene_image_renderer_preserves_rightmost_visible_pixels_beyond_the_declared_cell_span() {
    let mut renderer = SceneImageTerminalRenderer::default();
    let frame = renderer
        .render(&sample_frame_with_wide_glyph())
        .expect("render frame");
    let buffer = frame.image.to_rgba8().expect("rgba image");

    assert_eq!(buffer.width(), 4);
    assert_eq!(buffer.height(), 2);
    assert_eq!(pixel_argb(&buffer, 0, 0), 0xffff_0000);
    assert_eq!(pixel_argb(&buffer, 1, 0), 0xffff_0000);
    assert_eq!(pixel_argb(&buffer, 2, 0), 0xffff_0000);
    assert_eq!(pixel_argb(&buffer, 3, 0), 0xffff_0000);
}

#[test]
fn scene_image_renderer_keeps_glyph_origin_and_lets_viewport_clip_trim_overflow() {
    let mut renderer = SceneImageTerminalRenderer::default();
    let frame = renderer
        .render(&sample_frame_with_right_shifted_glyph())
        .expect("render frame");
    let buffer = frame.image.to_rgba8().expect("rgba image");

    assert_eq!(buffer.width(), 4);
    assert_eq!(pixel_argb(&buffer, 0, 0), 0xff00_0000);
    assert_eq!(pixel_argb(&buffer, 1, 0), 0xff00_0000);
    assert_eq!(pixel_argb(&buffer, 2, 0), 0xff00_0000);
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
fn terminal_session_host_keeps_bitmap_cursor_and_image_visibility_gated_to_bitmap_mode() {
    let host_source =
        fs::read_to_string("ui/shell/terminal-session-host.slint").expect("read terminal host");

    assert!(
        host_source.contains("visible: root.session-render-mode == \"bitmap\";"),
        "scene-image fallback should keep the host bitmap image hidden whenever native mode owns terminal presentation"
    );
    assert!(
        host_source.contains(
            "if root.session-render-mode == \"bitmap\" && root.session-cursor-visible && root.cursor-blink-visible : cursor-overlay := Rectangle {"
        ),
        "scene-image fallback should keep the Slint cursor overlay scoped to bitmap mode so the Windows native renderer can own cursor painting without double-draw"
    );
}

#[test]
fn scene_image_renderer_reuses_bitmap_when_only_cursor_overlay_changes() {
    let mut renderer = SceneImageTerminalRenderer::default();
    let first = sample_frame_with_cursor_only();
    let mut second = first.clone();
    second.frame_token += 1;
    second.presentable_frame.seqno += 1;
    second.presentable_frame.cursor.visible = false;
    second.presentable_frame.cursor.blinking = false;
    second.presentable_frame.cursor_overlay.visible = false;

    let first_bitmap = renderer.render(&first).expect("render first frame");
    let second_bitmap = renderer.render(&second).expect("render second frame");
    let first_rgba = first_bitmap.image.to_rgba8().expect("first rgba");
    let second_rgba = second_bitmap.image.to_rgba8().expect("second rgba");

    assert_eq!(
        renderer.bitmap_render_count(),
        1,
        "scene-image renderer should reuse the cached bitmap when only cursor host-overlay state changes"
    );
    assert_eq!(
        first_rgba.as_slice(),
        second_rgba.as_slice(),
        "cursor-only changes must not trigger a fresh bitmap with different scene pixels"
    );
}

#[test]
fn scene_image_renderer_reuses_base_bitmap_across_overlay_only_updates() {
    let mut renderer = SceneImageTerminalRenderer::default();
    let first = sample_frame_with_right_shifted_glyph();
    let mut second = first.clone();
    second.frame_token += 1;
    second.presentable_frame.seqno += 1;
    second.presentable_frame.selection_overlay = NativeSelectionOverlay {
        active: true,
        rect_count: 1,
        rects: vec![NativeSelectionRect {
            row: 0,
            start_col: 0,
            end_col: 0,
            overlay_rgba: 0x6600_ff00,
        }],
        start_row: 0,
        start_col: 0,
        end_row: 0,
        end_col: 0,
        overlay_rgba: 0x6600_ff00,
    };

    let mut third = first.clone();
    third.frame_token += 2;
    third.presentable_frame.seqno += 2;
    third.presentable_frame.underline_overlay = NativeUnderlineOverlay {
        visible: true,
        run_count: 1,
        runs: vec![NativeUnderlineRun {
            row: 0,
            start_col: 0,
            end_col: 0,
            fg_rgba: 0xff00_ffff,
        }],
    };

    let mut fourth = first.clone();
    fourth.frame_token += 3;
    fourth.presentable_frame.seqno += 3;
    fourth.presentable_frame.ime_preview_overlay = NativeImePreviewOverlay {
        active: true,
        row: 0,
        start_col: 0,
        end_col: 0,
        cursor_col: 0,
    };

    renderer.render(&first).expect("render base frame");
    renderer.render(&second).expect("render selection overlay frame");
    renderer.render(&third).expect("render underline overlay frame");
    renderer.render(&fourth).expect("render ime overlay frame");

    assert_eq!(
        renderer.base_render_count(),
        1,
        "scene-image renderer should reuse the cached base bitmap when only overlays change on top of the same glyph/background content"
    );
    assert_eq!(
        renderer.bitmap_render_count(),
        4,
        "overlay-only updates should still produce fresh composed bitmap frames while avoiding a second base raster pass"
    );
}

#[test]
fn scene_image_renderer_reuses_cached_base_bitmap_after_overlays_clear() {
    let mut renderer = SceneImageTerminalRenderer::default();
    let base = sample_frame_with_right_shifted_glyph();
    let mut overlay = base.clone();
    overlay.frame_token += 1;
    overlay.presentable_frame.seqno += 1;
    overlay.presentable_frame.selection_overlay = NativeSelectionOverlay {
        active: true,
        rect_count: 1,
        rects: vec![NativeSelectionRect {
            row: 0,
            start_col: 0,
            end_col: 0,
            overlay_rgba: 0x6600_ff00,
        }],
        start_row: 0,
        start_col: 0,
        end_row: 0,
        end_col: 0,
        overlay_rgba: 0x6600_ff00,
    };

    let mut cleared = base.clone();
    cleared.frame_token += 2;
    cleared.presentable_frame.seqno += 2;

    renderer.render(&base).expect("render base frame");
    renderer.render(&overlay).expect("render overlay frame");
    renderer.render(&cleared).expect("render cleared frame");

    assert_eq!(
        renderer.base_render_count(),
        1,
        "clearing overlays should keep reusing the same base glyph/background raster"
    );
    assert_eq!(
        renderer.bitmap_render_count(),
        2,
        "when overlays clear back to an unchanged base frame the renderer should reuse the cached base image instead of creating a third Image::from_rgba8 payload"
    );
}

#[test]
fn scene_image_renderer_reuses_overlay_work_buffer_for_same_sized_frames() {
    let mut renderer = SceneImageTerminalRenderer::default();
    let base = sample_frame_with_right_shifted_glyph();
    let mut first_overlay = base.clone();
    first_overlay.frame_token += 1;
    first_overlay.presentable_frame.seqno += 1;
    first_overlay.presentable_frame.selection_overlay = NativeSelectionOverlay {
        active: true,
        rect_count: 1,
        rects: vec![NativeSelectionRect {
            row: 0,
            start_col: 0,
            end_col: 0,
            overlay_rgba: 0x6600_ff00,
        }],
        start_row: 0,
        start_col: 0,
        end_row: 0,
        end_col: 0,
        overlay_rgba: 0x6600_ff00,
    };

    let mut second_overlay = base.clone();
    second_overlay.frame_token += 2;
    second_overlay.presentable_frame.seqno += 2;
    second_overlay.presentable_frame.underline_overlay = NativeUnderlineOverlay {
        visible: true,
        run_count: 1,
        runs: vec![NativeUnderlineRun {
            row: 0,
            start_col: 0,
            end_col: 0,
            fg_rgba: 0xffff_00ff,
        }],
    };

    renderer.render(&first_overlay).expect("render first overlay frame");
    renderer.render(&second_overlay).expect("render second overlay frame");

    assert_eq!(
        renderer.working_resize_count(),
        1,
        "overlay composition should keep reusing one working pixel buffer while the terminal grid size stays unchanged"
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

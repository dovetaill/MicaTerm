use mica_term::app::terminal_renderer::custom_grid_glyphs::{
    BlockElementGlyph, BoxDrawingGlyph, CustomGridGlyphKind, DevicePixelSnapper,
    classify_custom_grid_glyph, generate_custom_grid_mask,
};

#[test]
fn classifier_accepts_v1_whitelist_and_rejects_non_v1_clusters() {
    assert_eq!(
        classify_custom_grid_glyph("│", 1),
        Some(CustomGridGlyphKind::BoxDrawing(BoxDrawingGlyph::Vertical))
    );
    assert_eq!(
        classify_custom_grid_glyph("╭", 1),
        Some(CustomGridGlyphKind::BoxDrawing(
            BoxDrawingGlyph::RoundCornerTopLeft
        ))
    );
    assert_eq!(
        classify_custom_grid_glyph("█", 1),
        Some(CustomGridGlyphKind::BlockElement(BlockElementGlyph::Full))
    );
    assert_eq!(
        classify_custom_grid_glyph("┼", 1),
        Some(CustomGridGlyphKind::BoxDrawing(BoxDrawingGlyph::Cross))
    );
    assert_eq!(classify_custom_grid_glyph("⣿", 1), None);
    assert_eq!(classify_custom_grid_glyph("", 1), None);
    assert_eq!(classify_custom_grid_glyph("─\u{fe0f}", 1), None);
    assert_eq!(classify_custom_grid_glyph("─\u{200d}", 1), None);
    assert_eq!(classify_custom_grid_glyph("─\u{0301}", 1), None);
    assert_eq!(classify_custom_grid_glyph("ab", 2), None);
}

#[test]
fn device_pixel_snapper_returns_integer_aligned_rects_for_common_scales() {
    for (scale, expected_width, expected_height) in
        [(1.0, 8, 10), (1.25, 10, 12), (1.5, 12, 15), (2.0, 16, 20)]
    {
        let rect = DevicePixelSnapper::new(scale).snap_rect(0.5, 0.5, 8.0, 10.0);
        assert_eq!(
            rect.origin_x_px, 1,
            "snapped x origin should round to an integer device pixel at scale {scale}"
        );
        assert_eq!(
            rect.origin_y_px, 1,
            "snapped y origin should round to an integer device pixel at scale {scale}"
        );
        assert_eq!(rect.width_px, expected_width);
        assert_eq!(rect.height_px, expected_height);
    }
}

#[test]
fn vertical_and_horizontal_masks_full_bleed_in_the_connected_dimension() {
    let vertical = generate_custom_grid_mask(
        CustomGridGlyphKind::BoxDrawing(BoxDrawingGlyph::Vertical),
        8,
        10,
        1.0,
    );
    let horizontal = generate_custom_grid_mask(
        CustomGridGlyphKind::BoxDrawing(BoxDrawingGlyph::Horizontal),
        8,
        10,
        1.0,
    );

    assert!(row_has_ink(&vertical, 0));
    assert!(row_has_ink(&vertical, vertical.height_px - 1));
    assert!(column_has_ink(&horizontal, 0));
    assert!(column_has_ink(&horizontal, horizontal.width_px - 1));
}

#[test]
fn block_elements_fill_the_expected_portion_of_the_cell() {
    let full = generate_custom_grid_mask(
        CustomGridGlyphKind::BlockElement(BlockElementGlyph::Full),
        8,
        10,
        1.0,
    );
    let upper = generate_custom_grid_mask(
        CustomGridGlyphKind::BlockElement(BlockElementGlyph::UpperHalf),
        8,
        10,
        1.0,
    );
    let lower = generate_custom_grid_mask(
        CustomGridGlyphKind::BlockElement(BlockElementGlyph::LowerHalf),
        8,
        10,
        1.0,
    );
    let left = generate_custom_grid_mask(
        CustomGridGlyphKind::BlockElement(BlockElementGlyph::LeftHalf),
        8,
        10,
        1.0,
    );
    let right = generate_custom_grid_mask(
        CustomGridGlyphKind::BlockElement(BlockElementGlyph::RightHalf),
        8,
        10,
        1.0,
    );

    assert_eq!(filled_ratio(&full), 1.0);
    assert_eq!(filled_ratio(&upper), 0.5);
    assert_eq!(filled_ratio(&lower), 0.5);
    assert_eq!(filled_ratio(&left), 0.5);
    assert_eq!(filled_ratio(&right), 0.5);
}

#[test]
fn box_drawing_topology_matches_the_v1_contract() {
    let top_left = generate_custom_grid_mask(
        CustomGridGlyphKind::BoxDrawing(BoxDrawingGlyph::CornerTopLeft),
        8,
        10,
        1.0,
    );
    let tee_left = generate_custom_grid_mask(
        CustomGridGlyphKind::BoxDrawing(BoxDrawingGlyph::TeeLeft),
        8,
        10,
        1.0,
    );
    let cross = generate_custom_grid_mask(
        CustomGridGlyphKind::BoxDrawing(BoxDrawingGlyph::Cross),
        8,
        10,
        1.0,
    );
    let square_round = generate_custom_grid_mask(
        CustomGridGlyphKind::BoxDrawing(BoxDrawingGlyph::CornerTopLeft),
        8,
        10,
        1.0,
    );
    let rounded = generate_custom_grid_mask(
        CustomGridGlyphKind::BoxDrawing(BoxDrawingGlyph::RoundCornerTopLeft),
        8,
        10,
        1.0,
    );

    assert!(!row_has_ink(&top_left, 0), "┌ should not connect upward");
    assert!(
        !column_has_ink(&top_left, 0),
        "┌ should not connect leftward"
    );
    assert!(
        row_has_ink(&top_left, top_left.height_px - 1),
        "┌ should connect downward"
    );
    assert!(
        column_has_ink(&top_left, top_left.width_px - 1),
        "┌ should connect rightward"
    );

    assert!(row_has_ink(&tee_left, 0), "├ should connect upward");
    assert!(
        row_has_ink(&tee_left, tee_left.height_px - 1),
        "├ should connect downward"
    );
    assert!(
        !column_has_ink(&tee_left, 0),
        "├ should not connect leftward"
    );
    assert!(
        column_has_ink(&tee_left, tee_left.width_px - 1),
        "├ should connect rightward"
    );

    assert!(row_has_ink(&cross, 0), "┼ should connect upward");
    assert!(
        row_has_ink(&cross, cross.height_px - 1),
        "┼ should connect downward"
    );
    assert!(column_has_ink(&cross, 0), "┼ should connect leftward");
    assert!(
        column_has_ink(&cross, cross.width_px - 1),
        "┼ should connect rightward"
    );

    assert_eq!(filled_ratio(&rounded) < filled_ratio(&square_round), true);
    assert_eq!(row_has_ink(&rounded, rounded.height_px - 1), true);
    assert_eq!(column_has_ink(&rounded, rounded.width_px - 1), true);
}

fn row_has_ink(
    mask: &mica_term::app::terminal_renderer::custom_grid_glyphs::GeneratedMaskGlyph,
    row: u32,
) -> bool {
    let start = row as usize * mask.width_px as usize;
    let end = start + mask.width_px as usize;
    mask.alpha[start..end].iter().any(|value| *value == 255)
}

fn column_has_ink(
    mask: &mica_term::app::terminal_renderer::custom_grid_glyphs::GeneratedMaskGlyph,
    col: u32,
) -> bool {
    (0..mask.height_px).any(|row| {
        let index = row as usize * mask.width_px as usize + col as usize;
        mask.alpha[index] == 255
    })
}

fn filled_ratio(
    mask: &mica_term::app::terminal_renderer::custom_grid_glyphs::GeneratedMaskGlyph,
) -> f32 {
    let filled = mask.alpha.iter().filter(|value| **value == 255).count();
    filled as f32 / mask.alpha.len() as f32
}

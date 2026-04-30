//! Shared special-case geometry and alpha-mask generation for terminal box and block glyphs.

use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CellRenderKind {
    NormalText,
    BoxDrawing,
    BlockElement,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum CustomGridGlyphKind {
    BoxDrawing(BoxDrawingGlyph),
    BlockElement(BlockElementGlyph),
}

impl CustomGridGlyphKind {
    pub fn cell_render_kind(self) -> CellRenderKind {
        match self {
            Self::BoxDrawing(_) => CellRenderKind::BoxDrawing,
            Self::BlockElement(_) => CellRenderKind::BlockElement,
        }
    }
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum BoxDrawingGlyph {
    Horizontal,
    Vertical,
    CornerTopLeft,
    CornerTopRight,
    CornerBottomLeft,
    CornerBottomRight,
    TeeLeft,
    TeeRight,
    TeeTop,
    TeeBottom,
    Cross,
    RoundCornerTopLeft,
    RoundCornerTopRight,
    RoundCornerBottomLeft,
    RoundCornerBottomRight,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum BlockElementGlyph {
    Full,
    UpperHalf,
    LowerHalf,
    LeftHalf,
    RightHalf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeneratedMaskGlyph {
    pub width_px: u32,
    pub height_px: u32,
    pub alpha: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SnappedDeviceRect {
    pub origin_x_px: i32,
    pub origin_y_px: i32,
    pub width_px: u32,
    pub height_px: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DevicePixelSnapper {
    scale: f32,
}

impl DevicePixelSnapper {
    pub fn new(scale: f32) -> Self {
        Self {
            scale: sanitize_scale(scale),
        }
    }

    pub fn snap_rect(
        self,
        logical_origin_x: f32,
        logical_origin_y: f32,
        logical_width: f32,
        logical_height: f32,
    ) -> SnappedDeviceRect {
        let origin_x_px = (logical_origin_x * self.scale).round() as i32;
        let origin_y_px = (logical_origin_y * self.scale).round() as i32;
        let end_x_px = ((logical_origin_x + logical_width.max(0.0)) * self.scale).round() as i32;
        let end_y_px = ((logical_origin_y + logical_height.max(0.0)) * self.scale).round() as i32;

        SnappedDeviceRect {
            origin_x_px,
            origin_y_px,
            width_px: end_x_px.saturating_sub(origin_x_px).max(1) as u32,
            height_px: end_y_px.saturating_sub(origin_y_px).max(1) as u32,
        }
    }
}

pub fn classify_custom_grid_glyph(text: &str, cell_span: u32) -> Option<CustomGridGlyphKind> {
    if cell_span != 1 {
        return None;
    }
    if text.graphemes(true).count() != 1 {
        return None;
    }
    if text.chars().count() != 1 {
        return None;
    }

    let ch = text.chars().next()?;
    Some(match ch {
        '─' => CustomGridGlyphKind::BoxDrawing(BoxDrawingGlyph::Horizontal),
        '│' => CustomGridGlyphKind::BoxDrawing(BoxDrawingGlyph::Vertical),
        '┌' => CustomGridGlyphKind::BoxDrawing(BoxDrawingGlyph::CornerTopLeft),
        '┐' => CustomGridGlyphKind::BoxDrawing(BoxDrawingGlyph::CornerTopRight),
        '└' => CustomGridGlyphKind::BoxDrawing(BoxDrawingGlyph::CornerBottomLeft),
        '┘' => CustomGridGlyphKind::BoxDrawing(BoxDrawingGlyph::CornerBottomRight),
        '├' => CustomGridGlyphKind::BoxDrawing(BoxDrawingGlyph::TeeLeft),
        '┤' => CustomGridGlyphKind::BoxDrawing(BoxDrawingGlyph::TeeRight),
        '┬' => CustomGridGlyphKind::BoxDrawing(BoxDrawingGlyph::TeeTop),
        '┴' => CustomGridGlyphKind::BoxDrawing(BoxDrawingGlyph::TeeBottom),
        '┼' => CustomGridGlyphKind::BoxDrawing(BoxDrawingGlyph::Cross),
        '╭' => CustomGridGlyphKind::BoxDrawing(BoxDrawingGlyph::RoundCornerTopLeft),
        '╮' => CustomGridGlyphKind::BoxDrawing(BoxDrawingGlyph::RoundCornerTopRight),
        '╰' => CustomGridGlyphKind::BoxDrawing(BoxDrawingGlyph::RoundCornerBottomLeft),
        '╯' => CustomGridGlyphKind::BoxDrawing(BoxDrawingGlyph::RoundCornerBottomRight),
        '█' => CustomGridGlyphKind::BlockElement(BlockElementGlyph::Full),
        '▀' => CustomGridGlyphKind::BlockElement(BlockElementGlyph::UpperHalf),
        '▄' => CustomGridGlyphKind::BlockElement(BlockElementGlyph::LowerHalf),
        '▌' => CustomGridGlyphKind::BlockElement(BlockElementGlyph::LeftHalf),
        '▐' => CustomGridGlyphKind::BlockElement(BlockElementGlyph::RightHalf),
        _ => return None,
    })
}

pub fn generate_custom_grid_mask(
    kind: CustomGridGlyphKind,
    cell_width_px: u32,
    cell_height_px: u32,
    _scale: f32,
) -> GeneratedMaskGlyph {
    let width_px = cell_width_px.max(1);
    let height_px = cell_height_px.max(1);
    let mut alpha = vec![0u8; (width_px * height_px) as usize];

    match kind {
        CustomGridGlyphKind::BlockElement(block) => {
            paint_block_element(&mut alpha, width_px, height_px, block);
        }
        CustomGridGlyphKind::BoxDrawing(box_glyph) => {
            paint_box_drawing(&mut alpha, width_px, height_px, box_glyph);
        }
    }

    GeneratedMaskGlyph {
        width_px,
        height_px,
        alpha,
    }
}

fn sanitize_scale(scale: f32) -> f32 {
    if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    }
}

fn paint_block_element(alpha: &mut [u8], width_px: u32, height_px: u32, block: BlockElementGlyph) {
    match block {
        BlockElementGlyph::Full => fill_rect(alpha, width_px, height_px, 0, 0, width_px, height_px),
        BlockElementGlyph::UpperHalf => {
            fill_rect(alpha, width_px, height_px, 0, 0, width_px, height_px / 2)
        }
        BlockElementGlyph::LowerHalf => fill_rect(
            alpha,
            width_px,
            height_px,
            0,
            (height_px / 2) as i32,
            width_px,
            height_px - height_px / 2,
        ),
        BlockElementGlyph::LeftHalf => {
            fill_rect(alpha, width_px, height_px, 0, 0, width_px / 2, height_px)
        }
        BlockElementGlyph::RightHalf => fill_rect(
            alpha,
            width_px,
            height_px,
            (width_px / 2) as i32,
            0,
            width_px - width_px / 2,
            height_px,
        ),
    }
}

fn paint_box_drawing(alpha: &mut [u8], width_px: u32, height_px: u32, glyph: BoxDrawingGlyph) {
    let stroke_px = stroke_thickness_px(width_px, height_px);
    let vertical_x = centered_start(width_px, stroke_px);
    let horizontal_y = centered_start(height_px, stroke_px);

    match glyph {
        BoxDrawingGlyph::Horizontal => {
            draw_horizontal(alpha, width_px, height_px, horizontal_y, stroke_px)
        }
        BoxDrawingGlyph::Vertical => {
            draw_vertical(alpha, width_px, height_px, vertical_x, stroke_px)
        }
        BoxDrawingGlyph::CornerTopLeft => {
            draw_right_arm(
                alpha,
                width_px,
                height_px,
                vertical_x,
                horizontal_y,
                stroke_px,
            );
            draw_bottom_arm(
                alpha,
                width_px,
                height_px,
                vertical_x,
                horizontal_y,
                stroke_px,
            );
        }
        BoxDrawingGlyph::CornerTopRight => {
            draw_left_arm(
                alpha,
                width_px,
                height_px,
                vertical_x,
                horizontal_y,
                stroke_px,
            );
            draw_bottom_arm(
                alpha,
                width_px,
                height_px,
                vertical_x,
                horizontal_y,
                stroke_px,
            );
        }
        BoxDrawingGlyph::CornerBottomLeft => {
            draw_right_arm(
                alpha,
                width_px,
                height_px,
                vertical_x,
                horizontal_y,
                stroke_px,
            );
            draw_top_arm(
                alpha,
                width_px,
                height_px,
                vertical_x,
                horizontal_y,
                stroke_px,
            );
        }
        BoxDrawingGlyph::CornerBottomRight => {
            draw_left_arm(
                alpha,
                width_px,
                height_px,
                vertical_x,
                horizontal_y,
                stroke_px,
            );
            draw_top_arm(
                alpha,
                width_px,
                height_px,
                vertical_x,
                horizontal_y,
                stroke_px,
            );
        }
        BoxDrawingGlyph::TeeLeft => {
            draw_vertical(alpha, width_px, height_px, vertical_x, stroke_px);
            draw_right_arm(
                alpha,
                width_px,
                height_px,
                vertical_x,
                horizontal_y,
                stroke_px,
            );
        }
        BoxDrawingGlyph::TeeRight => {
            draw_vertical(alpha, width_px, height_px, vertical_x, stroke_px);
            draw_left_arm(
                alpha,
                width_px,
                height_px,
                vertical_x,
                horizontal_y,
                stroke_px,
            );
        }
        BoxDrawingGlyph::TeeTop => {
            draw_horizontal(alpha, width_px, height_px, horizontal_y, stroke_px);
            draw_bottom_arm(
                alpha,
                width_px,
                height_px,
                vertical_x,
                horizontal_y,
                stroke_px,
            );
        }
        BoxDrawingGlyph::TeeBottom => {
            draw_horizontal(alpha, width_px, height_px, horizontal_y, stroke_px);
            draw_top_arm(
                alpha,
                width_px,
                height_px,
                vertical_x,
                horizontal_y,
                stroke_px,
            );
        }
        BoxDrawingGlyph::Cross => {
            draw_horizontal(alpha, width_px, height_px, horizontal_y, stroke_px);
            draw_vertical(alpha, width_px, height_px, vertical_x, stroke_px);
        }
        BoxDrawingGlyph::RoundCornerTopLeft => {
            draw_right_arm(
                alpha,
                width_px,
                height_px,
                vertical_x,
                horizontal_y,
                stroke_px,
            );
            draw_bottom_arm(
                alpha,
                width_px,
                height_px,
                vertical_x,
                horizontal_y,
                stroke_px,
            );
            carve_round_joint(
                alpha,
                width_px,
                height_px,
                vertical_x,
                horizontal_y,
                stroke_px,
                CornerQuadrant::TopLeft,
            );
        }
        BoxDrawingGlyph::RoundCornerTopRight => {
            draw_left_arm(
                alpha,
                width_px,
                height_px,
                vertical_x,
                horizontal_y,
                stroke_px,
            );
            draw_bottom_arm(
                alpha,
                width_px,
                height_px,
                vertical_x,
                horizontal_y,
                stroke_px,
            );
            carve_round_joint(
                alpha,
                width_px,
                height_px,
                vertical_x,
                horizontal_y,
                stroke_px,
                CornerQuadrant::TopRight,
            );
        }
        BoxDrawingGlyph::RoundCornerBottomLeft => {
            draw_right_arm(
                alpha,
                width_px,
                height_px,
                vertical_x,
                horizontal_y,
                stroke_px,
            );
            draw_top_arm(
                alpha,
                width_px,
                height_px,
                vertical_x,
                horizontal_y,
                stroke_px,
            );
            carve_round_joint(
                alpha,
                width_px,
                height_px,
                vertical_x,
                horizontal_y,
                stroke_px,
                CornerQuadrant::BottomLeft,
            );
        }
        BoxDrawingGlyph::RoundCornerBottomRight => {
            draw_left_arm(
                alpha,
                width_px,
                height_px,
                vertical_x,
                horizontal_y,
                stroke_px,
            );
            draw_top_arm(
                alpha,
                width_px,
                height_px,
                vertical_x,
                horizontal_y,
                stroke_px,
            );
            carve_round_joint(
                alpha,
                width_px,
                height_px,
                vertical_x,
                horizontal_y,
                stroke_px,
                CornerQuadrant::BottomRight,
            );
        }
    }
}

#[derive(Clone, Copy)]
enum CornerQuadrant {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

fn carve_round_joint(
    alpha: &mut [u8],
    width_px: u32,
    height_px: u32,
    vertical_x: u32,
    horizontal_y: u32,
    stroke_px: u32,
    quadrant: CornerQuadrant,
) {
    if stroke_px <= 1 {
        return;
    }

    for dy in 0..stroke_px {
        for dx in 0..stroke_px {
            if dx + dy >= stroke_px.saturating_sub(1) {
                continue;
            }

            let x = match quadrant {
                CornerQuadrant::TopLeft | CornerQuadrant::BottomLeft => vertical_x + dx,
                CornerQuadrant::TopRight | CornerQuadrant::BottomRight => {
                    vertical_x + stroke_px.saturating_sub(1) - dx
                }
            };
            let y = match quadrant {
                CornerQuadrant::TopLeft | CornerQuadrant::TopRight => horizontal_y + dy,
                CornerQuadrant::BottomLeft | CornerQuadrant::BottomRight => {
                    horizontal_y + stroke_px.saturating_sub(1) - dy
                }
            };
            set_alpha(alpha, width_px, height_px, x as i32, y as i32, 0);
        }
    }
}

fn stroke_thickness_px(width_px: u32, height_px: u32) -> u32 {
    match width_px.min(height_px) {
        0..=6 => 1,
        _ => 2,
    }
}

fn centered_start(span_px: u32, stroke_px: u32) -> u32 {
    span_px.saturating_sub(stroke_px) / 2
}

fn draw_horizontal(alpha: &mut [u8], width_px: u32, height_px: u32, y_px: u32, stroke_px: u32) {
    fill_rect(
        alpha,
        width_px,
        height_px,
        0,
        y_px as i32,
        width_px,
        stroke_px,
    );
}

fn draw_vertical(alpha: &mut [u8], width_px: u32, height_px: u32, x_px: u32, stroke_px: u32) {
    fill_rect(
        alpha,
        width_px,
        height_px,
        x_px as i32,
        0,
        stroke_px,
        height_px,
    );
}

fn draw_left_arm(
    alpha: &mut [u8],
    width_px: u32,
    height_px: u32,
    vertical_x: u32,
    horizontal_y: u32,
    stroke_px: u32,
) {
    fill_rect(
        alpha,
        width_px,
        height_px,
        0,
        horizontal_y as i32,
        vertical_x.saturating_add(stroke_px),
        stroke_px,
    );
}

fn draw_right_arm(
    alpha: &mut [u8],
    width_px: u32,
    height_px: u32,
    vertical_x: u32,
    horizontal_y: u32,
    stroke_px: u32,
) {
    fill_rect(
        alpha,
        width_px,
        height_px,
        vertical_x as i32,
        horizontal_y as i32,
        width_px.saturating_sub(vertical_x),
        stroke_px,
    );
}

fn draw_top_arm(
    alpha: &mut [u8],
    width_px: u32,
    height_px: u32,
    vertical_x: u32,
    horizontal_y: u32,
    stroke_px: u32,
) {
    fill_rect(
        alpha,
        width_px,
        height_px,
        vertical_x as i32,
        0,
        stroke_px,
        horizontal_y.saturating_add(stroke_px),
    );
}

fn draw_bottom_arm(
    alpha: &mut [u8],
    width_px: u32,
    height_px: u32,
    vertical_x: u32,
    horizontal_y: u32,
    stroke_px: u32,
) {
    fill_rect(
        alpha,
        width_px,
        height_px,
        vertical_x as i32,
        horizontal_y as i32,
        stroke_px,
        height_px.saturating_sub(horizontal_y),
    );
}

fn fill_rect(
    alpha: &mut [u8],
    width_px: u32,
    height_px: u32,
    x_px: i32,
    y_px: i32,
    rect_width_px: u32,
    rect_height_px: u32,
) {
    for y in 0..rect_height_px {
        for x in 0..rect_width_px {
            set_alpha(
                alpha,
                width_px,
                height_px,
                x_px.saturating_add(x as i32),
                y_px.saturating_add(y as i32),
                255,
            );
        }
    }
}

fn set_alpha(alpha: &mut [u8], width_px: u32, height_px: u32, x_px: i32, y_px: i32, value: u8) {
    if x_px < 0 || y_px < 0 || x_px >= width_px as i32 || y_px >= height_px as i32 {
        return;
    }

    let index = y_px as usize * width_px as usize + x_px as usize;
    if let Some(slot) = alpha.get_mut(index) {
        *slot = value;
    }
}

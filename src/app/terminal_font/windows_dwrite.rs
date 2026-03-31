//! Staged Windows-native font backend contracts for terminal rasterization.

use ab_glyph::{Font, FontArc, Glyph, GlyphId, PxScale, ScaleFont, point};
use anyhow::{Result, anyhow};

use crate::app::terminal_font::backend::{
    FontFaceKey, FontMetrics, FontRequest, FontSystem, apply_synthetic_embolden,
    map_glyph_coverage_to_alpha,
};

const SARASA_FONT_BYTES: &[u8] = include_bytes!("../../../ui/fonts/SarasaTermSCNerd-Regular.ttf");
const DEFAULT_FACE_KEY: FontFaceKey = FontFaceKey(1);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlyphRasterRequest {
    pub face: FontFaceKey,
    pub glyph_id: u32,
    pub px_size: f32,
    pub bold: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RasterizedGlyph {
    pub width_px: u32,
    pub height_px: u32,
    pub bearing_x_px: i32,
    pub bearing_y_px: i32,
    pub advance_px: i32,
    pub coverage: Vec<u8>,
}

pub struct DirectWriteFontSystem {
    font: FontArc,
    font_bytes: &'static [u8],
}

impl DirectWriteFontSystem {
    pub fn new() -> Result<Self> {
        let font = FontArc::try_from_slice(SARASA_FONT_BYTES)
            .map_err(|error| anyhow!("failed to load bundled Sarasa terminal font: {error}"))?;
        Ok(Self {
            font,
            font_bytes: SARASA_FONT_BYTES,
        })
    }

    pub fn rasterize(&mut self, request: GlyphRasterRequest) -> Result<RasterizedGlyph> {
        if request.face != DEFAULT_FACE_KEY {
            return Err(anyhow!("unknown DirectWrite face key: {}", request.face.0));
        }

        let glyph_id = u16::try_from(request.glyph_id)
            .map(GlyphId)
            .map_err(|_| anyhow!("glyph id {} exceeds ab_glyph u16 range", request.glyph_id))?;
        let scaled = self.font.as_scaled(PxScale::from(request.px_size));
        let glyph = Glyph {
            id: glyph_id,
            scale: PxScale::from(request.px_size),
            position: point(0.0, scaled.ascent()),
        };
        let advance_px = scaled.h_advance(glyph_id).round() as i32;

        let Some(outlined) = self.font.outline_glyph(glyph) else {
            return Ok(RasterizedGlyph {
                width_px: 0,
                height_px: 0,
                bearing_x_px: 0,
                bearing_y_px: 0,
                advance_px,
                coverage: Vec::new(),
            });
        };

        let bounds = outlined.px_bounds();
        let width_px = bounds.width().ceil().max(0.0) as u32;
        let height_px = bounds.height().ceil().max(0.0) as u32;
        let mut coverage = vec![0_u8; (width_px as usize).saturating_mul(height_px as usize)];
        outlined.draw(|x, y, value| {
            let index = (y as usize)
                .saturating_mul(width_px as usize)
                .saturating_add(x as usize);
            if let Some(pixel) = coverage.get_mut(index) {
                *pixel = map_glyph_coverage_to_alpha(value);
            }
        });
        apply_synthetic_embolden(&mut coverage, width_px, height_px);
        if request.bold {
            apply_synthetic_embolden(&mut coverage, width_px, height_px);
        }

        Ok(RasterizedGlyph {
            width_px,
            height_px,
            bearing_x_px: bounds.min.x.floor() as i32,
            bearing_y_px: bounds.min.y.floor() as i32,
            advance_px,
            coverage,
        })
    }
}

impl FontSystem for DirectWriteFontSystem {
    fn resolve_face(&mut self, _request: &FontRequest) -> Result<FontFaceKey> {
        Ok(DEFAULT_FACE_KEY)
    }

    fn metrics(&mut self, face: FontFaceKey, px_size: f32) -> Result<FontMetrics> {
        if face != DEFAULT_FACE_KEY {
            return Err(anyhow!("unknown DirectWrite face key: {}", face.0));
        }

        let scaled = self.font.as_scaled(PxScale::from(px_size));
        let mono_advance = scaled
            .h_advance(scaled.glyph_id('M'))
            .max(scaled.h_advance(scaled.glyph_id('0')))
            .max(scaled.h_advance(scaled.glyph_id('W')))
            .max(scaled.h_advance(scaled.glyph_id('界')) / 2.0);
        let line_height = scaled.ascent() - scaled.descent() + scaled.line_gap();

        Ok(FontMetrics {
            units_per_em: self.font.units_per_em().unwrap_or(1000.0).round() as u32,
            ascent_px: scaled.ascent(),
            descent_px: scaled.descent(),
            line_gap_px: scaled.line_gap(),
            cell_width_px: mono_advance.ceil(),
            cell_height_px: line_height.ceil(),
        })
    }

    fn face_bytes(&self, face: FontFaceKey) -> Result<&[u8]> {
        if face != DEFAULT_FACE_KEY {
            return Err(anyhow!("unknown DirectWrite face key: {}", face.0));
        }

        Ok(self.font_bytes)
    }

    fn face_index(&self, _face: FontFaceKey) -> u32 {
        0
    }
}

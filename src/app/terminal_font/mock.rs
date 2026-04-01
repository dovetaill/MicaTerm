//! Test-oriented font system backed by the bundled Sarasa terminal font.

use ab_glyph::{Font, FontArc, PxScale, ScaleFont};
use anyhow::{Result, anyhow};

use crate::app::terminal_font::backend::{
    FontFaceKey, FontMetrics, FontRenderProfile, FontRequest, FontSystem, LoadedFont,
};
#[cfg(feature = "terminal-native-renderer")]
use crate::app::terminal_font::backend::{RasterizedGlyph, ShapedGlyph, shape_text_with_rustybuzz};
#[cfg(feature = "terminal-native-renderer")]
use ab_glyph::{Glyph, GlyphId, point};

const SARASA_FONT_BYTES: &[u8] = include_bytes!("../../../ui/fonts/SarasaTermSCNerd-Regular.ttf");
const DEFAULT_FACE_KEY: FontFaceKey = FontFaceKey(1);
#[cfg(feature = "terminal-native-renderer")]
const DEFAULT_FACE_INDEX: u32 = 0;

pub struct MockFontSystem {
    font: FontArc,
    font_bytes: &'static [u8],
}

pub fn mock_font_system() -> MockFontSystem {
    MockFontSystem::new().expect("bundled Sarasa mock font system should initialize")
}

impl MockFontSystem {
    pub fn new() -> Result<Self> {
        let font = FontArc::try_from_slice(SARASA_FONT_BYTES)
            .map_err(|error| anyhow!("failed to load bundled Sarasa terminal font: {error}"))?;
        Ok(Self {
            font,
            font_bytes: SARASA_FONT_BYTES,
        })
    }
}

impl FontSystem for MockFontSystem {
    fn load_font(&mut self, request: &FontRequest) -> Result<LoadedFont> {
        let face_key = DEFAULT_FACE_KEY;
        let px_size = request.px_size;
        if face_key != DEFAULT_FACE_KEY {
            return Err(anyhow!("unknown mock font face key: {}", face_key.0));
        }

        let scaled = self.font.as_scaled(PxScale::from(px_size));
        let mono_advance = scaled
            .h_advance(scaled.glyph_id('M'))
            .max(scaled.h_advance(scaled.glyph_id('0')))
            .max(scaled.h_advance(scaled.glyph_id('W')))
            .max(scaled.h_advance(scaled.glyph_id('界')) / 2.0);
        let line_height = scaled.ascent() - scaled.descent() + scaled.line_gap();

        Ok(LoadedFont::new(
            face_key,
            request.clone(),
            FontMetrics {
                units_per_em: self.font.units_per_em().unwrap_or(1000.0).round() as u32,
                ascent_px: scaled.ascent(),
                descent_px: scaled.descent(),
                line_gap_px: scaled.line_gap(),
                cell_width_px: mono_advance.ceil(),
                cell_height_px: line_height.ceil(),
            },
            FontRenderProfile::default(),
        ))
    }

    #[cfg(feature = "terminal-native-renderer")]
    fn shape_text(&mut self, font: &LoadedFont, text: &str) -> Result<Vec<ShapedGlyph>> {
        if font.face_key() != DEFAULT_FACE_KEY {
            return Err(anyhow!("unknown mock font face key: {}", font.face_key().0));
        }
        shape_text_with_rustybuzz(self.font_bytes, DEFAULT_FACE_INDEX, text)
    }

    #[cfg(feature = "terminal-native-renderer")]
    fn rasterize_glyph(
        &mut self,
        font: &LoadedFont,
        glyph_id: u32,
        _bold: bool,
    ) -> Result<RasterizedGlyph> {
        if font.face_key() != DEFAULT_FACE_KEY {
            return Err(anyhow!("unknown mock font face key: {}", font.face_key().0));
        }

        let glyph_id = u16::try_from(glyph_id)
            .map(GlyphId)
            .map_err(|_| anyhow!("glyph id {} exceeds ab_glyph u16 range", glyph_id))?;
        let scaled = self.font.as_scaled(PxScale::from(font.px_size()));
        let glyph = Glyph {
            id: glyph_id,
            scale: PxScale::from(font.px_size()),
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
                *pixel = (value.clamp(0.0, 1.0) * 255.0).round() as u8;
            }
        });

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

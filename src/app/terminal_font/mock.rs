//! Test-oriented font system backed by the bundled Sarasa terminal font.

use ab_glyph::{Font, FontArc, PxScale, ScaleFont};
use anyhow::{Result, anyhow};

use crate::app::terminal_font::backend::{FontFaceKey, FontMetrics, FontRequest, FontSystem};

const SARASA_FONT_BYTES: &[u8] = include_bytes!("../../../ui/fonts/SarasaTermSCNerd-Regular.ttf");
const DEFAULT_FACE_KEY: FontFaceKey = FontFaceKey(1);

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
    fn resolve_face(&mut self, _request: &FontRequest) -> Result<FontFaceKey> {
        Ok(DEFAULT_FACE_KEY)
    }

    fn metrics(&mut self, face: FontFaceKey, px_size: f32) -> Result<FontMetrics> {
        if face != DEFAULT_FACE_KEY {
            return Err(anyhow!("unknown mock font face key: {}", face.0));
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
            return Err(anyhow!("unknown mock font face key: {}", face.0));
        }
        Ok(self.font_bytes)
    }

    fn face_index(&self, _face: FontFaceKey) -> u32 {
        0
    }
}

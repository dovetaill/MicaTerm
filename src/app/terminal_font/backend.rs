//! Shared terminal font contracts used by the shaping and renderer pipeline.

use anyhow::Result;

const GLYPH_COVERAGE_GAMMA: f32 = 0.82;
const GLYPH_ALPHA_GAIN: f32 = 1.26;
const SYNTHETIC_EMBOLDEN_STRENGTH: f32 = 0.46;

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct FontFaceKey(pub u64);

#[derive(Clone, Debug, PartialEq)]
pub struct FontRequest {
    pub family_name: Option<String>,
    pub px_size: f32,
}

impl Default for FontRequest {
    fn default() -> Self {
        Self {
            family_name: None,
            px_size: 18.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FontMetrics {
    pub units_per_em: u32,
    pub ascent_px: f32,
    pub descent_px: f32,
    pub line_gap_px: f32,
    pub cell_width_px: f32,
    pub cell_height_px: f32,
}

pub trait FontSystem {
    fn resolve_face(&mut self, request: &FontRequest) -> Result<FontFaceKey>;
    fn metrics(&mut self, face: FontFaceKey, px_size: f32) -> Result<FontMetrics>;
    fn face_bytes(&self, face: FontFaceKey) -> Result<&[u8]>;
    fn face_index(&self, face: FontFaceKey) -> u32;
}

pub(crate) fn map_glyph_coverage_to_alpha(coverage: f32) -> u8 {
    let adjusted = coverage.clamp(0.0, 1.0).powf(GLYPH_COVERAGE_GAMMA) * GLYPH_ALPHA_GAIN;
    (adjusted.clamp(0.0, 1.0) * 255.0).round() as u8
}

pub(crate) fn apply_synthetic_embolden(alpha: &mut [u8], width: u32, height: u32) {
    let width = width as usize;
    let height = height as usize;
    if width < 2 || height == 0 || alpha.is_empty() {
        return;
    }

    let source = alpha.to_vec();
    for y in 0..height {
        let row_offset = y * width;
        for x in 0..(width - 1) {
            let base = source[row_offset + x];
            if base == 0 {
                continue;
            }

            let boosted = (f32::from(base) * SYNTHETIC_EMBOLDEN_STRENGTH).round() as u8;
            let target = &mut alpha[row_offset + x + 1];
            *target = (*target).max(boosted);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{apply_synthetic_embolden, map_glyph_coverage_to_alpha};

    #[test]
    fn glyph_coverage_mapping_lifts_mid_tone_alpha() {
        assert_eq!(map_glyph_coverage_to_alpha(1.0), 255);
        assert!(
            map_glyph_coverage_to_alpha(0.5) >= 180,
            "regular-weight stems should get a visibly stronger alpha curve than the raw coverage value"
        );
        assert!(
            map_glyph_coverage_to_alpha(0.2) > 70,
            "low-coverage anti-aliased edge pixels should stay visible instead of fading out too aggressively"
        );
    }

    #[test]
    fn synthetic_embolden_spreads_ink_one_pixel_to_the_right() {
        let mut alpha = vec![0, 200, 0, 0];

        apply_synthetic_embolden(&mut alpha, 4, 1);

        assert_eq!(alpha[1], 200);
        assert!(
            alpha[2] >= 90,
            "synthetic embolden should strengthen the adjacent pixel enough to visibly thicken a regular-weight stem"
        );
    }
}

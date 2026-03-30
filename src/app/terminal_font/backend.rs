//! Shared terminal font contracts used by the shaping and renderer pipeline.

use anyhow::Result;

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
            px_size: 17.5,
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

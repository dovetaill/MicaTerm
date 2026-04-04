//! Glyph atlas bookkeeping for the staged native terminal renderer.

use std::collections::HashMap;

use crate::app::terminal_font::{FontFaceKey, GlyphRasterRequest, LoadedFontKey, RasterizedGlyph};

pub const MONOCHROME_ATLAS_HORIZONTAL_PADDING_PX: u32 = 1;

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct GlyphAtlasKey {
    pub font_key: LoadedFontKey,
    pub face_key: FontFaceKey,
    pub glyph_id: u32,
    pub bold: bool,
    pub fractional_offset_x_bits: u32,
}

impl From<GlyphRasterRequest> for GlyphAtlasKey {
    fn from(request: GlyphRasterRequest) -> Self {
        Self {
            font_key: request.font_key,
            face_key: request.face_key,
            glyph_id: request.glyph_id,
            bold: request.bold,
            fractional_offset_x_bits: request.fractional_offset_x_bits,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GlyphCacheKind {
    Monochrome,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlyphAtlasEntry {
    pub slot: u32,
    pub width_px: u32,
    pub height_px: u32,
    pub padding_left_px: u32,
    pub padding_right_px: u32,
    pub cache_kind: GlyphCacheKind,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct ColorGlyphCacheKey {
    pub font_key: LoadedFontKey,
    pub face_key: FontFaceKey,
    pub glyph_id: u32,
}

impl ColorGlyphCacheKey {
    pub fn new(font_key: LoadedFontKey, face_key: FontFaceKey, glyph_id: u32) -> Self {
        Self {
            font_key,
            face_key,
            glyph_id,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ColorGlyphCacheEntry {
    pub slot: u32,
    pub width_px: u32,
    pub height_px: u32,
    pub rgba_bytes: usize,
}

#[derive(Default)]
pub struct GlyphAtlas {
    entries: HashMap<GlyphAtlasKey, GlyphAtlasEntry>,
    next_slot: u32,
}

impl GlyphAtlas {
    pub fn contains(&self, request: GlyphRasterRequest) -> bool {
        self.entries.contains_key(&GlyphAtlasKey::from(request))
    }

    pub fn upsert(
        &mut self,
        request: GlyphRasterRequest,
        rasterized: &RasterizedGlyph,
    ) -> GlyphAtlasEntry {
        let key = GlyphAtlasKey::from(request);
        if let Some(entry) = self.entries.get(&key) {
            return *entry;
        }

        let entry = GlyphAtlasEntry {
            slot: self.next_slot,
            width_px: rasterized
                .width_px
                .saturating_add(MONOCHROME_ATLAS_HORIZONTAL_PADDING_PX.saturating_mul(2)),
            height_px: rasterized.height_px,
            padding_left_px: MONOCHROME_ATLAS_HORIZONTAL_PADDING_PX,
            padding_right_px: MONOCHROME_ATLAS_HORIZONTAL_PADDING_PX,
            cache_kind: GlyphCacheKind::Monochrome,
        };
        self.next_slot = self.next_slot.saturating_add(1);
        self.entries.insert(key, entry);
        entry
    }

    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }
}

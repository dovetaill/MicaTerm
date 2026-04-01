//! Glyph atlas bookkeeping for the staged native terminal renderer.

use std::collections::HashMap;

use crate::app::terminal_font::{GlyphRasterRequest, LoadedFontKey, RasterizedGlyph};

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct GlyphAtlasKey {
    pub font_key: LoadedFontKey,
    pub glyph_id: u32,
    pub bold: bool,
}

impl From<GlyphRasterRequest> for GlyphAtlasKey {
    fn from(request: GlyphRasterRequest) -> Self {
        Self {
            font_key: request.font_key,
            glyph_id: request.glyph_id,
            bold: request.bold,
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
    pub cache_kind: GlyphCacheKind,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct ColorGlyphCacheKey {
    pub font_key: LoadedFontKey,
    pub glyph_id: u32,
}

impl ColorGlyphCacheKey {
    pub fn new(font_key: LoadedFontKey, glyph_id: u32) -> Self {
        Self { font_key, glyph_id }
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
            width_px: rasterized.width_px,
            height_px: rasterized.height_px,
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

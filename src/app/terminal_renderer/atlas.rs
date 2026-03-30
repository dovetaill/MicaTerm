//! Glyph atlas bookkeeping for the staged native terminal renderer.

use std::collections::HashMap;

use crate::app::terminal_font::{FontFaceKey, GlyphRasterRequest, RasterizedGlyph};

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct GlyphAtlasKey {
    pub face: FontFaceKey,
    pub glyph_id: u32,
    pub px_size_bits: u32,
}

impl From<GlyphRasterRequest> for GlyphAtlasKey {
    fn from(request: GlyphRasterRequest) -> Self {
        Self {
            face: request.face,
            glyph_id: request.glyph_id,
            px_size_bits: request.px_size.to_bits(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlyphAtlasEntry {
    pub slot: u32,
    pub width_px: u32,
    pub height_px: u32,
}

#[derive(Default)]
pub struct GlyphAtlas {
    entries: HashMap<GlyphAtlasKey, GlyphAtlasEntry>,
    next_slot: u32,
}

impl GlyphAtlas {
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
        };
        self.next_slot = self.next_slot.saturating_add(1);
        self.entries.insert(key, entry);
        entry
    }

    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }
}

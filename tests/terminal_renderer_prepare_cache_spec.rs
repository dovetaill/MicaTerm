#[cfg(feature = "terminal-native-renderer")]
use anyhow::Result;
#[cfg(feature = "terminal-native-renderer")]
use std::collections::hash_map::DefaultHasher;
#[cfg(feature = "terminal-native-renderer")]
use std::hash::{Hash, Hasher};
#[cfg(feature = "terminal-native-renderer")]
use mica_term::app::terminal_font::mock::MockFontSystem;
#[cfg(feature = "terminal-native-renderer")]
use mica_term::app::terminal_font::{
    FontFallbackFace, FontRequest, FontSystem, GlyphRasterRequest, LoadedFont, RasterizedGlyph,
    ShapedGlyph, ShapedGlyphRun, TextShapingRequest,
};
#[cfg(feature = "terminal-native-renderer")]
use mica_term::app::terminal_layout::{TerminalTextShaper, TextShaper};
#[cfg(feature = "terminal-native-renderer")]
use mica_term::app::terminal_model::{TerminalModelCell, TerminalModelRow};
#[cfg(feature = "terminal-native-renderer")]
use mica_term::app::terminal_renderer::{ShapedTerminalFrame, WgpuTerminalRenderer};

#[cfg(feature = "terminal-native-renderer")]
struct CountingRasterFontSystem {
    inner: MockFontSystem,
    rasterize_glyph_calls: usize,
}

#[cfg(feature = "terminal-native-renderer")]
impl CountingRasterFontSystem {
    fn new() -> Result<Self> {
        Ok(Self {
            inner: MockFontSystem::new()?,
            rasterize_glyph_calls: 0,
        })
    }

    fn rasterize_glyph_calls(&self) -> usize {
        self.rasterize_glyph_calls
    }
}

#[cfg(feature = "terminal-native-renderer")]
impl FontSystem for CountingRasterFontSystem {
    fn load_font(&mut self, request: &FontRequest) -> Result<LoadedFont> {
        self.inner.load_font(request)
    }

    fn shape_text(&mut self, font: &LoadedFont, text: &str) -> Result<Vec<ShapedGlyph>> {
        self.inner.shape_text(font, text)
    }

    fn rasterize_glyph(
        &mut self,
        font: &LoadedFont,
        request: GlyphRasterRequest,
    ) -> Result<RasterizedGlyph> {
        self.rasterize_glyph_calls = self.rasterize_glyph_calls.saturating_add(1);
        self.inner.rasterize_glyph(font, request)
    }

    fn discover_fallback_faces(
        &mut self,
        font: &LoadedFont,
        text: &str,
    ) -> Result<Vec<FontFallbackFace>> {
        self.inner.discover_fallback_faces(font, text)
    }

    fn shape_text_runs(
        &mut self,
        font: &LoadedFont,
        request: &TextShapingRequest,
    ) -> Result<Vec<ShapedGlyphRun>> {
        self.inner.shape_text_runs(font, request)
    }
}

#[cfg(feature = "terminal-native-renderer")]
fn build_ascii_row(row_index: u32, text: &str) -> TerminalModelRow {
    let cells = text
        .chars()
        .enumerate()
        .map(|(col, ch)| TerminalModelCell {
            row: row_index,
            col: col as u32,
            width: 1,
            text: ch.to_string(),
            bold: false,
            underline: false,
            fg_rgba: 0xffd8_dfe8,
            bg_rgba: 0xff0c_1014,
        })
        .collect::<Vec<_>>();

    let mut content_hasher = DefaultHasher::new();
    text.hash(&mut content_hasher);
    false.hash(&mut content_hasher);
    for cell in &cells {
        cell.col.hash(&mut content_hasher);
        cell.width.hash(&mut content_hasher);
        cell.text.hash(&mut content_hasher);
        cell.bold.hash(&mut content_hasher);
        cell.underline.hash(&mut content_hasher);
        cell.fg_rgba.hash(&mut content_hasher);
        cell.bg_rgba.hash(&mut content_hasher);
    }
    let content_hash = content_hasher.finish();

    let mut row_hasher = DefaultHasher::new();
    row_index.hash(&mut row_hasher);
    0xffd8_dfe8u32.hash(&mut row_hasher);
    0xff0c_1014u32.hash(&mut row_hasher);
    0xff0c_1014u32.hash(&mut row_hasher);
    0xff0c_1014u32.hash(&mut row_hasher);
    content_hash.hash(&mut row_hasher);

    TerminalModelRow {
        row_index,
        text: text.into(),
        wrapped: false,
        content_hash,
        row_hash: row_hasher.finish(),
        cells,
    }
}

#[cfg(feature = "terminal-native-renderer")]
fn shape_rows(
    font_system: &mut CountingRasterFontSystem,
    loaded_font: &LoadedFont,
    rows: &[TerminalModelRow],
) -> Result<Vec<mica_term::app::terminal_layout::ShapedRow>> {
    let mut shaper = TerminalTextShaper;
    rows.iter()
        .map(|row| shaper.shape_row(row, loaded_font, font_system))
        .collect()
}

#[cfg(feature = "terminal-native-renderer")]
#[test]
fn native_renderer_reuses_cached_glyph_rasters_for_partial_row_edits() -> Result<()> {
    let mut font_system = CountingRasterFontSystem::new()?;
    let loaded_font = font_system.load_font(&FontRequest::default())?;
    let mut renderer = WgpuTerminalRenderer::new_for_test()?;

    let first_rows = vec![build_ascii_row(0, "abc")];
    let first_frame = ShapedTerminalFrame {
        seqno: 1,
        font: loaded_font.clone(),
        rows: shape_rows(&mut font_system, &loaded_font, &first_rows)?,
    };
    renderer.prepare(&first_frame, &mut font_system)?;
    let first_raster_calls = font_system.rasterize_glyph_calls();

    let second_rows = vec![build_ascii_row(0, "abd")];
    let second_frame = ShapedTerminalFrame {
        seqno: 2,
        font: loaded_font.clone(),
        rows: shape_rows(&mut font_system, &loaded_font, &second_rows)?,
    };
    renderer.prepare(&second_frame, &mut font_system)?;

    assert_eq!(
        font_system.rasterize_glyph_calls(),
        first_raster_calls.saturating_add(1),
        "renderer prepare should reuse cached glyph rasters for unchanged glyphs and only rasterize the newly introduced glyph after a partial row edit"
    );
    assert!(
        renderer.glyph_raster_cache_entry_count() >= first_raster_calls.saturating_add(1),
        "renderer should retain glyph rasters across prepares so repeated scroll/edit frames stay off the font rasterizer hot path"
    );

    Ok(())
}

#[cfg(feature = "terminal-native-renderer")]
#[test]
fn native_renderer_reuses_prepared_rows_for_overlapping_scrollback_viewports() -> Result<()> {
    let mut font_system = CountingRasterFontSystem::new()?;
    let loaded_font = font_system.load_font(&FontRequest::default())?;
    let mut renderer = WgpuTerminalRenderer::new_for_test()?;

    let first_rows = vec![
        build_ascii_row(0, "one"),
        build_ascii_row(1, "two"),
        build_ascii_row(2, "three"),
    ];
    let first_frame = ShapedTerminalFrame {
        seqno: 10,
        font: loaded_font.clone(),
        rows: shape_rows(&mut font_system, &loaded_font, &first_rows)?,
    };
    let first_prepared = renderer.prepare(&first_frame, &mut font_system)?;

    let second_rows = vec![
        build_ascii_row(0, "zero"),
        build_ascii_row(1, "one"),
        build_ascii_row(2, "two"),
    ];
    let second_frame = ShapedTerminalFrame {
        seqno: 11,
        font: loaded_font.clone(),
        rows: shape_rows(&mut font_system, &loaded_font, &second_rows)?,
    };
    let second_prepared = renderer.prepare(&second_frame, &mut font_system)?;

    assert_eq!(
        renderer.last_prepared_row_reuse_count(),
        2,
        "renderer prepare should reuse the overlapping prepared rows when the viewport scrolls by one line instead of rebuilding every visible row from scratch"
    );
    let first_row0 = first_prepared
        .monochrome_glyph_draws
        .iter()
        .filter(|draw| draw.row == 0)
        .collect::<Vec<_>>();
    let first_row1 = first_prepared
        .monochrome_glyph_draws
        .iter()
        .filter(|draw| draw.row == 1)
        .collect::<Vec<_>>();
    let second_row1 = second_prepared
        .monochrome_glyph_draws
        .iter()
        .filter(|draw| draw.row == 1)
        .collect::<Vec<_>>();
    let second_row2 = second_prepared
        .monochrome_glyph_draws
        .iter()
        .filter(|draw| draw.row == 2)
        .collect::<Vec<_>>();

    assert_eq!(first_row0.len(), second_row1.len());
    assert_eq!(first_row1.len(), second_row2.len());
    for (previous, reused) in first_row0.iter().zip(second_row1.iter()) {
        assert_eq!(
            reused.dest_y_px - previous.dest_y_px,
            first_prepared.cell_height_px as i32,
            "reused prepared draws should be rebased downward by one row height when the viewport scrolls"
        );
        assert!(
            reused.upload.is_none(),
            "reused prepared draws should clear upload payloads so the downstream atlas/bitmap caches do not get force-refreshed on every scroll"
        );
    }
    for (previous, reused) in first_row1.iter().zip(second_row2.iter()) {
        assert_eq!(
            reused.dest_y_px - previous.dest_y_px,
            first_prepared.cell_height_px as i32,
            "second reused row should also keep its glyph origins aligned after the row rebase"
        );
        assert!(
            reused.upload.is_none(),
            "all reused rows should keep upload payloads stripped after rebase"
        );
    }

    Ok(())
}

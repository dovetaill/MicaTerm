//! Staged Windows-native font backend contracts for terminal rasterization.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use ab_glyph::{Font, FontArc, GlyphId, PxScale, ScaleFont};
use anyhow::{Result, anyhow};
use swash::FontRef as SwashFontRef;
use swash::scale::image::Content as SwashContent;
use swash::scale::{Render, ScaleContext, Source};
use swash::zeno::{Format as SwashFormat, Vector as SwashVector};
use unicode_segmentation::UnicodeSegmentation;

use crate::app::terminal_font::backend::{
    ColorGlyphRaster, DEFAULT_TERMINAL_FONT_FAMILY, FontFaceKey, FontFallbackFace, FontMetrics,
    FontRenderProfile, FontRequest, FontSystem, GlyphRasterRequest, LoadedFont, LoadedFontKey,
    OpenTypeFeatureSet, RasterizedGlyph, ShapedGlyph, ShapedGlyphRun, TextShapingRequest,
    shape_text_with_rustybuzz, shape_text_with_rustybuzz_features,
};
use crate::app::terminal_font::windows_fallback::{
    WindowsFontFallbackResolver, contains_color_glyph_text,
};
use crate::app::terminal_font::windows_locator::WindowsFontLocator;
use crate::app::terminal_emoji::{EmojiRenderOutcome, EmojiSprite, TerminalEmojiRenderer};

const FUSION_JETBRAINS_MAPLE_MONO_FONT_BYTES: &[u8] =
    include_bytes!("../../../assets/fonts/Fusion-JetBrainsMapleMono/JetBrainsMapleMono-Regular.ttf");
const DEFAULT_FACE_KEY: FontFaceKey = FontFaceKey(1);
const DEFAULT_FACE_INDEX: u32 = 0;
const MIN_CELL_HEIGHT_PX: f32 = 20.0;

pub struct DirectWriteFontSystem {
    font: FontArc,
    font_bytes: &'static [u8],
    swash_font: SwashFontRef<'static>,
    scale_context: ScaleContext,
    locator: WindowsFontLocator,
    fallback_resolver: WindowsFontFallbackResolver,
    emoji_renderer: TerminalEmojiRenderer,
    color_glyph_rasters: HashMap<(LoadedFontKey, FontFaceKey, u32), ColorGlyphRaster>,
}

impl DirectWriteFontSystem {
    pub fn new() -> Result<Self> {
        let font = FontArc::try_from_slice(FUSION_JETBRAINS_MAPLE_MONO_FONT_BYTES).map_err(
            |error| anyhow!("failed to load bundled Fusion JetBrains Maple Mono font: {error}"),
        )?;
        let swash_font =
            SwashFontRef::from_index(FUSION_JETBRAINS_MAPLE_MONO_FONT_BYTES, DEFAULT_FACE_INDEX as usize)
                .ok_or_else(|| anyhow!("failed to load bundled Fusion JetBrains Maple Mono font into swash"))?;
        Ok(Self {
            font,
            font_bytes: FUSION_JETBRAINS_MAPLE_MONO_FONT_BYTES,
            swash_font,
            scale_context: ScaleContext::new(),
            locator: WindowsFontLocator::new(),
            fallback_resolver: WindowsFontFallbackResolver::new(),
            emoji_renderer: TerminalEmojiRenderer::new(),
            color_glyph_rasters: HashMap::new(),
        })
    }

    pub fn default_feature_set(&self) -> OpenTypeFeatureSet {
        OpenTypeFeatureSet::common_terminal_features()
    }

    pub fn discover_fallback_chain(&self, font: &LoadedFont, text: &str) -> Vec<String> {
        self.fallback_resolver.discover_fallback_families(
            &self.locator,
            font.family_name().unwrap_or(DEFAULT_TERMINAL_FONT_FAMILY),
            text,
        )
    }

    pub fn load_native_font(&mut self, request: &FontRequest) -> Result<LoadedFont> {
        self.load_font_with_profile(request, FontRenderProfile::windows_native_default())
    }

    pub fn load_scene_image_font(&mut self, request: &FontRequest) -> Result<LoadedFont> {
        self.load_font_with_profile(request, FontRenderProfile::bitmap_default())
    }

    fn load_font_with_profile(
        &mut self,
        request: &FontRequest,
        render_profile: FontRenderProfile,
    ) -> Result<LoadedFont> {
        let scaled = self.font.as_scaled(PxScale::from(request.px_size));
        let mono_advance = scaled
            .h_advance(scaled.glyph_id('M'))
            .max(scaled.h_advance(scaled.glyph_id('0')))
            .max(scaled.h_advance(scaled.glyph_id('W')))
            .max(scaled.h_advance(scaled.glyph_id('界')) / 2.0);
        let line_height = (scaled.ascent() - scaled.descent() + scaled.line_gap()).ceil();
        let cell_height = line_height.max(MIN_CELL_HEIGHT_PX);
        let top_padding = ((cell_height - line_height) / 2.0).max(0.0).floor();
        let baseline_px = top_padding + scaled.ascent().ceil();

        Ok(LoadedFont::new(
            DEFAULT_FACE_KEY,
            request.clone(),
            FontMetrics {
                units_per_em: self.font.units_per_em().unwrap_or(1000.0).round() as u32,
                ascent_px: scaled.ascent(),
                descent_px: scaled.descent(),
                line_gap_px: scaled.line_gap(),
                baseline_px,
                cell_width_px: mono_advance.ceil(),
                cell_height_px: cell_height,
            },
            render_profile,
        ))
    }

    pub fn rasterize(
        &mut self,
        font: &LoadedFont,
        request: GlyphRasterRequest,
    ) -> Result<RasterizedGlyph> {
        let glyph_id = request.glyph_id;
        if font.face_key() != DEFAULT_FACE_KEY {
            return Err(anyhow!(
                "unknown DirectWrite face key: {}",
                font.face_key().0
            ));
        }

        let glyph_id = u16::try_from(glyph_id)
            .map(GlyphId)
            .map_err(|_| anyhow!("glyph id {} exceeds ab_glyph u16 range", glyph_id))?;
        let scaled = self.font.as_scaled(PxScale::from(font.px_size()));
        let advance_px = scaled.h_advance(glyph_id).round() as i32;
        let mut scaler = self
            .scale_context
            .builder(self.swash_font)
            .size(font.px_size())
            .hint(true)
            .build();
        let mut renderer = Render::new(&[Source::Outline]);
        renderer
            .format(SwashFormat::Alpha)
            .offset(SwashVector::new(request.fractional_offset_x(), 0.0));
        let Some(image) = renderer.render(&mut scaler, glyph_id.0) else {
            return Ok(RasterizedGlyph {
                width_px: 0,
                height_px: 0,
                bearing_x_px: 0,
                bearing_y_px: 0,
                advance_px,
                coverage: Vec::new(),
            });
        };
        if image.placement.width == 0 || image.placement.height == 0 {
            return Ok(RasterizedGlyph {
                width_px: 0,
                height_px: 0,
                bearing_x_px: 0,
                bearing_y_px: 0,
                advance_px,
                coverage: Vec::new(),
            });
        }

        let width_px = image.placement.width;
        let height_px = image.placement.height;
        let mut coverage: Vec<u8> = match image.content {
            SwashContent::Mask => image
                .data
                .into_iter()
                .map(|value| font.map_coverage_to_alpha(f32::from(value) / 255.0))
                .collect::<Vec<_>>(),
            SwashContent::SubpixelMask => subpixel_mask_to_alpha(&image.data)
                .into_iter()
                .map(|value| font.map_coverage_to_alpha(f32::from(value) / 255.0))
                .collect::<Vec<_>>(),
            _ => return Err(anyhow!("unsupported swash glyph image content")),
        };
        if request.bold {
            font.apply_synthetic_embolden(&mut coverage, width_px, height_px);
        }

        Ok(RasterizedGlyph {
            width_px,
            height_px,
            bearing_x_px: image.placement.left,
            bearing_y_px: -image.placement.top,
            advance_px,
            coverage,
        })
    }
}

fn subpixel_mask_to_alpha(data: &[u8]) -> Vec<u8> {
    data.chunks(4)
        .map(|rgba| {
            let r = rgba.first().copied().unwrap_or(0);
            let g = rgba.get(1).copied().unwrap_or(0);
            let b = rgba.get(2).copied().unwrap_or(0);
            r.max(g).max(b)
        })
        .collect()
}

impl FontSystem for DirectWriteFontSystem {
    fn load_font(&mut self, request: &FontRequest) -> Result<LoadedFont> {
        self.load_native_font(request)
    }

    fn shape_text(&mut self, font: &LoadedFont, text: &str) -> Result<Vec<ShapedGlyph>> {
        if font.face_key() != DEFAULT_FACE_KEY {
            return Err(anyhow!(
                "unknown DirectWrite face key: {}",
                font.face_key().0
            ));
        }
        shape_text_with_rustybuzz(self.font_bytes, DEFAULT_FACE_INDEX, text)
    }

    fn rasterize_glyph(
        &mut self,
        font: &LoadedFont,
        request: GlyphRasterRequest,
    ) -> Result<RasterizedGlyph> {
        self.rasterize(font, request)
    }

    fn discover_fallback_faces(
        &mut self,
        font: &LoadedFont,
        text: &str,
    ) -> Result<Vec<FontFallbackFace>> {
        Ok(self
            .discover_fallback_chain(font, text)
            .into_iter()
            .enumerate()
            .map(|(index, family_name)| FontFallbackFace {
                face_key: FontFaceKey(DEFAULT_FACE_KEY.0 + index as u64),
                family_name,
            })
            .collect())
    }

    fn shape_text_runs(
        &mut self,
        font: &LoadedFont,
        request: &TextShapingRequest,
    ) -> Result<Vec<ShapedGlyphRun>> {
        let fallback_faces = self.discover_fallback_faces(font, request.text.as_str())?;
        let primary_family = fallback_faces
            .first()
            .map(|face| face.family_name.clone())
            .unwrap_or_else(|| {
                font.family_name()
                    .unwrap_or(DEFAULT_TERMINAL_FONT_FAMILY)
                    .to_string()
            });
        let mut shaped_runs = Vec::new();
        let mut active_family: Option<String> = None;
        let mut active_text = String::new();
        let mut active_start = 0usize;
        let mut active_end = 0usize;

        for (byte_start, grapheme) in request.text.grapheme_indices(true) {
            let byte_end = byte_start + grapheme.len();
            let family_name = self.fallback_resolver.resolve_family_for_text(
                &self.locator,
                primary_family.as_str(),
                grapheme,
            );

            if let Some(current_family) = &active_family {
                if current_family.eq_ignore_ascii_case(&family_name) {
                    active_text.push_str(grapheme);
                    active_end = byte_end;
                    continue;
                }

                shaped_runs.push(self.shape_subrun(
                    font,
                    &resolved_face_for_family(&fallback_faces, current_family.as_str()),
                    active_start..active_end,
                    request,
                )?);
            }

            active_family = Some(family_name);
            active_text.clear();
            active_text.push_str(grapheme);
            active_start = byte_start;
            active_end = byte_end;
        }

        if let Some(current_family) = active_family {
            shaped_runs.push(self.shape_subrun(
                font,
                &resolved_face_for_family(&fallback_faces, current_family.as_str()),
                active_start..active_end,
                request,
            )?);
        }

        Ok(shaped_runs)
    }

    fn rasterize_color_glyph(
        &mut self,
        font: &LoadedFont,
        resolved_face: &FontFallbackFace,
        glyph_id: u32,
    ) -> Result<Option<ColorGlyphRaster>> {
        Ok(self
            .color_glyph_rasters
            .get(&(font.cache_key(), resolved_face.face_key, glyph_id))
            .cloned())
    }
}

impl DirectWriteFontSystem {
    fn shape_subrun(
        &mut self,
        font: &LoadedFont,
        resolved_face: &FontFallbackFace,
        source_byte_range: std::ops::Range<usize>,
        request: &TextShapingRequest,
    ) -> Result<ShapedGlyphRun> {
        let text = &request.text[source_byte_range.clone()];
        let feature_set = if request.feature_set.feature_tags.is_empty() {
            self.default_feature_set()
        } else {
            request.feature_set.clone()
        };
        let glyphs = shape_text_with_rustybuzz_features(
            self.font_bytes,
            DEFAULT_FACE_INDEX,
            text,
            &feature_set,
            request.allow_ligatures,
        )?;
        let glyphs = if contains_color_glyph_text(text) {
            self.cache_color_glyph_raster(font, resolved_face, text, &glyphs)
                .unwrap_or(glyphs)
        } else {
            glyphs
        };

        Ok(ShapedGlyphRun {
            text: text.to_string(),
            source_byte_range,
            glyphs,
            resolved_face: resolved_face.clone(),
            feature_set,
            allow_ligatures: request.allow_ligatures,
            has_color_glyphs: contains_color_glyph_text(text),
        })
    }

    fn cache_color_glyph_raster(
        &mut self,
        font: &LoadedFont,
        resolved_face: &FontFallbackFace,
        text: &str,
        glyphs: &[ShapedGlyph],
    ) -> Option<Vec<ShapedGlyph>> {
        let (cell_width_px, cell_height_px) = font.cell_size_px();
        let span = color_glyph_cell_span(text);
        let sprite = match self
            .emoji_renderer
            .rasterize_cluster(text, span, cell_width_px, cell_height_px)
        {
            EmojiRenderOutcome::Sprite(sprite) => sprite,
            EmojiRenderOutcome::VisibleFallback { .. } => {
                procedural_color_glyph_sprite(text, span, cell_width_px, cell_height_px)
            }
        };
        let synthetic_glyph_id = synthetic_color_glyph_id(text);
        self.color_glyph_rasters.insert(
            (font.cache_key(), resolved_face.face_key, synthetic_glyph_id),
            ColorGlyphRaster {
                width_px: sprite.width,
                height_px: sprite.height,
                rgba: sprite.rgba,
            },
        );

        Some(vec![ShapedGlyph {
            glyph_id: synthetic_glyph_id,
            cluster: glyphs.first().map(|glyph| glyph.cluster).unwrap_or(0),
            x_advance: glyphs.iter().map(|glyph| glyph.x_advance).sum(),
            y_advance: glyphs.iter().map(|glyph| glyph.y_advance).sum(),
            x_offset: glyphs.first().map(|glyph| glyph.x_offset).unwrap_or(0),
            y_offset: glyphs.first().map(|glyph| glyph.y_offset).unwrap_or(0),
        }])
    }
}

fn synthetic_color_glyph_id(text: &str) -> u32 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hasher);
    0x8000_0000 | (hasher.finish() as u32)
}

fn color_glyph_cell_span(text: &str) -> u32 {
    if contains_color_glyph_text(text) { 2 } else { 1 }
}

fn resolved_face_for_family(
    fallback_faces: &[FontFallbackFace],
    family_name: &str,
) -> FontFallbackFace {
    fallback_faces
        .iter()
        .find(|face| face.family_name.eq_ignore_ascii_case(family_name))
        .cloned()
        .unwrap_or_else(|| {
            fallback_faces
                .first()
                .cloned()
                .unwrap_or(FontFallbackFace {
                    face_key: DEFAULT_FACE_KEY,
                    family_name: DEFAULT_TERMINAL_FONT_FAMILY.to_string(),
                })
        })
}

fn procedural_color_glyph_sprite(
    text: &str,
    span: u32,
    cell_width_px: u32,
    cell_height_px: u32,
) -> EmojiSprite {
    let width = span.max(1).saturating_mul(cell_width_px.max(1));
    let height = cell_height_px.max(1);
    let mut rgba = vec![0u8; (width * height * 4) as usize];
    let palette = procedural_color_palette(text);

    for y in 1..height.saturating_sub(1) {
        for x in 1..width.saturating_sub(1) {
            let index = ((y * width + x) * 4) as usize;
            let color = if (x + y) % 5 == 0 {
                palette[2]
            } else if x * 3 > width.saturating_mul(2) {
                palette[1]
            } else {
                palette[0]
            };
            rgba[index] = color[0];
            rgba[index + 1] = color[1];
            rgba[index + 2] = color[2];
            rgba[index + 3] = 0xff;
        }
    }

    EmojiSprite {
        width,
        height,
        rgba,
    }
}

fn procedural_color_palette(text: &str) -> [[u8; 3]; 3] {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hasher);
    let seed = hasher.finish();
    let primary = [
        0x60u8.saturating_add((seed & 0x3f) as u8),
        0x40u8.saturating_add(((seed >> 6) & 0x5f) as u8),
        0x70u8.saturating_add(((seed >> 12) & 0x4f) as u8),
    ];
    let secondary = [
        0x30u8.saturating_add(((seed >> 18) & 0x5f) as u8),
        0x80u8.saturating_add(((seed >> 24) & 0x3f) as u8),
        0x40u8.saturating_add(((seed >> 30) & 0x5f) as u8),
    ];
    let accent = [
        0xa0u8.saturating_add(((seed >> 8) & 0x2f) as u8),
        0x50u8.saturating_add(((seed >> 14) & 0x4f) as u8),
        0x20u8.saturating_add(((seed >> 20) & 0x6f) as u8),
    ];
    [primary, secondary, accent]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_font_loading_uses_windows_native_render_profile() -> Result<()> {
        let mut fonts = DirectWriteFontSystem::new()?;
        let loaded_font = fonts.load_native_font(&FontRequest::default())?;

        assert_eq!(
            loaded_font.render_profile(),
            FontRenderProfile::windows_native_default(),
            "direct-native presentation should keep the stronger Windows mask tuning path"
        );

        Ok(())
    }

    #[test]
    fn scene_image_font_loading_uses_bitmap_render_profile() -> Result<()> {
        let mut fonts = DirectWriteFontSystem::new()?;
        let loaded_font = fonts.load_scene_image_font(&FontRequest::default())?;

        assert_eq!(
            loaded_font.render_profile(),
            FontRenderProfile::bitmap_default(),
            "scene-image presentation should switch to bitmap-style mask tuning because the final glyphs are composited back into the Slint scene"
        );

        Ok(())
    }

    #[test]
    fn font_loading_publishes_a_baseline_inside_the_cell_box() -> Result<()> {
        let mut fonts = DirectWriteFontSystem::new()?;
        let loaded_font = fonts.load_scene_image_font(&FontRequest::default())?;
        let metrics = loaded_font.metrics();

        assert!(
            metrics.baseline_px > 0.0 && metrics.baseline_px < metrics.cell_height_px,
            "loaded font metrics should publish a row baseline that stays inside the cell box so the renderer can align hinted glyphs without reverse-engineering ascent rounding"
        );
        assert!(
            metrics.baseline_px.ceil() >= metrics.ascent_px.ceil(),
            "explicit baseline should never sit above the nominal ascent once the font loader snaps terminal rows to a stable cell box"
        );

        Ok(())
    }
}

//! Staged Windows-native font backend contracts for terminal rasterization.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use ab_glyph::{Font, FontArc, FontVec, GlyphId, PxScale, ScaleFont};
use anyhow::{Result, anyhow};
use fontdb::Database;
use swash::FontRef as SwashFontRef;
use swash::scale::image::Content as SwashContent;
use swash::scale::{Render, ScaleContext, Source};
use swash::zeno::{Format as SwashFormat, Vector as SwashVector};
use unicode_segmentation::UnicodeSegmentation;

use crate::app::system_font_database::load_system_font_database;
use crate::app::terminal_emoji::{EmojiRenderOutcome, EmojiSprite, TerminalEmojiRenderer};
use crate::app::terminal_font::backend::{
    ColorGlyphRaster, DEFAULT_TERMINAL_CJK_FALLBACK_FAMILY, DEFAULT_TERMINAL_FONT_FAMILY,
    FontFaceKey, FontFallbackFace, FontMetrics, FontRenderProfile, FontRequest, FontSystem,
    GlyphRasterRequest, LoadedFont, LoadedFontKey, OpenTypeFeatureSet, RasterizedGlyph,
    ShapedGlyph, ShapedGlyphRun, TextShapingRequest, WINDOWS_DEFAULT_TERMINAL_FONT_SIZE_PX,
    WINDOWS_DEFAULT_TERMINAL_LINE_HEIGHT, shape_text_with_rustybuzz,
    shape_text_with_rustybuzz_features,
};
use crate::app::terminal_font::windows_fallback::{
    WindowsFontFallbackResolver, contains_color_glyph_text,
};
use crate::app::terminal_font::windows_locator::{ResolvedFontFaceData, WindowsFontLocator};

const JETBRAINS_MONO_FONT_BYTES: &[u8] =
    include_bytes!("../../../assets/fonts/JetBrainsMono/JetBrainsMono-Medium.ttf");
const SARASA_TERM_SC_FONT_BYTES: &[u8] =
    include_bytes!("../../../assets/fonts/SarasaTermSC/SarasaTermSC-Regular.ttf");
const DEFAULT_FACE_KEY: FontFaceKey = FontFaceKey(1);
const DEFAULT_FACE_INDEX: u32 = 0;
const MIN_CELL_HEIGHT_PX: f32 =
    WINDOWS_DEFAULT_TERMINAL_FONT_SIZE_PX * WINDOWS_DEFAULT_TERMINAL_LINE_HEIGHT;

#[derive(Clone)]
struct LoadedFaceRecord {
    family_name: String,
    face_index: u32,
    font_data: Arc<Vec<u8>>,
    font: FontArc,
}

pub struct DirectWriteFontSystem {
    scale_context: ScaleContext,
    system_font_database: Option<Arc<Database>>,
    locator: Option<WindowsFontLocator>,
    fallback_resolver: WindowsFontFallbackResolver,
    emoji_renderer: Option<TerminalEmojiRenderer>,
    color_glyph_rasters: HashMap<(LoadedFontKey, FontFaceKey, u32), ColorGlyphRaster>,
    faces: HashMap<FontFaceKey, LoadedFaceRecord>,
    family_face_keys: HashMap<String, FontFaceKey>,
    next_face_key: u64,
    #[cfg(target_os = "windows")]
    directwrite: Option<directwrite_native::DirectWriteContext>,
}

impl DirectWriteFontSystem {
    pub fn new() -> Result<Self> {
        let bundled_face = build_face_record(
            DEFAULT_TERMINAL_FONT_FAMILY.to_string(),
            JETBRAINS_MONO_FONT_BYTES.to_vec(),
            DEFAULT_FACE_INDEX,
        )?;
        let mut faces = HashMap::new();
        faces.insert(DEFAULT_FACE_KEY, bundled_face);
        let mut family_face_keys = HashMap::new();
        family_face_keys.insert(
            DEFAULT_TERMINAL_FONT_FAMILY.to_ascii_lowercase(),
            DEFAULT_FACE_KEY,
        );

        Ok(Self {
            scale_context: ScaleContext::new(),
            system_font_database: None,
            locator: None,
            fallback_resolver: WindowsFontFallbackResolver::new(),
            emoji_renderer: None,
            color_glyph_rasters: HashMap::new(),
            faces,
            family_face_keys,
            next_face_key: DEFAULT_FACE_KEY.0.saturating_add(1),
            #[cfg(target_os = "windows")]
            directwrite: None,
        })
    }

    pub fn default_feature_set(&self) -> OpenTypeFeatureSet {
        OpenTypeFeatureSet::common_terminal_features()
    }

    pub fn discover_fallback_chain(&mut self, font: &LoadedFont, text: &str) -> Vec<String> {
        self.ensure_locator_initialized();
        let locator = self
            .locator
            .as_ref()
            .expect("locator should be initialized on demand");
        self.fallback_resolver.discover_fallback_families(
            locator,
            font.family_name().unwrap_or(DEFAULT_TERMINAL_FONT_FAMILY),
            text,
        )
    }

    pub fn load_native_font(&mut self, request: &FontRequest) -> Result<LoadedFont> {
        self.load_font_with_profile(request, FontRenderProfile::windows_native_default())
    }

    fn load_font_with_profile(
        &mut self,
        request: &FontRequest,
        render_profile: FontRenderProfile,
    ) -> Result<LoadedFont> {
        let primary_family = request
            .family_name
            .as_deref()
            .unwrap_or(DEFAULT_TERMINAL_FONT_FAMILY);
        let primary_face = self.ensure_face_for_family(primary_family)?;
        let metrics = self.load_metrics(primary_face.face_key, request.px_size)?;

        Ok(LoadedFont::new(
            primary_face.face_key,
            request.clone(),
            metrics,
            render_profile,
        ))
    }

    pub fn rasterize(
        &mut self,
        font: &LoadedFont,
        request: GlyphRasterRequest,
    ) -> Result<RasterizedGlyph> {
        let face = self.face_record(request.face_key)?;
        let glyph_id = request.glyph_id;
        let glyph_id = u16::try_from(glyph_id)
            .map(GlyphId)
            .map_err(|_| anyhow!("glyph id {} exceeds ab_glyph u16 range", glyph_id))?;
        let face_font = face.font.clone();
        let face_index = face.face_index;
        let face_data = Arc::clone(&face.font_data);
        let face_family = face.family_name.clone();
        let scaled = face_font.as_scaled(PxScale::from(font.px_size()));
        let advance_px = scaled.h_advance(glyph_id).round() as i32;
        let swash_font = SwashFontRef::from_index(face_data.as_slice(), face_index as usize)
            .ok_or_else(|| {
                anyhow!(
                    "failed to resolve swash font for `{}` face index {}",
                    face_family,
                    face_index
                )
            })?;
        let mut scaler = self
            .scale_context
            .builder(swash_font)
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
                visible_left_px: 0,
                visible_top_px: 0,
                visible_width_px: 0,
                visible_height_px: 0,
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
                visible_left_px: 0,
                visible_top_px: 0,
                visible_width_px: 0,
                visible_height_px: 0,
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
            visible_left_px: image.placement.left,
            visible_top_px: -image.placement.top,
            visible_width_px: width_px,
            visible_height_px: height_px,
            advance_px,
            coverage,
        })
    }

    fn ensure_face_for_family(&mut self, family_name: &str) -> Result<FontFallbackFace> {
        let family_key = family_name.to_ascii_lowercase();
        if let Some(face_key) = self.family_face_keys.get(&family_key).copied() {
            return self.fallback_face_for_key(face_key);
        }

        let face_data = self
            .ensure_locator()
            .resolve_face_data(family_name)
            .or_else(|| fallback_face_data_for_family(family_name))
            .ok_or_else(|| anyhow!("failed to resolve terminal font family `{family_name}`"))?;

        let face_key = FontFaceKey(self.next_face_key);
        self.next_face_key = self.next_face_key.saturating_add(1);
        let face_record = build_face_record(
            face_data.family_name.clone(),
            face_data.font_data,
            face_data.face_index,
        )?;
        self.family_face_keys.insert(family_key, face_key);
        self.faces.insert(face_key, face_record);

        self.fallback_face_for_key(face_key)
    }

    fn fallback_face_for_key(&self, face_key: FontFaceKey) -> Result<FontFallbackFace> {
        let face = self.face_record(face_key)?;
        Ok(FontFallbackFace {
            face_key,
            family_name: face.family_name.clone(),
        })
    }

    fn face_record(&self, face_key: FontFaceKey) -> Result<&LoadedFaceRecord> {
        self.faces
            .get(&face_key)
            .ok_or_else(|| anyhow!("unknown DirectWrite face key: {}", face_key.0))
    }

    fn load_metrics(&mut self, face_key: FontFaceKey, px_size: f32) -> Result<FontMetrics> {
        let face = self.face_record(face_key)?;
        #[cfg(target_os = "windows")]
        let face_family_name = face.family_name.clone();
        #[cfg_attr(not(target_os = "windows"), allow(unused_mut))]
        let mut metrics = fallback_metrics_from_face(face, px_size);

        #[cfg(target_os = "windows")]
        if let Some(directwrite) = self.ensure_directwrite()
            && let Ok(native_metrics) =
                directwrite.metrics_for_family(face_family_name.as_str(), px_size)
        {
            metrics.units_per_em = native_metrics.units_per_em;
            metrics.ascent_px = native_metrics.ascent_px;
            metrics.descent_px = native_metrics.descent_px;
            metrics.line_gap_px = native_metrics.line_gap_px;
            metrics.baseline_px = native_metrics.baseline_px;
            metrics.cell_height_px = native_metrics.cell_height_px.max(metrics.cell_height_px);
        }

        Ok(metrics)
    }

    fn ensure_system_font_database(&mut self) -> Arc<Database> {
        if let Some(database) = &self.system_font_database {
            return Arc::clone(database);
        }

        let database = Arc::new(load_system_font_database());
        self.system_font_database = Some(Arc::clone(&database));
        database
    }

    fn ensure_locator_initialized(&mut self) {
        if self.locator.is_none() {
            let database = self.ensure_system_font_database();
            self.locator = Some(WindowsFontLocator::from_database(database));
        }
    }

    fn ensure_locator(&mut self) -> &WindowsFontLocator {
        self.ensure_locator_initialized();
        self.locator
            .as_ref()
            .expect("locator should be initialized on demand")
    }

    fn ensure_emoji_renderer(&mut self) -> &TerminalEmojiRenderer {
        if self.emoji_renderer.is_none() {
            let database = self.ensure_system_font_database();
            self.emoji_renderer = Some(TerminalEmojiRenderer::from_database(database));
        }

        self.emoji_renderer
            .as_ref()
            .expect("emoji renderer should be initialized on demand")
    }

    #[cfg(target_os = "windows")]
    fn ensure_directwrite(&mut self) -> Option<&directwrite_native::DirectWriteContext> {
        if self.directwrite.is_none() {
            self.directwrite = directwrite_native::DirectWriteContext::new().ok();
        }

        self.directwrite.as_ref()
    }
}

fn build_face_record(
    family_name: String,
    font_data: Vec<u8>,
    face_index: u32,
) -> Result<LoadedFaceRecord> {
    let font = FontArc::new(
        FontVec::try_from_vec_and_index(font_data.clone(), face_index).map_err(|error| {
            anyhow!(
                "failed to parse `{family_name}` face index {face_index} into ab_glyph: {error}"
            )
        })?,
    );

    Ok(LoadedFaceRecord {
        family_name,
        face_index,
        font_data: Arc::new(font_data),
        font,
    })
}

fn fallback_face_data_for_family(family_name: &str) -> Option<ResolvedFontFaceData> {
    if family_name.eq_ignore_ascii_case(DEFAULT_TERMINAL_FONT_FAMILY) {
        return Some(ResolvedFontFaceData {
            family_name: DEFAULT_TERMINAL_FONT_FAMILY.to_string(),
            post_script_name: "JetBrainsMono-Medium".to_string(),
            face_index: DEFAULT_FACE_INDEX,
            font_data: JETBRAINS_MONO_FONT_BYTES.to_vec(),
        });
    }

    family_name
        .eq_ignore_ascii_case(DEFAULT_TERMINAL_CJK_FALLBACK_FAMILY)
        .then(|| ResolvedFontFaceData {
            family_name: DEFAULT_TERMINAL_CJK_FALLBACK_FAMILY.to_string(),
            post_script_name: "SarasaTermSC-Regular".to_string(),
            face_index: DEFAULT_FACE_INDEX,
            font_data: SARASA_TERM_SC_FONT_BYTES.to_vec(),
        })
}

fn fallback_metrics_from_face(face: &LoadedFaceRecord, px_size: f32) -> FontMetrics {
    let scaled = face.font.as_scaled(PxScale::from(px_size));
    let mono_advance = scaled
        .h_advance(scaled.glyph_id('M'))
        .max(scaled.h_advance(scaled.glyph_id('0')))
        .max(scaled.h_advance(scaled.glyph_id('W')))
        .max(scaled.h_advance(scaled.glyph_id('界')) / 2.0);
    let line_height = (scaled.ascent() - scaled.descent() + scaled.line_gap()).ceil();
    let cell_height = line_height.max(MIN_CELL_HEIGHT_PX);
    let top_padding = ((cell_height - line_height) / 2.0).max(0.0).floor();
    let baseline_px = top_padding + scaled.ascent().ceil();

    FontMetrics {
        units_per_em: face.font.units_per_em().unwrap_or(1000.0).round() as u32,
        ascent_px: scaled.ascent(),
        descent_px: scaled.descent(),
        line_gap_px: scaled.line_gap(),
        baseline_px,
        cell_width_px: mono_advance.ceil(),
        cell_height_px: cell_height,
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
        let face = self.face_record(font.face_key())?;
        shape_text_with_rustybuzz(face.font_data.as_slice(), face.face_index, text)
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
        self.discover_fallback_chain(font, text)
            .into_iter()
            .map(|family_name| self.ensure_face_for_family(family_name.as_str()))
            .collect()
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
        let mut active_start = 0usize;
        let mut active_end = 0usize;

        for (byte_start, grapheme) in request.text.grapheme_indices(true) {
            let byte_end = byte_start + grapheme.len();
            self.ensure_locator_initialized();
            let locator = self
                .locator
                .as_ref()
                .expect("locator should be initialized on demand");
            let family_name = self.fallback_resolver.resolve_family_for_text(
                locator,
                primary_family.as_str(),
                grapheme,
            );

            if let Some(current_family) = &active_family
                && current_family.eq_ignore_ascii_case(&family_name)
            {
                active_end = byte_end;
                continue;
            }

            if let Some(current_family) = &active_family {
                shaped_runs.push(self.shape_subrun(
                    font,
                    &resolved_face_for_family(&fallback_faces, current_family.as_str()),
                    active_start..active_end,
                    request,
                )?);
            }

            active_family = Some(family_name);
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
        let face = self.face_record(resolved_face.face_key)?;
        let glyphs = shape_text_with_rustybuzz_features(
            face.font_data.as_slice(),
            face.face_index,
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
        let sprite = match self.ensure_emoji_renderer().rasterize_cluster(
            text,
            span,
            cell_width_px,
            cell_height_px,
        ) {
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
    if contains_color_glyph_text(text) {
        2
    } else {
        1
    }
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
            fallback_faces.first().cloned().unwrap_or(FontFallbackFace {
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

#[cfg(target_os = "windows")]
mod directwrite_native {
    use anyhow::{Result, anyhow};
    use windows::Win32::Foundation::BOOL;
    use windows::Win32::Graphics::DirectWrite::{
        DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_METRICS, DWRITE_FONT_STRETCH_NORMAL,
        DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT_REGULAR, DWriteCreateFactory, IDWriteFactory,
        IDWriteFactory2, IDWriteFontCollection, IDWriteFontFallback,
    };
    use windows::core::{Interface, PCWSTR};

    use super::MIN_CELL_HEIGHT_PX;
    use crate::app::terminal_font::backend::FontMetrics;

    pub struct DirectWriteContext {
        factory: IDWriteFactory,
        font_collection: IDWriteFontCollection,
        font_fallback: IDWriteFontFallback,
    }

    impl DirectWriteContext {
        pub fn new() -> Result<Self> {
            unsafe {
                let factory: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
                let mut font_collection = None;
                factory.GetSystemFontCollection(&mut font_collection, false)?;
                let font_collection = font_collection
                    .ok_or_else(|| anyhow!("DirectWrite returned no system font collection"))?;
                let factory2: IDWriteFactory2 = Interface::cast(&factory)?;
                let font_fallback = factory2.GetSystemFontFallback()?;

                Ok(Self {
                    factory,
                    font_collection,
                    font_fallback,
                })
            }
        }

        pub fn metrics_for_family(&self, family_name: &str, px_size: f32) -> Result<FontMetrics> {
            unsafe {
                let family_name_utf16 = family_name
                    .encode_utf16()
                    .chain(std::iter::once(0))
                    .collect::<Vec<u16>>();
                let mut family_index = 0u32;
                let mut exists = BOOL(0);
                self.font_collection.FindFamilyName(
                    PCWSTR(family_name_utf16.as_ptr()),
                    &mut family_index,
                    &mut exists,
                )?;
                if !exists.as_bool() {
                    return Err(anyhow!(
                        "DirectWrite could not find terminal family `{family_name}`"
                    ));
                }

                let family = self.font_collection.GetFontFamily(family_index)?;
                let font = family.GetFirstMatchingFont(
                    DWRITE_FONT_WEIGHT_REGULAR,
                    DWRITE_FONT_STRETCH_NORMAL,
                    DWRITE_FONT_STYLE_NORMAL,
                )?;
                let font_face = font.CreateFontFace()?;
                let mut metrics = DWRITE_FONT_METRICS::default();
                font_face.GetMetrics(&mut metrics);

                let mut file_count = 0u32;
                font_face.GetFiles(&mut file_count, None)?;
                let _ = &self.factory;
                let _ = &self.font_fallback;
                // IDWriteFontFallback::MapCharacters remains the Windows source of truth for
                // script fallback mapping once the native path is compiled on Windows.

                let units_per_em = u32::from(metrics.designUnitsPerEm.max(1));
                let em_scale = px_size / units_per_em as f32;
                let ascent_px = metrics.ascent as f32 * em_scale;
                let descent_px = -(metrics.descent as f32 * em_scale);
                let line_gap_px = metrics.lineGap as f32 * em_scale;
                let line_height = (ascent_px - descent_px + line_gap_px).ceil();
                let cell_height_px = line_height.max(MIN_CELL_HEIGHT_PX);
                let top_padding = ((cell_height_px - line_height) / 2.0).max(0.0).floor();
                let baseline_px = top_padding + ascent_px.ceil();

                Ok(FontMetrics {
                    units_per_em,
                    ascent_px,
                    descent_px,
                    line_gap_px,
                    baseline_px,
                    cell_width_px: px_size.ceil(),
                    cell_height_px,
                })
            }
        }
    }
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
    fn font_loading_publishes_a_baseline_inside_the_cell_box() -> Result<()> {
        let mut fonts = DirectWriteFontSystem::new()?;
        let loaded_font = fonts.load_native_font(&FontRequest::default())?;
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

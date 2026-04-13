//! Shared terminal font contracts used by the shaping and renderer pipeline.

use anyhow::Result;
#[cfg(feature = "terminal-native-renderer")]
use anyhow::anyhow;
#[cfg(feature = "terminal-native-renderer")]
use rustybuzz::{BufferClusterLevel, Face, Feature, UnicodeBuffer, shape};
#[cfg(feature = "terminal-native-renderer")]
use std::str::FromStr;

pub const DEFAULT_TERMINAL_FONT_FAMILY: &str = "Sarasa Term SC Nerd";
pub const DEFAULT_TERMINAL_CJK_FALLBACK_FAMILY: &str = DEFAULT_TERMINAL_FONT_FAMILY;
pub const DEFAULT_TERMINAL_EMOJI_FALLBACK_FAMILY: &str = "Segoe UI Emoji";
pub const WINDOWS_DEFAULT_TERMINAL_FONT_CHAIN: &[&str] = &[
    DEFAULT_TERMINAL_FONT_FAMILY,
    DEFAULT_TERMINAL_EMOJI_FALLBACK_FAMILY,
];
pub const DEFAULT_TERMINAL_FONT_SIZE_PX: f32 = 14.0;
pub const DEFAULT_TERMINAL_LINE_HEIGHT: f32 = 1.5;
pub const WINDOWS_DEFAULT_TERMINAL_FONT_SIZE_PX: f32 = 16.0;
pub const WINDOWS_DEFAULT_TERMINAL_CELL_HEIGHT_PX: u32 = 24;
pub const WINDOWS_DEFAULT_TERMINAL_LINE_HEIGHT: f32 =
    WINDOWS_DEFAULT_TERMINAL_CELL_HEIGHT_PX as f32 / WINDOWS_DEFAULT_TERMINAL_FONT_SIZE_PX;
pub const DEFAULT_TERMINAL_LETTER_SPACING_PX: f32 = 1.5;
pub const DEFAULT_TERMINAL_FONT_WEIGHT: &str = "SemiBold";

const GLYPH_COVERAGE_GAMMA: f32 = 1.0;
const GLYPH_ALPHA_GAIN: f32 = 1.0;
const SYNTHETIC_EMBOLDEN_STRENGTH: f32 = 0.46;
#[cfg(feature = "terminal-native-renderer")]
const WINDOWS_NATIVE_COVERAGE_GAMMA: f32 = 1.08;
#[cfg(feature = "terminal-native-renderer")]
const WINDOWS_NATIVE_ALPHA_GAIN: f32 = 1.10;
#[cfg(feature = "terminal-native-renderer")]
const WINDOWS_NATIVE_SYNTHETIC_EMBOLDEN_STRENGTH: f32 = 0.52;
const BITMAP_ATLAS_COVERAGE_GAMMA: f32 = 1.06;
const BITMAP_ATLAS_ALPHA_GAIN: f32 = 1.08;

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct FontFaceKey(pub u64);

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct LoadedFontKey {
    face: FontFaceKey,
    px_size_bits: u32,
    render_profile: FontRenderProfileKey,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FontRenderProfile {
    pub coverage_gamma: f32,
    pub alpha_gain: f32,
    pub synthetic_embolden_strength: f32,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct FontRenderProfileKey {
    coverage_gamma_bits: u32,
    alpha_gain_bits: u32,
    synthetic_embolden_strength_bits: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FontRequest {
    pub family_name: Option<String>,
    pub px_size: f32,
}

impl FontRequest {
    pub fn windows_default() -> Self {
        Self {
            family_name: Some(DEFAULT_TERMINAL_FONT_FAMILY.to_string()),
            px_size: WINDOWS_DEFAULT_TERMINAL_FONT_SIZE_PX,
        }
    }
}

impl Default for FontRequest {
    fn default() -> Self {
        Self {
            family_name: Some(DEFAULT_TERMINAL_FONT_FAMILY.to_string()),
            px_size: DEFAULT_TERMINAL_FONT_SIZE_PX,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FontMetrics {
    pub units_per_em: u32,
    pub ascent_px: f32,
    pub descent_px: f32,
    pub line_gap_px: f32,
    pub baseline_px: f32,
    pub cell_width_px: f32,
    pub cell_height_px: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LoadedFont {
    cache_key: LoadedFontKey,
    request: FontRequest,
    metrics: FontMetrics,
    render_profile: FontRenderProfile,
}

impl Default for FontRenderProfile {
    fn default() -> Self {
        Self {
            coverage_gamma: GLYPH_COVERAGE_GAMMA,
            alpha_gain: GLYPH_ALPHA_GAIN,
            synthetic_embolden_strength: SYNTHETIC_EMBOLDEN_STRENGTH,
        }
    }
}

impl FontRenderProfile {
    pub fn bitmap_default() -> Self {
        Self {
            coverage_gamma: BITMAP_ATLAS_COVERAGE_GAMMA,
            alpha_gain: BITMAP_ATLAS_ALPHA_GAIN,
            synthetic_embolden_strength: SYNTHETIC_EMBOLDEN_STRENGTH,
        }
    }

    #[cfg(feature = "terminal-native-renderer")]
    pub fn windows_native_default() -> Self {
        Self {
            coverage_gamma: WINDOWS_NATIVE_COVERAGE_GAMMA,
            alpha_gain: WINDOWS_NATIVE_ALPHA_GAIN,
            synthetic_embolden_strength: WINDOWS_NATIVE_SYNTHETIC_EMBOLDEN_STRENGTH,
        }
    }
}

impl FontRenderProfileKey {
    fn new(render_profile: FontRenderProfile) -> Self {
        Self {
            coverage_gamma_bits: render_profile.coverage_gamma.to_bits(),
            alpha_gain_bits: render_profile.alpha_gain.to_bits(),
            synthetic_embolden_strength_bits: render_profile.synthetic_embolden_strength.to_bits(),
        }
    }
}

impl LoadedFontKey {
    fn new(face: FontFaceKey, request: &FontRequest, render_profile: FontRenderProfile) -> Self {
        Self {
            face,
            px_size_bits: request.px_size.to_bits(),
            render_profile: FontRenderProfileKey::new(render_profile),
        }
    }
}

impl LoadedFont {
    pub fn new(
        face_key: FontFaceKey,
        request: FontRequest,
        metrics: FontMetrics,
        render_profile: FontRenderProfile,
    ) -> Self {
        Self {
            cache_key: LoadedFontKey::new(face_key, &request, render_profile),
            request,
            metrics,
            render_profile,
        }
    }

    pub fn cache_key(&self) -> LoadedFontKey {
        self.cache_key
    }

    pub fn metrics(&self) -> FontMetrics {
        self.metrics
    }

    pub fn render_profile(&self) -> FontRenderProfile {
        self.render_profile
    }

    pub fn cell_size_px(&self) -> (u32, u32) {
        (
            self.metrics.cell_width_px.ceil() as u32,
            self.metrics.cell_height_px.ceil() as u32,
        )
    }

    #[cfg(feature = "terminal-native-renderer")]
    pub fn raster_request(&self, glyph_id: u32, bold: bool) -> GlyphRasterRequest {
        GlyphRasterRequest::new(self, glyph_id, bold)
    }

    #[cfg(feature = "terminal-native-renderer")]
    pub fn raster_request_for_face(
        &self,
        face_key: FontFaceKey,
        glyph_id: u32,
        bold: bool,
    ) -> GlyphRasterRequest {
        GlyphRasterRequest::for_face(self, face_key, glyph_id, bold)
    }

    #[cfg(feature = "terminal-native-renderer")]
    pub fn raster_request_with_fractional_offset_x(
        &self,
        glyph_id: u32,
        bold: bool,
        fractional_offset_x: f32,
    ) -> GlyphRasterRequest {
        GlyphRasterRequest::with_fractional_offset_x(self, glyph_id, bold, fractional_offset_x)
    }

    #[cfg(feature = "terminal-native-renderer")]
    pub fn raster_request_with_fractional_offset_x_for_face(
        &self,
        face_key: FontFaceKey,
        glyph_id: u32,
        bold: bool,
        fractional_offset_x: f32,
    ) -> GlyphRasterRequest {
        GlyphRasterRequest::with_fractional_offset_x_for_face(
            self,
            face_key,
            glyph_id,
            bold,
            fractional_offset_x,
        )
    }

    #[cfg(feature = "terminal-native-renderer")]
    pub(crate) fn face_key(&self) -> FontFaceKey {
        self.cache_key.face
    }

    #[cfg(feature = "terminal-native-renderer")]
    pub(crate) fn px_size(&self) -> f32 {
        f32::from_bits(self.cache_key.px_size_bits)
    }

    #[cfg(feature = "terminal-native-renderer")]
    pub fn family_name(&self) -> Option<&str> {
        self.request.family_name.as_deref()
    }

    #[cfg(feature = "terminal-native-renderer")]
    pub fn map_coverage_to_alpha(&self, coverage: f32) -> u8 {
        map_glyph_coverage_to_alpha(coverage, self.render_profile())
    }

    #[cfg(feature = "terminal-native-renderer")]
    pub fn apply_synthetic_embolden(&self, alpha: &mut [u8], width: u32, height: u32) {
        apply_synthetic_embolden(
            alpha,
            width,
            height,
            self.render_profile().synthetic_embolden_strength,
        )
    }
}

#[cfg(feature = "terminal-native-renderer")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShapedGlyph {
    pub glyph_id: u32,
    pub cluster: u32,
    pub x_advance: i32,
    pub y_advance: i32,
    pub x_offset: i32,
    pub y_offset: i32,
}

#[cfg(feature = "terminal-native-renderer")]
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct GlyphRasterRequest {
    pub font_key: LoadedFontKey,
    pub face_key: FontFaceKey,
    pub glyph_id: u32,
    pub bold: bool,
    pub fractional_offset_x_bits: u32,
}

#[cfg(feature = "terminal-native-renderer")]
impl GlyphRasterRequest {
    pub fn new(font: &LoadedFont, glyph_id: u32, bold: bool) -> Self {
        Self::for_face(font, font.face_key(), glyph_id, bold)
    }

    pub fn for_face(font: &LoadedFont, face_key: FontFaceKey, glyph_id: u32, bold: bool) -> Self {
        Self::with_fractional_offset_x_for_face(font, face_key, glyph_id, bold, 0.0)
    }

    pub fn with_fractional_offset_x(
        font: &LoadedFont,
        glyph_id: u32,
        bold: bool,
        fractional_offset_x: f32,
    ) -> Self {
        Self::with_fractional_offset_x_for_face(
            font,
            font.face_key(),
            glyph_id,
            bold,
            fractional_offset_x,
        )
    }

    pub fn with_fractional_offset_x_for_face(
        font: &LoadedFont,
        face_key: FontFaceKey,
        glyph_id: u32,
        bold: bool,
        fractional_offset_x: f32,
    ) -> Self {
        Self {
            font_key: font.cache_key(),
            face_key,
            glyph_id,
            bold,
            fractional_offset_x_bits: normalize_fractional_offset_x(fractional_offset_x).to_bits(),
        }
    }

    pub fn fractional_offset_x(self) -> f32 {
        f32::from_bits(self.fractional_offset_x_bits)
    }
}

#[cfg(feature = "terminal-native-renderer")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RasterizedGlyph {
    pub width_px: u32,
    pub height_px: u32,
    pub bearing_x_px: i32,
    pub bearing_y_px: i32,
    pub visible_left_px: i32,
    pub visible_top_px: i32,
    pub visible_width_px: u32,
    pub visible_height_px: u32,
    pub advance_px: i32,
    pub coverage: Vec<u8>,
}

#[cfg(feature = "terminal-native-renderer")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FontFallbackFace {
    pub face_key: FontFaceKey,
    pub family_name: String,
}

#[cfg(feature = "terminal-native-renderer")]
impl FontFallbackFace {
    pub fn primary(font: &LoadedFont) -> Self {
        Self {
            face_key: font.face_key(),
            family_name: font.family_name().unwrap_or("terminal-primary").to_string(),
        }
    }
}

#[cfg(feature = "terminal-native-renderer")]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OpenTypeFeatureSet {
    pub feature_tags: Vec<String>,
}

#[cfg(feature = "terminal-native-renderer")]
impl OpenTypeFeatureSet {
    pub fn common_terminal_features() -> Self {
        Self {
            feature_tags: vec!["liga".to_string(), "calt".to_string()],
        }
    }
}

#[cfg(feature = "terminal-native-renderer")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextShapingRequest {
    pub text: String,
    pub feature_set: OpenTypeFeatureSet,
    pub allow_ligatures: bool,
}

#[cfg(feature = "terminal-native-renderer")]
impl TextShapingRequest {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            feature_set: OpenTypeFeatureSet::common_terminal_features(),
            allow_ligatures: true,
        }
    }
}

#[cfg(feature = "terminal-native-renderer")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShapedGlyphRun {
    pub text: String,
    pub source_byte_range: std::ops::Range<usize>,
    pub glyphs: Vec<ShapedGlyph>,
    pub resolved_face: FontFallbackFace,
    pub feature_set: OpenTypeFeatureSet,
    pub allow_ligatures: bool,
    pub has_color_glyphs: bool,
}

#[cfg(feature = "terminal-native-renderer")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColorGlyphRaster {
    pub width_px: u32,
    pub height_px: u32,
    pub rgba: Vec<u8>,
}

pub trait FontSystem {
    fn load_font(&mut self, request: &FontRequest) -> Result<LoadedFont>;
    #[cfg(feature = "terminal-native-renderer")]
    fn shape_text(&mut self, font: &LoadedFont, text: &str) -> Result<Vec<ShapedGlyph>>;
    #[cfg(feature = "terminal-native-renderer")]
    fn rasterize_glyph(
        &mut self,
        font: &LoadedFont,
        request: GlyphRasterRequest,
    ) -> Result<RasterizedGlyph>;

    #[cfg(feature = "terminal-native-renderer")]
    fn discover_fallback_faces(
        &mut self,
        font: &LoadedFont,
        text: &str,
    ) -> Result<Vec<FontFallbackFace>> {
        let _ = text;
        Ok(vec![FontFallbackFace::primary(font)])
    }

    #[cfg(feature = "terminal-native-renderer")]
    fn shape_text_runs(
        &mut self,
        font: &LoadedFont,
        request: &TextShapingRequest,
    ) -> Result<Vec<ShapedGlyphRun>> {
        let resolved_face = self
            .discover_fallback_faces(font, request.text.as_str())?
            .into_iter()
            .next()
            .unwrap_or_else(|| FontFallbackFace::primary(font));
        let glyphs = self.shape_text(font, request.text.as_str())?;

        Ok(vec![ShapedGlyphRun {
            text: request.text.clone(),
            source_byte_range: 0..request.text.len(),
            glyphs,
            resolved_face,
            feature_set: request.feature_set.clone(),
            allow_ligatures: request.allow_ligatures,
            has_color_glyphs: false,
        }])
    }

    #[cfg(feature = "terminal-native-renderer")]
    fn rasterize_color_glyph(
        &mut self,
        _font: &LoadedFont,
        _resolved_face: &FontFallbackFace,
        _glyph_id: u32,
    ) -> Result<Option<ColorGlyphRaster>> {
        Ok(None)
    }
}

#[cfg(feature = "terminal-native-renderer")]
pub(crate) fn shape_text_with_rustybuzz(
    face_data: &[u8],
    face_index: u32,
    text: &str,
) -> Result<Vec<ShapedGlyph>> {
    shape_text_with_rustybuzz_features(
        face_data,
        face_index,
        text,
        &OpenTypeFeatureSet::default(),
        true,
    )
}

#[cfg(feature = "terminal-native-renderer")]
pub(crate) fn shape_text_with_rustybuzz_features(
    face_data: &[u8],
    face_index: u32,
    text: &str,
    feature_set: &OpenTypeFeatureSet,
    allow_ligatures: bool,
) -> Result<Vec<ShapedGlyph>> {
    let face = Face::from_slice(face_data, face_index)
        .ok_or_else(|| anyhow!("failed to parse font face index {face_index}"))?;
    let mut buffer = UnicodeBuffer::new();
    buffer.push_str(text);
    buffer.set_cluster_level(BufferClusterLevel::MonotoneGraphemes);
    buffer.guess_segment_properties();
    let features = rustybuzz_features(feature_set, allow_ligatures)?;
    let glyph_buffer = shape(&face, &features, buffer);

    Ok(glyph_buffer
        .glyph_infos()
        .iter()
        .zip(glyph_buffer.glyph_positions())
        .map(|(info, position)| ShapedGlyph {
            glyph_id: info.glyph_id,
            cluster: info.cluster,
            x_advance: position.x_advance,
            y_advance: position.y_advance,
            x_offset: position.x_offset,
            y_offset: position.y_offset,
        })
        .collect())
}

#[cfg(feature = "terminal-native-renderer")]
fn rustybuzz_features(
    feature_set: &OpenTypeFeatureSet,
    allow_ligatures: bool,
) -> Result<Vec<Feature>> {
    let mut features = feature_set
        .feature_tags
        .iter()
        .map(|feature_tag| {
            Feature::from_str(feature_tag.as_str())
                .map_err(|error| anyhow!("invalid OpenType feature tag `{feature_tag}`: {error}"))
        })
        .collect::<Result<Vec<_>>>()?;

    if !allow_ligatures {
        for feature_tag in ["-liga", "-clig", "-dlig", "-hlig", "-calt"] {
            features.push(Feature::from_str(feature_tag).map_err(|error| {
                anyhow!("invalid OpenType feature tag `{feature_tag}`: {error}")
            })?);
        }
    }

    Ok(features)
}

pub(crate) fn map_glyph_coverage_to_alpha(coverage: f32, render_profile: FontRenderProfile) -> u8 {
    let adjusted =
        coverage.clamp(0.0, 1.0).powf(render_profile.coverage_gamma) * render_profile.alpha_gain;
    (adjusted.clamp(0.0, 1.0) * 255.0).round() as u8
}

pub(crate) fn terminal_cell_width_px(mono_advance_px: f32) -> f32 {
    mono_advance_px.ceil() + DEFAULT_TERMINAL_LETTER_SPACING_PX.max(0.0)
}

#[cfg(feature = "terminal-native-renderer")]
pub(crate) fn apply_synthetic_embolden(
    alpha: &mut [u8],
    width: u32,
    height: u32,
    synthetic_embolden_strength: f32,
) {
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

            let boosted = (f32::from(base) * synthetic_embolden_strength).round() as u8;
            let target = &mut alpha[row_offset + x + 1];
            *target = (*target).max(boosted);
        }
    }
}

#[cfg(feature = "terminal-native-renderer")]
fn normalize_fractional_offset_x(fractional_offset_x: f32) -> f32 {
    if !fractional_offset_x.is_finite() {
        return 0.0;
    }

    let normalized = fractional_offset_x.fract();
    if normalized < 0.0 {
        normalized + 1.0
    } else {
        normalized
    }
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "terminal-native-renderer")]
    use super::apply_synthetic_embolden;
    use super::{FontRenderProfile, map_glyph_coverage_to_alpha};

    #[test]
    fn glyph_coverage_mapping_keeps_regular_weight_edges_close_to_source_coverage() {
        let profile = FontRenderProfile::default();

        assert_eq!(map_glyph_coverage_to_alpha(1.0, profile), 255);
        assert!(
            map_glyph_coverage_to_alpha(0.5, profile) <= 140,
            "regular-weight grayscale coverage should stay close to the source mask instead of inflating edge alpha and making glyphs look soft"
        );
        assert!(
            map_glyph_coverage_to_alpha(0.2, profile) <= 60,
            "low-coverage anti-aliased edge pixels should not be boosted so much that thin stems start glowing"
        );
    }

    #[test]
    #[cfg(feature = "terminal-native-renderer")]
    fn synthetic_embolden_spreads_ink_one_pixel_to_the_right() {
        let mut alpha = vec![0, 200, 0, 0];

        apply_synthetic_embolden(
            &mut alpha,
            4,
            1,
            FontRenderProfile::default().synthetic_embolden_strength,
        );

        assert_eq!(alpha[1], 200);
        assert!(
            alpha[2] >= 90,
            "synthetic embolden should strengthen the adjacent pixel enough to visibly thicken a regular-weight stem"
        );
    }

    #[test]
    #[cfg(feature = "terminal-native-renderer")]
    fn windows_native_render_profile_makes_mid_coverage_darker_without_glowing_edges() {
        let neutral = FontRenderProfile::default();
        let windows_native = FontRenderProfile::windows_native_default();

        assert!(
            map_glyph_coverage_to_alpha(0.5, windows_native)
                > map_glyph_coverage_to_alpha(0.5, neutral),
            "windows-native tuning should darken mid-coverage pixels so regular stems stop looking washed out"
        );
        assert!(
            map_glyph_coverage_to_alpha(0.2, windows_native) <= 56,
            "windows-native tuning should still keep low-coverage fringe pixels restrained so anti-aliased edges do not start glowing"
        );
        assert!(
            windows_native.synthetic_embolden_strength > neutral.synthetic_embolden_strength,
            "windows-native tuning should slightly strengthen synthetic embolden so bold terminal runs feel closer to the software atlas path"
        );
    }

    #[test]
    fn bitmap_render_profile_darkens_mid_coverage_without_widening_fringe_pixels() {
        let neutral = FontRenderProfile::default();
        let bitmap = FontRenderProfile::bitmap_default();

        assert!(
            map_glyph_coverage_to_alpha(0.5, bitmap) > map_glyph_coverage_to_alpha(0.5, neutral),
            "bitmap tuning should darken mid-coverage pixels so the Windows atlas path looks less gray at Skia scale"
        );
        assert!(
            map_glyph_coverage_to_alpha(0.2, bitmap) <= 58,
            "bitmap tuning should keep low-coverage edge pixels restrained so hinted grayscale masks stay crisp"
        );
    }
}

//! Classifies terminal clusters that need color emoji rendering and resolves preferred emoji fonts.

use fontdb::{Database, ID};
use swash::FontRef;
use swash::scale::image::Content;
use swash::scale::{Render, ScaleContext, Source, StrikeWith};
use swash::shape::ShapeContext;
use swash::text::Script;
use unicode_properties::UnicodeEmoji;

const VISIBLE_EMOJI_FALLBACK_TEXT: &str = "\u{fffd}";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClusterRenderKind {
    Mono,
    Emoji,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EmojiFallbackReason {
    MissingPreferredFont,
    RasterizationFailed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedEmojiFont {
    pub face_id: ID,
    pub family_name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EmojiFontResolution {
    Resolved(ResolvedEmojiFont),
    VisibleFallback {
        replacement_text: String,
        reason: EmojiFallbackReason,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmojiSprite {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EmojiRenderOutcome {
    Sprite(EmojiSprite),
    VisibleFallback {
        replacement_text: String,
        reason: EmojiFallbackReason,
    },
}

pub struct EmojiFontRasterizeRequest<'a> {
    pub text: &'a str,
    pub span: u32,
    pub cell_width: u32,
    pub cell_height: u32,
    pub resolved_font: &'a ResolvedEmojiFont,
    pub font_data: Option<&'a [u8]>,
    pub face_index: Option<u32>,
}

pub trait EmojiRasterizerBackend: Send + Sync {
    fn rasterize(&self, request: EmojiFontRasterizeRequest<'_>) -> Option<EmojiSprite>;
}

pub struct TerminalEmojiRenderer {
    resolver: TerminalEmojiResolver,
    backend: Box<dyn EmojiRasterizerBackend>,
}

enum ResolverSource {
    Database(Database),
    Fixed(EmojiFontResolution),
}

pub struct TerminalEmojiResolver {
    source: ResolverSource,
}

impl TerminalEmojiRenderer {
    pub fn new() -> Self {
        Self::with_backend(
            TerminalEmojiResolver::new(),
            Box::new(SwashEmojiRasterizerBackend),
        )
    }

    pub fn with_backend(
        resolver: TerminalEmojiResolver,
        backend: Box<dyn EmojiRasterizerBackend>,
    ) -> Self {
        Self { resolver, backend }
    }

    pub fn rasterize_cluster(
        &self,
        text: &str,
        span: u32,
        cell_width: u32,
        cell_height: u32,
    ) -> EmojiRenderOutcome {
        match self.resolver.resolve_preferred_font() {
            EmojiFontResolution::Resolved(font) => {
                let sprite = self
                    .resolver
                    .with_face_data(font.face_id, |font_data, face_index| {
                        self.backend.rasterize(EmojiFontRasterizeRequest {
                            text,
                            span,
                            cell_width,
                            cell_height,
                            resolved_font: &font,
                            font_data: Some(font_data),
                            face_index: Some(face_index),
                        })
                    })
                    .flatten()
                    .or_else(|| {
                        self.backend.rasterize(EmojiFontRasterizeRequest {
                            text,
                            span,
                            cell_width,
                            cell_height,
                            resolved_font: &font,
                            font_data: None,
                            face_index: None,
                        })
                    });

                match sprite {
                    Some(sprite) => EmojiRenderOutcome::Sprite(sprite),
                    None => visible_fallback_outcome(EmojiFallbackReason::RasterizationFailed),
                }
            }
            EmojiFontResolution::VisibleFallback {
                replacement_text,
                reason,
            } => EmojiRenderOutcome::VisibleFallback {
                replacement_text,
                reason,
            },
        }
    }
}

impl TerminalEmojiResolver {
    pub fn new() -> Self {
        let mut database = Database::new();
        database.load_system_fonts();
        Self {
            source: ResolverSource::Database(database),
        }
    }

    pub fn from_database(database: Database) -> Self {
        Self {
            source: ResolverSource::Database(database),
        }
    }

    pub fn from_resolution(resolution: EmojiFontResolution) -> Self {
        Self {
            source: ResolverSource::Fixed(resolution),
        }
    }

    pub fn resolve_preferred_font(&self) -> EmojiFontResolution {
        match &self.source {
            ResolverSource::Database(database) => resolve_preferred_font_in_database(database),
            ResolverSource::Fixed(resolution) => resolution.clone(),
        }
    }

    fn with_face_data<T>(&self, face_id: ID, f: impl FnOnce(&[u8], u32) -> T) -> Option<T> {
        match &self.source {
            ResolverSource::Database(database) => database.with_face_data(face_id, f),
            ResolverSource::Fixed(_) => None,
        }
    }
}

pub fn classify_cluster_render_kind(text: &str) -> ClusterRenderKind {
    if text.is_empty() || text.chars().all(char::is_whitespace) || contains_private_use(text) {
        return ClusterRenderKind::Mono;
    }

    let saw_emoji = text.chars().any(|ch| ch.is_emoji_char());
    let has_emoji_presentation_markers =
        text.contains('\u{fe0f}') || text.contains('\u{200d}') || text.contains('\u{20e3}');

    if saw_emoji || has_emoji_presentation_markers {
        ClusterRenderKind::Emoji
    } else {
        ClusterRenderKind::Mono
    }
}

pub fn preferred_emoji_families() -> &'static [&'static str] {
    if cfg!(target_os = "windows") {
        &["Segoe UI Emoji"]
    } else if cfg!(target_os = "linux") {
        &[
            "Noto Color Emoji",
            "Twitter Color Emoji",
            "EmojiOne Color",
            "JoyPixels",
        ]
    } else if cfg!(target_os = "macos") {
        &["Apple Color Emoji"]
    } else {
        &["Noto Color Emoji", "Segoe UI Emoji"]
    }
}

fn resolve_preferred_font_in_database(database: &Database) -> EmojiFontResolution {
    for preferred_family in preferred_emoji_families() {
        if let Some(face) = database.faces().find(|face| {
            face.families
                .iter()
                .any(|family| family.0.eq_ignore_ascii_case(preferred_family))
        }) {
            return EmojiFontResolution::Resolved(ResolvedEmojiFont {
                face_id: face.id,
                family_name: preferred_family.to_string(),
            });
        }
    }

    visible_font_fallback(EmojiFallbackReason::MissingPreferredFont)
}

fn visible_font_fallback(reason: EmojiFallbackReason) -> EmojiFontResolution {
    EmojiFontResolution::VisibleFallback {
        replacement_text: VISIBLE_EMOJI_FALLBACK_TEXT.to_string(),
        reason,
    }
}

fn visible_fallback_outcome(reason: EmojiFallbackReason) -> EmojiRenderOutcome {
    EmojiRenderOutcome::VisibleFallback {
        replacement_text: VISIBLE_EMOJI_FALLBACK_TEXT.to_string(),
        reason,
    }
}

fn contains_private_use(text: &str) -> bool {
    text.chars().any(|ch| {
        matches!(
            ch as u32,
            0xE000..=0xF8FF | 0xF0000..=0xFFFFD | 0x100000..=0x10FFFD
        )
    })
}

struct SwashEmojiRasterizerBackend;

impl EmojiRasterizerBackend for SwashEmojiRasterizerBackend {
    fn rasterize(&self, request: EmojiFontRasterizeRequest<'_>) -> Option<EmojiSprite> {
        let font_data = request.font_data?;
        let face_index = request.face_index? as usize;
        let font = FontRef::from_index(font_data, face_index)?;
        let glyphs = shape_emoji_glyphs(font, request.text, request.cell_height as f32);
        if glyphs.is_empty() {
            return None;
        }

        let mut scale_context = ScaleContext::new();
        let mut scaler = scale_context
            .builder(font)
            .size(request.cell_height as f32)
            .build();
        let mut rendered_glyphs = Vec::new();

        for glyph in glyphs {
            let image = Render::new(&[
                Source::ColorOutline(0),
                Source::ColorBitmap(StrikeWith::BestFit),
            ])
            .render(&mut scaler, glyph.id)?;
            if image.content != Content::Color
                || image.placement.width == 0
                || image.placement.height == 0
            {
                continue;
            }

            rendered_glyphs.push(RenderedGlyphImage {
                left: glyph.x.round() as i32 + image.placement.left,
                top: glyph.y.round() as i32 + image.placement.top,
                width: image.placement.width,
                height: image.placement.height,
                rgba: image.data,
            });
        }

        if rendered_glyphs.is_empty() {
            return None;
        }

        compose_rendered_glyphs(
            request.span.max(1) * request.cell_width,
            request.cell_height,
            &rendered_glyphs,
        )
    }
}

#[derive(Clone, Copy)]
struct PositionedGlyph {
    id: u16,
    x: f32,
    y: f32,
}

struct RenderedGlyphImage {
    left: i32,
    top: i32,
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

fn shape_emoji_glyphs(font: FontRef<'_>, text: &str, size: f32) -> Vec<PositionedGlyph> {
    let mut context = ShapeContext::new();
    let mut shaper = context.builder(font).script(Script::Common).size(size).build();
    let mut glyphs = Vec::new();
    let mut pen_x = 0.0f32;

    shaper.add_str(text);
    shaper.shape_with(|cluster| {
        for glyph in cluster.glyphs {
            glyphs.push(PositionedGlyph {
                id: glyph.id,
                x: pen_x + glyph.x,
                y: glyph.y,
            });
            pen_x += glyph.advance;
        }
    });

    glyphs
}

fn compose_rendered_glyphs(
    sprite_width: u32,
    sprite_height: u32,
    glyphs: &[RenderedGlyphImage],
) -> Option<EmojiSprite> {
    let mut min_x = i32::MAX;
    let mut max_x = i32::MIN;
    let mut min_y = i32::MAX;
    let mut max_y = i32::MIN;

    for glyph in glyphs {
        min_x = min_x.min(glyph.left);
        max_x = max_x.max(glyph.left + glyph.width as i32);
        min_y = min_y.min(glyph.top - glyph.height as i32);
        max_y = max_y.max(glyph.top);
    }

    if min_x >= max_x || min_y >= max_y {
        return None;
    }

    let content_width = (max_x - min_x) as i32;
    let content_height = (max_y - min_y) as i32;
    let pad_x = ((sprite_width as i32 - content_width).max(0)) / 2;
    let pad_y = ((sprite_height as i32 - content_height).max(0)) / 2;
    let mut rgba = vec![0u8; (sprite_width * sprite_height * 4) as usize];

    for glyph in glyphs {
        let dest_x = pad_x + glyph.left - min_x;
        let dest_y = pad_y + max_y - glyph.top;
        blit_rgba_glyph(
            &mut rgba,
            sprite_width,
            sprite_height,
            dest_x,
            dest_y,
            glyph.width,
            glyph.height,
            &glyph.rgba,
        );
    }

    if rgba.chunks_exact(4).all(|pixel| pixel[3] == 0) {
        return None;
    }

    Some(EmojiSprite {
        width: sprite_width,
        height: sprite_height,
        rgba,
    })
}

fn blit_rgba_glyph(
    dst: &mut [u8],
    dst_width: u32,
    dst_height: u32,
    dest_x: i32,
    dest_y: i32,
    glyph_width: u32,
    glyph_height: u32,
    src: &[u8],
) {
    for y in 0..glyph_height as i32 {
        let target_y = dest_y + y;
        if !(0..dst_height as i32).contains(&target_y) {
            continue;
        }

        for x in 0..glyph_width as i32 {
            let target_x = dest_x + x;
            if !(0..dst_width as i32).contains(&target_x) {
                continue;
            }

            let src_index = ((y as u32 * glyph_width + x as u32) * 4) as usize;
            let dst_index = ((target_y as u32 * dst_width + target_x as u32) * 4) as usize;
            composite_rgba_pixel(&mut dst[dst_index..dst_index + 4], &src[src_index..src_index + 4]);
        }
    }
}

fn composite_rgba_pixel(dst: &mut [u8], src: &[u8]) {
    let src_alpha = f32::from(src[3]) / 255.0;
    if src_alpha <= 0.0 {
        return;
    }

    let dst_alpha = f32::from(dst[3]) / 255.0;
    let out_alpha = src_alpha + dst_alpha * (1.0 - src_alpha);
    if out_alpha <= 0.0 {
        return;
    }

    for channel in 0..3 {
        let src_value = f32::from(src[channel]) / 255.0;
        let dst_value = f32::from(dst[channel]) / 255.0;
        let out_value =
            (src_value * src_alpha + dst_value * dst_alpha * (1.0 - src_alpha)) / out_alpha;
        dst[channel] = (out_value.clamp(0.0, 1.0) * 255.0).round() as u8;
    }

    dst[3] = (out_alpha.clamp(0.0, 1.0) * 255.0).round() as u8;
}

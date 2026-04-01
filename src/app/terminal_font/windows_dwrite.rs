//! Staged Windows-native font backend contracts for terminal rasterization.

use ab_glyph::{Font, FontArc, GlyphId, PxScale, ScaleFont};
use anyhow::{Result, anyhow};
use swash::FontRef as SwashFontRef;
use swash::scale::image::Content as SwashContent;
use swash::scale::{Render, ScaleContext, Source};
use swash::zeno::{Format as SwashFormat, Vector as SwashVector};

use crate::app::terminal_font::backend::{
    FontFaceKey, FontMetrics, FontRenderProfile, FontRequest, FontSystem, LoadedFont,
    RasterizedGlyph, ShapedGlyph, shape_text_with_rustybuzz,
};

const SARASA_FONT_BYTES: &[u8] = include_bytes!("../../../ui/fonts/SarasaTermSCNerd-Regular.ttf");
const DEFAULT_FACE_KEY: FontFaceKey = FontFaceKey(1);
const DEFAULT_FACE_INDEX: u32 = 0;

pub struct DirectWriteFontSystem {
    font: FontArc,
    font_bytes: &'static [u8],
    swash_font: SwashFontRef<'static>,
    scale_context: ScaleContext,
}

impl DirectWriteFontSystem {
    pub fn new() -> Result<Self> {
        let font = FontArc::try_from_slice(SARASA_FONT_BYTES)
            .map_err(|error| anyhow!("failed to load bundled Sarasa terminal font: {error}"))?;
        let swash_font = SwashFontRef::from_index(SARASA_FONT_BYTES, DEFAULT_FACE_INDEX as usize)
            .ok_or_else(|| anyhow!("failed to load bundled Sarasa terminal font into swash"))?;
        Ok(Self {
            font,
            font_bytes: SARASA_FONT_BYTES,
            swash_font,
            scale_context: ScaleContext::new(),
        })
    }

    pub fn rasterize(
        &mut self,
        font: &LoadedFont,
        glyph_id: u32,
        bold: bool,
    ) -> Result<RasterizedGlyph> {
        let request = font.raster_request(glyph_id, bold);
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
            .offset(SwashVector::new(0.0, 0.0));
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
        let scaled = self.font.as_scaled(PxScale::from(request.px_size));
        let mono_advance = scaled
            .h_advance(scaled.glyph_id('M'))
            .max(scaled.h_advance(scaled.glyph_id('0')))
            .max(scaled.h_advance(scaled.glyph_id('W')))
            .max(scaled.h_advance(scaled.glyph_id('界')) / 2.0);
        let line_height = scaled.ascent() - scaled.descent() + scaled.line_gap();

        Ok(LoadedFont::new(
            DEFAULT_FACE_KEY,
            request.clone(),
            FontMetrics {
                units_per_em: self.font.units_per_em().unwrap_or(1000.0).round() as u32,
                ascent_px: scaled.ascent(),
                descent_px: scaled.descent(),
                line_gap_px: scaled.line_gap(),
                cell_width_px: mono_advance.ceil(),
                cell_height_px: line_height.ceil(),
            },
            FontRenderProfile::windows_native_default(),
        ))
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
        glyph_id: u32,
        bold: bool,
    ) -> Result<RasterizedGlyph> {
        self.rasterize(font, glyph_id, bold)
    }
}

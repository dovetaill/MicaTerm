//! Bundled terminal faces and the grid metrics shared by the terminal and its preview.

use std::{borrow::Cow, sync::Arc};

use gpui_kit::{
    App, Font, FontFallbacks, FontFeatures, FontWeight, Pixels, Point, SharedString, TextRun,
    WindowTextSystem, point, px,
};

pub use crate::domain::{BUNDLED_MAPLE_FONT_FAMILY, BUNDLED_SARASA_FONT_FAMILY};

#[derive(Clone, Copy)]
pub(crate) struct TerminalFontTuning {
    size_scale: f32,
    line_height_ratio: f32,
    baseline_offset_em: f32,
    dark_weight: FontWeight,
    pub bold_weight: FontWeight,
}

const STANDARD: TerminalFontTuning = TerminalFontTuning {
    size_scale: 1.0,
    line_height_ratio: 1.46,
    baseline_offset_em: 0.0,
    dark_weight: FontWeight::NORMAL,
    bold_weight: FontWeight::BOLD,
};

pub(crate) struct TerminalFontPreset {
    pub family: &'static str,
    pub label: &'static str,
    faces: &'static [&'static [u8]],
    tuning: TerminalFontTuning,
}

macro_rules! preset {
    ($family:expr, $label:expr, $dir:literal, [$($face:literal),+],
     $scale:literal, $line_height:literal, $baseline:expr, $dark:ident, $bold:ident) => {
        TerminalFontPreset {
            family: $family,
            label: $label,
            faces: &[$(include_bytes!(concat!("../../assets/fonts/", $dir, "/", $dir, "-", $face, ".ttf"))),+],
            tuning: TerminalFontTuning {
                size_scale: $scale,
                line_height_ratio: $line_height,
                baseline_offset_em: $baseline,
                dark_weight: FontWeight::$dark,
                bold_weight: FontWeight::$bold,
            },
        }
    };
}

// The five tuning columns are optical scale, line height, baseline in em, dark regular weight and ANSI bold weight.
#[rustfmt::skip]
const SARASA: TerminalFontPreset = preset!(BUNDLED_SARASA_FONT_FAMILY, "Sarasa Term SC Nerd (推荐)", "SarasaTermSCNerd",
    ["Regular", "SemiBold"], 1.00, 1.50, -0.015, NORMAL, SEMIBOLD);

#[rustfmt::skip]
pub(crate) const TERMINAL_FONT_PRESETS: &[TerminalFontPreset] = &[
    SARASA,
    preset!(BUNDLED_MAPLE_FONT_FAMILY, BUNDLED_MAPLE_FONT_FAMILY, "JetBrainsMapleMono",
        ["Regular", "Medium", "Bold"], 1.00, 1.48, -0.025, MEDIUM, BOLD),
    preset!("JetBrains Mono", "JetBrains Mono", "JetBrainsMono",
        ["Regular", "Medium", "Bold"], 0.99, 1.46, -0.015, MEDIUM, BOLD),
    preset!("Fira Code", "Fira Code", "FiraCode",
        ["Regular", "Medium", "Bold"], 0.99, 1.47, -0.025, MEDIUM, BOLD),
    preset!("Cascadia Mono", "Cascadia Mono", "CascadiaMono",
        ["Regular", "SemiBold"], 1.00, 1.45, -0.015, NORMAL, SEMIBOLD),
    preset!("Geist Mono", "Geist Mono", "GeistMono",
        ["Regular", "Medium", "SemiBold"], 1.00, 1.44, -0.015, MEDIUM, SEMIBOLD),
    preset!("CommitMono", "Commit Mono", "CommitMono",
        ["Regular", "Bold"], 1.00, 1.46, -0.01, NORMAL, BOLD),
    preset!("Meslo LG M", "Meslo LG M", "MesloLGM",
        ["Regular", "Bold"], 1.00, 1.45, -0.035, NORMAL, BOLD),
    preset!("Source Code Pro", "Source Code Pro", "SourceCodePro",
        ["Regular", "Medium", "Semibold"], 1.02, 1.47, -0.025, MEDIUM, SEMIBOLD),
    preset!("IBM Plex Mono", "IBM Plex Mono", "IBMPlexMono",
        ["Regular", "Medium", "SemiBold"], 1.01, 1.46, -0.02, MEDIUM, SEMIBOLD),
    preset!("Hack", "Hack", "Hack",
        ["Regular", "Bold"], 1.00, 1.48, -0.015, NORMAL, BOLD),
    preset!("Ubuntu Mono", "Ubuntu Mono", "UbuntuMono",
        ["Regular", "Bold"], 1.07, 1.42, 0.015, NORMAL, BOLD),
];

pub fn register_bundled_fonts(cx: &App) {
    let fonts = TERMINAL_FONT_PRESETS
        .iter()
        .flat_map(|preset| preset.faces.iter().map(|&bytes| Cow::Borrowed(bytes)))
        .collect();
    cx.text_system()
        .add_fonts(fonts)
        .expect("无法注册内置终端字体");
}

#[derive(Clone)]
pub(crate) struct TerminalFont {
    pub font: Font,
    pub tuning: TerminalFontTuning,
}

impl TerminalFont {
    pub fn resolve(
        family: &str,
        dark: bool,
        available_names: impl FnOnce() -> Vec<String>,
    ) -> Self {
        let name = family.trim();
        let preset = TERMINAL_FONT_PRESETS
            .iter()
            .find(|p| p.family.eq_ignore_ascii_case(name) || p.label.eq_ignore_ascii_case(name));
        let (family, tuning): (SharedString, _) = match preset {
            None if !name.is_empty()
                && let Some(installed) = {
                    let query = name.to_lowercase();
                    available_names()
                        .into_iter()
                        .find(|n| n.to_lowercase() == query)
                } =>
            {
                (installed.into(), STANDARD)
            }
            preset => {
                let preset = preset.unwrap_or(&SARASA);
                (preset.family.into(), preset.tuning)
            }
        };
        let mut fallbacks = Vec::new();
        if family != BUNDLED_SARASA_FONT_FAMILY {
            fallbacks.push(BUNDLED_SARASA_FONT_FAMILY.to_string());
        }
        #[cfg(target_os = "windows")]
        let platform = [
            "Consolas",
            "Microsoft YaHei UI",
            "Segoe UI Emoji",
            "Segoe UI Symbol",
        ];
        #[cfg(target_os = "macos")]
        let platform = ["Menlo", "PingFang SC", "Apple Color Emoji"];
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        let platform = ["DejaVu Sans Mono", "Noto Sans CJK SC"];
        fallbacks.extend(platform.into_iter().map(str::to_owned));
        let font = Font {
            family,
            features: FontFeatures(Arc::new(vec![
                ("calt".into(), 0),
                ("liga".into(), 0),
                ("clig".into(), 0),
                ("zero".into(), 1),
                ("tnum".into(), 1),
            ])),
            fallbacks: Some(FontFallbacks::from_fonts(fallbacks)),
            weight: if dark {
                tuning.dark_weight
            } else {
                FontWeight::NORMAL
            },
            style: Default::default(),
        };
        Self { font, tuning }
    }

    pub fn measure(&self, size: f32, text: &WindowTextSystem) -> TerminalFontMetrics {
        let size = size * self.tuning.size_scale;
        let width = |sample: &'static str| {
            let run = TextRun {
                len: sample.len(),
                font: self.font.clone(),
                ..Default::default()
            };
            text.shape_line(sample.into(), px(size), &[run], None)
                .width()
        };
        // GPUI only snaps glyphs more than one pixel off-grid. Keep the measured advance exact.
        let cell_w = width("M");
        let wide_gap = cell_w * 2.0 - width("汉");
        TerminalFontMetrics {
            font_size: px(size),
            cell_w,
            cell_h: px((size * self.tuning.line_height_ratio).round().max(14.0)),
            baseline_offset: px(size * self.tuning.baseline_offset_em),
            wide_offset: if wide_gap > px(0.5) {
                wide_gap * 0.5
            } else {
                px(0.0)
            },
        }
    }
}

pub(crate) struct TerminalFontMetrics {
    pub font_size: Pixels,
    pub cell_w: Pixels,
    pub cell_h: Pixels,
    baseline_offset: Pixels,
    wide_offset: Pixels,
}

impl TerminalFontMetrics {
    pub fn advance(&self, wide: bool) -> Pixels {
        if wide { self.cell_w * 2.0 } else { self.cell_w }
    }

    pub fn text_origin(&self, x: Pixels, y: Pixels, wide: bool) -> Point<Pixels> {
        point(
            if wide { x + self.wide_offset } else { x },
            y + self.baseline_offset,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_bundled_face_has_grid_safe_features_and_a_distinct_bold_weight() {
        let mut families = std::collections::HashSet::new();
        assert_eq!(TERMINAL_FONT_PRESETS.len(), 12);
        for preset in TERMINAL_FONT_PRESETS {
            assert!(families.insert(preset.family));
            assert!(preset.faces.len() >= 2);
            for face in preset.faces {
                assert!(face.starts_with(b"\0\x01\0\0") || face.starts_with(b"OTTO"));
                let tables = u16::from_be_bytes([face[4], face[5]]) as usize;
                assert!(
                    !face[12..12 + tables * 16]
                        .chunks_exact(16)
                        .any(|t| &t[..4] == b"fvar")
                );
            }
            for dark in [false, true] {
                let resolved = TerminalFont::resolve(preset.family, dark, || unreachable!());
                assert_eq!(resolved.font.family.as_str(), preset.family);
                assert!(resolved.font.weight < resolved.tuning.bold_weight);
                assert!((1.42..=1.50).contains(&resolved.tuning.line_height_ratio));
                assert_eq!(
                    resolved.font.features.tag_value_list(),
                    &[
                        ("calt".into(), 0),
                        ("liga".into(), 0),
                        ("clig".into(), 0),
                        ("zero".into(), 1),
                        ("tnum".into(), 1),
                    ]
                );
                if preset.family != BUNDLED_SARASA_FONT_FAMILY {
                    assert_eq!(
                        resolved.font.fallbacks.unwrap().fallback_list()[0],
                        BUNDLED_SARASA_FONT_FAMILY
                    );
                }
            }
        }
    }

    #[test]
    fn custom_families_resolve_to_the_installed_spelling_or_sarasa() {
        let installed = || vec!["Example Mono".to_owned()];
        assert_eq!(
            TerminalFont::resolve("  example MONO ", true, installed)
                .font
                .family
                .as_str(),
            "Example Mono"
        );
        for name in ["", "Missing Family"] {
            assert_eq!(
                TerminalFont::resolve(name, false, installed)
                    .font
                    .family
                    .as_str(),
                BUNDLED_SARASA_FONT_FAMILY
            );
        }
        let light = TerminalFont::resolve("geist mono", false, installed);
        let dark = TerminalFont::resolve("Geist Mono", true, installed);
        assert_eq!(light.font.weight, FontWeight::NORMAL);
        assert_eq!(dark.font.weight, FontWeight::MEDIUM);
        assert_eq!(
            TerminalFont::resolve("Ubuntu Mono", true, installed)
                .tuning
                .size_scale,
            1.07
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn registered_fonts_fit_narrow_and_wide_cells_at_the_effective_size() {
        gpui_kit::platform::headless().run(|cx| {
            register_bundled_fonts(cx);
            let text = WindowTextSystem::new(cx.text_system().clone());
            let installed = text.all_font_names();
            for preset in TERMINAL_FONT_PRESETS {
                assert!(
                    installed.iter().any(|name| name == preset.family),
                    "{}",
                    preset.family
                );
                let resolved = TerminalFont::resolve(preset.family, true, || unreachable!());
                let metrics = resolved.measure(16.0, &text);
                let natural = |sample: &'static str| {
                    let run = TextRun {
                        len: sample.len(),
                        font: resolved.font.clone(),
                        ..Default::default()
                    };
                    text.shape_line(sample.into(), metrics.font_size, &[run], None)
                        .width()
                };
                assert_eq!(metrics.advance(false), natural("M"));
                let gap = metrics.advance(true) - natural("汉");
                let offset = metrics.text_origin(px(0.), px(0.), true).x;
                assert_eq!(offset, if gap > px(0.5) { gap * 0.5 } else { px(0.) });
                assert_eq!(metrics.text_origin(px(0.), px(0.), false).x, px(0.));
            }
            // The Linux event loop starts after the launch callback returns.
            cx.spawn(async |cx| cx.update(|cx| cx.quit())).detach();
        });
    }
}

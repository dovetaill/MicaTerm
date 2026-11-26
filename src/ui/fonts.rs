//! Bundled terminal faces and the grid metrics shared by the terminal and its preview.

use std::{
    borrow::Cow,
    sync::{Arc, OnceLock},
};

use gpui_kit::component::searchable_list::{SearchableGroup, SearchableListItem, SearchableVec};

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

fn base_family(name: &str) -> &str {
    let mut name = name.trim();
    while let Some((family, style)) = name.rsplit_once(' ') {
        if ![
            "Regular",
            "Medium",
            "Semibold",
            "Bold",
            "Italic",
            "Light",
            "Thin",
            "Heavy",
            "Black",
            "ExtraBold",
            "ExtraLight",
        ]
        .iter()
        .any(|s| style.eq_ignore_ascii_case(s))
        {
            break;
        }
        name = family;
    }
    if name.eq_ignore_ascii_case("Meslo") {
        "Meslo LG M"
    } else {
        name
    }
}

fn bundled_preset(name: &str) -> Option<&'static TerminalFontPreset> {
    TERMINAL_FONT_PRESETS
        .iter()
        .find(|p| p.family.eq_ignore_ascii_case(name) || p.label.eq_ignore_ascii_case(name))
}

fn deduplicate_families(names: &mut Vec<String>) {
    names.sort_by_cached_key(|name| (base_family(name).to_lowercase(), name.len()));
    names.dedup_by(|a, b| base_family(a).to_lowercase() == base_family(b).to_lowercase());
}

pub(crate) fn installed_font_names(cx: &App) -> &'static [String] {
    static NAMES: OnceLock<Vec<String>> = OnceLock::new();
    NAMES.get_or_init(|| {
        let mut names = cx.text_system().all_font_names();
        deduplicate_families(&mut names);
        names
    })
}

#[derive(Clone)]
pub(crate) struct FontChoice {
    family: SharedString,
    label: SharedString,
}

impl SearchableListItem for FontChoice {
    type Value = SharedString;
    fn title(&self) -> SharedString {
        self.label.clone()
    }
    fn value(&self) -> &SharedString {
        &self.family
    }
}

pub(crate) fn font_choices(
    current: &SharedString,
    cx: &App,
) -> SearchableVec<SearchableGroup<FontChoice>> {
    let bundled = TERMINAL_FONT_PRESETS.iter().map(|p| FontChoice {
        family: p.family.into(),
        label: p.label.into(),
    });
    let text = cx.text_system();
    let local = installed_font_names(cx)
        .iter()
        .filter(|name| {
            if bundled_preset(base_family(name)).is_some() {
                return false;
            }
            if current.as_str() == name.as_str() {
                return true;
            }
            let id = text.resolve_font(&gpui_kit::font(name.as_str().to_owned()));
            let advance = |ch| text.advance(id, px(16.), ch).map(|size| size.width);
            advance('i').is_ok_and(|width| {
                ['W', '0', ' ']
                    .iter()
                    .all(|&ch| advance(ch).is_ok_and(|w| w == width))
            })
        })
        .map(|name| FontChoice {
            family: name.clone().into(),
            label: base_family(name).to_owned().into(),
        });
    SearchableVec::new(vec![
        SearchableGroup::new("内置精选等宽").items(bundled),
        SearchableGroup::new("本机已安装等宽").items(local),
    ])
}

#[derive(Clone)]
pub(crate) struct TerminalFont {
    pub font: Font,
    pub tuning: TerminalFontTuning,
}

impl TerminalFont {
    pub fn resolve<'a>(
        family: &str,
        dark: bool,
        available_names: impl FnOnce() -> &'a [String],
    ) -> Self {
        let name = base_family(family);
        let (family, tuning): (SharedString, _) = if let Some(preset) = bundled_preset(name) {
            (preset.family.into(), preset.tuning)
        } else {
            let query = name.to_lowercase();
            if let Some(installed) = available_names()
                .iter()
                .find(|n| base_family(n).to_lowercase() == query)
            {
                (installed.clone().into(), STANDARD)
            } else {
                let preset = if name.eq_ignore_ascii_case("Cascadia Code") {
                    bundled_preset("Cascadia Mono").unwrap()
                } else {
                    &SARASA
                };
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
    fn aliases_and_installed_weights_share_one_choice_per_family() {
        let mut names = [
            "Fira Mono Medium",
            "Fira Mono",
            "FIRA MONO Bold",
            "Cascadia Code",
            "Source Code Pro Medium",
            "Example Mono Regular",
            "Example Mono",
        ]
        .map(str::to_owned)
        .to_vec();
        deduplicate_families(&mut names);
        assert_eq!(
            names,
            [
                "Cascadia Code",
                "Example Mono",
                "Fira Mono",
                "Source Code Pro Medium"
            ]
        );
        for (old, expected) in [
            ("source code pro medium", "Source Code Pro"),
            ("meslo", "Meslo LG M"),
            ("Commit Mono", "CommitMono"),
            ("Fira Mono Medium", "Fira Mono"),
            ("Fira Mono", "Fira Mono"),
            ("cascadia code", "Cascadia Code"),
        ] {
            assert_eq!(
                TerminalFont::resolve(old, true, || &names)
                    .font
                    .family
                    .as_str(),
                expected
            );
        }
        assert_eq!(
            TerminalFont::resolve("Cascadia Code", false, || &[])
                .font
                .family
                .as_str(),
            "Cascadia Mono"
        );
        assert_eq!(
            TerminalFont::resolve("Fira Mono", false, || &[])
                .font
                .family
                .as_str(),
            BUNDLED_SARASA_FONT_FAMILY
        );
        let only_medium = ["Fira Mono Medium".to_owned()];
        assert_eq!(
            TerminalFont::resolve("Fira Mono", false, || &only_medium)
                .font
                .family
                .as_str(),
            "Fira Mono Medium"
        );
    }

    #[test]
    fn custom_families_resolve_to_the_installed_spelling_or_sarasa() {
        let names = ["Example Mono".to_owned()];
        let installed = || names.as_slice();
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
            use gpui_kit::component::{IndexPath, searchable_list::SearchableListDelegate};
            let choices = font_choices(&BUNDLED_SARASA_FONT_FAMILY.into(), cx);
            assert_eq!(choices.sections_count(cx), 2);
            assert_eq!(choices.items_count(0), 12);
            let mut families = std::collections::HashSet::new();
            for section in 0..2 {
                for row in 0..choices.items_count(section) {
                    let choice = choices.item(IndexPath::new(row).section(section)).unwrap();
                    assert!(
                        families.insert(base_family(&choice.family).to_lowercase()),
                        "{}",
                        choice.family
                    );
                }
            }
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

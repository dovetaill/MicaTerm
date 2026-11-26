//! Curated UI + Terminal theme catalog inspired by Superlogical Rex & Termius,
//! plus custom Ghostty & iTerm2 (`.itermcolors`) theme importer and runtime registry.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock};

use alacritty_terminal::vte::ansi::Rgb;
use anyhow::{Context, Result, bail};
use parking_lot::RwLock;

use crate::domain::{AppSettings, DEFAULT_DARK_THEME_ID, DEFAULT_LIGHT_THEME_ID};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeCategory {
    Signature,
    Modern,
    WarmInky,
    ArcticStudio,
    CyberNeon,
    Custom,
}

impl ThemeCategory {
    pub const BUILTIN_FILTERS: [Self; 5] = [
        Self::Signature,
        Self::Modern,
        Self::WarmInky,
        Self::ArcticStudio,
        Self::CyberNeon,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Signature => "Rex & Termius",
            Self::Modern => "现代人气",
            Self::WarmInky => "温润护眼",
            Self::ArcticStudio => "极地与经典",
            Self::CyberNeon => "赛博霓虹",
            Self::Custom => "自定义导入",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeSpec {
    pub id: String,
    pub name: String,
    pub subtitle: String,
    pub badge: String,
    pub category: ThemeCategory,
    pub dark: bool,
    pub term_bg: u32,
    pub term_fg: u32,
    pub ui_bg: u32,
    pub sidebar_bg: u32,
    pub titlebar_bg: u32,
    pub surface_bg: u32,
    pub muted_fg: u32,
    pub accent: u32,
    pub cursor: u32,
    pub cursor_text: Option<u32>,
    pub selection: u32,
    pub ansi: [u32; 16],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomThemeEntry {
    pub file_name: String,
    pub path: PathBuf,
    pub theme_id: Option<String>,
    pub theme_name: Option<String>,
    pub dark: Option<bool>,
    pub error: Option<String>,
}

pub struct ThemePack {
    pub revision: u64,
    pub themes: Vec<ThemeSpec>,
    pub builtin_count: usize,
    pub custom_entries: Vec<CustomThemeEntry>,
}

impl ThemePack {
    pub fn loaded_custom_count(&self) -> usize {
        self.custom_entries
            .iter()
            .filter(|e| e.error.is_none())
            .count()
    }

    pub fn get(&self, id: &str) -> Option<&ThemeSpec> {
        self.themes.iter().find(|t| t.id == id)
    }

    pub fn resolve(&self, id: &str, dark_fallback: bool) -> ThemeSpec {
        if let Some(found) = self.get(id).filter(|t| t.dark == dark_fallback) {
            return found.clone();
        }
        let fallback_id = if dark_fallback {
            DEFAULT_DARK_THEME_ID
        } else {
            DEFAULT_LIGHT_THEME_ID
        };
        self.get(fallback_id)
            .cloned()
            .expect("built-in default theme must exist in ThemePack")
    }

    /// Ensures `settings.dark_theme` and `settings.light_theme` point to valid themes of matching polarity.
    pub fn reconcile_settings(&self, settings: &mut AppSettings) -> bool {
        let mut changed = false;
        let resolved_dark = self.resolve(&settings.dark_theme, true).id;
        if settings.dark_theme != resolved_dark {
            settings.dark_theme = resolved_dark;
            changed = true;
        }
        let resolved_light = self.resolve(&settings.light_theme, false).id;
        if settings.light_theme != resolved_light {
            settings.light_theme = resolved_light;
            changed = true;
        }
        changed
    }
}

pub struct ThemeRegistry {
    current: RwLock<Arc<ThemePack>>,
    next_rev: AtomicU64,
}

impl ThemeRegistry {
    pub fn global() -> &'static Self {
        static GLOBAL: LazyLock<ThemeRegistry> = LazyLock::new(|| {
            let builtins = builtin_themes();
            let builtin_count = builtins.len();
            let pack = Arc::new(ThemePack {
                revision: 1,
                themes: builtins,
                builtin_count,
                custom_entries: Vec::new(),
            });
            ThemeRegistry {
                current: RwLock::new(pack),
                next_rev: AtomicU64::new(2),
            }
        });
        &GLOBAL
    }

    pub fn current(&self) -> Arc<ThemePack> {
        self.current.read().clone()
    }

    pub fn resolve(&self, id: &str, dark_fallback: bool) -> ThemeSpec {
        self.current.read().resolve(id, dark_fallback)
    }

    pub fn reload_from_dir(&self, custom_dir: Option<&Path>) -> Arc<ThemePack> {
        let mut themes = builtin_themes();
        let builtin_count = themes.len();
        let mut custom_entries = Vec::new();

        if let Some(dir) = custom_dir
            && dir.exists()
        {
            match fs::read_dir(dir) {
                Ok(read_dir) => {
                    let mut paths = Vec::new();
                    for item in read_dir {
                        match item {
                            Ok(entry) => {
                                let p = entry.path();
                                if p.is_file() && !is_hidden_file(&p) {
                                    paths.push(p);
                                }
                            }
                            Err(err) => {
                                custom_entries.push(CustomThemeEntry {
                                    file_name: "<读取目录项失败>".to_string(),
                                    path: dir.to_path_buf(),
                                    theme_id: None,
                                    theme_name: None,
                                    dark: None,
                                    error: Some(format!("枚举主题目录项失败: {err}")),
                                });
                            }
                        }
                    }
                    paths.sort();

                    for path in paths {
                        let file_name = path
                            .file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_default();
                        match fs::read_to_string(&path)
                            .with_context(|| format!("读取文件失败 `{file_name}`"))
                            .and_then(|raw| parse_custom_theme_file(&path, &raw))
                        {
                            Ok(spec) => {
                                custom_entries.push(CustomThemeEntry {
                                    file_name,
                                    path,
                                    theme_id: Some(spec.id.clone()),
                                    theme_name: Some(spec.name.clone()),
                                    dark: Some(spec.dark),
                                    error: None,
                                });
                                themes.push(spec);
                            }
                            Err(err) => {
                                custom_entries.push(CustomThemeEntry {
                                    file_name,
                                    path,
                                    theme_id: None,
                                    theme_name: None,
                                    dark: None,
                                    error: Some(format!("{err:#}")),
                                });
                            }
                        }
                    }
                }
                Err(err) => {
                    custom_entries.push(CustomThemeEntry {
                        file_name: dir
                            .file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_else(|| "themes".to_string()),
                        path: dir.to_path_buf(),
                        theme_id: None,
                        theme_name: None,
                        dark: None,
                        error: Some(format!("无法读取主题目录 `{}`: {err}", dir.display())),
                    });
                }
            }
        }

        let rev = self.next_rev.fetch_add(1, Ordering::Relaxed);
        let pack = Arc::new(ThemePack {
            revision: rev,
            themes,
            builtin_count,
            custom_entries,
        });
        *self.current.write() = pack.clone();
        pack
    }

    pub fn import_files(&self, target_dir: &Path, sources: &[PathBuf]) -> Result<Arc<ThemePack>> {
        if sources.is_empty() {
            return Ok(self.current());
        }
        // Validate every candidate file in memory and detect batch filename collisions before writing
        let mut staged: Vec<(&Path, String, String)> = Vec::with_capacity(sources.len());
        let mut seen_names = std::collections::HashSet::new();
        for src in sources {
            let fname = src
                .file_name()
                .and_then(|n| n.to_str())
                .filter(|n| !n.is_empty() && !n.starts_with('.'))
                .with_context(|| format!("无效的主题文件名 `{}`", src.display()))?
                .to_string();
            if !seen_names.insert(fname.clone()) {
                bail!("单次导入包含重复的主题文件名 `{fname}`");
            }
            let raw = fs::read_to_string(src)
                .with_context(|| format!("读取主题文件失败 `{}`", src.display()))?;
            let _ = parse_custom_theme_file(src, &raw)
                .with_context(|| format!("解析主题文件 `{fname}` 失败"))?;
            staged.push((src.as_path(), fname, raw));
        }

        fs::create_dir_all(target_dir)
            .with_context(|| format!("创建主题目录失败 `{}`", target_dir.display()))?;

        let write_res: Result<()> = staged.into_iter().try_for_each(|(src, fname, raw)| {
            let dst = target_dir.join(&fname);
            let same_file = dst.exists()
                && fs::canonicalize(src)
                    .ok()
                    .zip(fs::canonicalize(&dst).ok())
                    .is_some_and(|(a, b)| a == b);
            if !same_file {
                fs::write(&dst, raw.as_bytes())
                    .with_context(|| format!("写入主题文件到 `{}` 失败", dst.display()))?;
            }
            Ok(())
        });

        let pack = self.reload_from_dir(Some(target_dir));
        write_res.map(|()| pack)
    }

    pub fn remove_custom_file(&self, target_dir: &Path, file_name: &str) -> Result<Arc<ThemePack>> {
        if file_name.contains('/') || file_name.contains('\\') || file_name == ".." {
            bail!("非法的主题文件名");
        }
        let target = target_dir.join(file_name);
        if target.exists() {
            fs::remove_file(&target)
                .with_context(|| format!("删除主题文件失败 `{}`", target.display()))?;
        }
        Ok(self.reload_from_dir(Some(target_dir)))
    }
}

fn is_hidden_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.starts_with('.'))
}

// ---------------------------------------------------------------------------
// Color Science & Contrast Utilities
// ---------------------------------------------------------------------------

#[inline]
pub const fn u32_to_rgb(hex: u32) -> Rgb {
    Rgb {
        r: ((hex >> 16) & 0xff) as u8,
        g: ((hex >> 8) & 0xff) as u8,
        b: (hex & 0xff) as u8,
    }
}

#[inline]
pub const fn rgb_to_u32(rgb: Rgb) -> u32 {
    ((rgb.r as u32) << 16) | ((rgb.g as u32) << 8) | (rgb.b as u32)
}

#[inline]
pub fn srgb_channel_to_linear(c: u8) -> f32 {
    let v = f32::from(c) / 255.0;
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

#[inline]
pub fn relative_luminance(hex: u32) -> f32 {
    let rgb = u32_to_rgb(hex);
    0.2126 * srgb_channel_to_linear(rgb.r)
        + 0.7152 * srgb_channel_to_linear(rgb.g)
        + 0.0722 * srgb_channel_to_linear(rgb.b)
}

#[inline]
pub fn contrast_ratio(a: u32, b: u32) -> f32 {
    let la = relative_luminance(a);
    let lb = relative_luminance(b);
    let (hi, lo) = if la >= lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

#[inline]
pub fn mix_hex(a: u32, b: u32, t: f32) -> u32 {
    let t = t.clamp(0.0, 1.0);
    let ra = ((a >> 16) & 0xff) as f32;
    let ga = ((a >> 8) & 0xff) as f32;
    let ba = (a & 0xff) as f32;
    let rb = ((b >> 16) & 0xff) as f32;
    let gb = ((b >> 8) & 0xff) as f32;
    let bb = (b & 0xff) as f32;
    let r = (ra + (rb - ra) * t).round().clamp(0.0, 255.0) as u32;
    let g = (ga + (gb - ga) * t).round().clamp(0.0, 255.0) as u32;
    let bl = (ba + (bb - ba) * t).round().clamp(0.0, 255.0) as u32;
    (r << 16) | (g << 8) | bl
}

/// Picks whichever of deep ink (`#101216`) or pure white (`#FFFFFF`) achieves higher WCAG contrast.
#[inline]
pub fn contrasting_ink(bg: u32) -> u32 {
    if contrast_ratio(bg, 0x101216) >= contrast_ratio(bg, 0xFFFFFF) {
        0x101216
    } else {
        0xFFFFFF
    }
}

/// Ensures `fg` achieves at least `min_ratio` WCAG contrast against `bg`, choosing whichever
/// pole (`#FFFFFF` or `#0A0B0E`) can actually satisfy `min_ratio`.
pub fn ensure_readable_fg(fg: u32, bg: u32, dark_bg: bool, min_ratio: f32) -> u32 {
    if contrast_ratio(fg, bg) >= min_ratio {
        return fg;
    }
    let preferred_pole = if dark_bg { 0xFFFFFF } else { 0x0A0B0E };
    let alternate_pole = if dark_bg { 0x0A0B0E } else { 0xFFFFFF };
    let pole = if contrast_ratio(preferred_pole, bg) >= min_ratio
        || contrast_ratio(preferred_pole, bg) >= contrast_ratio(alternate_pole, bg)
    {
        preferred_pole
    } else {
        alternate_pole
    };
    if contrast_ratio(pole, bg) <= min_ratio {
        return pole;
    }
    let mut lo = 0.0f32;
    let mut hi = 1.0f32;
    let mut best = pole;
    for _ in 0..10 {
        let mid = (lo + hi) * 0.5;
        let cand = mix_hex(fg, pole, mid);
        if contrast_ratio(cand, bg) >= min_ratio {
            best = cand;
            hi = mid;
        } else {
            lo = mid;
        }
    }
    best
}

// ---------------------------------------------------------------------------
// Ghostty & iTerm2 (.itermcolors) Custom Theme Parser
// ---------------------------------------------------------------------------

pub fn parse_custom_theme_file(path: &Path, raw: &str) -> Result<ThemeSpec> {
    let file_name = path
        .file_name()
        .and_then(|s| s.to_str())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("Custom Theme");
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(file_name);
    let display_name = stem.replace(['_', '-'], " ");
    let id = custom_id_from_filename(file_name);

    let is_iterm = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("itermcolors"))
        || raw.contains("<plist")
        || raw.contains("Ansi 0 Color");

    if is_iterm {
        parse_itermcolors(&id, stem, raw)
    } else {
        parse_ghostty_or_kv(&id, &display_name, raw)
    }
}

/// Lossless, order-independent ID derived from the exact file name so `A B.conf`,
/// `A-B.conf`, and `A_B.conf` never collide with each other or built-in theme IDs.
fn custom_id_from_filename(file_name: &str) -> String {
    let mut out = String::from("custom-file:");
    for b in file_name.as_bytes() {
        match *b {
            b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' => out.push(*b as char),
            b'A'..=b'Z' => out.push(char::from(b + 32)),
            other => {
                use std::fmt::Write as _;
                let _ = write!(&mut out, "~{other:02x}");
            }
        }
    }
    out
}

fn parse_hex_color(raw: &str) -> Option<u32> {
    let clean = raw
        .trim()
        .trim_matches(['"', '\'', ','])
        .trim_start_matches("0x")
        .trim_start_matches('#');
    if clean.len() == 6 && clean.chars().all(|c| c.is_ascii_hexdigit()) {
        u32::from_str_radix(clean, 16).ok()
    } else if clean.len() == 8 && clean.chars().all(|c| c.is_ascii_hexdigit()) {
        // #RRGGBBAA -> take RGB
        u32::from_str_radix(&clean[..6], 16).ok()
    } else {
        None
    }
}

fn parse_ghostty_or_kv(id: &str, name: &str, raw: &str) -> Result<ThemeSpec> {
    let mut bg: Option<u32> = None;
    let mut fg: Option<u32> = None;
    let mut cursor: Option<u32> = None;
    let mut cursor_text: Option<u32> = None;
    let mut selection: Option<u32> = None;
    let mut ansi: [Option<u32>; 16] = [None; 16];
    let mut parsed_count = 0usize;

    for (line_idx, line) in raw.lines().enumerate() {
        let line_no = line_idx + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("//") {
            continue;
        }
        let Some((k_raw, v_raw)) = trimmed.split_once('=').or_else(|| trimmed.split_once(':'))
        else {
            continue;
        };
        let key = k_raw
            .trim()
            .trim_start_matches("*.")
            .to_ascii_lowercase();
        let val = v_raw.trim();

        if key == "palette" {
            let (idx_str, hex_str) = val
                .split_once('=')
                .with_context(|| format!("第 {line_no} 行 `palette` 格式无效（应为 N=#RRGGBB）"))?;
            let idx = idx_str
                .trim()
                .parse::<usize>()
                .with_context(|| format!("第 {line_no} 行 `palette` 索引无效 `{idx_str}`"))?;
            let c = parse_hex_color(hex_str)
                .with_context(|| format!("第 {line_no} 行 `palette` 颜色值无效 `{hex_str}`"))?;
            if idx < 16 {
                ansi[idx] = Some(c);
            }
            parsed_count += 1;
            continue;
        }

        let require_hex = |field: &str| -> Result<u32> {
            parse_hex_color(val)
                .with_context(|| format!("第 {line_no} 行 `{field}` 颜色值无效 `{val}`（需为 #RRGGBB）"))
        };

        match key.as_str() {
            "background" => {
                bg = Some(require_hex(&key)?);
                parsed_count += 1;
            }
            "foreground" => {
                fg = Some(require_hex(&key)?);
                parsed_count += 1;
            }
            "cursor-color" | "cursor" | "cursorcolor" => {
                cursor = Some(require_hex(&key)?);
                parsed_count += 1;
            }
            "cursor-text" | "cursor_text" | "cursortext" => {
                if !val.eq_ignore_ascii_case("cellbackground")
                    && !val.eq_ignore_ascii_case("cellforeground")
                {
                    cursor_text = Some(require_hex(&key)?);
                }
                parsed_count += 1;
            }
            "selection-background" | "selection_background" | "selection" => {
                selection = Some(require_hex(&key)?);
                parsed_count += 1;
            }
            _ => {
                if let Some(num) = key.strip_prefix("color")
                    && let Ok(idx) = num.parse::<usize>()
                    && idx < 16
                {
                    ansi[idx] = Some(require_hex(&key)?);
                    parsed_count += 1;
                }
            }
        }
    }

    if parsed_count == 0 {
        bail!("未识别到有效的 Ghostty / 终端配色字段（需包含 background / foreground / palette）");
    }

    Ok(assemble_imported_spec(
        id,
        name,
        "Ghostty 外部导入配色",
        bg,
        fg,
        cursor,
        cursor_text,
        selection,
        ansi,
    ))
}

fn parse_itermcolors(id: &str, name: &str, raw: &str) -> Result<ThemeSpec> {
    let mut bg: Option<u32> = None;
    let mut fg: Option<u32> = None;
    let mut cursor: Option<u32> = None;
    let mut cursor_text: Option<u32> = None;
    let mut selection: Option<u32> = None;
    let mut ansi: [Option<u32>; 16] = [None; 16];
    let mut found = 0usize;

    let mut rest = raw;
    while let Some(k_start) = rest.find("<key>") {
        let after_k = &rest[k_start + 5..];
        let Some(k_end) = after_k.find("</key>") else {
            break;
        };
        let raw_key = after_k[..k_end].trim();
        let (base_key, is_variant) = if let Some(b) = raw_key.strip_suffix(" (Dark)") {
            (b, true)
        } else if let Some(b) = raw_key.strip_suffix(" (Light)") {
            (b, true)
        } else {
            (raw_key, false)
        };
        let after_key = &after_k[k_end + 6..];

        let trimmed_after = after_key.trim_start();
        if let Some(after_dict_open) = trimmed_after.strip_prefix("<dict>")
            && let Some(d_end) = after_dict_open.find("</dict>")
        {
            let dict_body = &after_dict_open[..d_end];
            if let Some(rgb) = extract_plist_rgb(dict_body) {
                let mut set_slot = |slot: &mut Option<u32>| {
                    if !is_variant || slot.is_none() {
                        *slot = Some(rgb);
                    }
                    found += 1;
                };
                match base_key {
                    "Background Color" => set_slot(&mut bg),
                    "Foreground Color" => set_slot(&mut fg),
                    "Cursor Color" => set_slot(&mut cursor),
                    "Cursor Text Color" => set_slot(&mut cursor_text),
                    "Selection Color" => set_slot(&mut selection),
                    _ => {
                        if let Some(mid) = base_key
                            .strip_prefix("Ansi ")
                            .and_then(|s| s.strip_suffix(" Color"))
                            && let Ok(idx) = mid.trim().parse::<usize>()
                            && idx < 16
                        {
                            set_slot(&mut ansi[idx]);
                        }
                    }
                }
            }
            rest = &after_dict_open[d_end + 7..];
        } else {
            rest = after_key;
        }
    }

    if found == 0 {
        bail!("未识别到有效的 iTerm2 (.itermcolors) 颜色字典节点");
    }

    Ok(assemble_imported_spec(
        id,
        name,
        "iTerm2 外部导入配色",
        bg,
        fg,
        cursor,
        cursor_text,
        selection,
        ansi,
    ))
}

fn extract_plist_rgb(dict_xml: &str) -> Option<u32> {
    let r = extract_plist_component(dict_xml, "Red Component")?;
    let g = extract_plist_component(dict_xml, "Green Component")?;
    let b = extract_plist_component(dict_xml, "Blue Component")?;
    let to_u8 = |v: f32| {
        let scaled = if v > 1.001 { v } else { v * 255.0 };
        scaled.round().clamp(0.0, 255.0) as u32
    };
    Some((to_u8(r) << 16) | (to_u8(g) << 8) | to_u8(b))
}

fn extract_plist_component(dict_xml: &str, comp_key: &str) -> Option<f32> {
    let pat = format!("<key>{comp_key}</key>");
    let idx = dict_xml.find(&pat)?;
    let tail = dict_xml[idx + pat.len()..].trim_start();
    for tag in ["real", "integer"] {
        let open = format!("<{tag}>");
        let close = format!("</{tag}>");
        if let Some(inner) = tail.strip_prefix(&open)
            && let Some(e) = inner.find(&close)
        {
            return inner[..e].trim().parse::<f32>().ok();
        }
    }
    None
}

#[allow(clippy::too_many_arguments)]
fn assemble_imported_spec(
    id: &str,
    name: &str,
    subtitle: &str,
    bg: Option<u32>,
    fg: Option<u32>,
    cursor: Option<u32>,
    cursor_text: Option<u32>,
    selection: Option<u32>,
    ansi_opt: [Option<u32>; 16],
) -> ThemeSpec {
    let term_bg = bg.unwrap_or(0x181A20);
    let dark = relative_luminance(term_bg) < 0.42;
    let fallback = if dark {
        builtin_rex_obsidian()
    } else {
        builtin_rex_daylight()
    };
    let term_fg = fg.unwrap_or_else(|| {
        ensure_readable_fg(fallback.term_fg, term_bg, dark, 4.5)
    });
    let mut ansi = fallback.ansi;
    for (i, slot) in ansi_opt.into_iter().enumerate() {
        if let Some(c) = slot {
            ansi[i] = c;
        }
    }
    let accent = ensure_readable_fg(
        ansi_opt[4]
            .or(ansi_opt[12])
            .or(cursor)
            .unwrap_or(fallback.accent),
        term_bg,
        dark,
        if dark { 3.2 } else { 2.8 },
    );
    let cur = cursor.unwrap_or(accent);
    let sel = selection.unwrap_or(accent);
    let (sidebar_bg, titlebar_bg, surface_bg, muted_fg) = if dark {
        (
            mix_hex(term_bg, 0xFFFFFF, 0.06),
            mix_hex(term_bg, 0xFFFFFF, 0.06),
            term_bg,
            mix_hex(term_fg, term_bg, 0.44),
        )
    } else {
        (
            mix_hex(term_bg, 0x000000, 0.045),
            mix_hex(term_bg, 0x000000, 0.075),
            mix_hex(term_bg, 0xFFFFFF, 0.65),
            mix_hex(term_fg, term_bg, 0.42),
        )
    };

    ThemeSpec {
        id: id.to_string(),
        name: name.to_string(),
        subtitle: subtitle.to_string(),
        badge: "Custom".to_string(),
        category: ThemeCategory::Custom,
        dark,
        term_bg,
        term_fg,
        ui_bg: term_bg,
        sidebar_bg,
        titlebar_bg,
        surface_bg,
        muted_fg,
        accent,
        cursor: cur,
        cursor_text,
        selection: sel,
        ansi,
    }
}

// ---------------------------------------------------------------------------
// Compact Builder & 54 Hand-Crafted Built-in Themes (38 Dark + 16 Light)
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn spec(
    id: &'static str,
    name: &'static str,
    subtitle: &'static str,
    badge: &'static str,
    category: ThemeCategory,
    dark: bool,
    term_bg: u32,
    term_fg: u32,
    sidebar_bg: u32,
    titlebar_bg: u32,
    surface_bg: u32,
    muted_fg: u32,
    accent: u32,
    cursor: u32,
    selection: u32,
    ansi: [u32; 16],
) -> ThemeSpec {
    ThemeSpec {
        id: id.to_string(),
        name: name.to_string(),
        subtitle: subtitle.to_string(),
        badge: badge.to_string(),
        category,
        dark,
        term_bg,
        term_fg,
        ui_bg: term_bg,
        sidebar_bg,
        titlebar_bg,
        surface_bg,
        muted_fg,
        accent,
        cursor,
        cursor_text: None,
        selection,
        ansi,
    }
}

fn builtin_rex_obsidian() -> ThemeSpec {
    spec(
        DEFAULT_DARK_THEME_ID,
        "Rex Obsidian",
        "Superlogical 极简黑曜石与电光靛蓝",
        "Rex",
        ThemeCategory::Signature,
        true,
        0x12141A,
        0xE6E9F2,
        0x1E2025,
        0x1E2025,
        0x12141A,
        0x8A91A6,
        0x6E7BF2,
        0x7C86FF,
        0x6E7BF2,
        [
            0x1C1F27, 0xF45B69, 0x3DD68C, 0xF5B841, 0x6E7BF2, 0xC084FC, 0x38BDF8, 0xD4DAE8,
            0x565D73, 0xFF7A85, 0x5CE6A0, 0xFFD066, 0x8EA0FF, 0xD8A8FF, 0x67D4FF, 0xF8FAFC,
        ],
    )
}

fn builtin_rex_daylight() -> ThemeSpec {
    spec(
        DEFAULT_LIGHT_THEME_ID,
        "Rex Daylight",
        "Superlogical 冷白纸感与高锐度靛蓝",
        "Rex",
        ThemeCategory::Signature,
        false,
        0xFAFAFC,
        0x181A20,
        0xECEEF2,
        0xECEEF2,
        0xFFFFFF,
        0x646B7D,
        0x4F46E5,
        0x4F46E5,
        0x4F46E5,
        [
            0x181A20, 0xDC2626, 0x16A34A, 0xB45309, 0x4F46E5, 0x9333EA, 0x0284C7, 0x646B7D,
            0x858D9F, 0xEF4444, 0x22C55E, 0xD97706, 0x6366F1, 0xA855F7, 0x0EA5E9, 0x0F1117,
        ],
    )
}

pub fn builtin_themes() -> Vec<ThemeSpec> {
    use ThemeCategory::{ArcticStudio, CyberNeon, Modern, Signature, WarmInky};
    vec![
        // ==================== 1. Rex & Termius Signature (Dark) ====================
        builtin_rex_obsidian(),
        spec(
            "termius-dark",
            "Termius Dark",
            "Termius 经典深海石板蓝与翡翠绿",
            "Termius",
            Signature,
            true,
            0x141729,
            0xE6EDF7,
            0x191D33,
            0x101322,
            0x212640,
            0x8792B0,
            0x00CC74,
            0x00E582,
            0x2B6CB0,
            [
                0x1F243D, 0xFF5C6C, 0x00CC74, 0xFFCA42, 0x3B82F6, 0xB877FF, 0x2AD4E8, 0xD5DEED,
                0x586282, 0xFF7A88, 0x2EE592, 0xFFDB70, 0x60A5FA, 0xCE9BFF, 0x5EE6F7, 0xFFFFFF,
            ],
        ),
        spec(
            "rex-carbon",
            "Rex Carbon",
            "Superlogical 官网冷碳纯黑与冰青点缀",
            "Rex",
            Signature,
            true,
            0x0C0D10,
            0xECEFF4,
            0x121419,
            0x090A0D,
            0x191C24,
            0x7F8799,
            0x2DD4BF,
            0x2DD4BF,
            0x2DD4BF,
            [
                0x191C24, 0xF87171, 0x34D399, 0xFBBF24, 0x60A5FA, 0xC084FC, 0x2DD4BF, 0xCBD5E1,
                0x525A6B, 0xFCA5A5, 0x6EE7B7, 0xFCD34D, 0x93C5FD, 0xD8B4FE, 0x5EEAD4, 0xF8FAFC,
            ],
        ),
        spec(
            "termius-midnight",
            "Termius Midnight",
            "Termius Pro 极夜深蓝与亮海蓝工作台",
            "Termius",
            Signature,
            true,
            0x0E111F,
            0xE4ECFA,
            0x14182B,
            0x0A0D18,
            0x1C223B,
            0x7E8BA8,
            0x388BFD,
            0x388BFD,
            0x388BFD,
            [
                0x1A2038, 0xFF5D6C, 0x26D07C, 0xF7B93E, 0x388BFD, 0xA371F7, 0x39C5CF, 0xD0DCF0,
                0x53607E, 0xFF808C, 0x4CE396, 0xFFD166, 0x67A8FF, 0xBC93FF, 0x63DCE5, 0xFFFFFF,
            ],
        ),
        spec(
            "rex-slate",
            "Rex Slate",
            "Rex 多路复用青石板柔和低眩光",
            "Rex",
            Signature,
            true,
            0x171A21,
            0xE2E8F0,
            0x13161C,
            0x0F1117,
            0x202530,
            0x8994A6,
            0x38BDF8,
            0x38BDF8,
            0x38BDF8,
            [
                0x202530, 0xF06A73, 0x4ADE80, 0xFACC15, 0x38BDF8, 0xC084FC, 0x22D3EE, 0xCBD5E1,
                0x5A6475, 0xFB8A91, 0x6EE7A0, 0xFDE047, 0x7DD3FC, 0xD8B4FE, 0x67E8F9, 0xF8FAFC,
            ],
        ),
        spec(
            "termius-graphite",
            "Termius Graphite",
            "Termius 中性石墨暗灰与薄荷绿",
            "Termius",
            Signature,
            true,
            0x16181D,
            0xE7EAEE,
            0x121418,
            0x0E1014,
            0x20232B,
            0x8A909E,
            0x10B981,
            0x10B981,
            0x10B981,
            [
                0x22252E, 0xF25F5C, 0x10B981, 0xF59E0B, 0x3B82F6, 0xA855F7, 0x06B6D4, 0xD1D5DB,
                0x5A6070, 0xF87171, 0x34D399, 0xFBBF24, 0x60A5FA, 0xC084FC, 0x22D3EE, 0xF9FAFB,
            ],
        ),
        spec(
            "ghostty-dark",
            "Ghostty Default",
            "Libghostty 原生均衡深岩灰蓝",
            "Rex",
            Signature,
            true,
            0x282C34,
            0xDCDFE4,
            0x21252B,
            0x1C1F24,
            0x313640,
            0x8B929E,
            0x61AFEF,
            0x61AFEF,
            0x61AFEF,
            [
                0x1D1F21, 0xCC6666, 0xB5BD68, 0xF0C674, 0x81A2BE, 0xB294BB, 0x8ABEB7, 0xC5C8C6,
                0x666666, 0xD54E53, 0xB9CA4A, 0xE7C547, 0x7AA6DA, 0xC397D8, 0x70C0B1, 0xEAEAEA,
            ],
        ),
        spec(
            "cupertino-dark",
            "Cupertino Dark",
            "原生 macOS Sequoia 深空灰与苹果蓝",
            "macOS",
            Signature,
            true,
            0x19191C,
            0xF2F2F7,
            0x222226,
            0x26262A,
            0x2A2A2F,
            0x98989D,
            0x0A84FF,
            0x0A84FF,
            0x0A84FF,
            [
                0x2C2C2E, 0xFF453A, 0x30D158, 0xFFD60A, 0x0A84FF, 0xBF5AF2, 0x64D2FF, 0xD1D1D6,
                0x636366, 0xFF6961, 0x4CD964, 0xFFE066, 0x5EB0FF, 0xD982FF, 0x85E0FF, 0xF2F2F7,
            ],
        ),
        // ==================== 2. Modern Classics (Dark) ====================
        spec(
            "catppuccin-mocha",
            "Catppuccin Mocha",
            "丝滑摩卡紫黑与柔和马卡龙粉彩",
            "Termius",
            Modern,
            true,
            0x1E1E2E,
            0xCDD6F4,
            0x181825,
            0x11111B,
            0x313244,
            0x9399B2,
            0xCBA6F7,
            0xF5E0DC,
            0x89B4FA,
            [
                0x45475A, 0xF38BA8, 0xA6E3A1, 0xF9E2AF, 0x89B4FA, 0xF5C2E7, 0x94E2D5, 0xBAC2DE,
                0x585B70, 0xF38BA8, 0xA6E3A1, 0xF9E2AF, 0x89B4FA, 0xF5C2E7, 0x94E2D5, 0xA6ADC8,
            ],
        ),
        spec(
            "catppuccin-macchiato",
            "Catppuccin Macchiato",
            "浓郁玛奇朵蓝紫与温润糖霜色",
            "Rex",
            Modern,
            true,
            0x24273A,
            0xCAD3F5,
            0x1E2030,
            0x181926,
            0x363A4F,
            0x939AB7,
            0x8AADF4,
            0xF4DBD6,
            0x8AADF4,
            [
                0x494D64, 0xED8796, 0xA6DA95, 0xEED49F, 0x8AADF4, 0xF5BDE6, 0x8BD5CA, 0xB8C0E0,
                0x5B6078, 0xED8796, 0xA6DA95, 0xEED49F, 0x8AADF4, 0xF5BDE6, 0x8BD5CA, 0xA5ADCB,
            ],
        ),
        spec(
            "catppuccin-frappe",
            "Catppuccin Frappé",
            "轻柔暮色蓝灰与低对比粉彩",
            "Rex",
            Modern,
            true,
            0x303446,
            0xC6D0F5,
            0x292C3C,
            0x232634,
            0x414559,
            0x949CBB,
            0x8CAAEE,
            0xF2D5CF,
            0x8CAAEE,
            [
                0x51576D, 0xE78284, 0xA6D189, 0xE5C890, 0x8CAAEE, 0xF4B8E4, 0x81C8BE, 0xB5BFE2,
                0x626880, 0xE78284, 0xA6D189, 0xE5C890, 0x8CAAEE, 0xF4B8E4, 0x81C8BE, 0xA5ADCE,
            ],
        ),
        spec(
            "tokyo-night",
            "Tokyo Night",
            "东京午夜深蓝与清透霓虹光感",
            "Termius",
            Modern,
            true,
            0x1A1B26,
            0xC0CAF5,
            0x16161E,
            0x121219,
            0x24283B,
            0x7982A9,
            0x7AA2F7,
            0xC0CAF5,
            0x7AA2F7,
            [
                0x15161E, 0xF7768E, 0x9ECE6A, 0xE0AF68, 0x7AA2F7, 0xBB9AF7, 0x7DCFFF, 0xA9B1D6,
                0x414868, 0xF7768E, 0x9ECE6A, 0xE0AF68, 0x7AA2F7, 0xBB9AF7, 0x7DCFFF, 0xC0CAF5,
            ],
        ),
        spec(
            "tokyo-night-storm",
            "Tokyo Night Storm",
            "暴风夜蓝深灰与冰蓝高对比语法",
            "Rex",
            Modern,
            true,
            0x24283B,
            0xC0CAF5,
            0x1F2335,
            0x1A1D2C,
            0x2E344E,
            0x7F89B0,
            0x7DCFFF,
            0xC0CAF5,
            0x7AA2F7,
            [
                0x1D202F, 0xF7768E, 0x9ECE6A, 0xE0AF68, 0x7AA2F7, 0xBB9AF7, 0x7DCFFF, 0xA9B1D6,
                0x414868, 0xF7768E, 0x9ECE6A, 0xE0AF68, 0x7AA2F7, 0xBB9AF7, 0x7DCFFF, 0xC0CAF5,
            ],
        ),
        spec(
            "tokyo-night-moon",
            "Tokyo Night Moon",
            "月夜靛蓝与柔和紫罗兰高亮",
            "Rex",
            Modern,
            true,
            0x222436,
            0xC8D3F5,
            0x1E2030,
            0x191B29,
            0x2F334D,
            0x828BB8,
            0x82AAFF,
            0xC8D3F5,
            0x82AAFF,
            [
                0x1B1D2B, 0xFF757F, 0xC3E88D, 0xFFC777, 0x82AAFF, 0xC099FF, 0x86E1FC, 0x828BB8,
                0x444A73, 0xFF8D94, 0xC7FB6D, 0xFFD8AB, 0x9AB8FF, 0xCAABFF, 0xB2EBFF, 0xC8D3F5,
            ],
        ),
        spec(
            "dracula",
            "Dracula",
            "Termius 经典暗夜吸血鬼紫与荧光绿",
            "Termius",
            Modern,
            true,
            0x282A36,
            0xF8F8F2,
            0x21222C,
            0x191A21,
            0x343746,
            0x8B98C7,
            0xBD93F9,
            0xFF79C6,
            0xBD93F9,
            [
                0x21222C, 0xFF5555, 0x50FA7B, 0xF1FA8C, 0xBD93F9, 0xFF79C6, 0x8BE9FD, 0xF8F8F2,
                0x6272A4, 0xFF6E6E, 0x69FF94, 0xFFFFA5, 0xD6ACFF, 0xFF92DF, 0xA4FFFF, 0xFFFFFF,
            ],
        ),
        spec(
            "one-dark-pro",
            "One Dark Pro",
            "Atom / VS Code 殿堂级均衡暗色",
            "Termius",
            Modern,
            true,
            0x282C34,
            0xABB2BF,
            0x21252B,
            0x1B1F23,
            0x323842,
            0x7F848E,
            0x61AFEF,
            0x528BFF,
            0x61AFEF,
            [
                0x282C34, 0xE06C75, 0x98C379, 0xE5C07B, 0x61AFEF, 0xC678DD, 0x56B6C2, 0xABB2BF,
                0x5C6370, 0xE88388, 0xA9D48B, 0xF0D192, 0x79C0FF, 0xD892EF, 0x70C8D4, 0xFFFFFF,
            ],
        ),
        spec(
            "rose-pine",
            "Rosé Pine",
            "松烟暗紫与低饱和玫瑰金温柔质感",
            "Rex",
            Modern,
            true,
            0x191724,
            0xE0DEF4,
            0x1F1D2E,
            0x14121D,
            0x26233A,
            0x908CAA,
            0xC4A7E7,
            0xEBBCBA,
            0xC4A7E7,
            [
                0x26233A, 0xEB6F92, 0x31748F, 0xF6C177, 0x9CCFD8, 0xC4A7E7, 0xEBBCBA, 0xE0DEF4,
                0x6E6A86, 0xEB6F92, 0x56949F, 0xF6C177, 0x9CCFD8, 0xC4A7E7, 0xEBBCBA, 0xE0DEF4,
            ],
        ),
        spec(
            "rose-pine-moon",
            "Rosé Pine Moon",
            "月下玫瑰松针深紫灰",
            "Rex",
            Modern,
            true,
            0x232136,
            0xE0DEF4,
            0x1D1B2E,
            0x181626,
            0x393552,
            0x908CAA,
            0xEA9A97,
            0xEA9A97,
            0xC4A7E7,
            [
                0x393552, 0xEB6F92, 0x3E8FB0, 0xF6C177, 0x9CCFD8, 0xC4A7E7, 0xEA9A97, 0xE0DEF4,
                0x6E6A86, 0xEB6F92, 0x569FBA, 0xF6C177, 0x9CCFD8, 0xC4A7E7, 0xEA9A97, 0xE0DEF4,
            ],
        ),
        // ==================== 3. Warm, Earthy & Inky (Dark) ====================
        spec(
            "flexoki-dark",
            "Flexoki Dark",
            "Steph Ango 纸墨印刷温润真黑",
            "Termius",
            WarmInky,
            true,
            0x100F0F,
            0xCECDC3,
            0x1C1B1A,
            0x151413,
            0x282726,
            0x878580,
            0x4385BE,
            0xD0A215,
            0x4385BE,
            [
                0x1C1B1A, 0xD14D41, 0x879A39, 0xD0A215, 0x4385BE, 0xCE5D97, 0x3AA99F, 0xCECDC3,
                0x575653, 0xE8705F, 0xA0AF54, 0xDFB431, 0x66A0C8, 0xE47DA8, 0x5ABDAC, 0xFFFCF0,
            ],
        ),
        spec(
            "kanagawa-wave",
            "Kanagawa Wave",
            "北斋神奈川冲浪里浮世绘和风深色",
            "Termius",
            WarmInky,
            true,
            0x1F1F28,
            0xDCD7BA,
            0x16161D,
            0x121218,
            0x2A2A37,
            0x938AA9,
            0x7E9CD8,
            0xC8C093,
            0x7E9CD8,
            [
                0x16161D, 0xC34043, 0x76946A, 0xC0A36E, 0x7E9CD8, 0x957FB8, 0x6A9589, 0xC8C093,
                0x727169, 0xE82424, 0x98BB6C, 0xE6C384, 0x7FB4CA, 0x938AA9, 0x7AA89F, 0xDCD7BA,
            ],
        ),
        spec(
            "kanagawa-dragon",
            "Kanagawa Dragon",
            "浮世绘古龙沉墨极夜低蓝光",
            "Termius",
            WarmInky,
            true,
            0x181616,
            0xC5C9C5,
            0x12120F,
            0x0D0C0C,
            0x282727,
            0x8A8980,
            0x8BA4B0,
            0xC8C093,
            0x8BA4B0,
            [
                0x0D0C0C, 0xC4746E, 0x8A9A7B, 0xC4B28A, 0x8BA4B0, 0xA292A3, 0x8EA4A2, 0xC8C093,
                0x625E5A, 0xE46876, 0x87A987, 0xE6C384, 0x7FB4CA, 0x938AA9, 0x7AA89F, 0xC5C9C5,
            ],
        ),
        spec(
            "everforest-dark",
            "Everforest Dark",
            "温润迷雾森林绿意长效护眼",
            "Termius",
            WarmInky,
            true,
            0x2D353B,
            0xD3C6AA,
            0x232A2E,
            0x1E2326,
            0x343F44,
            0x859289,
            0xA7C080,
            0xD3C6AA,
            0x7FBBB3,
            [
                0x343F44, 0xE67E80, 0xA7C080, 0xDBBC7F, 0x7FBBB3, 0xD699B6, 0x83C092, 0xD3C6AA,
                0x7A8478, 0xE67E80, 0xA7C080, 0xDBBC7F, 0x7FBBB3, 0xD699B6, 0x83C092, 0xECE3CF,
            ],
        ),
        spec(
            "gruvbox-dark",
            "Gruvbox Dark",
            "经典复古暖炭黑与琥珀金",
            "Termius",
            WarmInky,
            true,
            0x282828,
            0xEBDBB2,
            0x1D2021,
            0x17191A,
            0x3C3836,
            0xA89984,
            0xFABD2F,
            0xFE8019,
            0x83A598,
            [
                0x282828, 0xCC241D, 0x98971A, 0xD79921, 0x458588, 0xB16286, 0x689D6A, 0xA89984,
                0x928374, 0xFB4934, 0xB8BB26, 0xFABD2F, 0x83A598, 0xD3869B, 0x8EC07C, 0xEBDBB2,
            ],
        ),
        spec(
            "gruvbox-material",
            "Gruvbox Material",
            "改良柔和暖木灰与奶油棕",
            "Rex",
            WarmInky,
            true,
            0x282828,
            0xD4BE98,
            0x1D2021,
            0x181A1B,
            0x32302F,
            0x928374,
            0x7DAEA3,
            0xEA6962,
            0x7DAEA3,
            [
                0x32302F, 0xEA6962, 0xA9B665, 0xD8A657, 0x7DAEA3, 0xD3869B, 0x89B482, 0xD4BE98,
                0x7C6F64, 0xEA6962, 0xA9B665, 0xD8A657, 0x7DAEA3, 0xD3869B, 0x89B482, 0xE2CCA9,
            ],
        ),
        spec(
            "vesper",
            "Vesper",
            "极简纯黑画布配香槟金与薄荷青",
            "Rex",
            WarmInky,
            true,
            0x101010,
            0xF2F2F2,
            0x161616,
            0x0B0B0B,
            0x222222,
            0x8B8B8B,
            0xFFC799,
            0xFFC799,
            0x99FFE4,
            [
                0x1C1C1C, 0xF58080, 0x99FFE4, 0xFFC799, 0x82AAFF, 0xD2A6FF, 0x7CE8FF, 0xD4D4D4,
                0x585858, 0xFF9999, 0xB3FFEB, 0xFFD9B8, 0xA3C2FF, 0xE0C2FF, 0xA3F0FF, 0xFFFFFF,
            ],
        ),
        spec(
            "zenbones-dark",
            "Zenbones Dark",
            "禅意暖石骨感排版配色",
            "Rex",
            WarmInky,
            true,
            0x1C1917,
            0xB4BDC3,
            0x161311,
            0x120F0E,
            0x292522,
            0x867A74,
            0x6099C0,
            0xC4CACF,
            0x6099C0,
            [
                0x1C1917, 0xDE6E7C, 0x819B69, 0xB77E64, 0x6099C0, 0xB279A7, 0x66A5AD, 0xB4BDC3,
                0x6E6763, 0xE8838F, 0x8BAE68, 0xD68C67, 0x61ABDA, 0xCF86C1, 0x65B8C1, 0xD5DDE2,
            ],
        ),
        // ==================== 4. Arctic & Studio IDE (Dark) ====================
        spec(
            "nord",
            "Nord",
            "北极冰川蓝灰极简美学",
            "Termius",
            ArcticStudio,
            true,
            0x2E3440,
            0xD8DEE9,
            0x242933,
            0x1E222B,
            0x3B4252,
            0x818C9E,
            0x88C0D0,
            0x88C0D0,
            0x81A1C1,
            [
                0x3B4252, 0xBF616A, 0xA3BE8C, 0xEBCB8B, 0x81A1C1, 0xB48EAD, 0x88C0D0, 0xE5E9F0,
                0x59647A, 0xD06F79, 0xB1D196, 0xF0D399, 0x8CAFCE, 0xC895BF, 0x8FBCBB, 0xECEFF4,
            ],
        ),
        spec(
            "ayu-dark",
            "Ayu Dark",
            "深邃暗夜与鲜亮琥珀金高对比",
            "Termius",
            ArcticStudio,
            true,
            0x0B0E14,
            0xBFBDB6,
            0x0F131A,
            0x080A0F,
            0x161C26,
            0x6C7686,
            0xFFB454,
            0xE6B450,
            0x39BAE6,
            [
                0x01060E, 0xEA6C73, 0x91B362, 0xF9AF4F, 0x53BDFA, 0xFAE994, 0x90E1C6, 0xC7C7C7,
                0x686868, 0xF07178, 0xC2D94C, 0xFFB454, 0x59C2FF, 0xFFEE99, 0x95E6CB, 0xFFFFFF,
            ],
        ),
        spec(
            "ayu-mirage",
            "Ayu Mirage",
            "海市蜃楼深蓝灰与暖黄青绿",
            "Termius",
            ArcticStudio,
            true,
            0x1F2430,
            0xCCCAC2,
            0x171B24,
            0x13171F,
            0x272D3C,
            0x7D8799,
            0xFFCC66,
            0xFFCC66,
            0x5CCFE6,
            [
                0x191E2A, 0xED8274, 0xA6CC70, 0xFAD07B, 0x6DCBFA, 0xCFBFFA, 0x90E1C6, 0xC7C7C7,
                0x686868, 0xF28779, 0xBAE67E, 0xFFD580, 0x73D0FF, 0xD4BFFF, 0x95E6CB, 0xFFFFFF,
            ],
        ),
        spec(
            "github-dark",
            "GitHub Dark",
            "GitHub 官方暗色工程标准色板",
            "Rex",
            ArcticStudio,
            true,
            0x0D1117,
            0xE6EDF3,
            0x161B22,
            0x090C10,
            0x21262D,
            0x8B949E,
            0x58A6FF,
            0x58A6FF,
            0x388BFD,
            [
                0x484F58, 0xFF7B72, 0x3FB950, 0xD29922, 0x58A6FF, 0xBC8CFF, 0x39C5CF, 0xB1BAC4,
                0x6E7681, 0xFFA198, 0x56D364, 0xE3B341, 0x79C0FF, 0xD2A8FF, 0x56D4DD, 0xFFFFFF,
            ],
        ),
        spec(
            "solarized-dark",
            "Solarized Dark",
            "Ethan Schoonover 实验室对比度青蓝",
            "Termius",
            ArcticStudio,
            true,
            0x002B36,
            0x93A1A1,
            0x00212B,
            0x001A22,
            0x073642,
            0x657B83,
            0x268BD2,
            0x2AA198,
            0x268BD2,
            [
                0x073642, 0xDC322F, 0x859900, 0xB58900, 0x268BD2, 0xD33682, 0x2AA198, 0xEEE8D5,
                0x586E75, 0xCB4B16, 0x859900, 0xB58900, 0x6C71C4, 0xD33682, 0x2AA198, 0xFDF6E3,
            ],
        ),
        spec(
            "monokai-pro",
            "Monokai Pro",
            "暖黑滤镜与鲜艳黄粉苹果绿",
            "Termius",
            ArcticStudio,
            true,
            0x2D2A2E,
            0xFCFCFA,
            0x221F22,
            0x1B191B,
            0x403E41,
            0x939293,
            0xFFD866,
            0xFCFCFA,
            0x78DCE8,
            [
                0x403E41, 0xFF6188, 0xA9DC76, 0xFFD866, 0xFC9867, 0xAB9DF2, 0x78DCE8, 0xFCFCFA,
                0x727072, 0xFF6188, 0xA9DC76, 0xFFD866, 0xFC9867, 0xAB9DF2, 0x78DCE8, 0xFFFFFF,
            ],
        ),
        spec(
            "nightfox",
            "Nightfox",
            "静谧午夜狐蓝高清晰终端色",
            "Rex",
            ArcticStudio,
            true,
            0x192330,
            0xCDCECF,
            0x131A24,
            0x0F151D,
            0x212E3F,
            0x7E8E9F,
            0x719CD6,
            0xCDCECF,
            0x719CD6,
            [
                0x393B44, 0xC94F6D, 0x81B29A, 0xDBC074, 0x719CD6, 0x9D79D6, 0x63CDCF, 0xDFDFE0,
                0x575860, 0xD16983, 0x8EBAA4, 0xE0C989, 0x86ABDC, 0xBAA1E2, 0x7AD5D6, 0xE4E4E5,
            ],
        ),
        spec(
            "carbonfox",
            "Carbonfox",
            "IBM Carbon 工业级冷黑与电光蓝",
            "Rex",
            ArcticStudio,
            true,
            0x161616,
            0xF2F4F8,
            0x0F0F0F,
            0x0A0A0A,
            0x262626,
            0x8D949E,
            0x78A9FF,
            0xF2F4F8,
            0x33B1FF,
            [
                0x282828, 0xEE5396, 0x25BE6A, 0x08BDBA, 0x78A9FF, 0xBE95FF, 0x33B1FF, 0xDFDFE0,
                0x525253, 0xF16DA6, 0x46C880, 0x2DC7C4, 0x8CB6FF, 0xC8A5FF, 0x52BDFF, 0xF2F4F8,
            ],
        ),
        // ==================== 5. Cyber & Neon (Dark) ====================
        spec(
            "poimandres",
            "Poimandres",
            "极简冷调蓝绿薄荷现代暗夜",
            "Rex",
            CyberNeon,
            true,
            0x1B1E28,
            0xE4F0FB,
            0x14161E,
            0x101218,
            0x252B3B,
            0x767C9D,
            0x5DE4C7,
            0xADD7FF,
            0x5DE4C7,
            [
                0x1B1E28, 0xD0679D, 0x5DE4C7, 0xFFFAC2, 0x89DDFF, 0xFCC5E9, 0xADD7FF, 0xFFFFFF,
                0x506477, 0xD0679D, 0x5DE4C7, 0xFFFAC2, 0xADD7FF, 0xFAE4FC, 0x89DDFF, 0xFFFFFF,
            ],
        ),
        spec(
            "material-palenight",
            "Material Palenight",
            "柔和暮紫设计感高颜值暗色",
            "Rex",
            CyberNeon,
            true,
            0x292D3E,
            0xA6ACCD,
            0x202331,
            0x1B1E2B,
            0x34394F,
            0x7E85A6,
            0x82AAFF,
            0xFFCB6B,
            0x82AAFF,
            [
                0x292D3E, 0xF07178, 0xC3E88D, 0xFFCB6B, 0x82AAFF, 0xC792EA, 0x89DDFF, 0xD0D0D0,
                0x676E95, 0xFF8B92, 0xDDFFA7, 0xFFE585, 0x9CC4FF, 0xE1ACFF, 0xA3F7FF, 0xFFFFFF,
            ],
        ),
        spec(
            "synthwave-84",
            "SynthWave '84",
            "复古霓虹日落紫粉与激光青",
            "Neon",
            CyberNeon,
            true,
            0x262335,
            0xF4EEFF,
            0x1E1A2B,
            0x171422,
            0x34294F,
            0x9589B8,
            0xFF7EDB,
            0x36F9F6,
            0xFF7EDB,
            [
                0x262335, 0xFE4450, 0x72F1B8, 0xFEDE5D, 0x36F9F6, 0xFF7EDB, 0x03EDF9, 0xF4EEFF,
                0x614D85, 0xFF6B74, 0x94F7CA, 0xFFE885, 0x6BFBFB, 0xFF9CE3, 0x6BFBFB, 0xFFFFFF,
            ],
        ),
        spec(
            "termius-hacker",
            "Termius Hacker",
            "矩阵磷光终端黑客极客绿",
            "Termius",
            CyberNeon,
            true,
            0x080D0A,
            0xB8F7D4,
            0x0C140F,
            0x050907,
            0x122017,
            0x5E9E79,
            0x00E676,
            0x00FF80,
            0x00E676,
            [
                0x0D1711, 0xFF5252, 0x00E676, 0xFFD740, 0x40C4FF, 0xB388FF, 0x18FFFF, 0xB8F7D4,
                0x38664B, 0xFF7979, 0x69F0AE, 0xFFE57F, 0x80D8FF, 0xD1C4E9, 0x84FFFF, 0xE8FFF2,
            ],
        ),
        // ==================== 6. Light Themes (16) ====================
        builtin_rex_daylight(),
        spec(
            "termius-light",
            "Termius Light",
            "Termius 清爽云白与翡翠绿标识",
            "Termius",
            Signature,
            false,
            0xF7F9FC,
            0x141729,
            0xEDF1F7,
            0xE4EAF3,
            0xFFFFFF,
            0x5E677D,
            0x00A85E,
            0x00A85E,
            0x2563EB,
            [
                0x141729, 0xD92D3E, 0x009954, 0xB46900, 0x2563EB, 0x7C3AED, 0x0891B2, 0x5E677D,
                0x8690A6, 0xEF4444, 0x00B865, 0xD97706, 0x3B82F6, 0x8B5CF6, 0x06B6D4, 0x0F1322,
            ],
        ),
        spec(
            "cupertino-light",
            "Cupertino Light",
            "原生 macOS Sequoia 纯净浅色",
            "macOS",
            Signature,
            false,
            0xFFFFFF,
            0x1D1D1F,
            0xF2F2F7,
            0xECECF2,
            0xF7F7FA,
            0x6E6E73,
            0x007AFF,
            0x007AFF,
            0x007AFF,
            [
                0x1D1D1F, 0xD70015, 0x1D8330, 0x9E6300, 0x0066D6, 0x8944AB, 0x007185, 0x6E6E73,
                0x8E8E93, 0xFF3B30, 0x28CD41, 0xC98200, 0x007AFF, 0xAF52DE, 0x32ADE6, 0x1D1D1F,
            ],
        ),
        spec(
            "catppuccin-latte",
            "Catppuccin Latte",
            "向日葵暖白拿铁与薰衣草紫",
            "Termius",
            Modern,
            false,
            0xEFF1F5,
            0x4C4F69,
            0xE6E9EF,
            0xDCE0E8,
            0xFFFFFF,
            0x7C7F93,
            0x8839EF,
            0xDC8A78,
            0x1E66F5,
            [
                0x5C5F77, 0xD20F39, 0x40A02B, 0xDF8E1D, 0x1E66F5, 0xEA76CB, 0x179299, 0xACB0BE,
                0x6C6F85, 0xD20F39, 0x40A02B, 0xDF8E1D, 0x1E66F5, 0xEA76CB, 0x179299, 0x4C4F69,
            ],
        ),
        spec(
            "tokyo-night-day",
            "Tokyo Night Day",
            "东京白昼微冷蓝灰高清晰浅色",
            "Rex",
            Modern,
            false,
            0xE1E2E7,
            0x2E3C64,
            0xD5D6DB,
            0xCBCCD1,
            0xEEEEF4,
            0x687089,
            0x2E7DE9,
            0x2E7DE9,
            0x2E7DE9,
            [
                0x2E3C64, 0xF52A65, 0x587539, 0x8C6C3E, 0x2E7DE9, 0x9854F1, 0x007197, 0x6172B0,
                0x848CB5, 0xF52A65, 0x587539, 0x8C6C3E, 0x2E7DE9, 0x9854F1, 0x007197, 0x1D253E,
            ],
        ),
        spec(
            "rose-pine-dawn",
            "Rosé Pine Dawn",
            "玫瑰破晓米白与鸢尾紫温润浅色",
            "Rex",
            Modern,
            false,
            0xFAF4ED,
            0x575279,
            0xF2E9E1,
            0xEAE0D7,
            0xFFFAF3,
            0x797593,
            0x907AA9,
            0xD7827E,
            0x907AA9,
            [
                0x575279, 0xB4637A, 0x286983, 0xEA9D34, 0x56949F, 0x907AA9, 0xD7827E, 0x797593,
                0x9893A5, 0xB4637A, 0x286983, 0xEA9D34, 0x56949F, 0x907AA9, 0xD7827E, 0x464261,
            ],
        ),
        spec(
            "one-light",
            "One Light",
            "Atom 经典清爽冷白编辑器配色",
            "Termius",
            Modern,
            false,
            0xFAFAFA,
            0x383A42,
            0xEFEFEF,
            0xE5E5E6,
            0xFFFFFF,
            0x737682,
            0x4078F2,
            0x526FFF,
            0x4078F2,
            [
                0x383A42, 0xE45649, 0x50A14F, 0xC18401, 0x4078F2, 0xA626A4, 0x0184BC, 0x696C77,
                0xA0A1A7, 0xE45649, 0x50A14F, 0x986801, 0x4078F2, 0xA626A4, 0x0184BC, 0x23252C,
            ],
        ),
        spec(
            "flexoki-light",
            "Flexoki Light",
            "Steph Ango 暖宣纸与印刷真墨",
            "Termius",
            WarmInky,
            false,
            0xFFFCF0,
            0x100F0F,
            0xF2F0E5,
            0xE6E4D9,
            0xFFFFFF,
            0x6F6E69,
            0x205EA6,
            0xAD8301,
            0x205EA6,
            [
                0x100F0F, 0xAF3029, 0x66800B, 0xAD8301, 0x205EA6, 0xA02F6F, 0x24837B, 0x6F6E69,
                0x878580, 0xD14D41, 0x879A39, 0xD0A215, 0x4385BE, 0xCE5D97, 0x3AA99F, 0x100F0F,
            ],
        ),
        spec(
            "kanagawa-lotus",
            "Kanagawa Lotus",
            "浮世绘和纸莲华暖黄护眼浅色",
            "Termius",
            WarmInky,
            false,
            0xF2ECBC,
            0x545464,
            0xE7DBA0,
            0xDFD191,
            0xF8F4D4,
            0x716E61,
            0x4D699B,
            0x43436C,
            0x4D699B,
            [
                0x1F1F28, 0xC84053, 0x6F894E, 0x77713F, 0x4D699B, 0xB35B79, 0x597B75, 0x545464,
                0x8A8980, 0xD7474B, 0x6E915F, 0x836F4A, 0x6693BF, 0x624C83, 0x5E857A, 0x43436C,
            ],
        ),
        spec(
            "everforest-light",
            "Everforest Light",
            "清新原木晨林柔和低刺激浅色",
            "Termius",
            WarmInky,
            false,
            0xFDF6E3,
            0x5C6A72,
            0xF4F0D9,
            0xEAE4CA,
            0xFFFBEF,
            0x829181,
            0x8DA101,
            0x5C6A72,
            0x3A94C5,
            [
                0x5C6A72, 0xF85552, 0x8DA101, 0xDFA000, 0x3A94C5, 0xDF69BA, 0x35A77C, 0x708089,
                0x939F91, 0xF85552, 0x8DA101, 0xDFA000, 0x3A94C5, 0xDF69BA, 0x35A77C, 0x4B565C,
            ],
        ),
        spec(
            "gruvbox-light",
            "Gruvbox Light",
            "复古暖羊皮纸与浓郁棕红墨迹",
            "Termius",
            WarmInky,
            false,
            0xFBF1C7,
            0x3C3836,
            0xEBDBB2,
            0xDFCE9F,
            0xFFF8DC,
            0x7C6F64,
            0xB57614,
            0xAF3A03,
            0x076678,
            [
                0x3C3836, 0xCC241D, 0x98971A, 0xD79921, 0x458588, 0xB16286, 0x689D6A, 0x7C6F64,
                0x928374, 0x9D0006, 0x79740E, 0xB57614, 0x076678, 0x8F3F71, 0x427B58, 0x282828,
            ],
        ),
        spec(
            "nord-light",
            "Nord Light",
            "北极雪原冷白与峡湾冰蓝",
            "Termius",
            ArcticStudio,
            false,
            0xECEFF4,
            0x2E3440,
            0xE5E9F0,
            0xD8DEE9,
            0xF8FAFC,
            0x616E88,
            0x5E81AC,
            0x5E81AC,
            0x5E81AC,
            [
                0x3B4252, 0xBF616A, 0x688B52, 0xB48228, 0x5E81AC, 0x9B6A94, 0x3F888F, 0x4C566A,
                0x7B88A1, 0xC94F5A, 0x789E60, 0xC79234, 0x4C739E, 0xAD78A5, 0x4D99A0, 0x2E3440,
            ],
        ),
        spec(
            "ayu-light",
            "Ayu Light",
            "极简净白与活力暖橙点缀",
            "Termius",
            ArcticStudio,
            false,
            0xFCFCFC,
            0x5C6166,
            0xF3F4F5,
            0xEAECEF,
            0xFFFFFF,
            0x828C99,
            0xFF8A2B,
            0xFF8A2B,
            0x399EE6,
            [
                0x000000, 0xF07171, 0x6CBF43, 0xCC7A00, 0x399EE6, 0xA37ACC, 0x4CBF99, 0x5C6166,
                0x8A9199, 0xF07171, 0x86B300, 0xF29718, 0x55B4D4, 0xA37ACC, 0x4CBF99, 0x24292F,
            ],
        ),
        spec(
            "github-light",
            "GitHub Light",
            "GitHub 现代高对比纯净浅色",
            "Rex",
            ArcticStudio,
            false,
            0xFFFFFF,
            0x1F2328,
            0xF6F8FA,
            0xEFF2F5,
            0xFFFFFF,
            0x656D76,
            0x0969DA,
            0x0969DA,
            0x0969DA,
            [
                0x24292F, 0xCF222E, 0x1A7F37, 0x9A6700, 0x0969DA, 0x8250DF, 0x1B7C83, 0x6E7781,
                0x57606A, 0xA40E26, 0x2DA44E, 0xBF8700, 0x218BFF, 0xA475F9, 0x3192AA, 0x1F2328,
            ],
        ),
        spec(
            "solarized-light",
            "Solarized Light",
            "经典暖米白精密对比护眼浅色",
            "Termius",
            ArcticStudio,
            false,
            0xFDF6E3,
            0x586E75,
            0xEEE8D5,
            0xE6DFC8,
            0xFFFBF0,
            0x839496,
            0x268BD2,
            0x2AA198,
            0x268BD2,
            [
                0x073642, 0xDC322F, 0x859900, 0xB58900, 0x268BD2, 0xD33682, 0x2AA198, 0x586E75,
                0x839496, 0xCB4B16, 0x859900, 0xB58900, 0x6C71C4, 0xD33682, 0x2AA198, 0x002B36,
            ],
        ),
        spec(
            "dawnfox",
            "Dawnfox",
            "破晓微光青瓷与柔和玫瑰灰",
            "Rex",
            ArcticStudio,
            false,
            0xFAF4ED,
            0x575279,
            0xEBE5DF,
            0xE1DAD2,
            0xFFFFFF,
            0x797593,
            0x286983,
            0x286983,
            0x286983,
            [
                0x575279, 0xB4637A, 0x618774, 0xEA9D34, 0x286983, 0x907AA9, 0x56949F, 0x625C87,
                0x9893A5, 0xC26D85, 0x629F81, 0xEEA846, 0x2D81A3, 0x9A80B9, 0x5CA7B4, 0x393552,
            ],
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_catalog_has_unique_ids_and_valid_defaults() {
        let themes = builtin_themes();
        assert!(themes.len() >= 50);
        let mut ids = std::collections::HashSet::new();
        for t in &themes {
            assert!(ids.insert(t.id.clone()), "duplicate theme id: {}", t.id);
            assert!(
                contrast_ratio(t.accent, contrasting_ink(t.accent)) >= 4.5,
                "primary button ink contrast < 4.5 for {}",
                t.id
            );
        }
        assert!(themes.iter().any(|t| t.id == DEFAULT_DARK_THEME_ID && t.dark));
        assert!(themes.iter().any(|t| t.id == DEFAULT_LIGHT_THEME_ID && !t.dark));
    }

    #[test]
    fn parses_ghostty_and_itermcolors_custom_themes() {
        let ghostty = r#"
            background = #1e1e2e
            foreground = cdd6f4
            cursor-color = #f5e0dc
            cursor-text = #11111b
            selection-background = #585b70
            palette = 0=#45475a
            palette = 1=#f38ba8
            palette = 2=#a6e3a1
            palette = 4=#89b4fa
        "#;
        let spec = parse_custom_theme_file(Path::new("My Mocha.ghostty"), ghostty).unwrap();
        assert!(spec.dark);
        assert_eq!(spec.term_bg, 0x1E1E2E);
        assert_eq!(spec.term_fg, 0xCDD6F4);
        assert_eq!(spec.cursor_text, Some(0x11111B));
        assert_eq!(spec.ansi[1], 0xF38BA8);
        assert_eq!(spec.ansi[4], 0x89B4FA);

        // Single-key Ghostty override and distinct IDs for similar names
        let single_bg = parse_custom_theme_file(Path::new("A-B.conf"), "background = #112233").unwrap();
        let single_ansi = parse_custom_theme_file(Path::new("A_B.conf"), "palette = 0=#000000").unwrap();
        assert_ne!(single_bg.id, single_ansi.id);
        assert_eq!(single_bg.term_bg, 0x112233);
        assert!(parse_custom_theme_file(Path::new("bad.conf"), "background = invalid").is_err());

        // Compact mixed <integer>/<real> plist components must not bleed into adjacent keys
        let iterm = r#"<?xml version="1.0" encoding="UTF-8"?>
        <plist version="1.0"><dict>
            <key>Background Color</key>
            <dict><key>Red Component</key><real>0.98</real><key>Green Component</key><real>0.97</real><key>Blue Component</key><real>0.94</real></dict>
            <key>Ansi 2 Color</key>
            <dict><key>Red Component</key><integer>0</integer><key>Green Component</key><real>1</real><key>Blue Component</key><integer>0</integer></dict>
        </dict></plist>"#;
        let light_spec = parse_custom_theme_file(Path::new("WarmPaper.itermcolors"), iterm).unwrap();
        assert!(!light_spec.dark);
        assert_eq!(light_spec.ansi[2], 0x00FF00);
        assert_eq!(light_spec.category, ThemeCategory::Custom);

        // Mid-gray background contrast adaptation
        let readable = ensure_readable_fg(0x999999, 0xAAAAAA, true, 4.5);
        assert!(contrast_ratio(readable, 0xAAAAAA) >= 4.5);
    }

    #[test]
    fn import_validates_atomically_preserves_same_file_and_reconciles_deleted_slot() {
        let dir = std::env::temp_dir().join(format!("mica-theme-test-{}", uuid::Uuid::new_v4()));
        let ext_dir = std::env::temp_dir().join(format!("mica-theme-src-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        fs::create_dir_all(&ext_dir).unwrap();

        let existing = dir.join("Alpha.conf");
        fs::write(&existing, "background = #101520\nforeground = #e0e6f0\n").unwrap();

        let good_src = ext_dir.join("Alpha.conf");
        fs::write(&good_src, "background = #222831\nforeground = #eeeeee\n").unwrap();
        let bad_src = ext_dir.join("Broken.conf");
        fs::write(&bad_src, "background = #nothex\n").unwrap();

        let reg = ThemeRegistry::global();
        // Batch with a broken file must fail before overwriting Alpha.conf
        assert!(reg.import_files(&dir, &[good_src, bad_src]).is_err());
        assert!(
            fs::read_to_string(&existing)
                .unwrap()
                .contains("#101520")
        );

        // Importing the already-installed file in place must not truncate it
        let pack = reg
            .import_files(&dir, std::slice::from_ref(&existing))
            .unwrap();
        let cid = custom_id_from_filename("Alpha.conf");
        assert!(pack.get(&cid).is_some());

        let mut settings = AppSettings {
            dark_mode: true,
            dark_theme: cid.clone(),
            ..Default::default()
        };
        assert!(!pack.reconcile_settings(&mut settings));

        let after_rm = reg.remove_custom_file(&dir, "Alpha.conf").unwrap();
        assert!(after_rm.reconcile_settings(&mut settings));
        assert_eq!(settings.dark_theme, DEFAULT_DARK_THEME_ID);

        let _ = fs::remove_dir_all(&dir);
        let _ = fs::remove_dir_all(&ext_dir);
        reg.reload_from_dir(None);
    }
}

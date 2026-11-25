//! Unified client-side syntax highlighting registry (`syntect` + `two-face`),
//! custom `.sublime-syntax` / simple `.tmLanguage.json` extension loader,
//! language auto-detection, and GPUI `EditorState` `InputHighlighter` adapter.

use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use gpui_kit::component::input::{
    EditorState, FoldRange, HighlightStyleResolver, InputEdit, InputHighlighter,
    InputHighlighterFactory, Rope,
};
use gpui_kit::{HighlightStyle, Hsla, Rgba, SharedString, Task};
use parking_lot::RwLock;
use serde_json::Value;
use syntect::parsing::{ParseState, Scope, ScopeStack, SyntaxDefinition, SyntaxReference, SyntaxSet};
use syntect::util::LinesWithEndings;

const EDITOR_SYNC_PARSE_MAX_BYTES: usize = 64 * 1024;
const EDITOR_PARSE_DEBOUNCE: Duration = Duration::from_millis(150);
const BUILTIN_POWERSHELL_SYNTAX: &str = include_str!("powershell.sublime-syntax");

fn base_syntax_builder() -> syntect::parsing::SyntaxSetBuilder {
    let mut builder = two_face::syntax::extra_newlines().into_builder();
    let ps_def = SyntaxDefinition::load_from_str(
        BUILTIN_POWERSHELL_SYNTAX,
        true,
        Some("PowerShell"),
    )
    .expect("bundled powershell.sublime-syntax must be valid");
    builder.add(ps_def);
    builder
}

/// Semantic code token roles shared between the Terminal Canvas and the SFTP Code Editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodeTokenRole {
    Comment,
    CommentDoc,
    String,
    StringEscape,
    Number,
    Boolean,
    Constant,
    Keyword,
    Operator,
    Function,
    Type,
    Tag,
    Attribute,
    Property,
    Variable,
    Heading,
    Inserted,
    Deleted,
}

impl CodeTokenRole {
    /// Maps onto `gpui-component`'s `HighlightTheme` syntax style keys.
    pub fn editor_theme_key(self) -> &'static str {
        match self {
            Self::Comment => "comment",
            Self::CommentDoc => "comment.doc",
            Self::String => "string",
            Self::StringEscape => "string.escape",
            Self::Number => "number",
            Self::Boolean => "boolean",
            Self::Constant => "constant",
            Self::Keyword => "keyword",
            Self::Operator => "operator",
            Self::Function => "function",
            Self::Type => "type",
            Self::Tag => "tag",
            Self::Attribute => "attribute",
            Self::Property => "property",
            Self::Variable => "variable",
            Self::Heading => "title",
            Self::Inserted => "diff.plus",
            Self::Deleted => "diff.minus",
        }
    }

    pub fn fallback_editor_style(self) -> Option<HighlightStyle> {
        let rgba = |r: u8, g: u8, b: u8| {
            Hsla::from(Rgba {
                r: f32::from(r) / 255.0,
                g: f32::from(g) / 255.0,
                b: f32::from(b) / 255.0,
                a: 1.0,
            })
        };
        match self {
            Self::Inserted => Some(HighlightStyle {
                color: Some(rgba(0x30, 0xd1, 0x58)),
                ..Default::default()
            }),
            Self::Deleted => Some(HighlightStyle {
                color: Some(rgba(0xff, 0x45, 0x3a)),
                ..Default::default()
            }),
            _ => None,
        }
    }
}

/// Metadata & diagnostic status for a user-supplied file in `<root_dir>/syntaxes/`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomSyntaxEntry {
    pub file_name: String,
    pub path: PathBuf,
    pub syntax_name: Option<String>,
    pub extensions: Vec<String>,
    pub error: Option<String>,
}

/// Immutable compiled snapshot of built-in + custom syntaxes.
pub struct SyntaxPack {
    pub revision: u64,
    pub syntax_set: SyntaxSet,
    pub builtin_count: usize,
    pub custom_entries: Vec<CustomSyntaxEntry>,
}

impl SyntaxPack {
    pub fn loaded_custom_count(&self) -> usize {
        self.custom_entries
            .iter()
            .filter(|e| e.error.is_none())
            .count()
    }

    /// Sorted list of visible syntax names for UI language switchers.
    pub fn available_languages(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .syntax_set
            .syntaxes()
            .iter()
            .filter(|s| !s.hidden && s.name != "Plain Text")
            .map(|s| s.name.clone())
            .collect();
        names.sort_by_key(|a| a.to_ascii_lowercase());
        names.dedup();
        names
    }

    /// Resolves a `SyntaxReference` from a language name / filename / path hint, falling back
    /// to first-line shebang or diff header inspection. Never touches the local filesystem.
    pub fn find_syntax<'a>(
        &'a self,
        hint: Option<&str>,
        first_line: Option<&str>,
    ) -> Option<&'a SyntaxReference> {
        if let Some(raw) = hint.map(str::trim).filter(|s| !s.is_empty()) {
            if matches!(raw.to_ascii_lowercase().as_str(), "plain" | "text" | "plain text" | "txt")
            {
                return None;
            }
            if let Some(syn) = self.find_by_token_or_path(raw) {
                return Some(syn);
            }
        }

        if let Some(line) = first_line {
            let clean = line.trim_start_matches('\u{feff}').trim_end_matches(['\r', '\n']);
            let head = &clean[..clean.floor_char_boundary(clean.len().min(256))];
            if head.starts_with("diff --git ") || head.starts_with("--- a/") {
                if let Some(diff_syn) = self.find_by_token_or_path("diff") {
                    return Some(diff_syn);
                }
            }
            if let Some(syn) = self.syntax_set.find_syntax_by_first_line(head)
                && syn.name != "Plain Text"
            {
                return Some(syn);
            }
        }

        None
    }

    fn find_by_token_or_path<'a>(&'a self, raw: &str) -> Option<&'a SyntaxReference> {
        let ss = &self.syntax_set;
        let accept = |s: &'a SyntaxReference| (s.name != "Plain Text").then_some(s);

        // 1. Direct name or token lookup (e.g. "Rust", "Bourne Again Shell (bash)", "sh", "yaml")
        if let Some(s) = ss.find_syntax_by_name(raw).and_then(accept) {
            return Some(s);
        }
        if let Some(s) = ss.find_syntax_by_token(raw).and_then(accept) {
            return Some(s);
        }

        // 2. Extract basename from Unix/Windows path
        let basename = raw.rsplit(['/', '\\']).next().unwrap_or(raw).trim();
        if basename.is_empty() {
            return None;
        }

        // 3. Exact basename match in syntect (`Dockerfile`, `Makefile`, `CMakeLists.txt`, etc.)
        if let Some(s) = ss.find_syntax_by_extension(basename).and_then(accept) {
            return Some(s);
        }
        if let Some(alias) = devops_basename_alias(basename)
            && let Some(s) = ss.find_syntax_by_token(alias).and_then(accept)
        {
            return Some(s);
        }

        // 4. File extension lookup (preserving case first, then lowercase, then DevOps alias)
        if let Some((stem, ext)) = basename.rsplit_once('.')
            && !stem.is_empty()
            && !ext.is_empty()
        {
            if let Some(s) = ss.find_syntax_by_extension(ext).and_then(accept) {
                return Some(s);
            }
            let lower_ext = ext.to_ascii_lowercase();
            if let Some(s) = ss.find_syntax_by_extension(&lower_ext).and_then(accept) {
                return Some(s);
            }
            if let Some(alias) = devops_extension_alias(&lower_ext)
                && let Some(s) = ss.find_syntax_by_token(alias).and_then(accept)
            {
                return Some(s);
            }
        }

        None
    }

    /// Parses a single logical line (expected to end with `\n` when complete) and returns
    /// coalesced UTF-8 byte spans within `0..content_len` (excluding trailing `\r`/`\n`).
    pub fn parse_line_spans(
        &self,
        parser: &mut ParseState,
        scopes: &mut ScopeStack,
        scope_cache: &mut HashMap<Scope, Option<CodeTokenRole>>,
        line_with_ending: &str,
    ) -> Vec<(Range<usize>, CodeTokenRole)> {
        let content_len = line_with_ending
            .trim_end_matches(['\r', '\n'])
            .len();
        let Ok(ops) = parser.parse_line(line_with_ending, &self.syntax_set) else {
            return Vec::new();
        };

        let mut out: Vec<(Range<usize>, CodeTokenRole)> = Vec::new();
        let mut cursor = 0usize;

        let mut push_range = |start: usize, end: usize, stack: &ScopeStack| {
            let s = start.min(content_len);
            let e = end.min(content_len);
            if s >= e {
                return;
            }
            let role = stack
                .scopes
                .iter()
                .rev()
                .find_map(|sc| *scope_cache.entry(*sc).or_insert_with(|| map_scope_to_role(*sc)));
            if let Some(role) = role {
                if let Some((last_range, last_role)) = out.last_mut()
                    && last_range.end == s
                    && *last_role == role
                {
                    last_range.end = e;
                } else {
                    out.push((s..e, role));
                }
            }
        };

        for (idx, op) in ops {
            push_range(cursor, idx, scopes);
            let _ = scopes.apply(&op);
            cursor = idx;
        }
        push_range(cursor, content_len, scopes);
        out
    }

    /// Parses an entire multi-line document into document-absolute UTF-8 byte spans.
    pub fn parse_document(&self, syntax: &SyntaxReference, text: &str) -> Vec<(Range<usize>, CodeTokenRole)> {
        let mut parser = ParseState::new(syntax);
        let mut scopes = ScopeStack::new();
        let mut cache = HashMap::new();
        let mut all: Vec<(Range<usize>, CodeTokenRole)> = Vec::new();
        let mut offset = 0usize;

        for line in LinesWithEndings::from(text) {
            let line_spans = self.parse_line_spans(&mut parser, &mut scopes, &mut cache, line);
            for (range, role) in line_spans {
                let abs = (offset + range.start)..(offset + range.end);
                if let Some((last_r, last_role)) = all.last_mut()
                    && last_r.end == abs.start
                    && *last_role == role
                {
                    last_r.end = abs.end;
                } else {
                    all.push((abs, role));
                }
            }
            offset += line.len();
        }
        all
    }
}

/// Process-wide hot-reloadable syntax registry shared across terminal sessions and SFTP editors.
pub struct SyntaxRegistry {
    current: RwLock<Arc<SyntaxPack>>,
    next_rev: AtomicU64,
}

static GLOBAL_REGISTRY: LazyLock<SyntaxRegistry> = LazyLock::new(|| {
    let base = base_syntax_builder().build();
    let builtin_count = base
        .syntaxes()
        .iter()
        .filter(|s| !s.hidden && s.name != "Plain Text")
        .count();
    let pack = Arc::new(SyntaxPack {
        revision: 1,
        syntax_set: base,
        builtin_count,
        custom_entries: Vec::new(),
    });
    SyntaxRegistry {
        current: RwLock::new(pack),
        next_rev: AtomicU64::new(2),
    }
});

impl SyntaxRegistry {
    pub fn global() -> &'static Self {
        &GLOBAL_REGISTRY
    }

    pub fn current(&self) -> Arc<SyntaxPack> {
        self.current.read().clone()
    }

    /// Rebuilds the syntax set from built-in `two-face` + PowerShell syntaxes plus any custom files in `dir`.
    pub fn reload_from_dir(&self, dir: Option<&Path>) -> Arc<SyntaxPack> {
        let mut builder = base_syntax_builder();
        let builtin_count = GLOBAL_REGISTRY.current().builtin_count;

        let mut custom_entries = Vec::new();
        if let Some(dir) = dir.filter(|d| d.is_dir()) {
            let mut files: Vec<PathBuf> = fs::read_dir(dir)
                .ok()
                .into_iter()
                .flatten()
                .filter_map(|e| e.ok().map(|entry| entry.path()))
                .filter(|p| p.is_file() && !is_hidden_file(p))
                .collect();
            files.sort_by(|a, b| a.file_name().cmp(&b.file_name()));

            let mut any_added = false;
            for path in files {
                let entry = load_single_custom_syntax(&path, &mut builder, &mut any_added);
                custom_entries.push(entry);
            }
        }
        let syntax_set = builder.build();

        let rev = self.next_rev.fetch_add(1, Ordering::Relaxed);
        let pack = Arc::new(SyntaxPack {
            revision: rev,
            syntax_set,
            builtin_count,
            custom_entries,
        });
        *self.current.write() = pack.clone();
        pack
    }

    /// Copies user-selected syntax files into `syntaxes_dir` and hot-reloads the registry.
    pub fn import_files(&self, syntaxes_dir: &Path, source_paths: &[PathBuf]) -> Result<Arc<SyntaxPack>> {
        if source_paths.is_empty() {
            return Ok(self.current());
        }
        fs::create_dir_all(syntaxes_dir)
            .with_context(|| format!("创建语法拓展目录失败 `{}`", syntaxes_dir.display()))?;

        let copied: Result<()> = source_paths.iter().try_for_each(|src| {
            let fname = src
                .file_name()
                .and_then(|n| n.to_str())
                .context("无法读取语法文件名")?;
            let dst = syntaxes_dir.join(fname);
            let same_file = dst.exists()
                && fs::canonicalize(src)
                    .ok()
                    .zip(fs::canonicalize(&dst).ok())
                    .is_some_and(|(a, b)| a == b);
            if !same_file {
                fs::copy(src, &dst)
                    .with_context(|| format!("复制语法文件 `{fname}` 失败"))?;
            }
            Ok(())
        });
        let pack = self.reload_from_dir(Some(syntaxes_dir));
        copied.map(|()| pack)
    }

    /// Removes a custom syntax file by filename from `syntaxes_dir` and hot-reloads.
    pub fn remove_custom_file(&self, syntaxes_dir: &Path, file_name: &str) -> Result<Arc<SyntaxPack>> {
        if file_name.contains('/') || file_name.contains('\\') || file_name == ".." {
            bail!("非法的语法文件名");
        }
        let target = syntaxes_dir.join(file_name);
        if target.exists() {
            fs::remove_file(&target)
                .with_context(|| format!("删除语法文件 `{file_name}` 失败"))?;
        }
        Ok(self.reload_from_dir(Some(syntaxes_dir)))
    }

    /// Creates a GPUI `InputHighlighterFactory` backed by the live `SyntaxRegistry`.
    pub fn editor_highlighter_factory() -> InputHighlighterFactory {
        Rc::new(|language| {
            let pack = Self::global().current();
            let syntax = pack.find_syntax(Some(language), None)?;
            let syntax_name = syntax.name.clone();
            Some(Box::new(SyntectEditorHighlighter::new(
                language,
                syntax_name,
                pack,
            )))
        })
    }
}

fn is_hidden_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|s| s.starts_with('.'))
}

fn load_single_custom_syntax(
    path: &Path,
    builder: &mut syntect::parsing::SyntaxSetBuilder,
    any_added: &mut bool,
) -> CustomSyntaxEntry {
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let lower = file_name.to_ascii_lowercase();
    let stem = file_stem_clean(&file_name);

    let raw_text = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(err) => {
            return CustomSyntaxEntry {
                file_name,
                path: path.to_path_buf(),
                syntax_name: None,
                extensions: Vec::new(),
                error: Some(format!("读取失败: {err}")),
            };
        }
    };

    let yaml_res: Result<String, String> = if lower.ends_with(".sublime-syntax") {
        Ok(raw_text)
    } else if lower.ends_with(".tmlanguage.json") || lower.ends_with(".json") {
        convert_tmlanguage_json_to_sublime(&raw_text, stem)
    } else if lower.ends_with(".tmlanguage") || lower.ends_with(".plist") {
        Err("syntect 5.3 不直接解析 XML/Plist .tmLanguage，请导入兼容的 .sublime-syntax 或简版 .tmLanguage.json".to_string())
    } else {
        Err("不支持的文件后缀（支持 .sublime-syntax 与简版 .tmLanguage.json）".to_string())
    };

    let definition = yaml_res.and_then(|yaml| {
        let def = SyntaxDefinition::load_from_str(&yaml, true, Some(stem))
            .map_err(|err| format!("语法解析失败: {err}"))?;
        // syntect validates match rules at load time, but compiles first_line_match on first use.
        if let Some(pattern) = &def.first_line_match
            && let Some(err) = syntect::parsing::Regex::try_compile(pattern)
        {
            return Err(format!("first_line_match 无效: {err}"));
        }
        Ok(def)
    });
    match definition {
        Ok(def) => {
            let syn_name = def.name.clone();
            let exts = def.file_extensions.clone();
            builder.add(def);
            *any_added = true;
            CustomSyntaxEntry {
                file_name,
                path: path.to_path_buf(),
                syntax_name: Some(syn_name),
                extensions: exts,
                error: None,
            }
        }
        Err(msg) => CustomSyntaxEntry {
            file_name,
            path: path.to_path_buf(),
            syntax_name: None,
            extensions: Vec::new(),
            error: Some(msg),
        },
    }
}

fn file_stem_clean(file_name: &str) -> &str {
    let lower = file_name.to_ascii_lowercase();
    if lower.ends_with(".tmlanguage.json") {
        &file_name[..file_name.len() - ".tmlanguage.json".len()]
    } else {
        file_name
            .rsplit_once('.')
            .map(|(s, _)| s)
            .unwrap_or(file_name)
    }
}

/// Lightweight converter for common VS Code `.tmLanguage.json` files (`match`, `begin`/`end`,
/// `captures`, and `#repository` includes) into `.sublime-syntax` YAML.
fn convert_tmlanguage_json_to_sublime(json_str: &str, fallback_name: &str) -> Result<String, String> {
    let root: Value =
        serde_json::from_str(json_str).map_err(|e| format!("JSON 格式错误: {e}"))?;
    let obj = root
        .as_object()
        .ok_or_else(|| "根节点必须是 JSON 对象".to_string())?;

    let name = obj
        .get("name")
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(fallback_name);
    let scope_name = obj
        .get("scopeName")
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("source.custom");
    let patterns = obj
        .get("patterns")
        .and_then(Value::as_array)
        .ok_or_else(|| "缺少顶层 patterns 数组".to_string())?;

    let mut yaml = String::from("%YAML 1.2\n---\n");
    yaml.push_str(&format!("name: {}\n", yaml_quote(name)));
    yaml.push_str(&format!("scope: {}\n", yaml_quote(scope_name)));

    if let Some(ft) = obj.get("fileTypes").and_then(Value::as_array) {
        let exts: Vec<&str> = ft
            .iter()
            .filter_map(Value::as_str)
            .map(|s| s.trim_start_matches('.'))
            .filter(|s| !s.is_empty())
            .collect();
        if !exts.is_empty() {
            yaml.push_str("file_extensions:\n");
            for ext in exts {
                yaml.push_str(&format!("  - {}\n", yaml_quote(ext)));
            }
        }
    }

    if let Some(first_line) = obj.get("firstLineMatch").and_then(Value::as_str)
        && !first_line.is_empty()
    {
        yaml.push_str(&format!("first_line_match: {}\n", yaml_quote(first_line)));
    }

    yaml.push_str("contexts:\n  main:\n");
    write_tm_patterns(&mut yaml, patterns, 4)?;

    if let Some(repo) = obj.get("repository").and_then(Value::as_object) {
        for (key, rule_val) in repo {
            let safe_key = sanitize_context_key(key);
            yaml.push_str(&format!("  {safe_key}:\n"));
            write_tm_patterns(&mut yaml, std::slice::from_ref(rule_val), 4)?;
        }
    }

    Ok(yaml)
}

fn write_tm_patterns(yaml: &mut String, patterns: &[Value], indent: usize) -> Result<(), String> {
    let mut wrote = false;
    for item in patterns {
        if let Some(obj) = item.as_object()
            && write_tm_single_rule(yaml, obj, indent)?
        {
            wrote = true;
        }
    }
    if !wrote {
        let pad = " ".repeat(indent);
        yaml.push_str(&format!("{pad}- match: '(?!x)x'\n"));
    }
    Ok(())
}

fn write_tm_single_rule(
    yaml: &mut String,
    obj: &serde_json::Map<String, Value>,
    indent: usize,
) -> Result<bool, String> {
    let pad = " ".repeat(indent);

    if obj.contains_key("while") {
        return Err("不支持 TextMate while 规则，请改用对应的 .sublime-syntax 文件".into());
    }

    if let Some(inc) = obj.get("include").and_then(Value::as_str) {
        if inc == "$self" || inc == "$base" {
            yaml.push_str(&format!("{pad}- include: main\n"));
            return Ok(true);
        }
        if let Some(repo_ref) = inc.strip_prefix('#') {
            yaml.push_str(&format!("{pad}- include: {}\n", sanitize_context_key(repo_ref)));
            return Ok(true);
        }
        // External `source.xyz` scope reference
        yaml.push_str(&format!("{pad}- include: {}\n", yaml_quote(&format!("scope:{inc}"))));
        return Ok(true);
    }

    if let Some(re) = obj.get("match").and_then(Value::as_str) {
        yaml.push_str(&format!("{pad}- match: {}\n", yaml_quote(re)));
        if let Some(scope) = obj.get("name").and_then(Value::as_str) {
            yaml.push_str(&format!("{pad}  scope: {}\n", yaml_quote(scope)));
        }
        if let Some(caps) = obj.get("captures").and_then(Value::as_object) {
            write_tm_captures(yaml, "captures", caps, indent + 2);
        }
        return Ok(true);
    }

    if let (Some(begin_re), Some(end_re)) = (
        obj.get("begin").and_then(Value::as_str),
        obj.get("end").and_then(Value::as_str),
    ) {
        if has_backreference(end_re) {
            return Err("该 .tmLanguage.json 在 end 规则中使用了跨行反向引用（\\1），请改用对应的 .sublime-syntax 文件".to_string());
        }
        yaml.push_str(&format!("{pad}- match: {}\n", yaml_quote(begin_re)));
        let begin_caps = obj
            .get("beginCaptures")
            .or_else(|| obj.get("captures"))
            .and_then(Value::as_object);
        if let Some(caps) = begin_caps {
            write_tm_captures(yaml, "captures", caps, indent + 2);
        }
        yaml.push_str(&format!("{pad}  push:\n"));
        if let Some(meta) = obj.get("name").and_then(Value::as_str) {
            yaml.push_str(&format!("{pad}    - meta_scope: {}\n", yaml_quote(meta)));
        }
        if let Some(c_meta) = obj.get("contentName").and_then(Value::as_str) {
            yaml.push_str(&format!(
                "{pad}    - meta_content_scope: {}\n",
                yaml_quote(c_meta)
            ));
        }
        yaml.push_str(&format!("{pad}    - match: {}\n", yaml_quote(end_re)));
        let end_caps = obj
            .get("endCaptures")
            .or_else(|| obj.get("captures"))
            .and_then(Value::as_object);
        if let Some(caps) = end_caps {
            write_tm_captures(yaml, "captures", caps, indent + 6);
        }
        yaml.push_str(&format!("{pad}      pop: true\n"));
        if let Some(sub) = obj.get("patterns").and_then(Value::as_array) {
            for item in sub {
                if let Some(sub_obj) = item.as_object() {
                    let _ = write_tm_single_rule(yaml, sub_obj, indent + 4)?;
                }
            }
        }
        return Ok(true);
    }

    if let Some(sub) = obj.get("patterns").and_then(Value::as_array) {
        let mut any = false;
        for item in sub {
            if let Some(sub_obj) = item.as_object()
                && write_tm_single_rule(yaml, sub_obj, indent)?
            {
                any = true;
            }
        }
        return Ok(any);
    }

    Ok(false)
}

fn write_tm_captures(
    yaml: &mut String,
    key_label: &str,
    caps: &serde_json::Map<String, Value>,
    indent: usize,
) {
    let entries: Vec<(usize, &str)> = caps
        .iter()
        .filter_map(|(k, v)| {
            let idx = k.parse::<usize>().ok()?;
            let name = v.get("name").and_then(Value::as_str)?;
            Some((idx, name))
        })
        .collect();
    if entries.is_empty() {
        return;
    }
    let pad = " ".repeat(indent);
    yaml.push_str(&format!("{pad}{key_label}:\n"));
    for (idx, scope) in entries {
        yaml.push_str(&format!("{pad}  {idx}: {}\n", yaml_quote(scope)));
    }
}

fn has_backreference(regex_str: &str) -> bool {
    let bytes = regex_str.as_bytes();
    let mut i = 0;
    while i + 1 < bytes.len() {
        if bytes[i] == b'\\' {
            if bytes[i + 1].is_ascii_digit() && bytes[i + 1] != b'0' {
                return true;
            }
            i += 2;
            continue;
        }
        i += 1;
    }
    false
}

fn sanitize_context_key(raw: &str) -> String {
    yaml_quote(&format!("repo_{raw}"))
}

fn yaml_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

fn devops_basename_alias(basename: &str) -> Option<&'static str> {
    let lower = basename.to_ascii_lowercase();
    match lower.as_str() {
        ".bashrc" | ".bash_profile" | ".bash_aliases" | ".bash_logout" | ".profile" | ".zshrc"
        | ".zprofile" | ".zshenv" | "bashrc" | "zshrc" | "pkgbuild" | "apkbuild" | ".envrc" => {
            Some("sh")
        }
        "dockerfile" | "containerfile" => Some("Dockerfile"),
        "makefile" | "gnumakefile" | "justfile" => Some("Makefile"),
        "cmakelists.txt" => Some("CMake"),
        "cargo.lock" | "poetry.lock" | "pipfile" => Some("toml"),
        ".gitconfig" | ".gitmodules" | ".editorconfig" | "sshd_config" | "ssh_config"
        | ".npmrc" | ".yarnrc" => Some("ini"),
        "hosts" | "fstab" | "sudoers" => Some("sh"),
        "nginx.conf" | "mime.types" => Some("nginx"),
        ".env" | ".env.local" | ".env.production" | ".env.development" | ".env.example" => {
            Some("sh")
        }
        _ if lower.starts_with("dockerfile.") => Some("Dockerfile"),
        _ if lower.starts_with(".env.") => Some("sh"),
        _ => None,
    }
}

fn devops_extension_alias(lower_ext: &str) -> Option<&'static str> {
    match lower_ext {
        "zsh" | "bash" | "bats" | "env" => Some("sh"),
        "conf" | "cfg" | "cnf" | "ini" | "service" | "socket" | "timer" | "target" | "mount"
        | "properties" => Some("ini"),
        "jsonc" | "json5" | "jsonl" | "geojson" | "har" => Some("json"),
        "mjs" | "cjs" => Some("js"),
        "mts" | "cts" => Some("ts"),
        "dockerfile" => Some("Dockerfile"),
        "mk" => Some("Makefile"),
        "plist" | "svg" | "csproj" | "props" | "targets" => Some("xml"),
        "gql" => Some("graphql"),
        "tfvars" => Some("tf"),
        "psql" => Some("sql"),
        _ => None,
    }
}

fn map_scope_to_role(scope: Scope) -> Option<CodeTokenRole> {
    let s = scope.build_string();
    let is = |prefix: &str| {
        s == prefix
            || s.strip_prefix(prefix)
                .is_some_and(|rest| rest.starts_with('.'))
    };

    if is("punctuation.definition.string")
        || is("punctuation.definition.comment")
        || is("invalid")
        || is("meta")
        || is("source")
        || is("text")
    {
        return None;
    }

    if is("markup.inserted") || is("punctuation.definition.inserted") {
        Some(CodeTokenRole::Inserted)
    } else if is("markup.deleted") || is("punctuation.definition.deleted") {
        Some(CodeTokenRole::Deleted)
    } else if is("comment.block.documentation") || is("comment.line.documentation") {
        Some(CodeTokenRole::CommentDoc)
    } else if is("comment") {
        Some(CodeTokenRole::Comment)
    } else if is("constant.character.escape") || is("string.regexp") {
        Some(CodeTokenRole::StringEscape)
    } else if is("string") {
        Some(CodeTokenRole::String)
    } else if is("constant.numeric") {
        Some(CodeTokenRole::Number)
    } else if is("constant.language.boolean") {
        Some(CodeTokenRole::Boolean)
    } else if is("constant") || is("support.constant") || is("entity.name.constant") {
        Some(CodeTokenRole::Constant)
    } else if is("keyword.operator") {
        Some(CodeTokenRole::Operator)
    } else if is("keyword") || is("storage.modifier") {
        Some(CodeTokenRole::Keyword)
    } else if is("storage.type") {
        if [
            "class", "enum", "function", "impl", "interface", "module", "namespace", "struct",
            "trait", "type", "let", "var", "const", "def",
        ]
        .iter()
        .any(|w| s.contains(w))
        {
            Some(CodeTokenRole::Keyword)
        } else {
            Some(CodeTokenRole::Type)
        }
    } else if is("entity.name.function")
        || is("support.function")
        || is("variable.function")
    {
        Some(CodeTokenRole::Function)
    } else if is("support.type.property-name") || is("entity.name.tag.yaml") {
        Some(CodeTokenRole::Property)
    } else if is("entity.name.type")
        || is("entity.name.class")
        || is("entity.name.struct")
        || is("entity.name.enum")
        || is("entity.name.trait")
        || is("entity.name.interface")
        || is("entity.other.inherited-class")
        || is("support.type")
        || is("support.class")
    {
        Some(CodeTokenRole::Type)
    } else if is("entity.name.tag") {
        Some(CodeTokenRole::Tag)
    } else if is("entity.other.attribute-name") {
        Some(CodeTokenRole::Attribute)
    } else if is("entity.name.section") || is("markup.heading") || is("markup.changed") {
        Some(CodeTokenRole::Heading)
    } else if is("variable.other.member")
        || is("variable.other.property")
        || is("support.variable.property")
    {
        Some(CodeTokenRole::Property)
    } else if is("variable") {
        Some(CodeTokenRole::Variable)
    } else {
        None
    }
}

// --- GPUI EditorState InputHighlighter Adapter ---

struct SyntectEditorHighlighter {
    language: SharedString,
    syntax_name: String,
    pack: Arc<SyntaxPack>,
    highlights: Rc<RefCell<Vec<(Range<usize>, CodeTokenRole)>>>,
    folds: Rc<RefCell<Vec<FoldRange>>>,
    parse_task: Option<Task<()>>,
}

impl SyntectEditorHighlighter {
    fn new(language: &str, syntax_name: String, pack: Arc<SyntaxPack>) -> Self {
        Self {
            language: SharedString::from(language.to_owned()),
            syntax_name,
            pack,
            highlights: Rc::default(),
            folds: Rc::default(),
            parse_task: None,
        }
    }
}

impl InputHighlighter for SyntectEditorHighlighter {
    fn language(&self) -> SharedString {
        self.language.clone()
    }

    fn update(
        &mut self,
        edit: Option<InputEdit>,
        text: &Rope,
        folding: bool,
        window: &mut gpui_kit::Window,
        cx: &mut gpui_kit::Context<EditorState>,
    ) {
        if let Some(edit) = edit {
            shift_highlights(&mut self.highlights.borrow_mut(), &edit);
        }

        // Refresh pack if extensions were reloaded
        let latest = SyntaxRegistry::global().current();
        if latest.revision != self.pack.revision {
            self.pack = latest;
        }

        if text.len() <= EDITOR_SYNC_PARSE_MAX_BYTES {
            self.parse_task = None;
            let doc = text.to_string();
            let parsed = self
                .pack
                .syntax_set
                .find_syntax_by_name(&self.syntax_name)
                .map(|syn| self.pack.parse_document(syn, &doc))
                .unwrap_or_default();
            *self.highlights.borrow_mut() = parsed;
            *self.folds.borrow_mut() = if folding {
                brace_fold_ranges(&doc)
            } else {
                Vec::new()
            };
            return;
        }

        let pack = self.pack.clone();
        let syntax_name = self.syntax_name.clone();
        let highlights = self.highlights.clone();
        let folds = self.folds.clone();
        let rope_snap = text.clone();
        self.folds.borrow_mut().clear();

        self.parse_task = Some(cx.spawn_in(window, async move |entity, cx| {
            cx.background_executor().timer(EDITOR_PARSE_DEBOUNCE).await;
            let (parsed, folded) = cx
                .background_executor()
                .spawn(async move {
                    let doc = rope_snap.to_string();
                    let parsed = pack
                        .syntax_set
                        .find_syntax_by_name(&syntax_name)
                        .map(|syn| pack.parse_document(syn, &doc))
                        .unwrap_or_default();
                    let folded = if folding {
                        brace_fold_ranges(&doc)
                    } else {
                        Vec::new()
                    };
                    (parsed, folded)
                })
                .await;
            *highlights.borrow_mut() = parsed;
            *folds.borrow_mut() = folded.clone();
            let _ = entity.update(cx, |state, cx| {
                state.apply_highlighter_fold_candidates(folded, cx);
            });
        }));
    }

    fn styles(
        &self,
        range: &Range<usize>,
        resolver: &dyn HighlightStyleResolver,
    ) -> Vec<(Range<usize>, HighlightStyle)> {
        let spans = self.highlights.borrow();
        let first = spans.partition_point(|(r, _)| r.end <= range.start);
        let mut runs = Vec::new();
        let mut cursor = range.start;

        for (span, role) in &spans[first..] {
            if span.start >= range.end {
                break;
            }
            let start = span.start.max(range.start);
            let end = span.end.min(range.end);
            if start >= end || end <= cursor {
                continue;
            }
            if cursor < start {
                runs.push((cursor..start, HighlightStyle::default()));
            }
            let style = resolver
                .style(role.editor_theme_key())
                .or_else(|| role.fallback_editor_style())
                .unwrap_or_default();
            runs.push((start..end, style));
            cursor = end;
        }

        if cursor < range.end {
            runs.push((cursor..range.end, HighlightStyle::default()));
        }
        runs
    }

    fn fold_ranges(&self, _: &Rope) -> Vec<FoldRange> {
        self.folds.borrow().clone()
    }
}

fn shift_highlights(highlights: &mut Vec<(Range<usize>, CodeTokenRole)>, edit: &InputEdit) {
    let delta = edit.new_end_byte as isize - edit.old_end_byte as isize;
    highlights.retain_mut(|(range, _)| {
        if range.end <= edit.start_byte {
            true
        } else if range.start >= edit.old_end_byte {
            range.start = range.start.saturating_add_signed(delta);
            range.end = range.end.saturating_add_signed(delta);
            true
        } else {
            false
        }
    });
}

fn brace_fold_ranges(text: &str) -> Vec<FoldRange> {
    let mut starts = Vec::new();
    let mut ranges = Vec::new();

    for (line_number, line) in text.lines().enumerate() {
        let mut chars = line.chars().peekable();
        let mut quote = None;
        let mut escaped = false;
        while let Some(ch) = chars.next() {
            match quote {
                Some(open) => {
                    if escaped {
                        escaped = false;
                    } else if ch == '\\' {
                        escaped = true;
                    } else if ch == open {
                        quote = None;
                    }
                }
                None => match ch {
                    '/' if chars.peek() == Some(&'/') => break,
                    '#' => break,
                    '"' | '\'' | '`' => quote = Some(ch),
                    '{' => starts.push(line_number),
                    '}' => {
                        if let Some(start) = starts.pop()
                            && line_number.saturating_sub(start) >= 2
                        {
                            ranges.push(FoldRange::new(start, line_number));
                        }
                    }
                    _ => {}
                },
            }
        }
    }
    ranges.sort_by_key(|r| r.start_line);
    ranges.dedup_by_key(|r| r.start_line);
    ranges
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_builtin_languages_devops_aliases_and_shebangs() {
        let pack = SyntaxRegistry::global().current();
        assert!(pack.builtin_count >= 100);

        for hint in [
            "tcpx.sh",
            "/etc/nginx/nginx.conf",
            ".bashrc",
            "Dockerfile",
            "app.service",
            "config.json",
            "Cargo.toml",
            "main.rs",
            "deploy.py",
            "setup.ps1",
            "module.psm1",
            "build.bat",
            "App.vue",
            "Widget.svelte",
            "infra.tf",
            "prod.tfvars",
            "api.proto",
            "schema.graphql",
            "query.gql",
            "config.fish",
            "/etc/hosts",
            "/etc/fstab",
            "diff",
        ] {
            assert!(
                pack.find_syntax(Some(hint), None).is_some(),
                "expected syntax match for `{hint}`"
            );
        }
        let ps = pack.find_syntax(Some("deploy.ps1"), None).unwrap();
        assert_eq!(ps.name, "PowerShell");
        let ps_spans = pack.parse_document(ps, "# comment\nGet-Content -Path $env:HOME\\a.txt\n");
        assert!(ps_spans.iter().any(|(_, r)| *r == CodeTokenRole::Comment));
        assert!(ps_spans.iter().any(|(_, r)| *r == CodeTokenRole::Function));
        assert!(ps_spans.iter().any(|(_, r)| *r == CodeTokenRole::Variable));

        let shebang_py = pack.find_syntax(None, Some("#!/usr/bin/env python3"));
        assert!(shebang_py.is_some());
        let plain_log = pack.find_syntax(None, Some("2026-11-24 INFO server started"));
        assert!(plain_log.is_none());
    }

    #[test]
    fn plain_text_and_multibyte_edit_offsets() {
        let pack = SyntaxRegistry::global().current();
        assert!(pack.find_syntax(Some("Plain Text"), Some("#!/bin/sh")).is_none());
        let factory = SyntaxRegistry::editor_highlighter_factory();
        assert!(factory("Plain Text").is_none());
        assert!(factory("Rust").is_some());
        let _ = pack.find_syntax(None, Some(&"中".repeat(100)));

        let point = gpui_kit::component::input::Point::new(0, 0);
        let edit = InputEdit {
            start_byte: 1, old_end_byte: 4, new_end_byte: 5,
            start_position: point, old_end_position: point, new_end_position: point,
        };
        let role = CodeTokenRole::String;
        let mut spans = vec![(0..1, role), (1..4, role), (4..7, role)];
        shift_highlights(&mut spans, &edit);
        assert_eq!(spans, [(0..1, role), (5..8, role)]);
        assert_eq!(&"a🦀界"[spans[1].0.clone()], "界");
    }

    #[test]
    fn imports_same_name_and_same_file_without_truncating_the_grammar() {
        let tmp = std::env::temp_dir().join(format!("mica-import-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&tmp).unwrap();
        let src = tmp.join("custom.sublime-syntax");
        let dir = tmp.join("syntaxes");
        let dst = dir.join(src.file_name().unwrap());
        let reg = SyntaxRegistry {
            current: RwLock::new(SyntaxRegistry::global().current()),
            next_rev: AtomicU64::new(1),
        };
        let grammar = "name: Review\nscope: source.review\ncontexts:\n  main:\n    - include: 'scope:source.missing'\n";
        fs::create_dir(&dir).unwrap();
        fs::write(&src, grammar).unwrap();
        fs::write(&dst, "old").unwrap();
        for source in [&src, &dst, &dir.join("../syntaxes/custom.sublime-syntax")] {
            let pack = reg.import_files(&dir, std::slice::from_ref(source)).unwrap();
            assert_eq!(pack.loaded_custom_count(), 1);
            assert_eq!(fs::read_to_string(&dst).unwrap(), grammar);
        }
        let invalid = format!("{grammar}first_line_match: '['\n");
        fs::write(&src, invalid).unwrap();
        assert!(reg.import_files(&dir, &[src, tmp.join("absent")]).is_err());
        let pack = reg.current();
        assert!(pack.custom_entries[0].error.as_ref().unwrap().contains("first_line_match"));
        assert!(pack.find_syntax(None, Some("#!/bin/sh")).is_some());
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn loads_custom_sublime_and_tmlanguage_json_while_isolating_bad_files() {
        let tmp = std::env::temp_dir().join(format!("mica-syntax-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&tmp).unwrap();

        let sublime_yaml = r#"%YAML 1.2
---
name: MicaCustomConf
file_extensions: [mconf]
scope: source.mconf
contexts:
  main:
    - match: '#.*$'
      scope: comment.line.mconf
    - match: '\b(enable|disable)\b'
      scope: keyword.control.mconf
"#;
        fs::write(tmp.join("custom.sublime-syntax"), sublime_yaml).unwrap();

        let tm_json = r##"{
  "name": "MicaWireDSL",
  "scopeName": "source.mwiredsl",
  "fileTypes": ["mwire"],
  "patterns": [
    { "include": "#comments" },
    { "match": "\\b(route|upstream)\\b", "name": "keyword.control.mwire" },
    { "begin": "\"", "end": "\"", "name": "string.quoted.double.mwire" }
  ],
  "repository": {
    "comments": {
      "include": "#comment-rule"
    },
    "comment.rule": { "match": "unused", "name": "string.unquoted" },
    "comment-rule": {
      "match": "//.*$",
      "name": "comment.line.double-slash.mwire"
    }
  }
}"##;
        fs::write(tmp.join("wire.tmLanguage.json"), tm_json).unwrap();
        assert!(convert_tmlanguage_json_to_sublime(
            r#"{"patterns":[{"begin":"x","while":"y"}]}"#, "unsupported"
        ).is_err());
        fs::write(tmp.join("broken.sublime-syntax"), "not: valid: sublime: syntax").unwrap();
        fs::write(tmp.join("legacy.tmLanguage"), "<plist></plist>").unwrap();

        let pack = SyntaxRegistry::global().reload_from_dir(Some(&tmp));
        assert_eq!(pack.custom_entries.len(), 4);
        assert_eq!(pack.loaded_custom_count(), 2);

        let mconf = pack.find_syntax(Some("server.mconf"), None).expect("mconf loaded");
        let spans = pack.parse_document(mconf, "enable # 中文注释\n");
        assert!(spans.iter().any(|(_, r)| *r == CodeTokenRole::Keyword));
        assert!(spans.iter().any(|(_, r)| *r == CodeTokenRole::Comment));

        let mwire = pack.find_syntax(Some("edge.mwire"), None).expect("mwire loaded");
        let wire_spans = pack.parse_document(mwire, "route \"10.0.0.1\" // edge\n");
        assert!(wire_spans.iter().any(|(_, r)| *r == CodeTokenRole::Keyword));
        assert!(wire_spans.iter().any(|(_, r)| *r == CodeTokenRole::String));
        assert!(wire_spans.iter().any(|(_, r)| *r == CodeTokenRole::Comment));

        let _ = fs::remove_dir_all(&tmp);
        SyntaxRegistry::global().reload_from_dir(None);
    }
}

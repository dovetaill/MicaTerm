//! Semantic terminal intelligence for developer workflows:
//! - OSC 133 & prompt-heuristic command blocks with status gutter
//! - CJK-accurate output rule highlighting (URLs, file:line:col, Unix/Win paths, IP:port, ERROR/WARN)
//! - Live shell input-line token highlighting (commands, subcommands, flags, strings, operators, vars)
//! - Scrollbar overview ruler markers

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SemanticOptions {
    pub command_blocks: bool,
    pub semantic_highlighting: bool,
    pub code_highlighting: bool,
    pub overview_markers: bool,
}

impl Default for SemanticOptions {
    fn default() -> Self {
        Self {
            command_blocks: true,
            semantic_highlighting: true,
            code_highlighting: true,
            overview_markers: true,
        }
    }
}

/// Describes whether a shell command's stdout is eligible for client-side syntax highlighting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandSyntaxTarget {
    /// Explicit filename, path, or language token hint (e.g. `"tcpx.sh"`, `"yaml"`, `"diff"`).
    Hint(String),
    /// Check the first output line via shebang / diff header only.
    FirstLineOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandBlockStatus {
    Running,
    Success,
    Failure(i32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VisibleCommandBlock {
    pub start_row: usize,
    pub end_row: usize,
    pub show_top_divider: bool,
    pub status: CommandBlockStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverviewMarkerKind {
    CommandRunning,
    CommandSuccess,
    CommandFailure,
    SearchMatch,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OverviewMarker {
    pub ratio_from_top: f32,
    pub kind: OverviewMarkerKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemanticRole {
    OutputUrl,
    OutputFileLine,
    OutputPath,
    OutputEndpoint,
    DiagnosticError,
    DiagnosticWarn,
    InputCommand,
    InputSubcommand,
    InputOption,
    InputString,
    InputOperator,
    InputVariable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowColSpan {
    pub start_col: usize,
    pub end_col: usize,
    pub role: SemanticRole,
}

/// Maps rendered UTF-8 byte offsets on a single terminal row back to exact terminal grid columns,
/// accounting for 2-column CJK / fullwidth characters without column drift.
#[derive(Debug, Clone)]
pub struct RowTextMap {
    pub text: String,
    /// `(byte_start, byte_end, start_col, end_col)` for each non-spacer character in `text`.
    entries: Vec<(usize, usize, usize, usize)>,
}

impl RowTextMap {
    pub fn from_cells(cells: impl Iterator<Item = (usize, usize, char)>) -> Self {
        let mut text = String::with_capacity(128);
        let mut entries = Vec::with_capacity(128);
        for (col, width, ch) in cells {
            let b_start = text.len();
            text.push(ch);
            let b_end = text.len();
            entries.push((b_start, b_end, col, col + width));
        }
        Self { text, entries }
    }

    pub fn byte_range_to_cols(&self, start_byte: usize, end_byte: usize) -> Option<(usize, usize)> {
        if start_byte >= end_byte || self.entries.is_empty() {
            return None;
        }
        let first_idx = self.entries.partition_point(|e| e.1 <= start_byte);
        let first = self.entries.get(first_idx)?;
        if first.0 >= end_byte {
            return None;
        }
        let last_idx = self.entries.partition_point(|e| e.0 < end_byte);
        let last = self.entries.get(last_idx.saturating_sub(1))?;
        (first.2 < last.3).then_some((first.2, last.3))
    }
}

const PROMPT_MARKERS: &[&str] = &["$ ", "# ", "% ", "❯ ", "➜ ", "> "];
const SHELL_OPERATORS: &[&str] = &["||", "&&", "2>&1", "2>>", "2>", ">>", "|", ";", ">", "<"];
const COMMAND_SEPARATORS: &[&str] = &["||", "&&", "|", ";"];
const COMPOUND_COMMANDS: &[&str] = &[
    "git", "cargo", "docker", "kubectl", "systemctl", "ssh", "npm", "pnpm", "yarn", "go", "uv",
    "pip", "apt", "dnf", "brew",
];

/// Returns the byte index in `row.text` right after a recognized shell prompt marker (if any),
/// along with whether a non-empty command follows on the same line.
pub fn detect_prompt_command_byte_offset(line: &str) -> Option<(usize, bool)> {
    let trimmed_end = line.trim_end();
    if trimmed_end.is_empty() {
        return None;
    }

    // Standard `user@host:dir$ `, `[root@host dir]# `, `~/proj ❯ `, `% `
    for marker in PROMPT_MARKERS {
        if let Some(pos) = trimmed_end.rfind(marker) {
            let prefix = &trimmed_end[..pos];
            if looks_like_shell_prompt_prefix(prefix, marker) {
                let cmd_start = pos + marker.len();
                let has_cmd = !trimmed_end[cmd_start..].trim().is_empty();
                return Some((cmd_start, has_cmd));
            }
        }
        // Prompt at end of line with trailing space stripped (e.g. `user@host:~$`)
        let bare = marker.trim_end();
        if trimmed_end.ends_with(bare) {
            let pos = trimmed_end.len() - bare.len();
            let prefix = &trimmed_end[..pos];
            if looks_like_shell_prompt_prefix(prefix, marker) {
                return Some((trimmed_end.len(), false));
            }
        }
    }
    None
}

fn looks_like_shell_prompt_prefix(prefix: &str, marker: &str) -> bool {
    let p = prefix.trim();
    if p.is_empty() {
        return matches!(marker, "$ " | "% ");
    }
    if matches!(marker, "❯ " | "➜ ") {
        return p.len() <= 96;
    }
    if p.len() > 96 {
        return false;
    }
    // Common PS1 shapes: `user@host:path`, `[user@host path]`, `(venv) user@host`, `PS C:\...`
    (p.contains('@') && (p.contains(':') || p.contains(' ') || p.ends_with(']')))
        || (p.starts_with('[') && p.ends_with(']'))
        || p.starts_with("~/")
        || p == "~"
        || p.starts_with("PS ")
}

/// Checks whether an output line strongly indicates a command failure when OSC 133 exit codes
/// are unavailable on an unconfigured remote shell.
pub fn line_has_error_signature(line: &str) -> bool {
    let t = line.trim();
    if t.is_empty() {
        return false;
    }
    t.starts_with("error:")
        || t.starts_with("error[E")
        || t.starts_with("Error:")
        || t.starts_with("FATAL")
        || t.starts_with(" bash: ")
        || t.contains(": command not found")
        || t.contains(": No such file or directory")
        || t.contains(": Permission denied")
        || t.contains("panicked at ")
        || t.starts_with("Traceback (most recent call last):")
}

pub fn detect_output_spans_for_row(map: &RowTextMap) -> Vec<RowColSpan> {
    let line = map.text.trim_end();
    if line.is_empty() {
        return Vec::new();
    }

    let mut out = Vec::new();
    for (tok_start, tok_end) in whitespace_token_ranges(line) {
        let Some((s_byte, e_byte, cleaned)) = trim_punctuation(line, tok_start, tok_end) else {
            continue;
        };

        let role = if is_url(cleaned) {
            Some(SemanticRole::OutputUrl)
        } else if is_file_line_reference(cleaned) {
            Some(SemanticRole::OutputFileLine)
        } else if is_windows_path(cleaned) || is_unix_path(cleaned) {
            Some(SemanticRole::OutputPath)
        } else if is_ipv4_endpoint(cleaned) {
            Some(SemanticRole::OutputEndpoint)
        } else if is_error_keyword(cleaned) {
            Some(SemanticRole::DiagnosticError)
        } else if is_warn_keyword(cleaned) {
            Some(SemanticRole::DiagnosticWarn)
        } else {
            None
        };

        if let Some(role) = role
            && let Some((sc, ec)) = map.byte_range_to_cols(s_byte, e_byte)
        {
            out.push(RowColSpan {
                start_col: sc,
                end_col: ec,
                role,
            });
        }
    }
    out
}

pub fn detect_input_spans_for_prompt_row(map: &RowTextMap, cmd_start_byte: usize) -> Vec<RowColSpan> {
    let line = map.text.trim_end();
    if cmd_start_byte >= line.len() {
        return Vec::new();
    }

    let mut out = Vec::new();
    let tokens = tokenize_shell_input(line, cmd_start_byte);
    let mut command_expected = true;
    let mut compound_cmd = false;
    let mut subcommand_seen = false;

    for tok in tokens {
        let slice = &line[tok.start_byte..tok.end_byte];
        let role = if tok.is_operator {
            if COMMAND_SEPARATORS.contains(&slice) {
                command_expected = true;
                compound_cmd = false;
                subcommand_seen = false;
            }
            SemanticRole::InputOperator
        } else if command_expected {
            // Skip leading `VAR=val` env assignments
            if !tok.quoted && slice.contains('=') && !slice.starts_with('-') {
                SemanticRole::InputVariable
            } else {
                command_expected = false;
                let base_cmd = slice.rsplit('/').next().unwrap_or(slice);
                compound_cmd = COMPOUND_COMMANDS.contains(&base_cmd);
                subcommand_seen = false;
                SemanticRole::InputCommand
            }
        } else if compound_cmd && !subcommand_seen && !tok.quoted && !slice.starts_with('-') {
            subcommand_seen = true;
            SemanticRole::InputSubcommand
        } else if tok.quoted {
            SemanticRole::InputString
        } else if slice.starts_with('-') {
            SemanticRole::InputOption
        } else if slice.starts_with('$') {
            SemanticRole::InputVariable
        } else if is_unix_path(slice) || is_windows_path(slice) {
            SemanticRole::OutputPath
        } else {
            continue;
        };

        if let Some((sc, ec)) = map.byte_range_to_cols(tok.start_byte, tok.end_byte) {
            out.push(RowColSpan {
                start_col: sc,
                end_col: ec,
                role,
            });
        }
    }
    out
}

pub fn find_url_at_col(map: &RowTextMap, col: usize) -> Option<String> {
    let line = map.text.trim_end();
    for (tok_start, tok_end) in whitespace_token_ranges(line) {
        let Some((s_byte, e_byte, cleaned)) = trim_punctuation(line, tok_start, tok_end) else {
            continue;
        };
        if is_url(cleaned)
            && let Some((sc, ec)) = map.byte_range_to_cols(s_byte, e_byte)
            && col >= sc
            && col < ec
        {
            return Some(cleaned.to_string());
        }
    }
    None
}

struct InputTok {
    start_byte: usize,
    end_byte: usize,
    quoted: bool,
    is_operator: bool,
}

fn tokenize_shell_input(line: &str, from_byte: usize) -> Vec<InputTok> {
    let mut out = Vec::new();
    let bytes = line.as_bytes();
    let mut i = from_byte;

    while i < bytes.len() {
        while i < bytes.len() && line[i..].chars().next().is_some_and(char::is_whitespace) {
            i += line[i..].chars().next().map_or(1, char::len_utf8);
        }
        if i >= bytes.len() {
            break;
        }

        if let Some(op) = SHELL_OPERATORS.iter().find(|op| line[i..].starts_with(*op)) {
            let end = i + op.len();
            out.push(InputTok {
                start_byte: i,
                end_byte: end,
                quoted: false,
                is_operator: true,
            });
            i = end;
            continue;
        }

        let ch = line[i..].chars().next().unwrap_or(' ');
        if ch == '\'' || ch == '"' {
            let quote = ch;
            let start = i;
            i += quote.len_utf8();
            while i < bytes.len() {
                let c = line[i..].chars().next().unwrap_or(' ');
                i += c.len_utf8();
                if c == '\\' && quote == '"' && i < bytes.len() {
                    let esc = line[i..].chars().next().unwrap_or(' ');
                    i += esc.len_utf8();
                    continue;
                }
                if c == quote {
                    break;
                }
            }
            out.push(InputTok {
                start_byte: start,
                end_byte: i,
                quoted: true,
                is_operator: false,
            });
            continue;
        }

        let start = i;
        while i < bytes.len() {
            if SHELL_OPERATORS.iter().any(|op| line[i..].starts_with(*op)) {
                break;
            }
            let c = line[i..].chars().next().unwrap_or(' ');
            if c.is_whitespace() || c == '\'' || c == '"' {
                break;
            }
            i += c.len_utf8();
        }
        if i > start {
            out.push(InputTok {
                start_byte: start,
                end_byte: i,
                quoted: false,
                is_operator: false,
            });
        }
    }
    out
}

fn whitespace_token_ranges(line: &str) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    let mut start = None;
    for (idx, ch) in line.char_indices() {
        if ch.is_whitespace() {
            if let Some(s) = start.take() {
                ranges.push((s, idx));
            }
        } else if start.is_none() {
            start = Some(idx);
        }
    }
    if let Some(s) = start {
        ranges.push((s, line.len()));
    }
    ranges
}

fn trim_punctuation(line: &str, start: usize, end: usize) -> Option<(usize, usize, &str)> {
    let mut s = start;
    let mut e = end;
    while s < e {
        let ch = line[s..].chars().next()?;
        if matches!(ch, '(' | '[' | '<' | '\'' | '"' | '`') {
            s += ch.len_utf8();
        } else {
            break;
        }
    }
    while e > s {
        let ch = line[..e].chars().next_back()?;
        if matches!(ch, ',' | ';' | ')' | ']' | '>' | '\'' | '"' | '`') {
            e -= ch.len_utf8();
        } else if (ch == '.' && is_url(&line[s..e]))
            || (ch == ':'
                && !line[s..e].contains("://")
                && !is_file_line_reference(&line[s..e]))
        {
            e -= 1;
        } else {
            break;
        }
    }
    (s < e).then_some((s, e, &line[s..e]))
}

fn is_url(tok: &str) -> bool {
    (tok.starts_with("https://") || tok.starts_with("http://")) && tok.len() > 10
}

fn is_unix_path(tok: &str) -> bool {
    if tok.len() < 3 || tok.contains("://") {
        return false;
    }
    (tok.starts_with('/') && !tok.starts_with("//"))
        || tok.starts_with("./")
        || tok.starts_with("../")
        || tok.starts_with("~/")
}

fn is_windows_path(tok: &str) -> bool {
    let b = tok.as_bytes();
    b.len() > 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b[2] == b'\\' || b[2] == b'/')
}

fn is_file_line_reference(tok: &str) -> bool {
    if tok.contains("://") {
        return false;
    }
    let Some((prefix, tail)) = tok.rsplit_once(':') else {
        return false;
    };
    if tail.is_empty() || !tail.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }
    let file_part = if let Some((f, line_part)) = prefix.rsplit_once(':')
        && !line_part.is_empty()
        && line_part.chars().all(|c| c.is_ascii_digit())
    {
        f
    } else {
        prefix
    };
    if file_part.is_empty() || file_part.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return false;
    }
    file_part.contains('/')
        || file_part.contains('\\')
        || [
            ".rs", ".py", ".go", ".ts", ".tsx", ".js", ".jsx", ".c", ".cpp", ".h", ".java",
            ".toml", ".json", ".yaml", ".yml", ".md", ".sh", ".sql", ".vue",
        ]
        .iter()
        .any(|ext| file_part.ends_with(ext))
}

fn is_ipv4_endpoint(tok: &str) -> bool {
    let Some((host, port)) = tok.rsplit_once(':') else {
        return false;
    };
    if port.is_empty() || port.len() > 5 || !port.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }
    let parts: Vec<&str> = host.split('.').collect();
    parts.len() == 4
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.len() <= 3 && p.parse::<u8>().is_ok())
}

fn is_error_keyword(tok: &str) -> bool {
    matches!(tok, "ERROR" | "FATAL" | "PANIC" | "FAILED")
        || tok.starts_with("error[E")
}

fn is_warn_keyword(tok: &str) -> bool {
    matches!(tok, "WARN" | "WARNING")
}

/// Extracts a remote directory path from standard Linux/macOS window titles such as
/// `user@host: /var/log/nginx` or `user@host:~/workspace`.
pub fn extract_cwd_from_window_title(title: &str, remote_home: Option<&str>) -> Option<String> {
    let t = title.trim();
    // Match `user@host: /path` or `user@host:/path` or `user@host:~/path`
    let candidate = if let Some((_, after_colon)) = t.split_once(':') {
        after_colon.trim()
    } else if t.starts_with('/') || t.starts_with("~/") || t == "~" {
        t
    } else {
        return None;
    };

    if candidate.starts_with('/') && !candidate.contains(' ') {
        return Some(candidate.to_string());
    }
    if candidate == "~" {
        return remote_home.map(ToString::to_string);
    }
    if let Some(rel) = candidate.strip_prefix("~/")
        && !rel.contains(' ')
        && let Some(home) = remote_home
    {
        let base = home.trim_end_matches('/');
        return Some(format!("{base}/{rel}"));
    }
    None
}

/// Determines whether a command string (after the shell prompt) produces syntax-highlightable
/// output on stdout, returning either a file/language hint or `FirstLineOnly` fallback.
pub fn detect_command_syntax_target(cmd_line: &str) -> Option<CommandSyntaxTarget> {
    let trimmed = cmd_line.trim();
    if trimmed.is_empty() {
        return None;
    }

    let tokens = tokenize_shell_input(trimmed, 0);
    if tokens.is_empty() {
        return None;
    }

    // Isolate the final `&&` / `||` / `;` command list so `cd /etc/nginx && cat nginx.conf` works
    let mut chain_start = 0usize;
    for (idx, tok) in tokens.iter().enumerate() {
        let s = &trimmed[tok.start_byte..tok.end_byte];
        if tok.is_operator && matches!(s, ";" | "&&" | "||") {
            chain_start = idx + 1;
        }
    }
    let chain = &tokens[chain_start..];
    if chain.is_empty() {
        return None;
    }

    // Split the active chain on `|` pipeline stages
    let mut stages: Vec<&[InputTok]> = Vec::new();
    let mut stage_start = 0usize;
    for (idx, tok) in chain.iter().enumerate() {
        let s = &trimmed[tok.start_byte..tok.end_byte];
        if tok.is_operator && s == "|" {
            stages.push(&chain[stage_start..idx]);
            stage_start = idx + 1;
        }
    }
    stages.push(&chain[stage_start..]);

    let mut current: Option<CommandSyntaxTarget> = None;
    for stage in stages {
        current = evaluate_pipeline_stage(trimmed, stage, current);
    }
    current
}

fn evaluate_pipeline_stage(
    line: &str,
    stage: &[InputTok],
    upstream: Option<CommandSyntaxTarget>,
) -> Option<CommandSyntaxTarget> {
    // Strip redirections (`> out`, `2>&1`, `< in`) while noting `< input_file` and stdout redirect
    let mut args: Vec<&str> = Vec::with_capacity(stage.len());
    let mut stdin_file: Option<&str> = None;
    let mut i = 0usize;
    while i < stage.len() {
        let tok = &stage[i];
        let raw = &line[tok.start_byte..tok.end_byte];
        if tok.is_operator {
            match raw {
                ">" | ">>" => return None, // stdout redirected to a file, nothing printed to terminal
                "<" => {
                    if let Some(next_tok) = stage.get(i + 1)
                        && !next_tok.is_operator
                    {
                        stdin_file = Some(unquote_shell_token(
                            &line[next_tok.start_byte..next_tok.end_byte],
                        ));
                        i += 2;
                        continue;
                    }
                }
                "2>" | "2>>" => {
                    i += 2;
                    continue;
                }
                _ => {}
            }
            i += 1;
            continue;
        }
        args.push(raw);
        i += 1;
    }

    // Strip leading env assignments (`FOO=1`) and transparent command wrappers (`sudo`, `env`, `time`, `command`)
    let mut idx = 0usize;
    while idx < args.len() {
        let raw = args[idx];
        let unq = unquote_shell_token(raw);
        if !raw.starts_with(['\'', '"', '-']) && unq.contains('=') {
            idx += 1;
            continue;
        }
        let base = unq.trim_start_matches('\\').rsplit('/').next().unwrap_or(unq);
        match base {
            "env" | "time" | "command" | "builtin" | "nohup" => {
                idx += 1;
                while idx < args.len() && args[idx].starts_with('-') {
                    idx += 1;
                }
            }
            "sudo" => {
                idx += 1;
                while idx < args.len() && args[idx].starts_with('-') {
                    let flag = args[idx];
                    idx += 1;
                    if matches!(flag, "-u" | "-g" | "-C" | "-D" | "-h" | "-p" | "-r" | "-t" | "-U")
                        && idx < args.len()
                    {
                        idx += 1;
                    }
                }
            }
            _ => break,
        }
    }

    let cmd_raw = *args.get(idx)?;
    let cmd_unq = unquote_shell_token(cmd_raw).trim_start_matches('\\');
    let cmd = cmd_unq.rsplit('/').next().unwrap_or(cmd_unq);
    let rest = &args[(idx + 1)..];

    match cmd {
        "cat" | "tac" | "head" | "tail" | "more" | "less" | "bat" | "batcat" => {
            let mut positional: Vec<&str> = Vec::new();
            let mut opts_done = false;
            let mut j = 0usize;
            while j < rest.len() {
                let a = unquote_shell_token(rest[j]);
                if !opts_done && a == "--" {
                    opts_done = true;
                    j += 1;
                    continue;
                }
                if !opts_done && a.starts_with('-') && a != "-" {
                    // Skip `cat -n` / `cat -b` line-numbered output to avoid column offset drift
                    if cmd == "cat"
                        && (a == "--number"
                            || a == "--number-nonblank"
                            || (!a.starts_with("--") && (a.contains('n') || a.contains('b'))))
                    {
                        return None;
                    }
                    if let Some(lang) = a
                        .strip_prefix("--language=")
                        .or_else(|| a.strip_prefix("-l="))
                        && !lang.is_empty()
                    {
                        return Some(CommandSyntaxTarget::Hint(lang.to_string()));
                    }
                    if matches!(a, "-l" | "--language")
                        && let Some(next_lang) = rest.get(j + 1)
                    {
                        return Some(CommandSyntaxTarget::Hint(
                            unquote_shell_token(next_lang).to_string(),
                        ));
                    }
                    if matches!(a, "-n" | "-c" | "--lines" | "--bytes")
                        && let Some(next_val) = rest.get(j + 1)
                        && next_val
                            .trim_start_matches(['+', '-'])
                            .chars()
                            .next()
                            .is_some_and(|c| c.is_ascii_digit())
                    {
                        j += 2;
                        continue;
                    }
                    j += 1;
                    continue;
                }
                positional.push(a);
                j += 1;
            }

            match positional.len() {
                1 if positional[0] != "-" => Some(CommandSyntaxTarget::Hint(positional[0].to_string())),
                0 | 1 => upstream
                    .or_else(|| stdin_file.map(|f| CommandSyntaxTarget::Hint(f.to_string())))
                    .or(Some(CommandSyntaxTarget::FirstLineOnly)),
                _ => None,
            }
        }
        "sed" => {
            let mut has_explicit_script = false;
            let mut opts_done = false;
            let mut positional: Vec<&str> = Vec::new();
            let mut j = 0usize;
            while j < rest.len() {
                let a = unquote_shell_token(rest[j]);
                if !opts_done && a == "--" {
                    opts_done = true;
                    j += 1;
                    continue;
                }
                if !opts_done && a.starts_with('-') && a != "-" {
                    if a == "--in-place" || a.starts_with("--in-place=") {
                        return None;
                    }
                    if matches!(a, "-e" | "-f" | "--expression" | "--file") {
                        has_explicit_script = true;
                        j += 2;
                        continue;
                    }
                    if a.starts_with("--expression=") || a.starts_with("--file=") {
                        has_explicit_script = true;
                        j += 1;
                        continue;
                    }
                    if !a.starts_with("--") {
                        // Reject `-i` (including combined `-ni`); recognize `-e<script>` / `-f<file>`
                        for (ci, ch) in a[1..].char_indices() {
                            if ch == 'i' {
                                return None;
                            }
                            if ch == 'e' || ch == 'f' {
                                has_explicit_script = true;
                                if ci + ch.len_utf8() == a.len() - 1 {
                                    j += 1; // script/file is next separate token
                                }
                                break;
                            }
                        }
                    }
                    j += 1;
                    continue;
                }
                if a != "-" {
                    positional.push(a);
                }
                j += 1;
            }
            let file_slice = if has_explicit_script {
                &positional[..]
            } else {
                positional.get(1..).unwrap_or(&[])
            };
            match file_slice {
                [single] => Some(CommandSyntaxTarget::Hint((*single).to_string())),
                [] => stdin_file
                    .map(|f| CommandSyntaxTarget::Hint(f.to_string()))
                    .or(upstream),
                _ => None,
            }
        }
        "diff" | "colordiff" => Some(CommandSyntaxTarget::Hint("diff".to_string())),
        "git" => {
            let sub = rest
                .iter()
                .map(|s| unquote_shell_token(s))
                .find(|s| !s.starts_with('-'))?;
            match sub {
                "diff" | "show" | "format-patch" => {
                    Some(CommandSyntaxTarget::Hint("diff".to_string()))
                }
                _ => None,
            }
        }
        "jq" => {
            let raw_mode = rest.iter().any(|r| {
                let a = unquote_shell_token(r);
                a == "--raw-output" || (!a.starts_with("--") && a.starts_with('-') && a.contains('r'))
            });
            (!raw_mode).then(|| CommandSyntaxTarget::Hint("json".to_string()))
        }
        "yq" => Some(CommandSyntaxTarget::Hint("yaml".to_string())),
        "kubectl" | "helm" | "oc" => {
            let mut j = 0usize;
            while j < rest.len() {
                let a = unquote_shell_token(rest[j]);
                if let Some(fmt) = a
                    .strip_prefix("-o=")
                    .or_else(|| a.strip_prefix("--output="))
                    .or_else(|| a.strip_prefix("-o"))
                    && !fmt.is_empty()
                {
                    if fmt == "yaml" || fmt == "yml" {
                        return Some(CommandSyntaxTarget::Hint("yaml".to_string()));
                    }
                    if fmt == "json" {
                        return Some(CommandSyntaxTarget::Hint("json".to_string()));
                    }
                }
                if matches!(a, "-o" | "--output")
                    && let Some(next_fmt) = rest.get(j + 1).map(|s| unquote_shell_token(s))
                {
                    if next_fmt == "yaml" || next_fmt == "yml" {
                        return Some(CommandSyntaxTarget::Hint("yaml".to_string()));
                    }
                    if next_fmt == "json" {
                        return Some(CommandSyntaxTarget::Hint("json".to_string()));
                    }
                }
                j += 1;
            }
            if cmd == "helm"
                && rest
                    .iter()
                    .map(|s| unquote_shell_token(s))
                    .find(|s| !s.starts_with('-'))
                    == Some("template")
            {
                return Some(CommandSyntaxTarget::Hint("yaml".to_string()));
            }
            None
        }
        "docker" | "podman" | "nerdctl" => {
            let sub = rest
                .iter()
                .map(|s| unquote_shell_token(s))
                .find(|s| !s.starts_with('-'))?;
            (sub == "inspect").then(|| CommandSyntaxTarget::Hint("json".to_string()))
        }
        "systemctl" => {
            let sub = rest
                .iter()
                .map(|s| unquote_shell_token(s))
                .find(|s| !s.starts_with('-'))?;
            (sub == "cat").then(|| CommandSyntaxTarget::Hint("ini".to_string()))
        }
        "crontab" => {
            let is_list = rest.iter().any(|s| unquote_shell_token(s) == "-l");
            is_list.then(|| CommandSyntaxTarget::Hint("sh".to_string()))
        }
        "curl" | "wget" => Some(CommandSyntaxTarget::FirstLineOnly),
        _ => None,
    }
}

fn unquote_shell_token(tok: &str) -> &str {
    if tok.len() >= 2
        && ((tok.starts_with('"') && tok.ends_with('"'))
            || (tok.starts_with('\'') && tok.ends_with('\'')))
    {
        &tok[1..tok.len() - 1]
    } else {
        tok
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distinguishes_comments_from_root_prompts() {
        assert_eq!(detect_prompt_command_byte_offset("# 常规交互式启动"), None);
        for prompt in ["root@host:~# ", "[root@host ~]# ", "$ ", "% "] {
            assert_eq!(
                detect_prompt_command_byte_offset(&format!("{prompt}tail tcpx.sh")),
                Some((prompt.len(), true))
            );
        }
    }

    #[test]
    fn cjk_row_map_aligns_columns_accurately() {
        // "中文: " -> '中'(0..2), '文'(2..4), ':'(4..5), ' '(5..6), then "src/main.rs:42"(6..20)
        let mut cells = vec![
            (0, 2, '中'),
            (2, 2, '文'),
            (4, 1, ':'),
            (5, 1, ' '),
        ];
        for (i, ch) in "src/main.rs:42 https://example.com".chars().enumerate() {
            cells.push((6 + i, 1, ch));
        }
        let map = RowTextMap::from_cells(cells.into_iter());
        let spans = detect_output_spans_for_row(&map);
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].role, SemanticRole::OutputFileLine);
        assert_eq!(spans[0].start_col, 6);
        assert_eq!(spans[0].end_col, 20);
        assert_eq!(spans[1].role, SemanticRole::OutputUrl);
        assert_eq!(spans[1].start_col, 21);
        assert_eq!(
            find_url_at_col(&map, 25).as_deref(),
            Some("https://example.com")
        );
    }

    #[test]
    fn extracts_cwd_from_standard_bash_titles() {
        assert_eq!(
            extract_cwd_from_window_title("ubuntu@prod-01: /etc/nginx", Some("/home/ubuntu"))
                .as_deref(),
            Some("/etc/nginx")
        );
        assert_eq!(
            extract_cwd_from_window_title("ubuntu@prod-01:~/app", Some("/home/ubuntu")).as_deref(),
            Some("/home/ubuntu/app")
        );
    }

    #[test]
    fn detects_syntax_targets_and_skips_non_code_commands() {
        assert_eq!(
            detect_command_syntax_target("cat tcpx.sh"),
            Some(CommandSyntaxTarget::Hint("tcpx.sh".into()))
        );
        assert_eq!(
            detect_command_syntax_target("sudo -u root head -n 50 /etc/nginx/nginx.conf"),
            Some(CommandSyntaxTarget::Hint("/etc/nginx/nginx.conf".into()))
        );
        assert_eq!(
            detect_command_syntax_target("cd /srv && cat deploy.py | head -n 20"),
            Some(CommandSyntaxTarget::Hint("deploy.py".into()))
        );
        assert_eq!(
            detect_command_syntax_target("sed -n '1,40p' Cargo.toml"),
            Some(CommandSyntaxTarget::Hint("Cargo.toml".into()))
        );
        assert_eq!(
            detect_command_syntax_target("kubectl get deploy api -o yaml"),
            Some(CommandSyntaxTarget::Hint("yaml".into()))
        );
        assert_eq!(
            detect_command_syntax_target("git diff HEAD~1"),
            Some(CommandSyntaxTarget::Hint("diff".into()))
        );
        assert_eq!(
            detect_command_syntax_target("curl -fsSL https://example.com/install"),
            Some(CommandSyntaxTarget::FirstLineOnly)
        );

        // Non-highlighted or redirected/numbered commands must return None
        assert_eq!(detect_command_syntax_target("cat -n tcpx.sh"), None);
        assert_eq!(detect_command_syntax_target("sed -i 's/a/b/' tcpx.sh"), None);
        assert_eq!(detect_command_syntax_target("cat tcpx.sh > copy.sh"), None);
        assert_eq!(detect_command_syntax_target("cat a.py b.py"), None);
        assert_eq!(detect_command_syntax_target("cat - a.py"), None);
        assert_eq!(detect_command_syntax_target("sed -n '1p' a.py b.py"), None);
        assert_eq!(
            detect_command_syntax_target("sed -nr '1,30p' nginx.conf"),
            Some(CommandSyntaxTarget::Hint("nginx.conf".into()))
        );
        assert_eq!(
            detect_command_syntax_target("sed -n -e '/中文/p' '配置.py'"),
            Some(CommandSyntaxTarget::Hint("配置.py".into()))
        );
        assert_eq!(detect_command_syntax_target("ls -la /var/log"), None);
        assert_eq!(detect_command_syntax_target("ps aux"), None);
    }
}

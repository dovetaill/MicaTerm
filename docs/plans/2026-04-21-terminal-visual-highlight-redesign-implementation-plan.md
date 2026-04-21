# Terminal Visual and Highlight Redesign Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Rebuild the shell chrome hierarchy, terminal palette, command/status decorations, and semantic highlight pipeline around a single calm default theme while preserving terminal performance and the user's current font choices.

**Architecture:** Introduce a Rust-side `AppThemeSpec` root that owns shell, terminal, decoration, and semantic style roles; project those values into Slint shell tokens and terminal palette conversion; then refactor semantic analyzers so they emit roles, command blocks, and overview markers instead of hard-coded RGBA overlays.

**Tech Stack:** Rust, Slint, existing terminal presenter/model pipeline, existing shell/view-model preference plumbing, targeted Rust tests, existing shell smoke tests.

---

### Task 1: Establish the unified theme root and persisted variant selection

**Files:**
- Modify: `src/theme/spec.rs`
- Modify: `src/theme/mod.rs`
- Modify: `src/app/ui_preferences.rs`
- Modify: `src/app/bootstrap.rs`
- Create: `tests/theme_terminal_redesign_spec.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn app_theme_spec_exposes_premium_default_and_legacy_variant() {
    let premium = app_theme_spec(ThemeMode::Dark, ThemeVariant::PremiumDefault);
    let legacy = app_theme_spec(ThemeMode::Dark, ThemeVariant::LegacyHackerGreen);

    assert_eq!(premium.variant, ThemeVariant::PremiumDefault);
    assert_eq!(legacy.variant, ThemeVariant::LegacyHackerGreen);
    assert_ne!(premium.terminal.background.base, legacy.terminal.background.base);
}

#[test]
fn ui_preferences_round_trip_persists_theme_variant() {
    let prefs = UiPreferences {
        theme_variant: ThemeVariant::PremiumDefault,
        ..UiPreferences::default()
    };

    let json = serde_json::to_string(&prefs).unwrap();
    let decoded: UiPreferences = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.theme_variant, ThemeVariant::PremiumDefault);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test theme_terminal_redesign_spec app_theme_spec_exposes_premium_default_and_legacy_variant -- --exact`
Expected: FAIL because `ThemeVariant` / `app_theme_spec` / `theme_variant` persistence do not exist yet.

**Step 3: Write minimal implementation**

```rust
pub enum ThemeVariant {
    PremiumDefault,
    LegacyHackerGreen,
}

pub struct AppThemeSpec {
    pub mode: ThemeMode,
    pub variant: ThemeVariant,
    pub shell: ShellChromeTheme,
    pub terminal: TerminalTheme,
    pub decoration: DecorationTheme,
    pub semantic: SemanticHighlightTheme,
}

pub fn app_theme_spec(mode: ThemeMode, variant: ThemeVariant) -> AppThemeSpec {
    match (mode, variant) {
        // fill in premium default first
        // keep legacy variant as a second preset with distinct terminal values
    }
}
```

Also add `theme_variant` to `UiPreferences`, default it to `PremiumDefault`, and thread it through bootstrap state loading so later tasks can consume it.

**Step 4: Run test to verify it passes**

Run: `cargo test --test theme_terminal_redesign_spec -- --nocapture`
Expected: PASS.

**Step 5: Commit**

```bash
git add src/theme/spec.rs src/theme/mod.rs src/app/ui_preferences.rs src/app/bootstrap.rs tests/theme_terminal_redesign_spec.rs
git commit -m "feat: add terminal redesign theme root"
```

### Task 2: Replace shell chrome token values and adopt them across Slint shell surfaces

**Files:**
- Modify: `ui/theme/tokens.slint`
- Modify: `ui/components/active-tab.slint`
- Modify: `ui/components/sidebar-nav-button.slint`
- Modify: `ui/components/asset-node-row.slint`
- Modify: `ui/shell/titlebar.slint`
- Modify: `ui/shell/tabbar.slint`
- Modify: `ui/shell/sidebar.slint`
- Modify: `ui/shell/assets-sidebar.slint`
- Modify: `ui/shell/terminal-session-host.slint`
- Modify: `ui/shell/right-panel.slint`
- Modify: `ui/shell/workspace-pane.slint`
- Create: `tests/theme_terminal_redesign_smoke.sh`
- Modify: `tests/theme_semantic_token_contract_spec.rs`

**Step 1: Write the failing test**

```bash
#!/usr/bin/env bash
set -euo pipefail

TOKENS=ui/theme/tokens.slint
ACTIVE_TAB=ui/components/active-tab.slint
ASSET_ROW=ui/components/asset-node-row.slint

grep -F 'out property <brush> titlebar-background:' "$TOKENS"
grep -F 'out property <brush> terminal-surface-background:' "$TOKENS"
grep -F 'out property <brush> sidebar-item-selected-border:' "$TOKENS"
grep -F 'ThemeTokens.tab-active-line' "$ACTIVE_TAB"
grep -F 'ThemeTokens.sidebar-item-selected-background' "$ASSET_ROW"
```

Add matching Rust assertions that old generic values are gone and the new shell ladder tokens exist.

**Step 2: Run test to verify it fails**

Run: `bash tests/theme_terminal_redesign_smoke.sh`
Expected: FAIL because the renamed token family and component references do not exist yet.

**Step 3: Write minimal implementation**

- Replace the current shell token values in `ui/theme/tokens.slint` with the approved Premium Default values.
- Rename or add semantic tokens so the shell components read from a consistent ladder:
  - `titlebar-background`
  - `tabbar-background`
  - `sidebar-background`
  - `sidebar-panel-background`
  - `terminal-frame-background`
  - `terminal-surface-background`
  - `sidebar-item-hover-background`
  - `sidebar-item-selected-background`
  - `sidebar-item-selected-border`
  - `tab-active-line`
- Update the Slint components so active tabs use a calmer filled surface plus hairline indicator, and selected asset rows use low-saturation fill plus light border instead of a hard outline.

**Step 4: Run test to verify it passes**

Run: `bash tests/theme_terminal_redesign_smoke.sh && cargo test --test theme_semantic_token_contract_spec -- --nocapture`
Expected: PASS.

**Step 5: Commit**

```bash
git add ui/theme/tokens.slint ui/components/active-tab.slint ui/components/sidebar-nav-button.slint ui/components/asset-node-row.slint ui/shell/titlebar.slint ui/shell/tabbar.slint ui/shell/sidebar.slint ui/shell/assets-sidebar.slint ui/shell/terminal-session-host.slint ui/shell/right-panel.slint ui/shell/workspace-pane.slint tests/theme_terminal_redesign_smoke.sh tests/theme_semantic_token_contract_spec.rs
git commit -m "feat: apply premium terminal shell chrome tokens"
```

### Task 3: Rework the terminal palette, ANSI mapping, and fallback projection

**Files:**
- Modify: `src/theme/spec.rs`
- Modify: `src/app/terminal_theme.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `ui/theme/tokens.slint`
- Modify: `tests/terminal_theme_selection_spec.rs`
- Modify: `tests/ssh_terminal_interaction_spec.rs`

**Step 1: Write the failing test**

Update or add tests so the new Premium Default palette is explicit:

```rust
#[test]
fn dark_theme_maps_terminal_palette_to_premium_default_graphite() {
    let preset = preset_for_theme_mode(ThemeMode::Dark);
    assert_eq!(preset.background, 0x0c_141c);
    assert_eq!(preset.foreground, 0xe3_eaf2);
    assert_eq!(preset.ansi[4], (0x7d, 0x9b, 0xc2));
}

#[test]
fn slint_terminal_tokens_match_new_terminal_defaults() {
    let tokens = std::fs::read_to_string("ui/theme/tokens.slint").unwrap();
    assert!(tokens.contains("terminal-default-fg: dark-mode ? #e3eaf2 : #263240;"));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test terminal_theme_selection_spec dark_theme_maps_terminal_palette_to_premium_default_graphite -- --exact`
Expected: FAIL because the old graphite/canvas values are still active.

**Step 3: Write minimal implementation**

```rust
fn terminal_theme_for(spec: &AppThemeSpec) -> TerminalThemePreset {
    TerminalThemePreset {
        name: spec.terminal.name,
        background: spec.terminal.background.base,
        foreground: spec.terminal.foreground.default,
        viewport_bg_top: spec.terminal.background.gradient_top,
        viewport_bg_bottom: spec.terminal.background.gradient_bottom,
        cursor_bg: spec.terminal.cursor.background,
        cursor_fg: spec.terminal.cursor.foreground,
        selection_bg: rgba_components(
            spec.terminal.selection.rgb,
            spec.terminal.selection.alpha,
        ),
        ansi: spec.terminal.ansi.as_rgb_tuples(),
        scrollbar_thumb: rgb_components(spec.terminal.scrollbar.thumb),
        scrollbar_thumb_active: rgb_components(spec.terminal.scrollbar.thumb_active),
        split: rgb_components(spec.shell.border),
    }
}
```

Make bootstrap project the same values into Slint terminal properties and selection overlay helpers.

**Step 4: Run test to verify it passes**

Run: `cargo test --test terminal_theme_selection_spec -- --nocapture`
Expected: PASS.

**Step 5: Commit**

```bash
git add src/theme/spec.rs src/app/terminal_theme.rs src/app/bootstrap.rs ui/theme/tokens.slint tests/terminal_theme_selection_spec.rs tests/ssh_terminal_interaction_spec.rs
git commit -m "feat: refresh terminal palette for premium default theme"
```

### Task 4: Refactor semantic types so analyzers emit roles instead of hard-coded colors

**Files:**
- Create: `src/app/terminal_semantic/types.rs`
- Modify: `src/app/terminal_semantic/mod.rs`
- Modify: `src/app/terminal_semantic/input_line.rs`
- Modify: `src/app/terminal_semantic/output_blocks.rs`
- Modify: `src/app/terminal_model.rs`
- Modify: `src/app/terminal_presenter.rs`
- Modify: `tests/terminal_scrollback_spec.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn semantic_input_roles_cover_command_path_variable_and_operator() {
    let frame = semantic_model_frame(&["$ cargo run --bin app ./fixtures $HOME && echo done"]);
    let spans = detect_input_line_spans(&frame);

    assert!(spans.iter().any(|span| span.role == SemanticStyleRole::InputCommand));
    assert!(spans.iter().any(|span| span.role == SemanticStyleRole::InputOption));
    assert!(spans.iter().any(|span| span.role == SemanticStyleRole::InputPath));
    assert!(spans.iter().any(|span| span.role == SemanticStyleRole::InputVariable));
    assert!(spans.iter().any(|span| span.role == SemanticStyleRole::InputOperator));
}
```

Also add assertions that no analyzer returns `overlay_rgba` fields anymore.

**Step 2: Run test to verify it fails**

Run: `cargo test --test terminal_scrollback_spec semantic_input_roles_cover_command_path_variable_and_operator -- --exact`
Expected: FAIL because the analyzer still returns color overlays and lacks the richer role set.

**Step 3: Write minimal implementation**

```rust
pub enum SemanticStyleRole {
    InputPrompt,
    InputCommand,
    InputSubcommand,
    InputOption,
    InputArgument,
    InputString,
    InputPath,
    InputVariable,
    InputInvalidCommand,
    InputOperator,
    OutputUrl,
    // ... remaining output roles
}

pub struct SemanticSpan {
    pub row: u32,
    pub start_col: u32,
    pub end_col: u32,
    pub role: SemanticStyleRole,
    pub priority: SemanticPriority,
}
```

Thread these spans into `TerminalPresenter` so later tasks can resolve them through theme styles.

**Step 4: Run test to verify it passes**

Run: `cargo test --test terminal_scrollback_spec -- --nocapture`
Expected: PASS.

**Step 5: Commit**

```bash
git add src/app/terminal_semantic/types.rs src/app/terminal_semantic/mod.rs src/app/terminal_semantic/input_line.rs src/app/terminal_semantic/output_blocks.rs src/app/terminal_model.rs src/app/terminal_presenter.rs tests/terminal_scrollback_spec.rs
git commit -m "refactor: emit semantic roles for terminal highlighting"
```

### Task 5: Add command-block ledgering and overview marker payloads

**Files:**
- Create: `src/app/terminal_semantic/command_blocks.rs`
- Modify: `src/app/terminal_semantic/mod.rs`
- Modify: `src/app/terminal_presenter.rs`
- Modify: `ui/shell/terminal-session-host.slint`
- Modify: `src/app/bootstrap.rs`
- Create: `tests/terminal_command_decorations_spec.rs`
- Modify: `tests/terminal_renderer_dwrite_spec.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn command_ledger_emits_running_failure_and_success_blocks() {
    let ledger = command_blocks_from_lines(&[
        "$ cargo test",
        "running...",
        "$ false",
        "command exited with 1",
    ]);

    assert!(ledger.blocks.iter().any(|b| b.status == CommandBlockStatus::Running));
    assert!(ledger.blocks.iter().any(|b| b.status == CommandBlockStatus::Failure));
}

#[test]
fn presenter_threads_overview_markers_with_command_failures() {
    let frame = presentable_frame_for_failure_case();
    assert!(frame.overview_markers.iter().any(|m| m.kind == OverviewMarkerKind::CommandFailure));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test terminal_command_decorations_spec command_ledger_emits_running_failure_and_success_blocks -- --exact`
Expected: FAIL because command blocks and overview markers do not exist yet.

**Step 3: Write minimal implementation**

```rust
pub struct CommandBlock {
    pub id: u64,
    pub command_text: String,
    pub prompt_row: u32,
    pub command_start_row: u32,
    pub command_end_row: u32,
    pub output_start_row: u32,
    pub output_end_row: u32,
    pub status: CommandBlockStatus,
    pub exit_code: Option<i32>,
    pub cwd: Option<String>,
}

pub enum OverviewMarkerKind {
    CommandRunning,
    CommandFailure,
    CommandSuccess,
    SearchMatch,
    Error,
    Warning,
}
```

Project marker and gutter-ready payloads out of the presenter. In `terminal-session-host.slint`, add the property/callback contract needed to paint a narrow decoration strip and future overview ruler markers without hard-coding colors.

**Step 4: Run test to verify it passes**

Run: `cargo test --test terminal_command_decorations_spec -- --nocapture && cargo test --test terminal_renderer_dwrite_spec terminal_presenter_threads_native_overlay_render_contracts -- --exact`
Expected: PASS.

**Step 5: Commit**

```bash
git add src/app/terminal_semantic/command_blocks.rs src/app/terminal_semantic/mod.rs src/app/terminal_presenter.rs ui/shell/terminal-session-host.slint src/app/bootstrap.rs tests/terminal_command_decorations_spec.rs tests/terminal_renderer_dwrite_spec.rs
git commit -m "feat: add terminal command block decorations"
```

### Task 6: Implement the default output rule engine with incremental matching

**Files:**
- Create: `src/app/terminal_semantic/rules.rs`
- Modify: `src/app/terminal_semantic/mod.rs`
- Modify: `src/app/terminal_semantic/output_blocks.rs`
- Modify: `src/app/terminal_presenter.rs`
- Create: `tests/terminal_output_rules_spec.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn output_rules_match_urls_paths_errors_git_diff_and_json_roles() {
    let annotations = analyze_output_rules(&[
        "see https://example.com/docs",
        "src/main.rs:42:7: error: boom",
        "+ added line",
        "@@ -1,2 +1,2 @@",
        "{ \"name\": \"mica-term\", \"ok\": true }",
    ]);

    assert!(has_role(&annotations, SemanticStyleRole::OutputUrl));
    assert!(has_role(&annotations, SemanticStyleRole::OutputFilePath));
    assert!(has_role(&annotations, SemanticStyleRole::OutputLevelError));
    assert!(has_role(&annotations, SemanticStyleRole::OutputGitAdded));
    assert!(has_role(&annotations, SemanticStyleRole::OutputGitHunk));
    assert!(has_role(&annotations, SemanticStyleRole::OutputJsonKey));
}
```

Add a second test that changes only one line and asserts the cached analyzer only recomputes the dirty row plus bounded lookbehind.

**Step 2: Run test to verify it fails**

Run: `cargo test --test terminal_output_rules_spec output_rules_match_urls_paths_errors_git_diff_and_json_roles -- --exact`
Expected: FAIL because no general output rule engine exists yet.

**Step 3: Write minimal implementation**

```rust
pub struct OutputRuleConfig {
    pub enabled: bool,
    pub max_lookbehind_lines: u32,
    pub profile: OutputRuleProfile,
    pub rules: Vec<OutputRule>,
}

pub fn analyze_output_rules(frame: &TerminalModelFrame, dirty_rows: &[u32]) -> Vec<SemanticSpan> {
    // single-line regex rules over dirty rows
    // bounded block detection for json/diff/log structures
    // emit roles only; let theme resolution happen later
}
```

Start with the approved default rules only: URL, paths, line/column references, IP:port, timestamps, error levels, success/failure keywords, grep/rg matches, git diff, JSON, and SSH/SFTP/rsync/kubectl/docker high-value patterns.

**Step 4: Run test to verify it passes**

Run: `cargo test --test terminal_output_rules_spec -- --nocapture`
Expected: PASS.

**Step 5: Commit**

```bash
git add src/app/terminal_semantic/rules.rs src/app/terminal_semantic/mod.rs src/app/terminal_semantic/output_blocks.rs src/app/terminal_presenter.rs tests/terminal_output_rules_spec.rs
git commit -m "feat: add incremental terminal output rule highlighting"
```

### Task 7: Expose the user-facing settings, polish the theme projection, and verify the slice end-to-end

**Files:**
- Modify: `src/app/ui_preferences.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `ui/components/settings-modal.slint`
- Modify: `src/shell/view_model.rs`
- Modify: `tests/ui_preferences.rs`
- Modify: `tests/theme_terminal_redesign_spec.rs`
- Modify: `tests/terminal_output_rules_spec.rs`
- Modify: `tests/theme_terminal_redesign_smoke.sh`

**Step 1: Write the failing test**

```rust
#[test]
fn ui_preferences_round_trip_terminal_redesign_settings() {
    let prefs = UiPreferences {
        theme_variant: ThemeVariant::PremiumDefault,
        terminal_input_highlighting_enabled: true,
        terminal_output_rule_highlighting_enabled: true,
        terminal_output_rule_profile: OutputRuleProfile::Default,
        terminal_command_decorations_enabled: true,
        terminal_overview_markers_enabled: true,
        ..UiPreferences::default()
    };

    let json = serde_json::to_string(&prefs).unwrap();
    let decoded: UiPreferences = serde_json::from_str(&json).unwrap();

    assert!(decoded.terminal_input_highlighting_enabled);
    assert!(decoded.terminal_overview_markers_enabled);
}
```

Add a smoke assertion that the settings modal exposes exactly the approved first-pass controls rather than raw per-color tuning.

**Step 2: Run test to verify it fails**

Run: `cargo test --test ui_preferences ui_preferences_round_trip_terminal_redesign_settings -- --exact`
Expected: FAIL because the new preference fields and settings controls are not wired yet.

**Step 3: Write minimal implementation**

- Add the approved first-pass preferences:
  - `theme_variant`
  - `terminal_input_highlighting_enabled`
  - `terminal_output_rule_highlighting_enabled`
  - `terminal_output_rule_profile`
  - `terminal_command_decorations_enabled`
  - `terminal_overview_markers_enabled`
  - `terminal_search_match_highlight`
- Thread them through bootstrap and view-model projection.
- Expose them in `settings-modal.slint` using product-level toggles and preset selectors only.

**Step 4: Run test to verify it passes**

Run:

```bash
cargo test --test ui_preferences -- --nocapture
cargo test --test theme_terminal_redesign_spec -- --nocapture
cargo test --test terminal_output_rules_spec -- --nocapture
bash tests/theme_terminal_redesign_smoke.sh
```

Expected: PASS.

Then run a broader confidence pass:

```bash
cargo test --test terminal_theme_selection_spec -- --nocapture
cargo test --test terminal_scrollback_spec -- --nocapture
cargo test --test terminal_command_decorations_spec -- --nocapture
cargo test --test terminal_output_rules_spec -- --nocapture
```

Expected: PASS.

**Step 5: Commit**

```bash
git add src/app/ui_preferences.rs src/app/bootstrap.rs ui/components/settings-modal.slint src/shell/view_model.rs tests/ui_preferences.rs tests/theme_terminal_redesign_spec.rs tests/terminal_output_rules_spec.rs tests/theme_terminal_redesign_smoke.sh
git commit -m "feat: wire terminal redesign settings and verification"
```

## Verification Checklist

Before claiming the implementation is complete, verify all of the following with fresh evidence:

- `cargo test --test theme_terminal_redesign_spec -- --nocapture`
- `cargo test --test terminal_theme_selection_spec -- --nocapture`
- `cargo test --test terminal_scrollback_spec -- --nocapture`
- `cargo test --test terminal_command_decorations_spec -- --nocapture`
- `cargo test --test terminal_output_rules_spec -- --nocapture`
- `cargo test --test ui_preferences -- --nocapture`
- `bash tests/theme_terminal_redesign_smoke.sh`

If any of these fail, do not mark the redesign slice complete.

## Execution Handoff

Plan complete and saved to `docs/plans/2026-04-21-terminal-visual-highlight-redesign-implementation-plan.md`.

Two execution options:

**1. Subagent-Driven (this session)** - I dispatch a fresh subagent per task, review between tasks, fast iteration.

**2. Parallel Session (separate)** - Open a new session with `superpowers:executing-plans`, batch execution with checkpoints.

Choose `1` or `2`.

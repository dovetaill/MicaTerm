# Terminal Visual and Highlight Redesign Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Ship a visibly upgraded Premium Default terminal theme with a stronger shell hierarchy, upgraded ANSI palette, clearly visible semantic highlighting, richer command/input emphasis, and persisted theme/highlight settings.

**Architecture:** Keep the existing Rust + Slint shell and terminal pipeline, but centralize theme values in `AppThemeSpec`, project variant-aware shell tokens into Slint, and convert semantic roles into a limited set of visual primitives that are applied incrementally to dirty rows. Preserve ANSI truth, reuse the current presenter/model contracts, and expand the existing semantic analyzer rather than replacing it.

**Tech Stack:** Rust, Slint, existing terminal presenter/model pipeline, existing shell/view-model preference plumbing, targeted Rust tests, shell smoke tests.

---

### Task 1: Rebuild the theme root around Premium Default v2 and Legacy Hacker Green v2

**Files:**
- Modify: `src/theme/spec.rs`
- Modify: `src/theme/mod.rs`
- Test: `tests/theme_terminal_redesign_spec.rs`
- Test: `tests/terminal_theme_selection_spec.rs`

**Step 1: Write the failing test**

Add / update assertions so the new Premium Default v2 values are explicit and visually stronger:

```rust
#[test]
fn premium_default_v2_dark_palette_uses_blue_black_surface_and_soft_fg() {
    let spec = app_theme_spec(ThemeMode::Dark, ThemeVariant::PremiumDefault);

    assert_eq!(spec.shell.app_background, 0x0f_16_1d);
    assert_eq!(spec.terminal.background.base, 0x08_13_1d);
    assert_eq!(spec.terminal.foreground.default, 0xd7_e0_e8);
    assert_eq!(spec.terminal.ansi[4], 0x7f_9e_c4);
}

#[test]
fn premium_default_v2_light_palette_uses_mist_surface_and_charcoal_fg() {
    let spec = app_theme_spec(ThemeMode::Light, ThemeVariant::PremiumDefault);

    assert_eq!(spec.shell.app_background, 0xe8_ed_f1);
    assert_eq!(spec.terminal.background.base, 0xf4_f6_f8);
    assert_eq!(spec.terminal.foreground.default, 0x1f_29_33);
    assert_eq!(spec.terminal.ansi[4], 0x56_7c_a8);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test theme_terminal_redesign_spec -- --nocapture && cargo test --test terminal_theme_selection_spec -- --nocapture`

Expected: FAIL because the current Premium Default theme still uses the older graphite/canvas values.

**Step 3: Write minimal implementation**

- Replace the Premium Default values in `src/theme/spec.rs` with the approved v2 dark/light tokens.
- Refresh the Legacy Hacker Green variant so it reuses the same hierarchy but with green-led values.
- Keep `ThemeVariant`, `AppThemeSpec`, and terminal palette conversion stable; only replace values and any now-missing helper fields.
- Ensure `src/theme/mod.rs` re-exports any new theme structs / helpers added during the refactor.

**Step 4: Run test to verify it passes**

Run: `cargo test --test theme_terminal_redesign_spec -- --nocapture && cargo test --test terminal_theme_selection_spec -- --nocapture`

Expected: PASS.

**Step 5: Commit**

```bash
git add src/theme/spec.rs src/theme/mod.rs tests/theme_terminal_redesign_spec.rs tests/terminal_theme_selection_spec.rs
git commit -m "feat: refresh premium default v2 theme root"
```

### Task 2: Project variant-aware shell tokens into Slint and strengthen the four-layer shell hierarchy

**Files:**
- Modify: `ui/theme/tokens.slint`
- Modify: `ui/components/active-tab.slint`
- Modify: `ui/components/sidebar-nav-button.slint`
- Modify: `ui/components/asset-node-row.slint`
- Modify: `ui/shell/titlebar.slint`
- Modify: `ui/shell/tabbar.slint`
- Modify: `ui/shell/sidebar.slint`
- Modify: `ui/shell/assets-sidebar.slint`
- Modify: `ui/shell/workspace-pane.slint`
- Test: `tests/theme_semantic_token_contract_spec.rs`
- Test: `tests/theme_terminal_redesign_smoke.sh`

**Step 1: Write the failing test**

Extend the token contract so the new ladder and variant projection are required:

```rust
#[test]
fn shell_tokens_define_premium_default_v2_ladder_and_sidebar_indicator() {
    let tokens = std::fs::read_to_string("ui/theme/tokens.slint").unwrap();

    assert!(tokens.contains("out property <string> theme-variant"));
    assert!(tokens.contains("sidebar-item-selected-indicator"));
    assert!(tokens.contains("tab-active-background"));
    assert!(tokens.contains("terminal-surface-background"));
}
```

Update the smoke script to grep for the new token names and their use sites.

**Step 2: Run test to verify it fails**

Run: `cargo test --test theme_semantic_token_contract_spec -- --nocapture && bash tests/theme_terminal_redesign_smoke.sh`

Expected: FAIL because the current tokens do not expose variant-aware shell projection or the new selected indicator family.

**Step 3: Write minimal implementation**

- Add `theme-variant` to `ThemeTokens` and branch Premium Default vs Legacy Hacker Green directly in Slint.
- Replace shell token values with the approved v2 dark/light values.
- Rework `ActiveTab` so the active tab reads as a container, not just a tab with a bottom line.
- Rework sidebar buttons and asset rows so selected state becomes `fill + soft border + thin left indicator + brighter label/icon`.
- Reduce the reliance on hard dividers; prefer hairlines and value separation.

**Step 4: Run test to verify it passes**

Run: `cargo test --test theme_semantic_token_contract_spec -- --nocapture && bash tests/theme_terminal_redesign_smoke.sh`

Expected: PASS.

**Step 5: Commit**

```bash
git add ui/theme/tokens.slint ui/components/active-tab.slint ui/components/sidebar-nav-button.slint ui/components/asset-node-row.slint ui/shell/titlebar.slint ui/shell/tabbar.slint ui/shell/sidebar.slint ui/shell/assets-sidebar.slint ui/shell/workspace-pane.slint tests/theme_semantic_token_contract_spec.rs tests/theme_terminal_redesign_smoke.sh
git commit -m "feat: restyle shell chrome for premium default v2"
```

### Task 3: Refresh the terminal fallback palette, cursor, selection, search, and ANSI mapping

**Files:**
- Modify: `src/app/terminal_theme.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `ui/theme/tokens.slint`
- Modify: `ui/shell/terminal-session-host.slint`
- Test: `tests/terminal_theme_selection_spec.rs`
- Test: `tests/ssh_terminal_interaction_spec.rs`

**Step 1: Write the failing test**

Add or update assertions so terminal token projection and fallback preset values match the v2 palette:

```rust
#[test]
fn slint_terminal_tokens_match_v2_terminal_defaults() {
    let tokens = std::fs::read_to_string("ui/theme/tokens.slint").unwrap();

    assert!(tokens.contains("terminal-default-fg: theme-variant == \"legacy-hacker-green\""));
    assert!(tokens.contains("#d7e0e8"));
    assert!(tokens.contains("#1f2933"));
}
```

Add preset assertions for cursor, selection, and ANSI bright colors.

**Step 2: Run test to verify it fails**

Run: `cargo test --test terminal_theme_selection_spec -- --nocapture`

Expected: FAIL because terminal fallback projection still points at the older values.

**Step 3: Write minimal implementation**

- Update `preset_from_spec()` inputs by feeding it the new v2 theme values.
- Ensure `bootstrap` projects variant-aware cursor, selection, and scrollbar colors into Slint.
- Update `terminal-session-host.slint` to consume the refreshed terminal frame and surface tokens without reintroducing hard border styling.

**Step 4: Run test to verify it passes**

Run: `cargo test --test terminal_theme_selection_spec -- --nocapture && cargo test --test ssh_terminal_interaction_spec -- --nocapture`

Expected: PASS.

**Step 5: Commit**

```bash
git add src/app/terminal_theme.rs src/app/bootstrap.rs ui/theme/tokens.slint ui/shell/terminal-session-host.slint tests/terminal_theme_selection_spec.rs tests/ssh_terminal_interaction_spec.rs
git commit -m "feat: refresh terminal palette and ansi mapping"
```

### Task 4: Introduce semantic visual primitives so roles can become visible terminal styling

**Files:**
- Modify: `src/theme/spec.rs`
- Modify: `src/app/terminal_semantic/types.rs`
- Modify: `src/app/terminal_semantic/mod.rs`
- Modify: `src/app/terminal_presenter.rs`
- Modify: `src/app/terminal_model.rs`
- Test: `tests/terminal_scrollback_spec.rs`

**Step 1: Write the failing test**

Add a presenter-level regression test that proves semantic roles now produce visual primitives instead of dead metadata:

```rust
#[test]
fn semantic_roles_project_visible_highlight_primitives_for_dirty_rows() {
    let frame = semantic_model_frame(&[
        "$ cargo run --release ./fixtures",
        "https://example.com failed",
    ]);

    let prepared = semantic_overlay_snapshot(&frame);
    assert!(prepared.output_fg_overrides > 0 || prepared.output_underlines > 0 || prepared.output_tints > 0);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test terminal_scrollback_spec semantic_roles_project_visible_highlight_primitives_for_dirty_rows -- --exact`

Expected: FAIL because semantic spans are currently stored in the frame but never converted into visible styling data.

**Step 3: Write minimal implementation**

- Add a small visual-style struct family in `src/theme/spec.rs`, for example: `SemanticInkTheme`, `SemanticHighlightPrimitive`, or equivalent.
- Keep `SemanticStyleRole` as the semantic contract.
- In `terminal_presenter.rs`, map those roles to limited visual primitives: foreground override, underline, and optional subtle tint.
- Scope application to dirty rows plus retained cached spans.
- Thread only the minimum extra data needed through the frame model and presenter contracts.

**Step 4: Run test to verify it passes**

Run: `cargo test --test terminal_scrollback_spec semantic_roles_project_visible_highlight_primitives_for_dirty_rows -- --exact`

Expected: PASS.

**Step 5: Commit**

```bash
git add src/theme/spec.rs src/app/terminal_semantic/types.rs src/app/terminal_semantic/mod.rs src/app/terminal_presenter.rs src/app/terminal_model.rs tests/terminal_scrollback_spec.rs
git commit -m "feat: map terminal semantic roles to visual primitives"
```

### Task 5: Expand output rule highlighting coverage without breaking incremental analysis

**Files:**
- Modify: `src/app/terminal_semantic/rules.rs`
- Modify: `src/app/terminal_semantic/output_blocks.rs`
- Modify: `src/app/terminal_presenter.rs`
- Test: `tests/terminal_output_rules_spec.rs`
- Test: `tests/terminal_scrollback_spec.rs`

**Step 1: Write the failing test**

Add cases for the new visible high-value rules:

```rust
#[test]
fn output_rules_cover_paths_urls_ip_ports_levels_and_json_structure() {
    let frame = output_rule_frame(
        uuid::Uuid::new_v4(),
        1,
        &[
            "ssh://host.example.com ~/.ssh/config 141.98.197.55:38010",
            "WARN timeout connected disconnected",
            "-rw-r--r-- 1 root root 42 Apr 21 10:30 archive.tar.gz",
            "{ \"ok\": true, \"count\": 3 }",
        ],
        None,
    );

    let analysis = analyze_output_rules(&frame, &frame.dirty_rows);
    assert!(has_role(&analysis.spans, SemanticStyleRole::OutputUrl));
    assert!(has_role(&analysis.spans, SemanticStyleRole::OutputFilePath));
    assert!(has_role(&analysis.spans, SemanticStyleRole::OutputIpPort));
    assert!(has_role(&analysis.spans, SemanticStyleRole::OutputLevelWarn));
    assert!(has_role(&analysis.spans, SemanticStyleRole::OutputJsonKey));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test terminal_output_rules_spec -- --nocapture`

Expected: FAIL because the current rule analyzer does not cover all required patterns or their distinct roles.

**Step 3: Write minimal implementation**

- Extend `rules.rs` to recognize:
  - `ssh://`, `sftp://`, `relay+tls://`
  - explicit path references and path-like shell fragments
  - IPv4 + port
  - timestamps / permissions / obvious `ls -l` structures
  - `grep` / `rg` hit rows and obvious JSON values
- Keep the analysis incremental by continuing to respect `dirty_rows` + bounded lookbehind.
- If a rule cannot safely recolor because ANSI is already strong, map it to underline or tint instead.

**Step 4: Run test to verify it passes**

Run: `cargo test --test terminal_output_rules_spec -- --nocapture && cargo test --test terminal_scrollback_spec -- --nocapture`

Expected: PASS.

**Step 5: Commit**

```bash
git add src/app/terminal_semantic/rules.rs src/app/terminal_semantic/output_blocks.rs src/app/terminal_presenter.rs tests/terminal_output_rules_spec.rs tests/terminal_scrollback_spec.rs
git commit -m "feat: expand visible terminal output rule highlighting"
```

### Task 6: Upgrade input-line highlighting so command composition is visibly easier to scan

**Files:**
- Modify: `src/app/terminal_semantic/input_line.rs`
- Modify: `src/app/terminal_semantic/types.rs`
- Modify: `src/app/terminal_presenter.rs`
- Test: `tests/terminal_scrollback_spec.rs`

**Step 1: Write the failing test**

Add a focused input-line tokenizer regression:

```rust
#[test]
fn input_highlighting_covers_command_option_argument_path_variable_string_and_redirects() {
    let frame = semantic_model_frame(&[
        "$ cargo run --bin mica --profile dev ./fixtures > out.log 2>&1 && echo \"done\" $HOME &",
    ]);

    let spans = detect_input_line_spans(&frame);
    assert!(spans.iter().any(|span| span.role == SemanticStyleRole::InputCommand));
    assert!(spans.iter().any(|span| span.role == SemanticStyleRole::InputOption));
    assert!(spans.iter().any(|span| span.role == SemanticStyleRole::InputArgument));
    assert!(spans.iter().any(|span| span.role == SemanticStyleRole::InputPath));
    assert!(spans.iter().any(|span| span.role == SemanticStyleRole::InputVariable));
    assert!(spans.iter().any(|span| span.role == SemanticStyleRole::InputString));
    assert!(spans.iter().any(|span| span.role == SemanticStyleRole::InputOperator));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test terminal_scrollback_spec input_highlighting_covers_command_option_argument_path_variable_string_and_redirects -- --exact`

Expected: FAIL because the current tokenizer misses redirection / background operators and does not separate argument classes strongly enough.

**Step 3: Write minimal implementation**

- Extend the shell token logic to recognize `>`, `>>`, `<`, `2>`, `2>>`, `2>&1`, `&`, `|`, `||`, `&&`, and `;` as operators.
- Keep `invalid command` dormant unless trusted shell integration data is available.
- Ensure the presenter applies the new input roles through the same semantic visual primitive mapping introduced in Task 4.

**Step 4: Run test to verify it passes**

Run: `cargo test --test terminal_scrollback_spec input_highlighting_covers_command_option_argument_path_variable_string_and_redirects -- --exact && cargo test --test terminal_scrollback_spec -- --nocapture`

Expected: PASS.

**Step 5: Commit**

```bash
git add src/app/terminal_semantic/input_line.rs src/app/terminal_semantic/types.rs src/app/terminal_presenter.rs tests/terminal_scrollback_spec.rs
git commit -m "feat: improve terminal input command highlighting"
```

### Task 7: Align command block decorations, overview markers, and terminal shell chrome with the new theme

**Files:**
- Modify: `src/theme/spec.rs`
- Modify: `src/app/terminal_semantic/command_blocks.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `ui/shell/terminal-session-host.slint`
- Test: `tests/terminal_command_decorations_spec.rs`
- Test: `tests/theme_terminal_redesign_smoke.sh`

**Step 1: Write the failing test**

Add or update assertions so command decorations and overview markers pick up the v2 calmer theme:

```rust
#[test]
fn command_decorations_use_v2_running_success_and_failure_tones() {
    let spec = app_theme_spec(ThemeMode::Dark, ThemeVariant::PremiumDefault);
    assert_eq!(spec.decoration.running, 0x7d_97_b8);
    assert_eq!(spec.decoration.success, 0x7f_b0_8d);
    assert_eq!(spec.decoration.failure, 0xc9_7d_88);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test terminal_command_decorations_spec -- --nocapture`

Expected: FAIL because decorations still use the older palette and host-side visual treatment.

**Step 3: Write minimal implementation**

- Refresh decoration colors in `spec.rs` to match Premium Default v2.
- Ensure command blocks and overview markers remain lightweight and terminal-first.
- Update `terminal-session-host.slint` so these decorations feel integrated with the new surface and hairline system rather than bolted on.

**Step 4: Run test to verify it passes**

Run: `cargo test --test terminal_command_decorations_spec -- --nocapture && bash tests/theme_terminal_redesign_smoke.sh`

Expected: PASS.

**Step 5: Commit**

```bash
git add src/theme/spec.rs src/app/terminal_semantic/command_blocks.rs src/app/bootstrap.rs ui/shell/terminal-session-host.slint tests/terminal_command_decorations_spec.rs tests/theme_terminal_redesign_smoke.sh
git commit -m "feat: align terminal decorations with premium default v2"
```

### Task 8: Finish user settings wiring and run the full redesign verification suite

**Files:**
- Modify: `src/app/ui_preferences.rs`
- Modify: `src/shell/view_model.rs`
- Modify: `src/shell/view_model/projection.rs`
- Modify: `src/app/bootstrap/shell_chrome.rs`
- Modify: `ui/components/settings-modal.slint`
- Test: `tests/ui_preferences.rs`
- Test: `tests/terminal_theme_selection_spec.rs`
- Test: `tests/terminal_output_rules_spec.rs`
- Test: `tests/terminal_command_decorations_spec.rs`
- Test: `tests/terminal_scrollback_spec.rs`
- Test: `tests/theme_terminal_redesign_spec.rs`
- Test: `tests/theme_semantic_token_contract_spec.rs`
- Test: `tests/theme_terminal_redesign_smoke.sh`

**Step 1: Write the failing test**

Add a settings persistence regression for the final surface area:

```rust
#[test]
fn ui_preferences_persist_theme_variant_and_highlight_toggles() {
    let prefs = UiPreferences {
        theme_variant: ThemeVariant::LegacyHackerGreen,
        terminal_input_highlighting_enabled: false,
        terminal_output_rule_highlighting_enabled: true,
        ..UiPreferences::default()
    };

    let json = serde_json::to_string(&prefs).unwrap();
    let decoded: UiPreferences = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.theme_variant, ThemeVariant::LegacyHackerGreen);
    assert!(!decoded.terminal_input_highlighting_enabled);
    assert!(decoded.terminal_output_rule_highlighting_enabled);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test ui_preferences -- --nocapture`

Expected: FAIL if any final wiring drift remains between persisted preferences, view-model projection, or settings modal callbacks.

**Step 3: Write minimal implementation**

- Make sure the settings modal exposes the final copy and toggle behavior for:
  - theme variant selection
  - output rule highlighting
  - shell input highlighting
- Keep the existing persistence path and only repair projection or callback gaps.

**Step 4: Run test to verify it passes**

Run the full verification suite:

```bash
cargo test --test theme_terminal_redesign_spec -- --nocapture
cargo test --test terminal_theme_selection_spec -- --nocapture
cargo test --test terminal_scrollback_spec -- --nocapture
cargo test --test terminal_command_decorations_spec -- --nocapture
cargo test --test terminal_output_rules_spec -- --nocapture
cargo test --test ui_preferences -- --nocapture
cargo test --test theme_semantic_token_contract_spec -- --nocapture
bash tests/theme_terminal_redesign_smoke.sh
```

Expected: PASS.

**Step 5: Commit**

```bash
git add src/app/ui_preferences.rs src/shell/view_model.rs src/shell/view_model/projection.rs src/app/bootstrap/shell_chrome.rs ui/components/settings-modal.slint tests/ui_preferences.rs tests/theme_terminal_redesign_spec.rs tests/terminal_theme_selection_spec.rs tests/terminal_scrollback_spec.rs tests/terminal_command_decorations_spec.rs tests/terminal_output_rules_spec.rs tests/theme_semantic_token_contract_spec.rs tests/theme_terminal_redesign_smoke.sh
git commit -m "feat: finish premium default v2 terminal redesign"
```

## Final Verification Checklist

Do not claim completion until all of the following have been run fresh and inspected:

```bash
cargo test --test theme_terminal_redesign_spec -- --nocapture
cargo test --test terminal_theme_selection_spec -- --nocapture
cargo test --test terminal_scrollback_spec -- --nocapture
cargo test --test terminal_command_decorations_spec -- --nocapture
cargo test --test terminal_output_rules_spec -- --nocapture
cargo test --test ui_preferences -- --nocapture
cargo test --test theme_semantic_token_contract_spec -- --nocapture
bash tests/theme_terminal_redesign_smoke.sh
```

If any command fails, stop, fix the underlying issue with a new failing test first, and rerun the relevant command plus the full verification set before calling the redesign complete.

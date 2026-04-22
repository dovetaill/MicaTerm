# Terminal Visual and Highlight Redesign Design

**Date:** 2026-04-21
**Status:** Approved for planning
**Scope:** Rust + Slint desktop terminal shell chrome, terminal palette, semantic highlighting, command decorations

## Goal

Upgrade the terminal experience from "already usable" to a calmer, more product-grade default that feels closer to mature tools like VS Code, Windows Terminal, Termius, and iTerm2 without copying any single product's look. The redesign must keep the terminal as the visual and functional focal point, improve long-session readability, unify dark and light modes, and introduce maintainable semantic highlighting and command-state decorations.

## Product Constraints

- Keep the current font family. Do not replace the terminal font stack.
- Only make small terminal metric adjustments; preserve the current user-tuned feel.
- Avoid cyberpunk styling, neon accents, heavy shadows, fake glass, or game-like chrome.
- Favor low-saturation blue-black / graphite dark surfaces and misty cool off-white light surfaces.
- Keep the implementation incremental and maintainable; avoid WebView or heavy new dependencies.
- Do not compromise terminal performance, especially under large scrollback and high-output sessions.

## Current State Audit

### What already works

- The terminal already owns most of the window's attention.
- The shell chrome and terminal projection already have semantic structure via `ui/theme/tokens.slint`, `src/theme/spec.rs`, and `src/app/terminal_theme.rs`.
- The app already has a `terminal_semantic` module with early input and output detectors.
- The renderer pipeline already carries retained overlays and dirty-row information, which is a good base for incremental semantic work.

### Problems observed in the current UI

#### Light theme

- `titlebar`, `tabbar`, `sidebar`, and terminal shell frame sit too close together in value, so the app reads like one large white-gray surface.
- The active tab is recognizable but still reads like a selected control rather than the current working context.
- The asset tree selected row still feels border-driven instead of content-driven.

#### Dark theme

- The terminal background is close to pure black, which makes the shell chrome and content feel slightly disconnected.
- The shell surfaces sit too close together, so the tab strip and side panel do not step back cleanly behind the terminal.
- ANSI accents work functionally but still lean closer to theme-pack colors than to a premium product palette.

#### Structural problems in code

- Shell chrome tokens live in `ui/theme/tokens.slint`, while terminal palette values live separately in `src/theme/spec.rs` and `src/app/terminal_theme.rs`.
- Semantic detectors in `src/app/terminal_semantic/input_line.rs` and `src/app/terminal_semantic/output_blocks.rs` currently hard-code overlay colors instead of outputting semantic roles.
- Command decorations and overview markers do not yet exist as first-class themeable structures.
- Terminal-specific text attributes are not fully modeled beyond the current bold/underline/fg/bg set.

## Design Direction

### Recommended approach: Premium Default / Product-grade Calm

Keep the existing layout and typography direction, but rebuild the shell ladder, terminal palette, and semantic decoration stack so the product feels calmer, more deliberate, and more maintainable.

### Rejected alternatives

- **Fluent-heavy shell:** too much risk of making the desktop chrome louder than the terminal.
- **Terminal-first minimal shell:** too much risk of weakening the integrated assets / SSH / SFTP product identity.

## Visual Hierarchy

The app should read as four layers in both dark and light modes:

1. `titlebar`: lightest, least assertive, carries window controls and global actions.
2. `tab strip`: one step stronger than the titlebar, but still clearly navigation chrome.
3. `sidebar`: slightly deeper than the tab strip, giving the tool area its own plane.
4. `terminal surface`: the visual anchor of the window, calm and readable, always more important than surrounding chrome.

### Desired behavior by mode

#### Dark

- Use low-saturation blue-black / graphite surfaces.
- Let the terminal be the deepest surface, but stop short of pure black.
- Keep body text off-white and cool, not bright white.
- Let non-active chrome fall back by half a step.

#### Light

- Use cool mist-gray shell surfaces with small but clear value separation.
- Let the terminal read as a cleaner working surface rather than another part of the shell.
- Use charcoal body text, not pure black.
- Avoid the washed-out "single white sheet" look.

## Premium Default Tokens

### Core shell tokens

#### Dark

- `APP_BG_DARK = #141B23`
- `TITLEBAR_BG_DARK = #181F27`
- `TABBAR_BG_DARK = #1A222C`
- `SIDEBAR_BG_DARK = #18212B`
- `SIDEBAR_PANEL_BG_DARK = #1B2430`
- `TERMINAL_BG_DARK = #0C141C`
- `TERMINAL_BG_TOP_DARK = #101924`
- `TERMINAL_BG_BOTTOM_DARK = #0C141C`
- `TEXT_PRIMARY_DARK = #E6ECF3`
- `TEXT_SECONDARY_DARK = #B8C2CF`
- `TEXT_MUTED_DARK = #8B98A9`
- `TEXT_INACTIVE_DARK = #738095`
- `ACCENT_DARK = #6F8FB7`
- `ACCENT_SOFT_DARK = #5E7EA8`
- `FOCUS_RING_DARK = #7C9BC3`
- `TAB_ACTIVE_BG_DARK = #223040`
- `TAB_INACTIVE_BG_DARK = #1A222C`
- `TAB_HOVER_BG_DARK = #202B38`
- `TAB_ACTIVE_LINE_DARK = #7A97BC`
- `SIDEBAR_ITEM_HOVER_DARK = #22303D`
- `SIDEBAR_ITEM_SELECTED_DARK = #2A3949`
- `SIDEBAR_ITEM_BORDER_DARK = #6C88AE66`
- `SEPARATOR_DARK = #FFFFFF14`
- `BORDER_DARK = #FFFFFF1E`
- `HAIRLINE_STRONG_DARK = #FFFFFF24`

#### Light

- `APP_BG_LIGHT = #F2F5F8`
- `TITLEBAR_BG_LIGHT = #F7F9FC`
- `TABBAR_BG_LIGHT = #EEF2F7`
- `SIDEBAR_BG_LIGHT = #EBF0F5`
- `SIDEBAR_PANEL_BG_LIGHT = #F1F5F9`
- `TERMINAL_BG_LIGHT = #F8FAFC`
- `TERMINAL_BG_TOP_LIGHT = #FBFCFD`
- `TERMINAL_BG_BOTTOM_LIGHT = #F6F8FB`
- `TEXT_PRIMARY_LIGHT = #24303D`
- `TEXT_SECONDARY_LIGHT = #49586A`
- `TEXT_MUTED_LIGHT = #677789`
- `TEXT_INACTIVE_LIGHT = #7B8A9C`
- `ACCENT_LIGHT = #587DAA`
- `ACCENT_SOFT_LIGHT = #6E90B8`
- `FOCUS_RING_LIGHT = #7E9EC5`
- `TAB_ACTIVE_BG_LIGHT = #FFFFFF`
- `TAB_INACTIVE_BG_LIGHT = #EEF2F7`
- `TAB_HOVER_BG_LIGHT = #E8EDF4`
- `TAB_ACTIVE_LINE_LIGHT = #6388B4`
- `SIDEBAR_ITEM_HOVER_LIGHT = #E4EBF3`
- `SIDEBAR_ITEM_SELECTED_LIGHT = #DCE6F2`
- `SIDEBAR_ITEM_BORDER_LIGHT = #6E8CB333`
- `SEPARATOR_LIGHT = #1C27330F`
- `BORDER_LIGHT = #1C273319`
- `HAIRLINE_STRONG_LIGHT = #1C273324`

### Terminal reading tokens

#### Dark

- `TERMINAL_FG_DARK = #E3EAF2`
- `TERMINAL_FG_DIM_DARK = #93A0B2`
- `TERMINAL_FG_SOFT_DARK = #C8D1DC`
- `TERMINAL_CURSOR_BG_DARK = #DCE6F3`
- `TERMINAL_CURSOR_FG_DARK = #0C141C`
- `TERMINAL_SELECTION_DARK = #6C88AE42`
- `TERMINAL_SEARCH_MATCH_DARK = #7B68402E`
- `TERMINAL_SEARCH_CURRENT_DARK = #8E79B845`
- `SCROLLBAR_TRACK_DARK = #FFFFFF0C`
- `SCROLLBAR_THUMB_DARK = #536274`
- `SCROLLBAR_THUMB_ACTIVE_DARK = #66788E`

#### Light

- `TERMINAL_FG_LIGHT = #263240`
- `TERMINAL_FG_DIM_LIGHT = #758395`
- `TERMINAL_FG_SOFT_LIGHT = #4B596A`
- `TERMINAL_CURSOR_BG_LIGHT = #2C3948`
- `TERMINAL_CURSOR_FG_LIGHT = #F8FAFC`
- `TERMINAL_SELECTION_LIGHT = #7F9BC233`
- `TERMINAL_SEARCH_MATCH_LIGHT = #D8C79A52`
- `TERMINAL_SEARCH_CURRENT_LIGHT = #A98DDA52`
- `SCROLLBAR_TRACK_LIGHT = #1C273308`
- `SCROLLBAR_THUMB_LIGHT = #B7C3D0`
- `SCROLLBAR_THUMB_ACTIVE_LIGHT = #9FAFBE`

### Status tokens

#### Dark

- `STATUS_SUCCESS_DARK = #7FB08D`
- `STATUS_WARNING_DARK = #C9A86A`
- `STATUS_ERROR_DARK = #C98A94`
- `STATUS_INFO_DARK = #7D9BC2`
- `STATUS_RUNNING_DARK = #7B96B8`

#### Light

- `STATUS_SUCCESS_LIGHT = #5E8A68`
- `STATUS_WARNING_LIGHT = #9B7A3C`
- `STATUS_ERROR_LIGHT = #A55C67`
- `STATUS_INFO_LIGHT = #5E81AE`
- `STATUS_RUNNING_LIGHT = #6B85A9`

### ANSI 16-color palettes

#### Dark

- black `#4A5260`
- red `#C37A86`
- green `#86B48F`
- yellow `#C6A56A`
- blue `#7D9BC2`
- magenta `#A78CBF`
- cyan `#78AFAE`
- white `#C8D1DC`
- bright black `#667180`
- bright red `#D6949F`
- bright green `#9BC6A4`
- bright yellow `#D8BA83`
- bright blue `#94AED0`
- bright magenta `#B79CCB`
- bright cyan `#8EC0C0`
- bright white `#E7EDF4`

#### Light

- black `#5A6573`
- red `#B86470`
- green `#5F8D69`
- yellow `#9D7C41`
- blue `#5B80AE`
- magenta `#8F73AA`
- cyan `#4E9090`
- white `#AAB5C1`
- bright black `#738090`
- bright red `#C77A85`
- bright green `#769E7E`
- bright yellow `#B08D53`
- bright blue `#7295BF`
- bright magenta `#A286BB`
- bright cyan `#68A4A3`
- bright white `#D5DCE4`

## Typography and Reading

- Keep the current font family.
- Preserve the current default font size unless a very small alignment tweak is needed during verification.
- Restrict line-height changes to subtle preset-level refinement rather than open-ended retuning.
- Keep dim text readable; do not let it collapse into muddy gray.
- If supported by the native renderer, preserve stable weight and edge clarity instead of chasing a softer but blurrier look.
- Allow an optional tiny inactive-terminal contrast drop, but keep it under approximately five percent.

## Theme Architecture

Introduce a single Rust-side theme root and let both Slint shell chrome and terminal renderer projection flow from it.

```text
AppThemeSpec
|- id / name / variant / mode
|- shell: ShellChromeTheme
|- terminal: TerminalTheme
|- decoration: DecorationTheme
`- semantic: SemanticHighlightTheme
```

### Shell theme responsibilities

- App chrome surfaces
- Tab strip states
- Sidebar row states
- Shell text hierarchy
- Hairlines and focus rings
- Status semantic colors

### Terminal theme responsibilities

- Default background/foreground
- Cursor colors and emphasis
- Selection overlay
- Search match colors
- Scrollbar chrome
- ANSI 16-color palette

### Decoration theme responsibilities

- Command gutter markers
- Overview ruler markers
- Running block emphasis
- Status-pill color families

### Semantic highlight theme responsibilities

- Input command roles
- Output rule roles
- Light-weight style payloads (foreground, underline, bold, optional background)

## Highlighter Architecture

Adopt four explicit layers:

1. **ANSI Native Layer** - render ANSI truth first.
2. **Input Command Highlight Layer** - analyze only the editable command line near the bottom of the terminal.
3. **Command Block / Status Decoration Layer** - derive command lifecycle blocks and project gutter/overview status.
4. **Output Rule Highlight Layer** - apply configurable high-value regex/structured rules incrementally.

### Key rules

- Do not rewrite ANSI output.
- Do not run regex at renderer draw time.
- Use dirty-row and append-window analysis by default.
- Limit multi-line re-analysis to a bounded lookbehind window.

## Semantic Roles and Decorations

### Input roles

- Prompt
- Command
- Subcommand
- Option
- Argument
- String
- Path
- Variable
- InvalidCommand
- Operator

### Output roles

- Url
- FilePath
- LineColumn
- IpPort
- Timestamp
- LevelError / Warn / Info / Debug
- SuccessKeyword / FailureKeyword
- GrepMatch
- GitAdded / GitRemoved / GitHunk
- JsonKey / JsonString / JsonNumber / JsonBoolean

### Command block status

- Running
- Success
- Failure
- Unknown

## Default Rule Set

Enable these by default:

- URL detection
- Unix/Windows/home-relative paths
- Path + line/column references
- IPv4 + port
- ISO/syslog-like timestamps
- ERROR / WARN / INFO / DEBUG tokens
- Success/failure keywords
- `grep` / `rg` match lines
- Git diff added/removed/hunk markers
- JSON block roles when output clearly forms a JSON block
- SSH / SFTP / rsync / kubectl / docker high-value patterns, starting with errors, host/path references, and transfer summaries

Keep XML and richer structured modes conservative in the first pass.

## Code Landing Plan

### Files to refactor or extend

- `src/theme/spec.rs`
- `src/theme/mod.rs`
- `src/app/terminal_theme.rs`
- `src/app/bootstrap.rs`
- `ui/theme/tokens.slint`
- `ui/components/active-tab.slint`
- `ui/components/sidebar-nav-button.slint`
- `ui/components/asset-node-row.slint`
- `ui/shell/titlebar.slint`
- `ui/shell/tabbar.slint`
- `ui/shell/sidebar.slint`
- `ui/shell/assets-sidebar.slint`
- `ui/shell/terminal-session-host.slint`
- `src/app/terminal_model.rs`
- `src/app/terminal_semantic/mod.rs`
- `src/app/terminal_semantic/input_line.rs`
- `src/app/terminal_semantic/output_blocks.rs`

### Suggested new files

- `src/app/terminal_semantic/types.rs`
- `src/app/terminal_semantic/rules.rs`
- `src/app/terminal_semantic/command_blocks.rs`

## User-facing Settings

First-pass exposed settings should stay limited to:

- Theme mode
- Theme variant
- Terminal font size
- Cursor shape
- Input command highlighting enabled
- Output rule highlighting enabled
- Output rule profile
- Command decorations enabled
- Overview markers enabled
- Search match highlight strength

Avoid first-pass color pickers, arbitrary regex editors, or per-role manual theming.

## Acceptance Criteria

### Visual

- Dark and light each show clear four-layer hierarchy.
- The terminal remains the visual center.
- Light mode no longer reads as one white-gray plane.
- Dark mode no longer reads as pure black behind bright white text.
- Active tabs and selected asset rows are clear without loud borders.

### Reading

- Default text is calm and readable in long sessions.
- Dim/secondary/inactive text form a usable hierarchy.
- Small metric adjustments do not disrupt the existing user-tuned feel.

### Highlighting and state

- ANSI output stays correct.
- Input command highlighting improves readability without becoming editor-like.
- Running / success / failure decorations are visible at a glance.
- Overview markers help with long outputs.
- Rule highlighting is useful but restrained.

### Performance and stability

- Large scrollback remains smooth.
- Alternate-screen and mouse-grabbed TUIs are not polluted by shell-line overlays.
- Theme switches update shell chrome, terminal chrome, and semantic styles together.

### Maintainability

- Theme tokens are no longer scattered through ad hoc constants.
- Semantic detectors output roles, not hard-coded colors.
- Theme variants can reuse the same semantic pipeline.

## External Method References

The redesign borrows method, not direct styling, from mature products and documentation:

- VS Code shell integration command decorations and overview ruler ideas: <https://code.visualstudio.com/docs/terminal/shell-integration>
- Windows Terminal paired dark/light profile and scheme design: <https://learn.microsoft.com/en-us/windows/terminal/customize-settings/profile-appearance>
- Windows Terminal color scheme structure: <https://learn.microsoft.com/en-us/windows/terminal/customize-settings/color-schemes>
- iTerm2 trigger-based output enhancement model: <https://iterm2.com/documentation-triggers.html>

## Recommended Next Step

Create an implementation plan that keeps the work incremental in this order:

1. Theme root structures and preferences
2. Shell chrome token replacement
3. Terminal palette and ANSI projection
4. Semantic type refactor
5. Input highlighter enhancement
6. Command block decorations and overview markers
7. Output rule highlighting and settings polish

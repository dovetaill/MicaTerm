# Terminal Visual and Highlight Redesign Design

**Date:** 2026-04-21
**Status:** Approved for implementation
**Scope:** Rust + Slint desktop shell chrome, terminal palette, ANSI mapping, semantic highlighting, user preferences

## Goal

Deliver a visually obvious terminal redesign that feels like a premium desktop product instead of a lightly polished default terminal. The terminal must become the primary visual surface in both dark and light mode, the shell chrome must establish a clear four-layer hierarchy, and semantic highlighting must become visibly useful without turning the output into a noisy rainbow.

## Product Constraints

- Keep the current font family. Do not replace the terminal font stack.
- Restrict terminal metric changes to small padding / radius / hairline adjustments.
- Preserve native ANSI output as the ground truth; semantic highlighting must layer on top of it.
- Keep semantic detection and semantic styling decoupled.
- Do not run regex analysis in the renderer.
- Do not rescan the full scrollback every frame; only process dirty rows plus a bounded lookbehind.
- Do not introduce WebView or heavy dependencies.
- Reuse the existing terminal presenter / model / Slint shell structure.
- Stay restrained: no neon blue, no neon green outside the legacy theme, no cyberpunk purple, no thick shadows, no glossy game-skin look.

## Current State Audit

### Why the previous redesign still looked almost unchanged

1. `src/theme/spec.rs` and `ui/theme/tokens.slint` keep the shell surfaces too close together in value. Dark mode stays in a narrow graphite band and light mode stays in a narrow fog-white band, so the app still reads as one large surface.
2. Terminal foreground is still too close to conventional white / black defaults. It is technically tuned, but not enough to create a visible before/after difference.
3. `ui/components/sidebar-nav-button.slint` and `ui/components/asset-node-row.slint` still express selection mostly through a 1px border and a mild fill, which reads like a highlighted control instead of a mature active destination.
4. `ui/components/active-tab.slint` still relies on a bottom indicator and a small background delta, so active and inactive tabs remain too similar.
5. The Premium Default ANSI palette is improved but still feels like a cautious recolor of a traditional 16-color set rather than a product-level palette with a clear taste.
6. `src/app/terminal_semantic/rules.rs` and `src/app/terminal_semantic/input_line.rs` already emit semantic spans, but those spans are not yet transformed into visible terminal styling, so the rules exist more on paper than on screen.
7. Light theme is still too close to a flat white-gray app shell. Terminal, tab strip, sidebar, and titlebar do not step apart enough.
8. Dark theme is still too close to dark slab + bright text. The terminal does not yet feel like a calm blue-black working surface.

### Strong parts worth preserving

- The terminal already owns the central workspace layout and does not need structural repositioning.
- The app already has a stable theme root split across Rust and Slint, which we can consolidate rather than replace.
- The semantic analyzer already supports incremental dirty-row analysis. The architecture is suitable for visible highlighting without a performance regression.
- Command blocks, overview markers, and settings wiring already exist and should be expanded instead of rewritten.

## Visual Direction

## Recommended direction: Premium Default v2

A calm, product-grade terminal aesthetic with explicit chrome hierarchy and a visibly richer terminal palette.

- **Dark mode:** blue-black terminal surface, cool gray-white body text, restrained blue-gray accenting.
- **Light mode:** mist-gray terminal surface, charcoal body text, soft blue-gray accenting.
- **Chrome behavior:** titlebar, tab strip, sidebar, and terminal surface each occupy their own tonal layer, with the terminal clearly in focus.
- **Highlight behavior:** semantic highlighting improves scanning through selective emphasis, subtle tints, and light underlines, not through broad saturated blocks.

## Rejected alternatives

- **Warp-like heavy block UI:** too likely to overpower ANSI truth and make the shell chrome louder than the terminal.
- **Minimal palette-only tweak:** too likely to repeat the same “changed, but barely noticeable” outcome.
- **Aggressive Fluent glass treatment:** too decorative, too shell-focused, and too risky for long-session readability.

## Visual Hierarchy

The shell must read as four layers in both modes:

1. **Titlebar** — least assertive, utility-only layer.
2. **Tab strip** — navigation layer, clearly distinct from titlebar.
3. **Sidebar** — tool/navigation plane, slightly behind active workspace.
4. **Terminal surface** — focal working plane, visually strongest without using loud effects.

### Dark mode behavior

- Application shell moves to a slightly cooler and deeper blue-black family.
- Terminal surface becomes deeper than chrome but avoids pure black.
- Foreground text softens from near-white to cool gray-white.
- Selected controls gain presence through structured fills and slim indicators, not bright outlines.

### Light mode behavior

- Titlebar, tab strip, sidebar, and terminal surface all separate by clear but restrained value shifts.
- Terminal surface becomes a cleaner working plane rather than more white shell.
- Text moves to charcoal rather than pure black.
- Hover and selected states become more content-driven and less border-driven.

## Premium Default v2 Tokens

### Global tokens

#### Dark

- `APP_BG_DARK = #0F161D`
- `TITLEBAR_BG_DARK = #18212B`
- `TAB_STRIP_BG_DARK = #111923`
- `SIDEBAR_BG_DARK = #15202A`
- `BORDER_DARK = #2D3A48`
- `HAIRLINE_DARK = #FFFFFF14`
- `ACCENT_DARK = #7D97B8`

#### Light

- `APP_BG_LIGHT = #E8EDF1`
- `TITLEBAR_BG_LIGHT = #F7F9FB`
- `TAB_STRIP_BG_LIGHT = #EDF2F6`
- `SIDEBAR_BG_LIGHT = #E3EAF0`
- `BORDER_LIGHT = #C9D3DD`
- `HAIRLINE_LIGHT = #10203012`
- `ACCENT_LIGHT = #6B87AB`

### Sidebar tokens

#### Dark

- `SIDEBAR_ITEM_HOVER_DARK = #1C2A36`
- `SIDEBAR_ITEM_SELECTED_BG_DARK = #223444`
- `SIDEBAR_ITEM_SELECTED_BORDER_DARK = #35506A`
- `SIDEBAR_ITEM_SELECTED_INDICATOR_DARK = #96AFCA`
- `SIDEBAR_TEXT_DARK = #9AABBA`
- `SIDEBAR_TEXT_ACTIVE_DARK = #ECF3F9`

#### Light

- `SIDEBAR_ITEM_HOVER_LIGHT = #DDE6EE`
- `SIDEBAR_ITEM_SELECTED_BG_LIGHT = #D2DFEA`
- `SIDEBAR_ITEM_SELECTED_BORDER_LIGHT = #AEC0D1`
- `SIDEBAR_ITEM_SELECTED_INDICATOR_LIGHT = #6F89AB`
- `SIDEBAR_TEXT_LIGHT = #516173`
- `SIDEBAR_TEXT_ACTIVE_LIGHT = #12202C`

### Tab tokens

#### Dark

- `TAB_ACTIVE_BG_DARK = #1C2937`
- `TAB_INACTIVE_BG_DARK = #121B24`
- `TAB_HOVER_BG_DARK = #182430`
- `TAB_ACTIVE_TEXT_DARK = #EEF4F9`
- `TAB_INACTIVE_TEXT_DARK = #93A4B4`

#### Light

- `TAB_ACTIVE_BG_LIGHT = #FFFFFF`
- `TAB_INACTIVE_BG_LIGHT = #E8EEF4`
- `TAB_HOVER_BG_LIGHT = #E0E8F0`
- `TAB_ACTIVE_TEXT_LIGHT = #15222E`
- `TAB_INACTIVE_TEXT_LIGHT = #5A6A79`

### Terminal tokens

#### Dark

- `TERMINAL_BG_DARK = #08131D`
- `TERMINAL_FG_DARK = #D7E0E8`
- `TERMINAL_FG_DIM_DARK = #8FA0AE`
- `TERMINAL_CURSOR_DARK = #DCE6EE`
- `TERMINAL_SELECTION_BG_DARK = #7A8FA94A`
- `TERMINAL_SEARCH_MATCH_DARK = #A17A2430`

#### Light

- `TERMINAL_BG_LIGHT = #F4F6F8`
- `TERMINAL_FG_LIGHT = #1F2933`
- `TERMINAL_FG_DIM_LIGHT = #6C7A86`
- `TERMINAL_CURSOR_LIGHT = #24313C`
- `TERMINAL_SELECTION_BG_LIGHT = #7895B33A`
- `TERMINAL_SEARCH_MATCH_LIGHT = #C49B3952`

### ANSI palettes

#### Premium Default dark

- `ANSI_BLACK_DARK = #3E4A57`
- `ANSI_RED_DARK = #C97D88`
- `ANSI_GREEN_DARK = #7FB08D`
- `ANSI_YELLOW_DARK = #C6A066`
- `ANSI_BLUE_DARK = #7F9EC4`
- `ANSI_MAGENTA_DARK = #A88DBF`
- `ANSI_CYAN_DARK = #74B1B7`
- `ANSI_WHITE_DARK = #CBD5DF`
- `ANSI_BRIGHT_BLACK_DARK = #5F6D7C`
- `ANSI_BRIGHT_RED_DARK = #D9939D`
- `ANSI_BRIGHT_GREEN_DARK = #97C3A1`
- `ANSI_BRIGHT_YELLOW_DARK = #D8B780`
- `ANSI_BRIGHT_BLUE_DARK = #9AB5D6`
- `ANSI_BRIGHT_MAGENTA_DARK = #BEA2D1`
- `ANSI_BRIGHT_CYAN_DARK = #90C8CC`
- `ANSI_BRIGHT_WHITE_DARK = #ECF2F7`

#### Premium Default light

- `ANSI_BLACK_LIGHT = #4E5C6A`
- `ANSI_RED_LIGHT = #B76470`
- `ANSI_GREEN_LIGHT = #5F8969`
- `ANSI_YELLOW_LIGHT = #9B7A40`
- `ANSI_BLUE_LIGHT = #567CA8`
- `ANSI_MAGENTA_LIGHT = #866EA2`
- `ANSI_CYAN_LIGHT = #4C8D8F`
- `ANSI_WHITE_LIGHT = #A7B4BF`
- `ANSI_BRIGHT_BLACK_LIGHT = #6C7B89`
- `ANSI_BRIGHT_RED_LIGHT = #C87984`
- `ANSI_BRIGHT_GREEN_LIGHT = #769D7D`
- `ANSI_BRIGHT_YELLOW_LIGHT = #AD8B54`
- `ANSI_BRIGHT_BLUE_LIGHT = #7095BF`
- `ANSI_BRIGHT_MAGENTA_LIGHT = #9C83B6`
- `ANSI_BRIGHT_CYAN_LIGHT = #66A4A7`
- `ANSI_BRIGHT_WHITE_LIGHT = #D9E0E6`

## Legacy Hacker Green Variant

Keep a clearly optional green-skewed variant for nostalgic users while preserving the same structural hierarchy and style system.

### Legacy Hacker Green tokens

- Use dark shell values in the `#0B120F` to `#173223` range.
- Use light shell values in the `#E1ECE5` to `#FDFEFD` range.
- Use terminal foreground in dark mode near `#9BE6B3` and light mode near `#213128`.
- Keep ANSI green-led but still restrained: green remains visible, but side chrome and tab chrome still follow the same four-layer hierarchy.

This variant must reuse the same semantic roles, settings flow, and renderer behavior as Premium Default.

## Shell Chrome Adjustments

### Titlebar

- Reduce the sense of a heavy top slab.
- Use a quieter surface with only a weak bottom hairline.
- Keep controls legible, but avoid making the titlebar more eye-catching than the terminal.

### Tab strip

- Treat the strip as a navigation plane distinct from the titlebar.
- Make the active tab a visible container instead of a slightly lighter chip with a thin bottom line.
- Reduce inactive tab contrast so the active tab becomes obvious even at a glance.
- Keep hover readable but calm.

### Sidebar

- Replace the current “outlined selected control” look with a proper active destination treatment.
- Selected rows and icon buttons use: low-saturation fill, soft border, brighter active text/icon, and a slim left indicator.
- Hover remains one level lighter than idle but does not compete with selected.

### Terminal frame and surface

- Move from a generic framed rectangle to a focused workspace plane.
- Keep borders thin and quiet.
- Let the terminal surface visually separate from surrounding chrome by a whole level rather than a subtle tint shift.

## Semantic Highlighting System

## Architecture

1. `terminal_semantic` modules remain responsible for detection only.
2. Semantic analyzers emit `SemanticStyleRole` spans, not colors.
3. The theme layer maps roles to a small set of visual primitives.
4. The presenter merges those primitives into the frame for dirty rows only.
5. The renderer consumes precomputed runs; it does not do regex or parsing.

## Supported first-wave rules

### Output rules

- Paths: `/root/gost`, `./foo/bar`, `~/.ssh/config`
- URLs / schemes / domains: `http://`, `https://`, `ssh://`, `sftp://`, `relay+tls://`
- IP + port: `185.241.40.72:38005`
- Shell composition in visible input / prompt lines: command, option, argument, operator, variable, string, path
- Semantic words: `ERROR`, `WARN`, `INFO`, `DEBUG`, `success`, `failed`, `denied`, `timeout`, `connected`, `disconnected`
- Structural patterns: timestamps, permissions, `ls -l` style file kinds, `grep`/`rg` hits, obvious JSON key/string/number/boolean spans

### Visual treatment rules

- Primary tool: foreground emphasis
- Secondary tool: subtle underline
- Optional supporting tool: extremely light background tint
- No broad, fully opaque color blocks
- If the original ANSI foreground is already meaningful, prefer underline or tint rather than overwriting that foreground

## Input highlighting

- Reuse the current prompt-line tokenizer and expand it to reliably distinguish command, option, argument, path, variable, string, and operator.
- Keep invalid-command styling dormant unless the shell integration can provide trusted validity information. The role may exist, but this redesign should not guess remote command validity.

## User Settings

Expose and persist:

- Theme variant: `Premium Default` / `Legacy Hacker Green`
- `Enable output rule highlighting`
- `Enable shell input highlighting`

These options already have partial plumbing in `UiPreferences` and `ShellViewModel`; the redesign should finish the UX and keep persistence stable.

## Testing Strategy

- Extend theme-token contract tests so the new token families are explicit and stable.
- Extend terminal palette tests to assert the new Premium Default and Legacy Hacker Green values.
- Extend scrollback / semantic tests to cover rule roles and incremental dirty-row behavior.
- Add or update smoke checks for shell chrome token adoption.
- Keep final verification as a real command run, not a claim based on expected behavior.

## Acceptance Criteria

The redesign is accepted only if all of the following are visibly true:

- Before / after screenshots are obviously different.
- Dark mode is no longer dead black with bright white text.
- Light mode is no longer a mostly flat pale sheet.
- Terminal surface is clearly the visual focus.
- Active and inactive tabs are immediately distinguishable.
- Sidebar selected state no longer reads like a boxed outline.
- ANSI colors feel more mature and cohesive.
- Output history shows visible information hierarchy from semantic highlighting.
- The code path remains incremental and maintainable.

## References

These references informed the direction without being copied literally:

- Warp theme design notes for extending terminal themes into full-UI surfaces.
- Ghostty documentation for terminal-first, native, restrained UI priorities.
- Windows Terminal color scheme guidance for explicit background / foreground / selection / cursor / ANSI token completeness.

# Terminal Atlas Typography Theme Refresh Design

## Goal

Keep the Rust atlas renderer as the single terminal rendering path while making the terminal look materially closer to the reference screenshots:

1. glyphs must read sharper and less swollen;
2. horizontal and vertical spacing must feel tighter and more coherent;
3. dark and light themes must look cleaner and more neutral;
4. alternating row bands must improve scanability without breaking ANSI backgrounds.

## Current State

- [`src/app/terminal_atlas.rs`](../../src/app/terminal_atlas.rs) loads a bundled Sarasa Nerd font and rasterizes outlined glyphs into a sprite atlas with `ab_glyph`.
- The current renderer uses `SarasaTermSCNerd-SemiBold-Unhinted.ttf`, elevated glyph alpha gain, non-zero left insets, and roomy vertical metrics.
- [`src/app/terminal_theme.rs`](../../src/app/terminal_theme.rs) already owns the terminal palette projection for dark and light modes.
- [`ui/shell/terminal-session-host.slint`](../../ui/shell/terminal-session-host.slint) defines padding, scrollbar reservation, and the Slint fallback colors around the atlas image.

## Root Cause

The “ugly / blurry / loose” impression is not one single bug.

- `SemiBold` plus aggressive alpha shaping makes strokes look overfilled and slightly muddy.
- The current cell metrics and host padding leave too much slack vertically and around the grid.
- Manual horizontal offsets make ASCII clusters feel visually displaced.
- The terminal palette still reads like an app theme surface rather than a purpose-built terminal canvas.
- There is currently no row banding, so dense command output lacks the subtle rhythm visible in the reference terminal screenshots.

## Constraints

- Do not abandon the single-image atlas renderer.
- Do not switch away from the Sarasa Term SC Nerd family in this iteration.
- Do not rely on TrueType hinting as the primary fix; the current `ab_glyph` outline pipeline is still the rendering core.
- Do not paint zebra bands over cells with explicit ANSI background colors.

## Approved Approach

### Typography

- Switch the bundled atlas face back to the regular Sarasa weight instead of the current semi-bold file.
- Re-tune font size, baseline, cell width/height, and atlas padding as one set.
- Lower the current glyph “ink” amplification so strokes look sharper rather than thicker.
- Reduce or remove custom left inset bias so ASCII, mixed text, and symbols align more naturally inside the fixed grid.

### Theme

- Rebuild dark and light terminal defaults around cleaner neutral backgrounds and stronger foreground contrast.
- Keep ANSI colors vivid enough for `ls`/`ll`, but reduce the current blue-gray cast of the canvas itself.
- Add explicit row band colors to the terminal preset instead of hard-coding them in the renderer.

### Row Banding

- Paint alternating row backgrounds only when the row/cell uses the default terminal background.
- Dark mode should alternate between near-black and a very subtle cool dark gray.
- Light mode should alternate between near-white and a very faint sky-blue tint.
- Explicit per-cell background colors from terminal output must still win over row bands.

### Host Layout

- Tighten content padding and scrollbar reservation in [`ui/shell/terminal-session-host.slint`](../../ui/shell/terminal-session-host.slint) so the canvas feels more like a native terminal viewport.
- Keep selection, cursor, and scrollbar behavior unchanged.

## Files

- Modify [`src/app/terminal_atlas.rs`](../../src/app/terminal_atlas.rs)
- Modify [`src/app/terminal_theme.rs`](../../src/app/terminal_theme.rs)
- Modify [`ui/shell/terminal-session-host.slint`](../../ui/shell/terminal-session-host.slint)
- Modify [`tests/terminal_atlas_renderer_spec.rs`](../../tests/terminal_atlas_renderer_spec.rs)
- Modify [`tests/startup_font_memory_regression.rs`](../../tests/startup_font_memory_regression.rs)
- Modify [`tests/terminal_font_registration_smoke.rs`](../../tests/terminal_font_registration_smoke.rs)
- Modify [`build.rs`](../../build.rs)
- Update terminal-font notes in [`readme.md`](../../readme.md) if the bundled asset contract changes

## Validation

- A renderer test must fail first when the expected metrics or alternating row behavior are not present.
- Theme tests must prove the dark and light preset defaults match the refreshed palette contract.
- Startup font contract tests must fail first if they still point to the old unhinted/semi-bold naming.
- Focused Rust tests and `cargo check` must pass before claiming the refresh is complete.

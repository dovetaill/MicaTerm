# Terminal Color Emoji Rendering Design

## Goal

Make terminal body rendering show true color emoji on Windows and Linux without abandoning the current atlas-based terminal renderer.

This work is scoped strictly to terminal body cells. It does not change title bars, tabs, or any other Slint text widgets.

## Current State

- [`src/app/ssh/runtime.rs`](../../src/app/ssh/runtime.rs) already projects terminal output into per-cell state:
  - each [`TerminalCellState`](../../src/app/ssh/runtime.rs) carries `row`, `col`, `width`, `text`, `fg_rgba`, and `bg_rgba`;
  - the renderer therefore already receives terminal cells at the right granularity for emoji clusters.
- [`src/app/terminal_atlas.rs`](../../src/app/terminal_atlas.rs) loads one bundled Sarasa font and rasterizes cluster outlines through `ab_glyph`.
- The current sprite cache is alpha-only. It assumes every rendered cluster can be represented as a monochrome mask tinted by the terminal foreground color.
- Existing renderer coverage proves CJK and Nerd Font private-use glyphs work, but there is no contract for color emoji rendering.

## Root Cause

The current terminal renderer cannot display color emoji for three separate reasons:

1. it only loads one bundled mono terminal font;
2. it only knows how to rasterize outline glyphs into an alpha mask;
3. when the bundled font has no glyph for a cluster, the renderer silently returns a transparent sprite, which makes missing emoji appear blank.

This is why prompt glyphs such as `` can work while `📦`, `🌐`, or `🦀` disappear: the first is a Nerd Font private-use symbol contained in the bundled terminal font, while the latter are emoji codepoints that require a color emoji font plus fallback resolution.

## Constraints

- Keep the terminal body as one atlas-backed surface image.
- Keep the existing Sarasa atlas path for ASCII, CJK, box drawing, and Nerd Font symbols.
- Do not route terminal body text through generic Slint text widgets.
- Do not introduce platform-specific native rendering backends unless they are unavoidable.
- The result must support true color emoji, not monochrome substitutions.
- Windows and Linux must both be covered by the design.

## Approved Approach

### 1. Split terminal cluster rendering into mono and emoji paths

- Keep the current atlas path as the default path for non-emoji clusters.
- Add an emoji-specific rendering path that activates only when a cell's `text` is an emoji-presenting grapheme cluster.
- Continue trusting `TerminalCellState.width` as the terminal-authoritative cell span. Emoji rendering must fit the sprite into the same fixed cell geometry the terminal runtime already computed.

### 2. Add a dedicated emoji classifier

- Introduce a small terminal-only classifier that determines whether a cell cluster should be treated as emoji content.
- Classification should treat these as emoji-rendered clusters:
  - clusters containing `Variation Selector-16`;
  - clusters using `ZWJ` emoji composition;
  - clusters containing scalars with the Unicode `Emoji_Presentation` or `Extended_Pictographic` properties.
- Because terminal state is already emitted per cell cluster, this classifier operates on `TerminalCellState.text`; it does not need to reshape the whole row.

### 3. Resolve emoji fonts from the host system, not the bundled terminal font

- Add a dedicated system emoji font resolver for terminal rendering.
- Use a cross-platform Rust font database to discover installed fonts on Windows and Linux.
- Prefer known color emoji families in this order:
  - Windows: `Segoe UI Emoji`
  - Linux: `Noto Color Emoji`, `Twitter Color Emoji`, `Emoji One Color`
- If none of the preferred color emoji faces are present, keep a logged diagnostic and fall back to a visible replacement glyph instead of a silent blank sprite.

### 4. Render color emoji with a color-glyph-capable rasterizer

- Add a terminal emoji renderer module that uses a color-glyph-capable Rust stack rather than `ab_glyph`.
- The implementation should use:
  - `fontdb` for system font discovery/loading;
  - `swash` for shaping and color glyph rasterization;
  - `unicode-properties` for emoji-property checks.
- `swash` is chosen because it supports color glyph formats and runs cross-platform in Rust, which avoids writing separate DirectWrite and Linux font backends for this feature.

### 5. Extend sprite caching from alpha-only to dual sprite kinds

- Refactor the cached cluster sprite model into two variants:
  - monochrome alpha mask sprites for the existing Sarasa path;
  - RGBA sprites for color emoji output.
- Cache keys must continue to include terminal cluster text and span.
- Emoji sprites should be cached after rasterization exactly like mono sprites so repeated prompts do not trigger repeated color glyph work.

### 6. Compose emoji sprites into the same terminal surface

- Background fill, selection fill, cursor, and dirty-row logic stay where they are.
- Emoji sprites are blitted as premultiplied or opaque RGBA image content over the already-painted terminal cell background.
- Selection changes should continue to repaint the row background, but the emoji sprite itself should keep its intrinsic colors rather than being re-tinted by the terminal foreground-color selection logic.

### 7. Replace silent blanks with visible failure behavior

- If the renderer cannot find a color emoji font or cannot rasterize a cluster, it must not silently produce a fully transparent cell.
- The fallback behavior should render a visible replacement glyph through the mono atlas path, using a cluster such as `□` or `�`, and emit a rate-limited warning.

## Architecture

### New Module

- Create [`src/app/terminal_emoji.rs`](../../src/app/terminal_emoji.rs) to own:
  - emoji cluster classification;
  - system emoji font discovery;
  - color emoji rasterization;
  - emoji sprite caching support helpers.

### Renderer Integration

- [`src/app/terminal_atlas.rs`](../../src/app/terminal_atlas.rs) remains the primary renderer entrypoint.
- The atlas renderer gains:
  - a lazy emoji renderer dependency;
  - a dual sprite enum;
  - branch logic that routes emoji clusters away from `ab_glyph`.

### Dependency Additions

- [`Cargo.toml`](../../Cargo.toml) adds:
  - `fontdb`
  - `swash`
  - `unicode-properties`

These crates are all Rust-native and fit the goal of keeping terminal rendering portable and self-contained.

## Testing Strategy

Automated coverage should focus on deterministic renderer behavior instead of assuming particular system fonts exist in CI.

### Unit-Level Contracts

- Emoji classifier tests:
  - plain ASCII stays on the mono path;
  - Nerd Font private-use icons stay on the mono path;
  - emoji scalars, VS16 sequences, and ZWJ sequences route to the emoji path.
- Font family preference tests:
  - Windows and Linux preferred-family ordering is stable;
  - missing-system-font cases produce a visible fallback decision instead of a silent blank.

### Renderer Integration Contracts

- Add test seams so the atlas renderer can be exercised with a fake emoji rasterizer.
- Integration tests must prove:
  - emoji clusters no longer collapse into blank cells;
  - color RGBA sprites are composited into the terminal surface;
  - repeated emoji clusters reuse cache entries;
  - selection/background repainting does not erase emoji sprite pixels.

### Manual Verification

- Windows smoke:
  - confirm `📦`, `🦀`, `🌐`, and a ZWJ emoji cluster render in terminal output.
- Linux smoke:
  - confirm the same set renders when `Noto Color Emoji` is installed;
  - confirm missing-font environments produce a visible replacement glyph plus diagnostic log.

## Files

- Create [`src/app/terminal_emoji.rs`](../../src/app/terminal_emoji.rs)
- Modify [`src/app/mod.rs`](../../src/app/mod.rs)
- Modify [`src/app/terminal_atlas.rs`](../../src/app/terminal_atlas.rs)
- Modify [`Cargo.toml`](../../Cargo.toml)
- Modify [`tests/terminal_atlas_renderer_spec.rs`](../../tests/terminal_atlas_renderer_spec.rs)
- Create [`tests/terminal_color_emoji_spec.rs`](../../tests/terminal_color_emoji_spec.rs)
- Update terminal renderer notes in [`readme.md`](../../readme.md)

## Risks

- Linux emoji availability is distribution-dependent. The implementation must make absence visible and diagnosable.
- Color emoji glyph metrics will not behave exactly like Sarasa's outline metrics. The renderer must center emoji sprites inside the already-authoritative terminal cell span rather than inventing new widths.
- Some emoji sequences are multi-codepoint clusters. Classification must respect VS16 and ZWJ composition or the renderer will regress from blank cells to split/wrong cells.

## Validation

- A failing test must be added first for the current “blank emoji” behavior.
- The final renderer must preserve existing CJK and Nerd Font contracts while adding color emoji rendering.
- Focused tests and `cargo check` must pass before claiming the terminal emoji renderer is complete.

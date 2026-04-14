# Terminal Row Banding Contrast Design

## Goal

Increase the perceptibility of the terminal viewport's existing alternating row banding so long command output is easier to scan, while preserving a mainstream, professional terminal look.

## Current State

- The terminal already uses alternating row background colors rather than a separate scanline or texture overlay.
- The palette source lives in `src/theme/spec.rs` and is projected through `src/app/terminal_theme.rs` into both the atlas renderer and the Windows native renderer.
- The current row band values are extremely close together:
  - dark: `#111821` / `#121a24`
  - light: `#fbfcfe` / `#f8fbfd`
- In practice this makes the banding feel nearly invisible instead of providing a subtle reading rhythm.

## Industry Direction

The reviewed mainstream terminal products and terminal frameworks point in the same direction:

- `xterm.js` and VS Code focus on clean backgrounds, transparent selections where needed, and minimum-contrast safeguards rather than visible texture effects.
- `Ghostty` and `kitty` keep readability first and generally scope atmosphere/opacity features to default background regions so explicit cell backgrounds stay clean.
- Strong scanlines, CRT glow, or obvious noise are not common default treatments in mainstream terminal design.

References:

- <http://xtermjs.org/docs/api/terminal/interfaces/iterminaloptions/>
- <http://xtermjs.org/docs/api/terminal/interfaces/itheme/>
- <https://code.visualstudio.com/docs/terminal/appearance>
- <https://ghostty.org/docs/config/reference>
- <https://sw.kovidgoyal.net/kitty/conf/>

## Constraints

- Keep the current rendering structure intact.
- Do not introduce a new scanline, noise, gradient, phosphor, or CRT layer in this iteration.
- Do not change the default terminal background, foreground, cursor, selection, scrollbar, or host layout in this iteration.
- Continue to let explicit ANSI cell backgrounds override the default row banding.

## Approaches Considered

### 1. Pure palette retune

- Only adjust the alternating row colors.
- Keep the implementation and renderer behavior exactly as-is.

Pros:

- Lowest risk.
- Matches mainstream terminal aesthetics.
- Easy to validate and easy to roll back.

Cons:

- Less distinctive than a more stylized atmosphere treatment.

### 2. Palette retune plus ultra-subtle atmosphere

- Adjust row colors and add an almost invisible mood layer such as a tiny drift or noise component.

Pros:

- Could add some character without becoming obviously stylized.

Cons:

- Easy to overshoot.
- Adds implementation and tuning complexity.
- More likely to interfere with selection/highlight clarity.

### 3. Stylized scanline/CRT texture

- Add visible scanlines, noise, or phosphor-like treatment.

Pros:

- Strong personality.

Cons:

- Not aligned with the requested mainstream look.
- Higher risk of looking noisy or cheap.
- More likely to hurt readability.

## Approved Approach

Use approach 1: retune only the existing alternating row band colors.

### Approved Values

- Dark theme:
  - even row: `#111821`
  - odd row: `#14212d`
- Light theme:
  - even row: `#fbfcfe`
  - odd row: `#f3f7fb`

These values aim to make the banding readable in long output without turning the terminal background into a visible texture effect.

## Implementation Scope

### Files to modify

- `src/theme/spec.rs`
- `tests/terminal_atlas_renderer_spec.rs`

### Files intentionally not modified

- `src/app/terminal_theme.rs`
- `src/app/terminal_atlas.rs`
- `src/app/terminal_renderer/platform/windows.rs`
- `ui/shell/terminal-session-host.slint`

The current palette propagation path is already correct; this change is a visual retune, not a renderer redesign.

## Validation

Evaluate the change in three situations:

1. Empty terminal viewport
2. Long command output such as logs, tables, or `ls` listings
3. Content with explicit ANSI background colors such as editors, status bars, or highlighted blocks

Success criteria:

- The background remains clean and mainstream-looking.
- Alternating rows are perceptible when scanning dense output.
- Explicit background colors, selection, cursor, and highlights still read cleanly.
- Light mode remains crisp and does not feel dirty or foggy.

Failure signals:

- The first impression is "texture" instead of "terminal."
- Empty space becomes more visually prominent than terminal content.
- Light mode starts to look muddy.
- Explicit background regions feel contaminated by the banding treatment.

## Rollback Strategy

- If both themes feel too strong, move the odd-row color back toward the existing value.
- If only light mode feels too heavy, reduce only the light odd-row delta.
- Because the change is palette-only, rollback remains trivial.

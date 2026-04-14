# Terminal Viewport Background Design

## Goal

Replace the current row-banded terminal viewport background with a quieter background model that reads as one continuous surface. The viewport should feel calm, professional, and modern, with text remaining the dominant visual layer.

## Approved Direction

- Remove the visible alternating row stripe / banding effect from the terminal viewport.
- Change the viewport background model from per-row background consumption to viewport-level background painting.
- Dark theme uses a unified dark base plus a very subtle top-to-bottom gradient.
- Light theme also removes row banding, but may temporarily fall back to a uniform solid background until light-theme gradient values are designed.
- Keep existing renderer structure as intact as possible.
- Do not change terminal typography, cursor behavior, ANSI cell background semantics, shell content, or tab logic.

## Visual Target

### Dark theme

The dark terminal viewport should become a single deep surface with only a restrained vertical lift near the top. The intended result is:

- base background: `#07111A`
- gradient top: `#0A1621`
- gradient bottom: `#07111A`
- gradient strength: extremely low, roughly a 2% to 4% perceptual lift
- no visible scanline, zebra, or ledger-paper feel in empty rows

The gradient must be subtle enough that users mostly perceive a stable dark surface rather than a decorative effect.

### Light theme

The light theme follows the same design principle but not the same color values. For this pass:

- remove viewport row banding entirely
- keep the viewport on a uniform light surface
- avoid inventing an unapproved light-theme gradient treatment

That keeps the background model consistent without forcing an unfinished light visual system.

## Non-Goals

This change must not alter:

- terminal text layout
- font family, size, weight, or letter spacing
- cursor geometry or blink behavior
- ANSI cell background rendering
- selection overlay behavior
- tab behavior, shell content, or viewport sizing logic
- renderer architecture beyond the minimum needed to stop consuming row banding as viewport chrome

## Root Cause

The current horizontal stripe feeling comes from the background model itself, not from stale damage, transparent blending residue, or text sampling.

### Current stripe sources

- `src/theme/spec.rs` provides `row_bg_even` / `row_bg_odd` values.
- `src/app/terminal_core/alacritty_adapter.rs` and `src/app/terminal_core/wezterm_adapter.rs` project those colors into `TerminalSurfaceState`.
- `src/app/terminal_atlas.rs` uses the per-row values to paint the software viewport background row-by-row.
- `src/app/terminal_renderer/platform/windows.rs` fills the native retained viewport row-by-row using the same even/odd row colors.

This means blank rows expose the stripe system most strongly, which is exactly the opposite of the desired visual hierarchy.

## Design Decision

### Keep the transport fields, stop consuming them as viewport stripes

`row_bg_even` / `row_bg_odd` and their projected runtime fields stay in place for now to keep the change small and avoid widening the cross-layer contract.

However, renderers will stop using those fields to paint the viewport chrome. Instead:

- viewport background becomes a renderer-owned whole-surface fill
- optional subtle vertical treatment is applied at the viewport level
- ANSI background runs remain independent and still draw on top where a cell explicitly owns a background color

This preserves compatibility while removing the visual stripe model.

## Background Model

### Shared model

Every backend should conceptually follow this order:

1. Paint the full terminal viewport rect with a single base background.
2. Optionally overlay a very subtle top-to-bottom vertical treatment.
3. Paint ANSI cell background runs.
4. Paint selection overlays, glyphs, cursor, and other existing layers in their current order.

### Software atlas path

The atlas renderer should stop filling each row from `row_bg_rgba` and instead paint the entire viewport background once per frame (or once per invalidation path) using the viewport-level background colors.

If a true per-pixel vertical gradient is easy in the atlas buffer, apply it across the full terminal image. If not, use the allowed fallback:

- solid base fill for the full image
- a very low-alpha overlay confined to the top ~15% to 20% of the viewport height

### Windows retained native path

The native Windows background pass should stop iterating rows for viewport chrome. It should fill the entire terminal clip rect once, then optionally draw a subtle vertical overlay. ANSI background runs remain as their own pass afterward.

If Direct2D gradient setup is inconvenient in this pass, a single solid fill is acceptable as the initial fallback, as long as row banding is gone.

## Tunable Parameters

The implementation should centralize the viewport background knobs so later visual tuning does not require another structural rewrite.

Required knobs:

- `TERMINAL_ROW_BANDING_ENABLED`
- `TERMINAL_ROW_BANDING_ALPHA`
- `TERMINAL_BG_GRAIN_ALPHA`
- `TERMINAL_BG_BASE_DARK`
- `TERMINAL_BG_GRADIENT_TOP_DARK`
- `TERMINAL_BG_GRADIENT_BOTTOM_DARK`
- `TERMINAL_BG_BASE_LIGHT`
- `TERMINAL_BG_GRADIENT_TOP_LIGHT`
- `TERMINAL_BG_GRADIENT_BOTTOM_LIGHT`

Approved defaults for this pass:

- `TERMINAL_ROW_BANDING_ENABLED = false`
- `TERMINAL_ROW_BANDING_ALPHA = 0.0`
- `TERMINAL_BG_GRAIN_ALPHA = 0.0`
- `TERMINAL_BG_BASE_DARK = #07111A`
- `TERMINAL_BG_GRADIENT_TOP_DARK = #0A1621`
- `TERMINAL_BG_GRADIENT_BOTTOM_DARK = #07111A`
- light values may initially collapse to one uniform light base if no approved gradient is available yet

## Testing Strategy

The old tests that lock alternating row backgrounds are no longer valid.

They should be replaced with assertions that verify:

- dark theme no longer alternates visible row fills
- dark theme keeps a continuous base background with only a very subtle top-vs-bottom difference
- light theme no longer alternates visible row fills
- renderers still preserve ANSI background runs, selection overlays, and glyph visibility
- row banding fields may still exist in the contract, but viewport paint paths no longer consume them as chrome

## Acceptance Criteria

The implementation is correct when:

- the first thing users notice is the text, not the background pattern
- empty terminal space no longer reads like horizontal striping or table banding
- dark theme feels stable, calm, and slightly lifted rather than dead flat or stylized
- light theme is uniform and quiet rather than row-striped
- performance stays effectively unchanged
- no text readability regression is introduced

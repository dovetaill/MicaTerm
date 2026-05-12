# Ayu Terminal Neighborhood Design

Date: 2026-05-12
Owner: Codex
Status: Approved for planning

## Goal

Refine the post-migration default terminal experience so MicaTerm reads as one
coherent Ayu product in both dark and light mode.

This is not a full shell redesign. It is a focused terminal-neighborhood pass
that:

- keeps `src/theme/spec.rs` as the single color source of truth
- keeps the default preset names `Ayu Dark` and `Ayu Light`
- projects terminal and shell-neighborhood colors through Rust runtime state
- aligns bitmap, native, fallback, and Slint host surfaces to the same preset
- removes the remaining visual split between the Ayu terminal viewport and the
  surrounding titlebar / tabbar / sidebar / session host surfaces

## Why This Exists

The 2026-05-11 Ayu migration successfully moved the default terminal palette
away from the older premium palette, but the current result still feels split:

- dark mode terminal content is close to Ayu, while titlebar / sidebar / shell
  chrome still read as a separate slate / Mica family
- dark mode terminal foreground is slightly too dim in use
- light mode viewport still feels too stark and close to a white console
- light mode shell chrome and terminal viewport do not yet feel like the same
  Ayu Light product surface
- selection, scrollbar, terminal frame, and fallback paths are still easier to
  drift than they should be because not every active surface is runtime
  projected from the same preset

The user explicitly chose the broader scope variant: include titlebar, tabbar,
and sidebar in the runtime-projected Ayu family so the next implementation does
not need another refactor just to close the remaining gap.

## Scope Decision

### Approved Direction

Use the expanded runtime projection strategy.

That means:

- terminal viewport palette stays rooted in `src/theme/spec.rs`
- terminal-adjacent chrome stays projected through `src/app/terminal_theme.rs`
  and `src/app/bootstrap.rs`
- titlebar / tabbar / sidebar / workspace shell surfaces also move onto the
  same runtime projection path
- `ui/theme/tokens.slint` remains only a boot-time parity default layer

### Out Of Scope

This design does not include:

- a new shell layout
- typography changes
- motion redesign
- new component structure unrelated to color projection
- a second independent Ayu ladder inside Slint
- changing the default dark preset from `Ayu Dark` to `Ayu Mirage`
- a broad ANSI 16 recolor unless a specific mismatch is found during focused
  visual verification

## Source Policy

### Priority Order

1. Official Ayu semantics from `ayu-theme/ayu-colors`
2. Terminal mappings from `terminalcolors.com` for Ayu Dark / Light / Mirage
3. Mature ports:
   - `hwyncho/ayu-Terminal-app`
   - `hwyncho/ayu-iTerm`
   - `joshtynjala` Windows Terminal Ayu gist
4. The user-provided MicaTerm screenshots as the tie-breaker for final feel

### Reference Notes

The external sweep confirms a few stable anchors:

- classic Ayu Dark terminal background is consistently near `#0A0E14`
- classic Ayu Dark terminal foreground is consistently near `#B3B1AD`
- Ayu warm gold / orange cursor accents are consistently in the
  `#E6B450` to `#FFB454` family
- official Ayu Light editor / surface colors are softer than a stark white
  console and cluster around `#F8F9FA` / `#FCFCFC` with `#5C6166` text

The design uses the classic Ayu terminal anchors for viewport and ANSI values,
then pulls the shell-neighborhood surfaces closer to official Ayu surface
semantics so the app reads as one family rather than a terminal pasted into a
different shell.

## Current Repo Findings

### What Is Already Good

- `src/theme/spec.rs` already owns the terminal preset values and publishes
  `Ayu Dark` / `Ayu Light`
- `src/app/terminal_theme.rs` already converts that preset into runtime-facing
  terminal colors
- `src/app/bootstrap.rs` already projects selection, scrollbar, frame, cursor,
  and fallback default fg/bg into `AppWindow`
- renderer drift is already controlled by keeping dark and light viewport
  backgrounds flat

### What Still Causes The Split

- titlebar / tabbar / sidebar / shell surfaces still primarily consume
  hardcoded `ThemeTokens` values rather than active runtime-projected Ayu
  values
- `ui/theme/tokens.slint` currently acts as more than a boot-time default for
  several terminal-neighborhood surfaces
- tests still mostly prove terminal-local palette projection but do not fully
  lock the broader shell-neighborhood runtime projection contract

## Design Principles

### 1. One palette truth

`src/theme/spec.rs` remains the only place where the actual Ayu hex values are
authored.

### 2. One runtime projection chain

`src/app/terminal_theme.rs` and `src/app/bootstrap.rs` become the only active
projection path for terminal-neighborhood and shell-neighborhood colors.

### 3. Slint consumes, not invents

Slint may keep boot defaults, but live titlebar / tabbar / sidebar /
workspace / terminal host surfaces should consume runtime-projected colors
instead of maintaining a detached palette.

### 4. Renderer parity beats decoration

Viewport top and bottom remain flat and equal to the chosen terminal background
for both dark and light defaults so bitmap and native presentation cannot drift.

### 5. Focused visual change only

Only color and close-to-color host chrome behavior is in scope. No unrelated UI
restyling is introduced.

## Chosen Palette Direction

### Ayu Dark

### Terminal Core

- preset name: `Ayu Dark`
- terminal background: `#0A0E14`
- terminal foreground: `#C5C1B8`
- viewport background top: `#0A0E14`
- viewport background bottom: `#0A0E14`
- cursor background: `#E6B450`
- cursor foreground: `#0A0E14`
- selection overlay rgb: `#2A3541`
- selection overlay alpha: `0.78`

### Terminal Neighborhood

- terminal host / active terminal frame surface: `#141B24`
- terminal frame border / split: `#1B2530`
- scrollbar track: `#111821`
- scrollbar thumb: `#2F3944`
- scrollbar thumb active: `#3C4856`

### Shell Neighborhood

- deepest app / shell background: `#0A0E14`
- titlebar background: `#10151D`
- tabbar background: `#10151D`
- sidebar background: `#10151D`
- sidebar panel / raised shell surface: `#111821`
- right panel background: `#111821`
- active tab / active session shell surface: `#141B24`
- separator / subtle border family: `#1B2530`
- shell primary text: `#C5C1B8`
- shell secondary text: `#9AA4AE`
- shell muted text: `#7D8790`
- shell accent / focus / terminal-adjacent active accent: `#E6B450`

### ANSI Policy

Keep the current Ayu migration ANSI 16 set unless a narrow visual mismatch is
found during implementation verification. The current anchors already match the
mature Windows Terminal / Terminal.app / iTerm Ayu family well.

### Ayu Light

### Terminal Core

- preset name: `Ayu Light`
- terminal background: `#F8F9FA`
- terminal foreground: `#5C6166`
- viewport background top: `#F8F9FA`
- viewport background bottom: `#F8F9FA`
- cursor background: `#FFAA33`
- cursor foreground: `#F8F9FA`
- selection overlay rgb: `#55B4D4`
- selection overlay alpha: `0.20`

### Terminal Neighborhood

- terminal host / active terminal frame surface: `#FAFAFA`
- terminal frame border / split: `#D8DEE6`
- scrollbar track: `#F0F3F6`
- scrollbar thumb: `#D1D7DE`
- scrollbar thumb active: `#C1C8D1`

### Shell Neighborhood

- deepest app / shell background: `#F4F6F8`
- titlebar background: `#EEF2F5`
- tabbar background: `#EEF2F5`
- sidebar background: `#EEF2F5`
- sidebar panel / raised shell surface: `#F0F3F6`
- right panel background: `#F0F3F6`
- active tab / active session shell surface: `#FAFAFA`
- separator / subtle border family: `#D8DEE6`
- shell primary text: `#5C6166`
- shell secondary text: `#7A838C`
- shell muted text: `#8A939C`
- shell accent / focus / terminal-adjacent active accent: `#FFAA33`

### ANSI Policy

Keep the current Ayu Light ANSI family unless a targeted readability fix is
required. The present mapping already fits the classic Ayu Light terminal
ecosystem and avoids introducing a second light ANSI interpretation.

## Values Intentionally Left Unchanged

These values are intentionally preserved from the previous Ayu migration unless
focused testing proves they must move:

- dark terminal background anchor stays `#0A0E14`
- dark cursor accent stays in the warm Ayu gold family at `#E6B450`
- light cursor accent stays in the warm Ayu orange family at `#FFAA33`
- default presets remain `Ayu Dark` and `Ayu Light`
- viewport top and bottom remain flat and equal to the base background
- ANSI 16 colors remain the classic Ayu migration anchors by default

## Architecture

### Theme Source Of Truth

`src/theme/spec.rs` remains the source of truth for:

- terminal fg/bg/cursor/selection/scrollbar
- shell app / titlebar / tabbar / sidebar / panel / active session surfaces
- shell text hierarchy and accent values

No independent Ayu shell ladder should be authored in Slint.

### Projection Layer

`src/app/terminal_theme.rs` should expand from a terminal-local preset adapter
into a shared runtime projection layer for terminal-neighborhood surfaces.

It should expose or help derive:

- terminal bg/fg/cursor/selection
- terminal frame / split
- terminal scrollbar track / thumb / active
- shell-neighborhood app / titlebar / tabbar / sidebar / panel / active surface
  colors that belong to the same preset family

This remains projection code only. It must not become a second authored palette.

### Bootstrap Wiring

`src/app/bootstrap.rs` remains the one runtime downlink into `AppWindow`.

At startup, on theme toggles, and on theme-variant changes, bootstrap should
refresh:

- terminal fallback fg/bg/cursor
- terminal selection / scrollbar / frame
- titlebar / tabbar / sidebar / workspace-neighborhood active surfaces
- any terminal host chrome that currently falls back to a token constant

Implementation may route part of that through helper modules under the bootstrap
tree, but the runtime projection source still conceptually lives in the
bootstrap path.

### Slint Contract

`ui/theme/tokens.slint` stays as boot-time parity defaults only.

Active runtime surfaces should flow through explicit `AppWindow` properties,
then through `ui/app-window.slint`, `ui/shell/workspace-pane.slint`, and
`ui/shell/terminal-session-host.slint` into the final consumers.

This means:

- titlebar, tabbar, sidebar, and workspace shell surfaces should prefer
  runtime-projected properties
- terminal host frame / selection / scrollbar must continue to prefer
  runtime-projected session properties
- fallback behavior must remain visually in-family with the active preset

### Renderer Consistency

Bitmap and native renderers must continue to share the same viewport background.

No new gradient, row banding, or grain should be introduced for the Ayu
defaults unless there is a specifically tested reason to do so.

## File Boundaries

Primary authored and projection files:

- `src/theme/spec.rs`
- `src/app/terminal_theme.rs`
- `src/app/bootstrap.rs`
- `src/app/bootstrap/shell_chrome.rs`

Terminal runtime / renderer consumers to verify:

- `src/app/terminal_core/wezterm_adapter.rs`
- `src/app/terminal_presenter.rs`
- `src/app/terminal_renderer/platform/windows.rs`

Slint runtime consumers:

- `ui/theme/tokens.slint`
- `ui/app-window.slint`
- `ui/shell/workspace-pane.slint`
- `ui/shell/terminal-session-host.slint`

## Testing Strategy

### Add Or Update Tests Before Implementation Where Practical

The implementation should lock behavior at four layers.

### 1. Theme source tests

Cover:

- `ThemeMode::Dark` maps to `Ayu Dark`
- `ThemeMode::Light` maps to `Ayu Light`
- dark bg / fg / cursor / selection values match the chosen target
- light bg / fg / cursor / selection values match the chosen target
- viewport top / bottom remain equal to the base background in both modes

Likely files:

- `tests/terminal_theme_selection_spec.rs`
- `tests/theme_terminal_redesign_spec.rs`

### 2. Runtime and fallback projection tests

Cover:

- no-surface fallback fg/bg/cursor match the active preset
- theme toggles refresh fallback values immediately
- frame / selection / scrollbar track / thumb / active are all derived from the
  projected preset
- runtime shell-neighborhood surfaces update on theme mode and theme variant
  changes without depending on stale token defaults

Likely files:

- `tests/bootstrap_smoke.rs`
- `tests/terminal_session_spec.rs`
- `tests/ui_preferences.rs`

### 3. Slint parity and contract tests

Cover:

- `ui/theme/tokens.slint` boot defaults remain in sync with the Rust preset
- active terminal-neighborhood values are threaded as explicit runtime
  properties rather than read from detached token constants
- titlebar / sidebar / workspace host contracts expose the projected shell
  surfaces as first-class properties

Likely files:

- `tests/theme_semantic_token_contract_spec.rs`
- `tests/native_terminal_surface_contract_spec.rs`

### 4. Legacy wording cleanup

Cover:

- default terminal theme tests no longer describe the active default as
  Catppuccin, Mica Graphite, or Mica Canvas
- old wording may remain only where a test is explicitly describing retired
  behavior or migration history

## Acceptance Criteria

### Dark Mode

- terminal viewport, titlebar, tabbar, sidebar, and session host feel like one
  Ayu Dark family
- terminal text is easier to read than the current screenshot and still does
  not become pure white
- cursor and active accent read as warm Ayu gold
- borders and inactive UI remain visible but restrained

### Light Mode

- terminal viewport no longer feels stark or near-pure-white
- terminal text reads as cool gray rather than harsh black
- shell and terminal feel like one Ayu Light product surface
- borders provide structure without looking heavy

### Cross-Path Consistency

- `src/theme/spec.rs` remains the source of truth
- runtime, fallback, bitmap, native, and Slint host all consume one projected
  preset family
- no second independent Ayu palette ladder is introduced in Slint
- default Ayu viewport backgrounds stay flat and equal across renderers

## Verification Commands

The implementation plan should verify with:

- `cargo fmt`
- `cargo test terminal_theme -- --nocapture`
- `cargo test ui_preferences -- --nocapture`
- `cargo test bootstrap_smoke -- --nocapture`
- `cargo test native_terminal_surface_contract_spec -- --nocapture`
- `cargo test`

If any command is unavailable in the implementation phase, the final report must
state exactly which commands were run and why others were skipped.

# Ayu Light/Dark Refinement Design

Date: 2026-05-14
Owner: Codex
Status: Approved for planning

## Goal

Refine MicaTerm's existing `Ayu Light` and `Ayu Dark` implementation so the
shell and terminal read as one Ayu product in both modes, with no second
independent palette and no redesign of layout, typography, or renderer
behavior.

This is a focused polish pass, not a re-theme from scratch.

## Inputs And References

This design is based on four inputs used together rather than in isolation:

1. The current MicaTerm implementation and its Rust -> runtime -> Slint theme
   projection chain.
2. The latest MicaTerm screenshots described by the user:
   - light mode is improved but still reads too much like a white console
   - dark mode is broadly good and should only be polished
3. The user-provided Kiro Ayu light/dark screenshots from 2026-05-14.
4. Public Ayu references checked during design:
   - `ayu-theme/ayu-colors`
   - VS Code Ayu marketplace screenshots, especially the lighter,
     lower-separator shell direction
   - mature list/tree selection patterns from VS Code theme color guidance

The screenshot references are used as visual tie-breakers. They do not replace
codebase reality or the single-source architecture.

## Screenshot Findings

### Kiro Ayu Light

The provided light screenshot confirms several things:

- the global page reads as a unified off-white sheet, not as white editor
  content framed by visibly different shell slabs
- the dominant surface family is close to `#F8F9FA` / `#FCFCFC`
- separators are present but very soft
- orange is used sparingly for active underlines, highlighted controls, and
  small accents rather than large rectangular focus boxes
- text remains cool gray instead of stark black

### Kiro Ayu Dark

The provided dark screenshot confirms:

- the global base stays very close to `#0D1017`
- raised surfaces are only slightly lighter than the base, around the
  `#141820` family
- separators are subtle enough to almost disappear at rest
- active emphasis comes from restrained accent use rather than heavy boxed
  outlines

## Why This Pass Exists

The existing Ayu migration already solved the big architectural problem: the
active shell/terminal palette is authored in Rust and projected into runtime
Slint consumers.

The remaining issues are narrower but still visible:

- `Ayu Light` still reads too much like a white console because the viewport,
  app shell, and raised shell surfaces are not unified enough
- some shell surfaces in light mode still feel too detached from the terminal
- selected and focused rows still read as boxed controls in several places,
  especially where a bright orange rectangular border is used
- dark mode is visually close, but selected/focus states and separators are a
  little too hard

These issues are not just hex-value problems. They come from a combination of:

- surface ladder calibration
- inconsistent selected/focus treatment across components
- a few Slint consumers still expressing the same state differently

## Scope Decision

### Approved Scope

Use a bounded medium-sized refinement pass:

- keep the existing Ayu architecture
- refine the authored shell and terminal values in `src/theme/spec.rs`
- preserve `src/app/terminal_theme.rs` as the runtime projection layer
- preserve `src/app/bootstrap.rs` and `src/app/bootstrap/shell_chrome.rs` as
  the runtime publishing path
- make the active Slint consumers converge on one selected/focus treatment
- allow a minimal semantic token addition only if the current fields cannot
  cleanly express selected fill, selected accent rail, and keyboard focus

### Out Of Scope

This pass does not include:

- layout changes
- typography changes
- gradients, blur, grain, or row banding
- terminal renderer redesign
- a new preset name
- a second Ayu palette authored in Slint
- broad rework of unrelated modals or views outside the shell neighborhoods
  touched by the task

## Architecture Constraints

The implementation must preserve these rules:

- `src/theme/spec.rs` remains the single authored color source of truth
- `src/app/terminal_theme.rs` remains the runtime projection layer
- `src/app/bootstrap.rs` and `src/app/bootstrap/shell_chrome.rs` publish active
  runtime shell/session colors
- Slint consumes runtime-projected properties where possible
- `ui/theme/tokens.slint` stays boot-time parity only
- bitmap and native terminal viewport backgrounds stay flat and identical
- preset names remain `Ayu Dark` and `Ayu Light`

## Current Code Findings

The current repo already has the right high-level chain:

- authored values live in `src/theme/spec.rs`
- `src/app/terminal_theme.rs` projects those values into terminal and shell
  runtime presets
- `src/app/bootstrap.rs` publishes those values into `AppWindow`
- `ui/app-window.slint` threads them into shell and terminal consumers

The remaining inconsistency is mostly in how consumers express state:

- `ui/components/asset-node-row.slint` still uses a full selected/focused border
  approach
- `ui/components/sidebar-nav-button.slint` still reads active state as a boxed
  button
- `ui/shell/right-panel.slint` uses a different row-selection treatment from the
  asset tree
- `ui/components/open-saved-ssh-modal.slint` already demonstrates a more mature
  pattern: subtle fill plus a narrow active rail

That internal component is the best local behavioral reference for the row-state
rewrite, but it still must be driven by the runtime theme contract rather than
becoming a second independent Ayu definition.

## Palette Strategy

### Core Decision

Do not redesign dark mode. Polish it.

Do not make light mode brighter. Make it more unified and less stiff.

### Chosen Light Targets

These are the preferred authored targets for the Premium Default / Ayu Light
family:

- terminal background: `#F8F9FA`
- terminal foreground: keep `#5C6166`
- cursor background: keep `#FFAA33`
- cursor foreground: `#F8F9FA`
- app background: `#F8F9FA`
- titlebar background: `#F8F9FA`
- tabbar background: `#F8F9FA`
- sidebar background: `#F8F9FA`
- sidebar panel background: `#F6F8FA`
- right panel background: `#F6F8FA`
- active raised surface: `#FCFCFC`
- terminal frame surface: align with the same raised active surface family
- border / separator family: soften toward `#E5E9EF`
- scrollbar track: `#F4F6F8`
- scrollbar thumb: `#D6DCE3`
- scrollbar thumb active: `#C6CDD6`
- text primary: `#5C6166`
- text secondary: `#7A838C`
- text muted / inactive: `#8A939C`
- accent: `#FFAA33`

### Chosen Dark Targets

Dark mode stays close to the current approved Ayu Dark family:

- terminal background: keep `#0A0E14`
- terminal foreground: keep the current warm Ayu dark foreground family
- cursor / accent: keep `#E6B450`
- titlebar / sidebar base: around `#10151D` / `#111821`
- raised active surface: around `#141B24`
- border / separator family: around `#1B2530`, but visually softer in use

### Values Intentionally Left Unchanged Unless Testing Proves Otherwise

The following are intentionally preserved for this pass:

- `Ayu Dark` terminal background family
- `Ayu Dark` cursor / accent family
- `Ayu Light` terminal foreground family, unless it still reads too heavy after
  the shell ladder is corrected
- the current Ayu ANSI 16 tables
- flat viewport backgrounds for both dark and light

## Selected And Focus State Strategy

This is the central behavioral change.

### Target Interaction Pattern

For shell lists, trees, and active rows, use one visual language:

- selected state:
  - subtle fill
  - narrow 2px accent rail on the leading edge
  - no bright full rectangular outline
- hover state:
  - quiet neutral fill
  - cooler and softer than selected
- keyboard focus:
  - only shown when the control actually has keyboard focus
  - separate from selected state
  - not reused as the default selected outline

### Light Mode Row-State Targets

- selected fill: `#FFF7EA`
- hover fill: `#EEF2F5`
- selected accent rail: `#FFAA33`
- selected outline: transparent or near-transparent

### Dark Mode Row-State Targets

- selected fill: `#141B24`
- hover fill: a slightly raised neutral dark shell surface in the existing Ayu
  family
- selected accent rail: `#E6B450`
- selected outline: transparent or near-transparent

## Theme Semantic Strategy

### Preferred Path

Prefer reusing existing runtime shell fields first:

- `sidebar_item_hover`
- `sidebar_item_selected`
- `sidebar_item_selected_border`
- `focus_ring`

The first implementation attempt should reinterpret these as:

- `sidebar_item_selected`: selected fill
- `sidebar_item_selected_border`: selected accent rail
- `focus_ring`: actual keyboard focus only

### Conditional Minimal Extension

If implementation shows that the current contract cannot express keyboard focus
separately without regressions, add only the smallest missing semantic field,
threaded end-to-end through Rust and Slint.

The intended rule is:

- do not add a new field just because a name is slightly awkward
- add one only if it prevents a consumer from reintroducing a hardcoded full
  outline or from conflating focus with selection

This keeps the pass medium-sized and architecture-safe instead of pretending a
field shortage does not exist.

## Consumer Strategy

The selected/focus rewrite should at minimum converge these consumers:

- `ui/components/asset-node-row.slint`
- `ui/components/sidebar-nav-button.slint`
- `ui/shell/assets-sidebar.slint`
- `ui/shell/sidebar.slint`
- `ui/shell/right-panel.slint`

Terminal-adjacent shell polish should preserve the runtime session path in:

- `ui/app-window.slint`
- `ui/shell/workspace-pane.slint`
- `ui/shell/terminal-session-host.slint`

`ui/theme/tokens.slint` may only be updated for boot-time parity defaults. It
must not grow a detached live Ayu system.

## Testing Strategy

The test suite should lock four things:

### 1. Palette Identity

- `ThemeMode::Dark` still maps to `Ayu Dark`
- `ThemeMode::Light` still maps to `Ayu Light`
- terminal bg/fg/cursor/selection/scrollbar values match the refined targets

### 2. Single-Truth Projection

- shell-neighborhood values come from `src/theme/spec.rs`
- runtime `AppWindow` shell/session properties are populated from the projected
  preset
- fallback/no-surface terminal colors remain in the same preset family after
  toggles

### 3. Slint Consumer Contract

- active shell/session consumers use runtime-projected properties instead of
  detached `ThemeTokens` constants where the runtime values already exist
- `ThemeTokens` remains boot-time parity only

### 4. Interaction Contract

- selected rows no longer depend on a hardcoded full bright orange border
- selected rows use subtle fill plus a leading accent rail
- bitmap/native terminal backgrounds stay flat and equal

## Risks And Guardrails

### Risk: Scope Creep Into A Full Shell Restyle

Guardrail:

- only touch the files and semantics needed for the Ayu light/dark shell and
  terminal neighborhood pass

### Risk: Reintroducing A Second Ayu Palette In Slint

Guardrail:

- any new active color must originate in `src/theme/spec.rs` and travel through
  runtime projection instead of being authored directly in a Slint component

### Risk: Selected State Still Feels Boxed After Palette Updates

Guardrail:

- treat the row-state rewrite as a first-class deliverable, not as a cosmetic
  afterthought to the palette change

### Risk: Dark Mode Accidentally Gets Redesigned

Guardrail:

- preserve the dark core values and only soften state and separator treatment

## Success Criteria

This refinement is successful when:

- `Ayu Light` no longer reads as a white console dropped into a separate shell
- `Ayu Dark` remains recognizably the current approved dark implementation, only
  calmer and more cohesive
- selected and focused rows use subtle fill plus accent rail instead of hard
  orange boxes
- the active palette still has one authored truth in Rust and one runtime
  projection path into Slint
- bitmap and native terminal backgrounds remain flat and identical

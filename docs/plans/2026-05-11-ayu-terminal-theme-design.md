# Ayu Terminal Theme Migration Design

**Status:** Approved for planning

**Goal:** Replace the current default terminal dark/light palette in MicaTerm with a Termius-like Ayu Dark / Ayu Light while keeping one coherent palette source across runtime, fallback, renderer, and Slint terminal-adjacent chrome.

## Why This Change Exists

The current default terminal path still presents a custom premium palette rather than Ayu. The user-provided Termius screenshots show a more productized Ayu look:

- dark mode: deep blue-black canvas, warm off-white text, warm gold cursor/accent, restrained ANSI colors
- light mode: soft off-white canvas, cool gray text, blue/orange accents, not a legacy stark-white console

This is not just a 16-color swap. In the current repo, terminal colors already flow from a Rust theme spec into the runtime and renderers, but some Slint fallback and terminal-adjacent surfaces still carry separate hardcoded values. The migration must finish as a single palette projection, not a partial reskin.

## Source Policy

### Priority Order

1. Official / upstream Ayu palette semantics: `ayu-theme/ayu-colors`
2. Terminal mappings: `terminalcolors.com` Ayu Dark / Light / Mirage
3. Other terminal app ports and community references: `hwyncho/ayu-Terminal-app`, `joshtynjala` Windows Terminal gist, `Gogh`, `iTerm2-Color-Schemes`
4. User-provided Termius screenshots as tie-breaker for visual feel

### How Sources Are Used

- `ANSI 16` and terminal-centric `fg/bg` are chosen from the mature terminal-app Ayu family, not from the newer official generated terminal table when they conflict.
- `cursor`, `accent`, and `selection` semantics are guided by official Ayu tokens and adjusted to match the provided screenshots.
- When the classic terminal Ayu family conflicts internally, screenshot feel wins over a literal one-file download match.

## Upstream Findings

### Official Ayu vs Classic Terminal Ayu

Current official `ayu-colors` terminal tables diverge from the classic Ayu terminal family used across Windows Terminal, Terminal.app, Gogh, and iTerm ports.

- Official Ayu Dark trends toward `#0D1017` / `#10141C` surfaces and a regenerated terminal table.
- Classic terminal Ayu Dark trends toward `#0A0E14` background and `#B3B1AD` foreground.
- Official Ayu Light trends toward `#F8F9FA` / `#FCFCFC` surfaces and `#5C6166` text.
- Classic terminal Ayu Light ports vary between `#F8F9FA` and `#FAFAFA` backgrounds and between `#5C6166` and `#6C7680` foregrounds.

The screenshots lean closer to the classic terminal family than to the regenerated official terminal table.

### Dark Mode Decision

Use `Ayu Dark` as the default dark mapping, not Mirage.

Reasoning:

- the user explicitly wants `ThemeMode::Dark -> "Ayu Dark"`
- the screenshots read closer to the deep `#0A0E14` classic dark canvas than to Mirage's lighter slate `#1F2430`
- Mirage remains a useful reference for restrained dark chrome, but not the default dark preset name or base canvas

### Light Mode Decision

Use `Ayu Light` as the default light mapping with an off-white background and cool gray foreground.

Reasoning:

- the screenshots show a near-`#FAFAFA` workspace rather than a hard white terminal
- the design should stay within Ayu Light's cool, low-stress light palette and avoid black-on-white console contrast

## Chosen Palette Direction

### Ayu Dark

Use the classic terminal Ayu family for the default dark terminal palette:

- name: `Ayu Dark`
- default background: `#0A0E14`
- viewport background top/bottom: `#0A0E14`
- default foreground: `#B3B1AD`
- cursor background: `#E6B450`
- cursor foreground: `#0A0E14`
- selection: a semi-transparent cool blue-gray overlay derived from Ayu semantics and projected from Rust as one RGBA source for renderer and Slint host overlay
- scrollbar thumb / frame / split: cool blue-gray values derived from the same preset so the terminal neighborhood stays unified

Chosen dark ANSI 16 colors:

- `0 #01060E`
- `1 #EA6C73`
- `2 #91B362`
- `3 #F9AF4F`
- `4 #53BDFA`
- `5 #FAE994`
- `6 #90E1C6`
- `7 #C7C7C7`
- `8 #686868`
- `9 #F07178`
- `10 #C2D94C`
- `11 #FFB454`
- `12 #59C2FF`
- `13 #FFEE99`
- `14 #95E6CB`
- `15 #FFFFFF`

This table best matches the screenshot feel and is corroborated by the mature Windows Terminal / Terminal.app / Gogh-style Ayu family.

### Ayu Light

Use the classic Ayu Light terminal family with a soft off-white canvas and cool gray text:

- name: `Ayu Light`
- default background: `#FAFAFA`
- viewport background top/bottom: `#FAFAFA`
- default foreground target: `#5C6166`
- cursor background: `#FFAA33`
- cursor foreground: same as background canvas
- selection: a semi-transparent cool blue overlay projected from Rust, not separately hardcoded in Slint
- scrollbar thumb / frame / split: cool light grays from the same preset so the host stays modern rather than chalk-white

Chosen light ANSI 16 colors:

- `0 #000000`
- `1 #EA6C6D`
- `2 #6CBF43`
- `3 #ECA944`
- `4 #3199E1`
- `5 #9E75C7`
- `6 #46BA94`
- `7 #C7C7C7`
- `8 #686868`
- `9 #F07171`
- `10 #86B300`
- `11 #F2AE49`
- `12 #399EE6`
- `13 #A37ACC`
- `14 #4CBF99`
- `15 #D1D1D1`

This keeps the mature classic Ayu Light family and avoids switching to the regenerated official light terminal table, which drifts away from the screenshot feel and existing terminal-app expectations.

## Architecture Decision

### Scope Choice

Use the terminal-neighborhood sync strategy, not a full app-shell Ayu rewrite.

That means the migration includes:

- terminal preset values in Rust
- fallback / no-surface terminal fg/bg/cursor
- bitmap and native renderer palette projection
- Slint terminal selection overlay
- terminal scrollbar track/thumb/active colors
- terminal frame / terminal-adjacent host background

It does not include a wholesale retheme of titlebar, sidebar, welcome, or the broader app shell in this slice.

### Single Source Of Truth

Keep `src/theme/spec.rs` as the palette truth.

The migration should not introduce new independent Ayu hex ladders in:

- Slint terminal host
- bootstrap fallback path
- renderer backends
- native fallback paint

Instead:

- Rust theme spec owns the preset
- terminal preset projection owns the runtime palette
- bootstrap publishes terminal-neighborhood surfaces to Slint as explicit properties
- Slint host consumes those projected properties instead of carrying its own terminal palette copy

### Why Flat Viewport Backgrounds

Current native presentation only fills `default_bg_rgba`, while bitmap presentation can conceptually honor `row_bg_even/odd`. To eliminate mode drift, Ayu Dark and Ayu Light should use flat viewport backgrounds by setting base / top / bottom to the same final background color for the default preset.

That preserves the one-preset rule and avoids native-vs-bitmap background mismatch without renderer redesign.

## Required Code Paths To Touch

Primary source and projection files:

- `src/theme/spec.rs`
- `src/app/terminal_theme.rs`
- `src/app/bootstrap.rs`
- `src/app/terminal_core/wezterm_adapter.rs`

Terminal-adjacent Slint consumers:

- `ui/theme/tokens.slint`
- `ui/app-window.slint`
- `ui/shell/workspace-pane.slint`
- `ui/shell/terminal-session-host.slint`

Verification targets:

- `tests/terminal_theme_selection_spec.rs`
- `tests/ui_preferences.rs`
- `tests/bootstrap_smoke.rs`
- `tests/native_terminal_surface_contract_spec.rs`
- relevant runtime / terminal interaction tests that assert preset propagation

## Risks And Guardrails

### Main Risks

- partial migration leaves Rust runtime on Ayu while Slint fallback still shows old values
- selection remains split between Rust renderer overlay and Slint host overlay
- scrollbar track remains a token-only constant while thumb comes from Rust
- test helpers with hardcoded colors hide regressions in fallback paths

### Guardrails

- every terminal-neighborhood surface must either come from the preset directly or from a value derived from that preset in one place
- token defaults may remain as boot-time defaults, but active terminal-neighborhood values must be overridden from Rust and tested for parity
- remove or update any test language that still describes the default preset as Catppuccin or Mica Graphite / Canvas

## Success Criteria

The work is successful when:

- `ThemeMode::Dark` visibly reads as `Ayu Dark`
- `ThemeMode::Light` visibly reads as `Ayu Light`
- runtime, fallback, bitmap, native, and Slint host selection all use the same projected preset family
- no user-visible terminal path still shows the old default palette
- the migration lands as a full terminal-theme replacement, not a half-Ayu overlay on top of the previous defaults

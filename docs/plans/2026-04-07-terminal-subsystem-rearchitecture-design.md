# Terminal Subsystem Re-architecture Design

**Status:** Approved for planning

**Goal:** Preserve the existing Slint shell, sidebar, SFTP, assets, and workspace UX while replacing the current terminal subsystem with a simpler, faster, single-path design that is ready for Catppuccin themes and full 24-bit color.

## Why This Change Exists

The current repository does not use a complete upstream terminal application stack. Instead, it combines:

- `wezterm-term` as the terminal emulation/state engine
- `termwiz` for input encoding
- custom projection into `TerminalSurfaceState`
- custom frame modeling and hashing
- custom shaping and glyph preparation
- custom GPU preparation and atlas handling
- custom native-surface replay and Slint host synchronization

This creates too many boundaries in the hot path for scrolling, painting, shaping, and present scheduling. The observed symptom is poor smoothness and elevated CPU usage during viewport-heavy interactions such as wheel scrolling and scrollbar dragging.

## Current Diagnosis

The likely architecture-level cost is in the display chain, not just the parser/core:

- `src/app/ssh/runtime/terminal.rs` snapshots visible rows/cells into `TerminalSurfaceState`
- `src/app/terminal_model.rs` rebuilds row/cell models and hashes them
- `src/app/terminal_presenter.rs` reshapes and repackages frames for multiple presenter variants
- `src/app/terminal_renderer/wgpu_renderer.rs` prepares glyph/background draws
- `src/app/terminal_renderer/native_surface.rs` replays retained native content after host redraws

Even after removing one obvious full-workspace scroll refresh path, the subsystem still pays too much per viewport change because the render architecture remains layered and redundant.

## Decision Summary

### Keep

- Slint shell and window host
- sidebar, workspace tabs, SFTP, asset systems
- current shell/theme mode model (`Dark` / `Light`)
- terminal feature set expectations: SSH, selection, scrollback, mouse, paste, IME, hyperlinks, shell integration

### Replace

- terminal subsystem internals
- render/presenter architecture
- terminal core adapter boundary

### Recommended Direction

- **Primary terminal core direction:** `Alacritty`-style core boundary
- **Primary render architecture influence:** `Rio`-style simplification and single render path
- **Minimalism/performance reference:** `foot`
- **Theme target:** `Catppuccin Mocha` for dark mode and `Catppuccin Latte` for light mode

## Why Alacritty Over Rio For The Core

`Rio` is an excellent architecture reference for a modern GPU terminal, but it is closer to a complete terminal product than an embeddable subsystem. This repository needs to preserve a substantial existing desktop shell around the terminal.

`Alacritty` is the better fit for the terminal core direction because it cleanly separates terminal functionality into a reusable library boundary. That maps better onto a migration where the app shell remains intact and only the terminal subsystem is replaced.

The practical recommendation is:

- do **not** turn the entire app into Rio
- do borrow Rio's render-path discipline
- do move the terminal core toward an `Alacritty`-style adapter boundary

## Target Architecture

### 1. TerminalCoreAdapter

The core layer owns:

- ANSI/VT parsing
- scrollback
- cursor state
- selection state
- mouse reporting
- paste handling
- shell integration signals
- truecolor palette application

The rest of the app must not know whether the active implementation is backed by `wezterm-term` or a future `alacritty_terminal` adapter.

### 2. TerminalFrameSnapshot

The app-facing terminal output becomes a compact snapshot contract that contains only what the renderer and UI need:

- visible viewport rows
- style spans / runs
- cursor data
- selection rectangles
- dirty regions
- hyperlinks and semantic overlays
- IME preview state
- palette values

This replaces the current heavier model/projection chain as the primary renderer input.

### 3. TerminalRendererHost

The renderer host becomes the single display path for terminal content.

Responsibilities:

- convert `TerminalFrameSnapshot` into GPU draw data
- manage glyph cache and texture cache
- expose a single host-facing present contract
- keep scrolling and incremental updates on a surface-only path

Non-goals:

- no long-term parallel bitmap/native/multi-presenter product paths
- no repeated frame repackaging across multiple abstraction layers

### 4. Shell Integration Layer

The Slint shell remains responsible for:

- layout
- panel chrome
- sidebars
- SFTP panes
- asset workflows
- shell-level theme mode

But terminal content rendering is treated as one bounded subsystem rather than a set of intertwined helpers spread across bootstrap, presenter, and platform surface code.

## Migration Strategy

### Phase 0: Baseline

Before deep refactoring, establish reproducible performance and behavior baselines for:

- scrollbar thumb drag
- wheel scrolling
- rapid terminal output
- text selection drag
- window resize
- first paint / first text
- CPU and frame-time consistency

### Phase 1: Single Render Path

Keep the current emulation engine temporarily, but replace the current multi-path presentation structure with a single renderer host contract. This is the first major simplification and should happen before the core swap.

### Phase 2: Core Boundary Extraction

Wrap the current `wezterm-term` integration behind a `TerminalCoreAdapter` trait and remove direct UI dependencies on its concrete types.

### Phase 3: Introduce Alacritty-Style Core

Add a second adapter implementation using `alacritty_terminal`-style contracts, initially behind a feature flag or runtime switch. Run parity checks against the same interaction suite before making it default.

### Phase 4: Theme And Visual Integration

Switch terminal palette presets to Catppuccin-backed definitions and synchronize shell-adjacent tokens so the terminal no longer feels visually detached from the surrounding Slint chrome.

## Theme, Light/Dark Mode, And 24-Bit Color

### Theme Mapping

The first theme mapping should be simple and stable:

- `Dark` -> `Catppuccin Mocha`
- `Light` -> `Catppuccin Latte`

Do not add `Frappé` / `Macchiato` in the first migration slice.

### Theme Scope

The new palette model must cover:

- terminal foreground/background
- full ANSI and bright ANSI sets
- cursor colors
- selection colors
- underline/search/link emphasis colors
- terminal-adjacent shell tokens such as terminal scrollbar/thumb and terminal host background

### Truecolor Requirement

Truecolor is mandatory, not optional:

- keep `COLORTERM=truecolor`
- preserve RGB SGR handling
- render using actual RGBA colors, not 256-color approximation

## Compatibility Boundaries

### Must Stay Correct

- scrollback behavior
- alternate screen
- selection and copy
- cursor shape/visibility
- mouse reporting
- bracketed paste
- IME preview
- CJK and emoji layout
- resize behavior
- shell integration hooks

### Allowed To Follow Later

- advanced shader effects
- nonessential visual embellishments
- secondary theme variants beyond Mocha/Latte
- noncritical protocol extras

## Risks

### Biggest Technical Risks

- glyph metrics drift causing cursor/selection misalignment
- fallback font differences for CJK, emoji, Nerd Font glyphs
- dirty-region logic still causing near-full redraws during scroll
- hidden coupling to the current `TerminalSurfaceState` contract
- host redraw timing creating visible jitter even after core changes

### Migration Risks

- changing too much at once and losing the ability to bisect regressions
- swapping core before simplifying rendering, hiding the real cause
- theme work ballooning into a full shell redesign

## Success Criteria

### Functional

- existing shell workflows still work
- SSH, SFTP-adjacent terminal usage, scrolling, selection, paste, and resize remain correct

### Visual

- Catppuccin Mocha/Latte correctly map to dark/light
- 24-bit color is preserved
- CJK, emoji, Nerd Font glyphs, cursor, underline, and selection remain stable

### Performance

- scrollbar dragging and wheel scrolling feel clearly smoother than today
- CPU usage during viewport-heavy interactions drops materially
- frame pacing is more consistent
- the system avoids obvious whole-frame or whole-workspace refreshes during terminal-only updates

## Primary Source References

- Rio README: `https://github.com/raphamorim/rio`
- Rio workspace manifest: `https://github.com/raphamorim/rio/blob/main/Cargo.toml`
- Rio site / 24-bit true color: `https://rioterm.com/`
- Alacritty README: `https://github.com/alacritty/alacritty`
- Alacritty app manifest: `https://raw.githubusercontent.com/alacritty/alacritty/master/alacritty/Cargo.toml`
- Alacritty terminal library manifest: `https://raw.githubusercontent.com/alacritty/alacritty/master/alacritty_terminal/Cargo.toml`
- Alacritty config docs: `https://alacritty.org/releases/0.14.0/config-alacritty.html`
- foot README: `https://codeberg.org/dnkl/foot/raw/branch/master/README.md`
- horizon README: `https://github.com/peters/horizon`
- Catppuccin for Alacritty: `https://github.com/catppuccin/alacritty`
- Catppuccin for foot: `https://github.com/catppuccin/foot`


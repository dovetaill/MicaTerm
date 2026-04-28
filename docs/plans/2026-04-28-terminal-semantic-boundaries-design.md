# Terminal Semantic Boundary Repair Design

**Date:** 2026-04-28

## Goal

Repair terminal semantic highlighting so it stays within reliable boundaries: keep input highlighting for real shell command lines, dramatically reduce output recoloring, and prevent Codex/TUI/app-mode content from being polluted by transcript-oriented heuristics.

## Problem Statement

The current semantic pipeline treats too much terminal text as if it were a plain shell transcript. That creates three classes of failures:

- transcript heuristics leak into Codex/TUI/app-mode output, so ordinary prose, headings, lists, and Chinese text are recolored without stable meaning,
- ordinary shell output is over-processed by aggressive keyword and block rules, so natural language, file listings, and explanatory logs are highlighted even when the product does not have reliable semantic truth,
- prompt/input/output responsibilities are blended together, so prompt guessing, command-block inference, output phrase detection, and style projection all amplify each other's mistakes.

The net result is worse readability, weaker ANSI fidelity, and a high-maintenance semantic system that diverges from mature terminal products.

## Chosen Approach

Adopt a conservative layered model that matches industry practice:

1. introduce lightweight terminal presentation modes so shell transcript, inline interactive app, and alternate-screen TUI are handled differently,
2. thread shell integration truth (`OSC 133` / `OSC 7`) into the runtime surface and terminal model so prompt/input/output detection can use real markers before falling back to text guesses,
3. restrict input highlighting to the live shell input region only, with a much narrower prompt fallback,
4. reduce output highlighting to low-risk, high-value detection only: URL, file path, line reference, network endpoint, and search matches,
5. prefer ANSI/native colors over semantic recoloring and disable transcript-oriented semantic layers in app/TUI modes.

This restores correctness first. It does not attempt to invent a universal output syntax highlighter.

## Alternatives Considered

### Option A — Tune colors and keep the existing heuristics

Pros:

- smallest code delta,
- preserves current feature list,
- no new state needs to flow through the runtime.

Cons:

- does not fix the actual boundary problem,
- leaves prompt inference and output inference coupled,
- still misclassifies TUI/app content and ordinary prose.

Rejected because the failure is structural, not cosmetic.

### Option B — Build a richer semantic engine with more transcript rules

Pros:

- could look impressive on carefully curated output,
- offers room for future status and block analysis.

Cons:

- increases false positives,
- duplicates work mature terminals avoid,
- requires a reliable transcript segment store the current presenter/model layers do not yet provide,
- raises performance and invalidation complexity before correctness is restored.

Rejected because it expands the exact direction that is already harming usability.

### Option C — Conservative boundary repair with shell-integration truth and smaller rules

Pros:

- aligns with VS Code, WezTerm, Windows Terminal, xterm.js link handling, and shell-native input highlighters,
- materially reduces false positives,
- keeps ANSI as the truth source,
- is realistic to ship with minimal structural change.

Cons:

- intentionally removes some current decorative behavior,
- command status markers become less ambitious until stronger transcript truth exists.

Chosen because it restores reliability with the least risky architecture change.

## Industry References

The external review confirmed that mature terminals do not generally perform broad semantic recoloring over arbitrary command output. Instead they combine shell integration markers, prompt/input/output boundaries, native ANSI colors, and narrowly scoped link/path detection.

References:

- VS Code Terminal Shell Integration — `https://code.visualstudio.com/docs/terminal/shell-integration`
- WezTerm Shell Integration — `https://wezterm.org/shell-integration.html`
- WezTerm Semantic Zones — `https://wezterm.org/config/lua/pane/get_semantic_zones.html`
- Windows Terminal Shell Integration / OSC 133 — `https://learn.microsoft.com/en-us/windows/terminal/tutorials/shell-integration`
- xterm.js Link Handling — `https://xtermjs.org/docs/guides/link-handling/`
- VS Code terminal link parsing implementation — `https://github.com/microsoft/vscode/blob/main/src/vs/workbench/contrib/terminalContrib/links/browser/terminalLinkParsing.ts`
- zsh-syntax-highlighting — `https://github.com/zsh-users/zsh-syntax-highlighting/blob/master/docs/highlighters/main.md`
- fish interactive highlighting — `https://fishshell.com/docs/current/interactive.html`

## Architecture

### 1. Add lightweight presentation modes

Add a minimal presentation classification to the terminal model/semantic layer:

- `ShellLive`
- `ShellScrollback`
- `InlineInteractiveApp`
- `AlternateScreenTui`

This is intentionally thin. It is not a new retained transcript engine.

Classification should prefer stable runtime signals:

- `alternate_screen_active` => `AlternateScreenTui`
- `mouse_grabbed || application_cursor_keys` => `InlineInteractiveApp`
- otherwise `viewport_at_bottom` => `ShellLive`
- otherwise => `ShellScrollback`

The goal is simple policy gating, not perfect app detection.

### 2. Carry shell-integration truth into the model

The runtime already parses:

- `OSC 7` current directory,
- `OSC 133;A` prompt start,
- `OSC 133;B` prompt end,
- `OSC 133;C` command start,
- `OSC 133;D[;code]` command finished.

Today only current directory is consumed at runtime. The repair should propagate the rest into `TerminalSurfaceState` / `TerminalModelFrame` so semantic analysis can tell whether the visible bottom region is actually inside prompt/input/output boundaries.

The first pass does not need a full history store. It only needs enough surface-level state to gate prompt/input inference and disable command-status guessing when semantic truth is absent.

### 3. Separate input highlighting from output highlighting

Input highlighting should apply only when all of these are true:

- presentation mode is `ShellLive`,
- viewport is at bottom,
- not alternate screen,
- not inline interactive app,
- the candidate row is the active input row.

Prompt fallback should become much stricter:

- keep shell-like prompt markers that are low-risk for transcript rows,
- remove `# ` and `> ` from the generic fallback set,
- only allow fallback on the bottom live row rather than any visible row.

This moves input highlighting closer to shell-native highlighters such as `zsh-syntax-highlighting` and fish: highlight command input, not command output.

### 4. Make output highlighting conservative by default

Retain only stable output enhancements:

- URL,
- Unix path,
- Windows path,
- `file:line[:column]`,
- network endpoint,
- explicit search matches.

Remove transcript-wide semantic recoloring for:

- generic success keywords,
- generic failure keywords like bare `failed` / `fatal`,
- `INFO` / `DEBUG`,
- diff whole-line recoloring,
- JSON/XML/log semantic recoloring overlays.

These rules are too eager for general terminal output and are the main source of natural-language false positives.

### 5. Keep command decorations behind stronger truth and stricter defaults

Command blocks and overview markers should stop inferring status from generic lexical failure signals. In the first repair pass:

- do not derive failure/success from output phrases,
- only expose command decorations when a trustworthy prompt/input boundary exists,
- default overview markers off,
- default output profile to `Focused` so only the conservative output roles remain active.

This keeps the feature available without letting it pollute ordinary output.

### 6. Preserve ANSI precedence

Semantic recoloring must continue to respect explicit ANSI foreground/background values. In app/TUI modes, the pipeline should effectively behave as ANSI-first with at most non-invasive link/search enhancements.

### 7. Separate source-frame damage tracking from styled-frame reuse

The presenter currently feeds a styled previous frame back into the next raw frame diff path. That mixes source truth with presentation output and can keep semantic invalidation alive longer than intended.

Split presenter retention into:

- previous source frame for content damage calculation,
- previous styled frame for renderer/style reuse.

This keeps semantic policies from contaminating future raw diff decisions.

## Data Flow

1. SSH runtime parses shell integration sequences and updates a richer terminal surface state.
2. `TerminalModelFrame::from_surface` derives a raw frame plus presentation mode.
3. Semantic analysis reads the raw frame, presentation mode, and shell-integration hints.
4. Conservative spans/overlays are generated according to policy.
5. Semantic style projection recolors only default-colored cells.
6. Presenter caches store source and styled frames separately.

## Testing Strategy

The repair must be locked with regression tests before implementation:

1. mode separation contract
   - shell transcript vs inline interactive app vs alt-screen TUI use different semantic policies.
2. prompt/input/output separation contract
   - input highlighting only affects input,
   - transcript prose/output does not get reinterpreted as input.
3. conservative output contract
   - URL/path/file-location survive,
   - natural language, headings, lists, and generic logs do not get semantic recolor.
4. ANSI precedence contract
   - ANSI colors remain truth,
   - app/TUI modes do not get transcript recolor overlays.
5. presenter retention contract
   - source-frame diffing does not reuse styled previous-frame hashes.

## Scope Boundaries

This repair intentionally does not:

- add more output regex rules,
- attempt full transcript segmentation across scrollback history,
- add a general-purpose output syntax highlighter,
- redesign theme colors.

The objective is smaller, safer, and more explainable highlighting.

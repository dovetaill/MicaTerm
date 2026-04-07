# Terminal Subsystem Completion Audit And Corrective Design

**Status:** Drafted from repository reality after packaged Windows regression

**Goal:** Separate what is truly implemented from what is only structural, experimental, or documented, then define the remaining work required to make the terminal subsystem actually shippable in packaged Windows builds.

## Why This Document Exists

The existing `2026-04-07-terminal-subsystem-rearchitecture-design.md` and matching implementation plan were executed as if the migration were complete. The repository does contain a series of commits matching the planned task names, but the current shipped behavior still shows a blank terminal region in packaged Windows builds and the "new subsystem" is not actually fully switched in.

This corrective design exists to stop mixing together four different states:

- truly landed behavior
- structural refactors that are real but not user-visible
- experimental seams that are not production implementations
- design intent that was written down but never fully shipped

## Repository Reality Snapshot

### Terminal Core

- `wezterm-term` is still present in dependencies and still drives the default terminal core.
- `TerminalSession::new()` still defaults to `TerminalCoreKind::Wezterm`.
- the "Alacritty" path is still an experimental adapter seam, but it now binds to a real `alacritty_terminal` core instead of wrapping the WezTerm adapter internally.
- `alacritty_terminal` is now present in `Cargo.toml`, but only behind the experimental core path and not the shipped default runtime.

### Rendering / Presentation

- `TerminalRendererHost` is real and is now the single bootstrap-facing rendering seam.
- presenter variants still exist underneath that host:
  - bitmap atlas presenter
  - Windows scene-image presenter
  - Windows native presenter
- the packaged Windows mainline wrapper currently pins the terminal subsystem back to `scene-image`.
- the retained native surface path still exists as an explicit bring-up switch, not the default shipped path.

### Theme State

- Catppuccin Mocha and Latte palette values do exist in code.
- terminal-adjacent shell tokens also exist in code.
- however, a render failure can leave only the background visible, which makes the package appear as if "theme was not changed" even though the terminal background token is being applied.

### Packaged Runtime Gap

- bootstrap tests do not exercise the real packaged presenter path.
- under `#[cfg(test)]`, `ensure_workspace_terminal_presenter(...)` always installs `BitmapAtlasPresenter`.
- real packaged Windows can therefore fail while bootstrap tests still pass.
- on presenter failure during `present_surface_update(...)`, runtime code currently logs the error and clears the terminal frame, which leaves a blank terminal region.

## Audit Of The Existing 2026-04-07 Plan

### Task 1: Freeze Behavior And Performance Contracts

**Actual state:** Partially landed, but incomplete as shipped protection.

What is real:

- the contract/spec files exist
- source-level assertions for terminal-local refresh routing exist
- perf- and seam-oriented tests exist

What is missing:

- no test here proves packaged Windows presentation works
- no test here catches presenter failure leaving the UI blank
- the contract coverage is stronger at source-structure level than at real runtime behavior level

### Task 2: Introduce A Terminal Core Adapter Boundary

**Actual state:** Landed.

What is real:

- `src/app/terminal_core/mod.rs`
- `src/app/terminal_core/types.rs`
- `src/app/terminal_core/wezterm_adapter.rs`
- runtime now depends on a `dyn TerminalCoreAdapter`

What is missing:

- nothing essential for the seam itself; this task is genuinely done

### Task 3: Replace Multi-Path Presenter Wiring With A Single Renderer Host Contract

**Actual state:** Structurally landed, behaviorally partial.

What is real:

- `src/app/terminal_renderer/host.rs` exists
- bootstrap routes terminal presentation through `TerminalRendererHost`

What is missing:

- the system still has multiple presenter implementations underneath the host
- this is not yet a true single render path in product terms
- the host does not yet downgrade/retry automatically when the active presenter fails at runtime

### Task 4: Keep Terminal-Only Updates On A Surface-Local Path

**Actual state:** Mostly landed as an architectural cleanup, but not sufficient for shipped correctness.

What is real:

- terminal-local refresh work was narrowed
- scroll/theme tests for active session surface refresh exist

What is missing:

- packaged runtime verification still was not part of the acceptance gate
- local refresh optimization does not help if the active presenter fails and leaves no frame visible

### Task 5: Add Catppuccin-Backed Terminal Theme Presets And Shell Token Sync

**Actual state:** Landed in code, incomplete in user-visible packaged verification.

What is real:

- `src/theme/spec.rs` maps dark mode to Catppuccin Mocha
- `src/theme/spec.rs` maps light mode to Catppuccin Latte
- `ui/theme/tokens.slint` exposes terminal foreground/background/cursor/selection tokens

What is missing:

- no acceptance gate proves these tokens remain visibly correct in packaged fallback/error paths
- because the package can render a blank terminal region, users only see the background and not the intended completed terminal presentation

### Task 6: Introduce An Experimental Alacritty-Style Core Adapter

**Actual state:** Experimental real-core adapter landed, but still not the shipped default.

What is real:

- feature flag exists
- `src/app/terminal_core/alacritty_adapter.rs` exists
- real `alacritty_terminal` state now sits behind that adapter
- parity tests exist

What is missing:

- the shipped default core is still WezTerm-backed
- packaged Windows verification does not yet justify flipping the default
- this task is not a real core migration and must not be described as one

### Task 7: Switch The Default Terminal Subsystem And Retire Legacy Paths

**Actual state:** Not complete in the shipped repository state.

What is real:

- there was a commit attempting to switch the default path
- rollback/override plumbing exists

What is missing:

- packaged Windows mainline currently defaults back to `scene-image`
- retained native surface is still opt-in
- legacy WezTerm-based core is still active
- old presenter paths are not retired

This task must be considered incomplete.

## What Went Wrong

### 1. Structural completion was treated as shipped completion

Several tasks were completed as code-shape refactors or contract scaffolding, but the overall migration was described too confidently as if the new subsystem had already become the real packaged product path.

### 2. The plan jumped from seam work to "default switched" too early

The earlier plan assumed that once adapter seams, host seams, and theme presets existed, it was appropriate to flip the default terminal subsystem. In reality, packaged Windows rendering still needed direct runtime validation.

### 3. Tests masked the real packaged failure mode

The test-only presenter installation path always used the bitmap atlas presenter. That meant tests did not exercise the same presenter selection and failure handling path that packaged Windows uses.

### 4. Presenter failure handling is not robust enough

When presenter rendering fails at runtime, the app currently clears the terminal image and leaves the region blank. There is no automatic swap to a safer presenter and no retry on the same frame.

## Corrective Design Decision

The repository should stop treating this as one finished migration and instead split the work into two explicit tracks.

### Track A: Ship-Stopper Closure On The Current Core

This is the immediate work required to make packaged Windows builds behave correctly.

Keep:

- the current WezTerm-backed terminal core
- the current `TerminalRendererHost`
- the current scene-image and native presenter implementations
- rollback and feature-flag controls

Add:

- real presenter-failure downgrade and retry behavior
- tests that can exercise the real presenter path instead of the test-only bitmap shortcut
- packaged-runtime diagnostics that make fallback selection visible and debuggable
- end-to-end theme verification for fallback and no-frame states

Do not:

- claim Alacritty migration is done
- claim Rio code was ported
- flip defaults again until the packaged verification matrix passes

### Track B: Honest Future Core Migration

Only after Track A is stable should the repository resume the longer migration.

That future track must include:

- a real `alacritty_terminal`-backed adapter, not a wrapper around WezTerm
- parity tests against the same interaction suite
- explicit performance comparison against the WezTerm control path
- a staged default switch with rollback preserved

Rio remains an architectural reference, not a source of transplanted runtime code.

## Corrected Target Architecture

### Short-Term Shipped Architecture

- terminal core: `wezterm-term`
- input encoding: `termwiz`
- bootstrap-facing render seam: `TerminalRendererHost`
- packaged Windows default subsystem: `scene-image`
- retained native surface: explicit opt-in only
- presenter failure policy: downgrade to bitmap-compatible output before ever leaving the terminal blank

### Medium-Term Migration Architecture

- terminal core boundary remains `TerminalCoreAdapter`
- second adapter becomes real Alacritty-based implementation
- presenter stack can be simplified further only after packaged correctness is proven
- default switch happens after parity, performance, and packaged Windows validation all pass

## Acceptance Criteria

### Immediate Acceptance Criteria

- packaged Windows build from `./build-win-x64.sh` shows live terminal text, not a blank region
- if the preferred presenter fails, runtime automatically falls back instead of clearing the view permanently
- theme mode changes keep terminal colors coherent in normal, fallback, and no-frame states
- logs and tests make the selected presenter/subsystem/fallback path explicit
- fast upward scroll through deep history does not force a visibly stuttery full terminal rerender on every tick
- dense Chinese and mixed CJK/Latin terminal text reads with looser vertical rhythm and more obviously native Windows text treatment
- Catppuccin is visible not only in the terminal palette contract but also in terminal-adjacent shell chrome such as scrollbars, paused-follow affordances, and fallback/no-frame states

### Deferred Acceptance Criteria

- WezTerm is no longer the default core
- a real Alacritty implementation exists
- old presenter paths are retired from the shipped mainline path

Those are not done today and should remain marked as future work.

## Non-Goals For The Corrective Phase

- no claim that Rio code is imported
- no deletion of WezTerm during packaged runtime stabilization
- no second attempt to flip the default subsystem during the same change set that fixes the blank terminal bug
- no broad renderer rewrite before the current packaged path is hardened

## Risks

- presenter fallback could hide a deeper native presenter bug if diagnostics are not recorded
- test seams could become too synthetic again if they do not mirror packaged bootstrap closely enough
- theme verification could still look correct in tests while failing on packaged Windows if the visual state is not projected through the real host path
- trying to resume Alacritty migration before packaged closure is complete would repeat the same mistake
- scroll perf work could target only event throttling and miss the real hot path inside row shaping / prepared-frame generation
- typography polish could regress glyph fit, emoji fallback, or Nerd Font baseline alignment if metrics changes are not verified against the existing renderer contracts
- stronger Catppuccin shell expression could drift away from the terminal preset values if shell tokens are not sourced from the same palette model

## Final Position

The honest state of the repository is:

- adapter seam: real
- renderer host seam: real
- Catppuccin palette definitions: real
- packaged ship-ready terminal subsystem migration: not complete
- Alacritty core migration: not complete
- Rio code migration: not present

The next plan must optimize for real packaged behavior first, not for preserving the appearance of plan completion.

## Remaining Shipped Blockers After The First Corrective Pass

The blank-terminal regression is no longer the only blocker. The next corrective phase must close three user-visible gaps before any honest default-switch discussion can resume.

### 1. Fast Scrollback Performance

Current packaged `scene-image` rendering still feels heavy when the user drags the scrollbar or spins the wheel upward through large histories. The current debounce and surface-local projection seams are real, but the hot path still reshapes and re-prepares the visible frame too aggressively.

The corrective direction is:

- reuse row-shaping work across adjacent viewport shifts
- keep scroll refreshes on the terminal-local path
- avoid pretending that event debouncing alone solves the packaged runtime cost

### 2. Windows Typography Polish

The text is now serviceable, but still not at the level of Windows Terminal / WezTerm polish. The remaining gap is not "whether DirectWrite exists" but how the current scene-image/native presenters tune line height, fallback faces, and dense CJK readability.

The corrective direction is:

- preserve the existing DirectWrite ownership
- tune font metrics and line rhythm with explicit tests
- verify mixed Chinese / Latin / Nerd Font rows instead of relying on screenshots alone

### 3. Catppuccin Expression In Shell Chrome

Catppuccin palette values already exist, but the packaged UI still does not strongly communicate the theme outside the raw terminal fg/bg contract. The corrective phase needs to project the palette through terminal-adjacent chrome so the theme reads as intentional rather than incidental.

The corrective direction is:

- keep terminal and shell tokens sourced from the same preset model
- verify scrollbar, paused-follow, fallback, and no-frame states explicitly
- avoid claiming a "full redesign" when this is really a consistency pass over the shipped shell chrome

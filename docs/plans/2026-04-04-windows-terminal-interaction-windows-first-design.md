# Windows-First Terminal Interaction and Latency Design

## Context

The current terminal stack has already moved to a grid-first glyph placement model and fixed the bitmap host selection overlay coordinate-space bug. The remaining quality gaps now cluster into three Windows-facing experience problems:

1. Selection and hit-testing still use a simple `floor(pointer / cell_size)` mapping, which makes drag boundaries feel soft and can leave wide-character trailing cells visually inconsistent with copy/selection semantics.
2. Held-key input still feels sticky because the visible echo path is gated by a `40ms` runtime dirty flush and a `50ms` UI projection poll, even after removing the worst per-keystroke full projection refresh.
3. Text quality is much better than before, but Windows still needs a finishing pass around baseline, line-height, and selection/cursor shaping boundaries to feel closer to mature terminals like Windows Terminal, WezTerm, Ghostty, and xterm.js.

This design is intentionally **Windows-first**. Linux-hosted Windows packaging and Windows-hosted Windows packaging should keep producing the same Windows-target runtime profile; the priority is the runtime experience on Windows, not keeping Linux-native rendering visually identical.

## Goals

- Make pointer hit-testing and drag selection feel cell-accurate on Windows, especially around CJK/fullwidth characters.
- Reduce perceived held-key latency so repeated input feels continuous instead of bursty.
- Improve Windows text polish without destabilizing the newly-correct grid-first renderer.
- Preserve copy/paste correctness, existing packaged Windows runtime profile, and current fallback paths.

## Non-Goals

- Re-architecting the entire renderer in one pass.
- Forcing Linux-native rendering to match Windows pixel-for-pixel.
- Implementing optimistic local echo before measured low-risk latency reductions are exhausted.
- Replacing the current native/bitmap fallback strategy.

## External Patterns Driving the Design

- `xterm.js` and `Kitty` use half-cell-style hit-testing so pointer decisions are based on cell edges rather than raw `floor()` of the left edge.
- `xterm.js`, `Ghostty`, `Alacritty`, and `Kitty` normalize wide-character trailing cells back to the leading cell in hit-testing and selection semantics.
- `Ghostty` and `WezTerm` cut shaping/render runs at selection and cursor boundaries so visual highlighting remains aligned even when glyph shaping spans multiple cells.
- `xterm.js` and other mature terminals treat responsiveness as a flow-control/timing problem, not just a renderer problem: they reduce callback work and avoid long synchronous GUI hot paths under high throughput.

## Chosen Approach

### 1. Unify pointer hit-testing around a Windows-first cell-edge model

Introduce a dedicated hit-test result model derived from local pointer coordinates:

- use half-cell semantics on the X axis so the right half of a cell maps to the next logical column when appropriate;
- clamp all hit-tested positions into the visible grid;
- normalize wide-character trailing cells back to the leading cell before selection/copy/mouse reporting consume the result;
- keep the selection overlay row-span geometry grid-first and fed from the normalized cell range.

This should make selection start/end behavior feel more like xterm.js/Ghostty without destabilizing the renderer itself.

### 2. Add an input-active fast path for visible terminal projection

Keep the existing stable snapshot path, but stop forcing held-key UX to wait on the current `40ms + 50ms` cadence during active input bursts.

The design target is:

- normal idle output keeps today's conservative batching;
- active local keyboard input temporarily arms a faster refresh lane with a shorter budget (targeting one frame-ish cadence instead of 40/50ms stacking);
- repeated input coalesces rather than causing one heavy refresh per event;
- all timings become instrumented so we can observe whether the bottleneck is still command writeback, runtime dirty batching, UI projection polling, or native present/shaping.

This is the lowest-risk way to improve held-key responsiveness before considering optimistic local echo.

### 3. Finish with a Windows-only text polish pass

Once hit-testing and latency stop masking the experience, finish the Windows path by tightening:

- line-height and baseline tuning for dense Chinese text blocks;
- selection/cursor boundary run splitting where shaping spans multiple cells;
- diagnostics for `text_renderer_path`, `text_antialias_mode`, baseline, pixel alignment, and DPI so Windows screenshots can be correlated with actual runtime text path decisions.

This pass is intentionally last, because tuning text while input and selection still feel wrong would produce noisy feedback.

## Risks and Mitigations

### Risk: hit-testing changes break copy or mouse reporting

Mitigation:
- add regression tests for ASCII, wide-char, and wide-char-trailing hit results;
- keep normalized hit results as the single source of truth for selection state and host-side copy requests.

### Risk: fast refresh path reintroduces UI thrash

Mitigation:
- coalesce active-input refresh requests behind a gate/timer;
- measure counts and durations before shrinking more aggressively;
- keep the existing conservative path as fallback when no input burst is active.

### Risk: text polish regresses already-fixed glyph placement

Mitigation:
- no glyph-origin offset patches;
- only allow grid-first cell anchoring plus run-splitting or metric tuning;
- require targeted renderer tests and packaged Windows verification after each text-quality change.

## Testing Strategy

### Selection / Hit-testing
- source-contract tests for the Slint hit-test helpers and selection overlay contract;
- Rust tests for normalized selection bounds around wide characters and trailing spacers;
- interaction smoke tests that drag-select across ASCII + CJK boundaries.

### Input latency
- unit tests for input-active refresh gating/coalescing behavior;
- runtime/session-manager tests for dirty notification cadence and projection wakeups;
- targeted package verification on Windows using held-key input and screenshot/log capture.

### Windows text polish
- renderer tests around baseline, line-height, selection/cursor boundary splitting;
- Windows packaged verification across 100/125/150% DPI and 12/13/14/15 px sizes;
- diagnostics log inspection to confirm actual text path (`directwrite-d2d` vs fallback) during comparisons.

## Rollout Order

1. Half-cell + wide-char trailing-cell normalization.
2. Input-active low-latency projection path and instrumentation.
3. Windows text polish and diagnostics improvements.

## Expected Outcome

After these three phases, Windows should feel better in the order users actually notice it:

- selection starts and ends on the cell users expect,
- held-key input no longer feels like the app stalls and then dumps a burst,
- text presentation gets the final quality lift without relying on fragile offset hacks.

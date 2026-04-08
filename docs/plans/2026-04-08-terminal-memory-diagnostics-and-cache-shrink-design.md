# Terminal Memory Diagnostics and Cache Shrink Design

## Goal

Make Windows terminal memory behavior explainable and easier to verify in the field, then reduce the specific class of retained terminal-renderer memory that stays elevated after scroll-heavy sessions are closed.

## Problem Summary

Two different symptoms are currently mixed together when looking at Windows Task Manager:

1. After a large `cat` burst, working set can jump up and then collapse to a very small number.
2. After `history` plus heavy scrolling across multiple sessions, memory rises and does not fall back after closing those sessions.

These do not appear to share the same root cause.

### Confirmed behavior from the current code

- Large post-connect output is tracked by the SSH channel pump.
- After `2s` of idle and at least `1 MiB` of sanitized output, Windows builds call `K32EmptyWorkingSet(GetCurrentProcess())`.
- That explains the sudden `~160 MB -> ~0.4 MB` drop: it is a working-set eviction event, not proof that all app-side state was actually freed.
- Startup does not currently trigger the same trim path, so a cold launch that stays idle should not auto-fall through the existing mechanism.
- Scroll and render paths now intentionally retain several caches for performance:
  - presenter shaped-row LRU
  - renderer glyph raster caches
  - scene-image bitmap/base-frame caches
  - previous-frame prepared row caches
- Session/tab close removes session/runtime registry state, but does not currently reset the global workspace terminal presenter host.

## User-Facing Questions This Design Must Answer

1. Why is startup memory visibly higher than later steady-state memory?
2. Does startup memory fall on its own if the user does nothing?
3. What exactly causes the large-output trim event?
4. Which retained caches survive scroll-heavy use and remain after closing sessions?
5. Can we shrink retained caches on close and idle without regressing terminal smoothness?

## Constraints

- Keep default behavior quiet. Diagnostics must be opt-in.
- Preserve current terminal scroll optimizations unless diagnostics prove a cache is oversized or lifetime is wrong.
- Avoid making startup numbers look better by relying only on `EmptyWorkingSet`; prefer shrinking real retained state first.
- Keep the packaged Windows mainline path aligned with the current `scene-image` subsystem default.
- Make field reproduction easy for the user: one environment variable, one known log location.

## Design Principles

### 1. Separate working-set behavior from retained-state behavior

Task Manager RSS/working set is useful, but it is not enough on its own:

- `EmptyWorkingSet` can make memory look dramatically lower without actually destroying caches.
- Subsequent render/snapshot access can fault the same pages back in.

So the app must log both:

- when the OS working-set trim path fires
- what terminal caches the process still retains before and after that event

### 2. Prefer observable cache shrink over blind process trim

If startup or post-close memory is too high, the preferred fix is:

- identify which terminal caches are still retained
- shrink or clear them at safe lifecycle boundaries

Only after that should we consider extra startup/idle working-set trim, and even then it should be deliberate and well-labeled in logs.

### 3. Keep diagnostics cheap and explicit

Diagnostics should be enabled only when the user opts in:

- `MICA_TERM_LOG=debug`
- `MICA_TERM_MEMORY_DIAGNOSTICS=1`

When disabled, there should be no noisy runtime memory logging.

## Proposed Approach

## Part A: Add opt-in terminal memory diagnostics

### Activation

Windows packaged reproduction flow:

```powershell
cd .\dist\mica-term-x86_64-pc-windows-msvc-release-skia
ni .mica-term-portable -ItemType File -Force
$env:MICA_TERM_LOG = "debug"
$env:MICA_TERM_MEMORY_DIAGNOSTICS = "1"
.\mica-term.exe
```

Log destination stays consistent with the existing logging system:

- portable mode: `logs/system-error.log.YYYY-MM-DD`
- standard mode: `%LOCALAPPDATA%\MicaTerm\MicaTerm\logs\`

### Diagnostic event families

Emit structured debug logs under a dedicated target such as `app.memory` or `app.terminal.memory` for:

- `startup-snapshot`
  - runtime profile
  - terminal subsystem mode
  - active presenter render mode
  - cache sizes if presenter already exists
- `surface-refresh`
  - session id
  - seqno
  - visible rows
  - viewport offset
  - cache sizes before/after refresh
- `scroll-snapshot`
  - session id
  - viewport movement
  - shaped-row cache size/capacity
  - glyph raster cache size
  - scene-image bitmap cache footprint
- `trim-request`
  - bytes accumulated since last idle window
  - whether trim threshold was met
  - trigger reason (`large-output-idle`)
- `trim-executed`
  - success/failure
  - cache snapshot around the trim
- `close-session`
  - session id
  - active presenter state before and after close-driven shrink
- `idle-shrink`
  - trigger reason (`no-active-surface-idle`)
  - presenter/cache snapshot before and after shrink

### Cache metrics to expose

Add lightweight introspection methods, not expensive deep dumps:

- presenter shaped-row cache length and capacity
- previous-frame row count
- renderer glyph raster cache entry count
- renderer prepared-row cache entry count
- scene-image mono/color glyph cache entry count
- scene-image retained pixel-buffer dimensions / byte counts where available

The goal is to explain trends, not to produce heap-profiler-grade dumps.

## Part B: Add real cache shrink hooks

### Close/session-empty shrink

When the last active workspace terminal surface disappears after tab/session close:

- keep UI state reset behavior as-is
- also clear or shrink terminal presenter caches that no longer provide value

Recommended minimum behavior:

- clear presenter previous-frame state
- clear presenter shaped-row cache
- clear scene-image retained frame/bitmap caches
- clear renderer prepared-row caches
- clear renderer glyph caches if no active workspace session remains

This specifically targets the user report:

- open several sessions
- run `history`
- scroll heavily
- close them
- memory does not fall back

### Idle shrink when no active surface remains

If there is no active workspace terminal surface for a short idle interval, schedule a second-stage shrink:

- shrink caches only when the workspace has no active terminal surface
- avoid firing during active typing, scrolling, or connected rendering
- log the shrink as `idle-shrink`

This is intended to recover memory after interaction-heavy sessions without waiting for process restart.

### Startup behavior

Do not add a blind startup `EmptyWorkingSet` pass as the first fix.

Instead:

1. measure startup cache state
2. verify whether startup eagerly materializes renderer state that is not needed yet
3. if startup memory is mostly retained renderer/presenter state, shrink that state directly

Only if diagnostics later show startup idle still keeps too much unavoidable working set should a startup-idle trim be considered.

## Approaches Considered

### Approach 1: Diagnostics plus lifecycle cache shrink

Pros:

- explains both symptoms
- attacks real retained state
- creates field evidence for future regressions

Cons:

- slightly more implementation work

### Approach 2: Only clear caches on close

Pros:

- smaller change
- likely helps the `history + scroll + close` symptom

Cons:

- does not explain startup vs trim behavior
- leaves large-output trim path opaque

### Approach 3: Add startup automatic `EmptyWorkingSet`

Pros:

- Task Manager number may look smaller quickly

Cons:

- mostly cosmetic
- can hide real retained-state issues
- likely to bounce back once rendering touches memory again

## Recommendation

Implement Approach 1.

That gives us:

- concrete diagnostics the user can run on Windows
- a principled explanation of startup, trim, scroll, and close behavior
- a targeted fix for caches that survive longer than intended

## Testing Strategy

- Add source/contract tests for:
  - new diagnostics env var gating
  - cache stats APIs
  - close/session-empty shrink hooks
  - idle shrink scheduling only when no active surface exists
- Add focused unit tests in presenter/renderer modules for:
  - cache clear/shrink methods
  - stats reporting
- Keep existing startup memory regression and terminal perf contract suites green.

## Success Criteria

- With diagnostics enabled, the user can reproduce startup, `history + scroll`, and large `cat` scenarios and see exactly which path fired.
- Closing all scroll-heavy sessions clears or shrinks terminal caches enough that memory meaningfully falls toward the previous steady-state band.
- Large-output trim remains observable and clearly distinguished from real cache release.
- Startup memory behavior becomes explainable from logs, even if not fully minimized in the first patch.

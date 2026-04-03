# Windows Startup Memory Design

## Goal

Reduce cold-start memory for both Windows packaging paths, `build-win-x64.sh` and
`build-win-x64-software.sh`, without regressing terminal rendering stability or
welcome-screen startup behavior.

## Problem

The app starts on the welcome surface, but bootstrap still eagerly installs the
workspace terminal presenter. In the native renderer build this pulls in the
DirectWrite font system and the terminal renderer before the user opens a
terminal. In the software compatibility build, bootstrap also touches the same
presenter installation path even though the packaged profile is bitmap-first.

That makes startup memory look higher than it needs to be and moves work onto
the first paint path that the user does not need yet.

## Constraints

- Preserve the current welcome-first startup flow.
- Keep both Windows packaging wrappers working:
  - `build-win-x64.sh` -> native terminal renderer path
  - `build-win-x64-software.sh` -> bitmap compatibility path
- Do not reintroduce the blank native terminal surface regression that was fixed
  in the retained frame pipeline.
- Keep cell metrics and native frame token state valid before a terminal session
  exists.

## Approaches Considered

### 1. Lazy-init only the workspace terminal presenter

Create the presenter on first terminal use instead of during bootstrap. Use a
cheap fallback cell size while the app is still on the welcome surface.

Pros:
- Attacks the most obvious startup-only cost.
- Smallest code change and lowest regression risk.
- Applies cleanly to both Windows packaging modes.

Cons:
- First terminal activation may pay a one-time initialization cost.

### 2. Lazy-init the presenter plus session bridge and vault-adjacent services

Move more bootstrap work behind first-use triggers.

Pros:
- Potentially larger startup memory win.

Cons:
- Broad lifecycle changes.
- Higher risk of state sync regressions.
- Harder to validate quickly.

### 3. Tune caches or renderer allocations without changing startup order

Reduce font, glyph, or renderer memory after eager startup.

Pros:
- May reduce peak memory further.

Cons:
- Does not remove unnecessary first-paint work.
- Higher chance of destabilizing rendering code.

## Recommendation

Use approach 1 now.

Lazy-initialize the workspace terminal presenter and keep bootstrap on a
lightweight fallback path until a real terminal surface needs rendering. This
matches common desktop-app practice: keep first paint lean, defer heavy
rendering and font setup until the user reaches that flow, and measure the
result before broadening the optimization.

## Design

### Presenter lifecycle

- Replace eager presenter installation during bootstrap with a lazy holder.
- Store `Option<Box<dyn TerminalPresenter>>` in the workspace presenter slot.
- Add a helper that initializes the presenter on demand when the runtime profile
  actually supports that renderer path.
- For bitmap-only profiles, keep the slot empty and never try to construct the
  native presenter.

### Welcome-state fallback

- Before a presenter exists, publish a stable default cell size to Slint using a
  cheap constant fallback.
- Continue clearing the retained native frame token and native frame payload when
  no active terminal surface exists.

### Render-time activation

- When shell state sync sees an active workspace terminal surface, call the
  lazy-init helper before asking for cell metrics or rendering a frame.
- If initialization fails, log the error, keep the fallback cell metrics, and
  avoid crashing bootstrap or the active session flow.

### Build-path coverage

- `build-win-x64.sh` should still initialize the native presenter when the user
  first enters a terminal.
- `build-win-x64-software.sh` should avoid any startup attempt to construct the
  native presenter and stay on the bitmap compatibility path.

## Testing

- Add a source-level regression test that bootstrap no longer installs the
  workspace terminal presenter eagerly.
- Add a source-level regression test that the presenter slot is optional and is
  initialized through a helper on demand.
- Run targeted Rust tests covering bootstrap/runtime-profile contracts.
- Run both Windows packaging commands to ensure the packaging wrappers still
  build with the lazy-init lifecycle.

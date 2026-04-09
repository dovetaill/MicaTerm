# UI and Terminal Typography Refresh Design

## Problem

The current typography contract still mixes terminal-oriented decisions into the broader desktop shell:

- The terminal default stack is still anchored on `Cascadia Mono` + `Sarasa Term SC`, with `Regular` as the default weight.
- The Slint UI does not define its own explicit global font family, so the app shell and popup chrome do not have a deliberate, product-level type direction.
- The user explicitly wants a mature commercial look, with Chinese support handled intentionally instead of by accidental fallback.

The approved direction is now fixed:

- **UI:** `Sarasa UI SC`
- **Terminal:** `JetBrains Mono` + `Sarasa Term SC`
- **UI default weight:** `Regular`
- **Terminal default weight:** `Medium`
- **Semibold:** emphasis only, not the global default

## Chosen Approach

### 1. Keep UI and terminal typography fully separate

The Slint shell should own UI typography, and the Rust terminal renderer should own terminal typography. We should not reuse the terminal Latin face for the app chrome, and we should not route UI choices through the terminal renderer.

This is the biggest correctness win from the earlier exploration: the shell and the terminal solve different readability problems.

### 2. Use a conservative UI rollout

For the UI, use `Sarasa UI SC` as the explicit default family and keep the default weight at `Regular`. Existing `600` / `700` emphasis points in the Slint tree can stay in place, but the bundled UI assets should stay narrow:

- `SarasaUiSC-Regular.ttf`
- `SarasaUiSC-SemiBold.ttf`

That keeps the primary shell readable in Chinese, avoids reusing terminal glyph metrics for dialogs/toolbars, and avoids introducing a full multi-weight UI bundle on day one.

### 3. Make terminal Latin and CJK defaults intentional

For the terminal, switch the bundled Latin default from `Cascadia Mono` to `JetBrains Mono`, and switch the shared default weight contract from `Regular` to `Medium`.

The terminal fallback chain should become:

1. `JetBrains Mono`
2. `Sarasa Term SC`
3. `Segoe UI Emoji`

The actual bundled assets should align with that contract:

- `JetBrainsMono-Medium.ttf`
- `SarasaTermSC-Medium.ttf`

Bold terminal text should continue to use the existing explicit bold path / synthetic embolden flow, rather than changing the global default to Semibold.

## Architecture

### UI path

- Add a dedicated typography theme file for shared UI font constants.
- Set `default-font-family` and `default-font-weight` on `ui/app-window.slint`.
- Explicitly wire the `PopupWindow`-based titlebar menu to the same UI font family, because `PopupWindow` is separate from the main `Window` default-font contract.
- Keep terminal font assets out of the Slint startup import path.

### Terminal path

- Update `src/app/terminal_font/backend.rs` so the shared terminal typography constants expose `JetBrains Mono`, `Sarasa Term SC`, and `Medium`.
- Update all bundled font loaders that still hard-code `Cascadia Mono` / `Regular`:
  - `src/app/terminal_font/windows_dwrite.rs`
  - `src/app/terminal_atlas.rs`
  - `src/app/terminal_font/mock.rs`
- Update test/source-contract files so the repo describes the new bundled contract consistently.

### Asset strategy

Vendor only the weights needed for the approved direction:

- UI: `Regular`, `SemiBold`
- Terminal: `Medium`

This keeps the change product-focused instead of turning into a font-packaging expansion project.

## Risks and Mitigations

### Risk 1: UI startup memory may rise slightly

Bundling a Chinese UI font is heavier than relying on a pure Latin/system-default startup path.

Mitigation:

- Keep the UI bundle limited to the exact approved weights.
- Keep terminal fonts separate from Slint UI imports.
- Do not reopen startup-memory experiments in this change.

### Risk 2: Popup/menu typography may drift from the main window

`PopupWindow` is not the same root type as `Window`, so relying only on `AppWindow` defaults is risky.

Mitigation:

- Add an explicit shared UI typography token import to `ui/components/titlebar-menu.slint`.

### Risk 3: Terminal metrics may shift slightly

Switching from `Cascadia Mono Regular` to `JetBrains Mono Medium` can change cell metrics or raster density.

Mitigation:

- Keep the existing terminal size/line-height contract unless tests prove a required adjustment.
- Verify with focused renderer tests plus a Windows package build.

## Testing Strategy

1. Add/adjust source-contract tests before implementation.
2. Verify the new tests fail for the expected reason.
3. Implement the smallest code change that satisfies the approved contract.
4. Run focused typography tests.
5. Run `cargo check`.
6. Run `./build-win-x64.sh`.

## Success Criteria

This refresh is successful when all of the following are true:

1. The app shell defaults to `Sarasa UI SC` instead of inheriting an accidental/system UI font mix.
2. The terminal default contract is `JetBrains Mono` + `Sarasa Term SC` + emoji fallback, with `Medium` as the default weight.
3. UI and terminal typography no longer share the same visual identity by accident.
4. The change lands without reopening the startup-memory optimization work.

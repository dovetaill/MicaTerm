# Terminal/UI Font Unification Design

## Problem

The current typography split does not match the intended product direction:

- The terminal body still defaults to `JetBrains Mono`, with `Sarasa Term SC` only acting as a CJK fallback.
- The bitmap atlas path and the Windows native text path do not share the same "Sarasa-first" contract, so mixed Chinese/English terminal content can feel visually inconsistent.
- The desktop shell UI still defaults to `Sarasa UI SC`, not `JetBrains Maple Mono`.
- The repository still bundles several historical font packages (`JetBrainsMono`, `CascadiaMono`, `SarasaUiSC`, `Fusion-JetBrainsMapleMono`) that the user no longer wants to keep.

The approved product decision is now explicit:

- **Terminal area:** use `Sarasa Term SC` as the unified main typeface.
- **Terminal exception:** keep system color emoji fallback only for emoji-capable glyphs.
- **UI shell outside the terminal:** use bundled `JetBrains Maple Mono`.
- **Asset policy:** delete the old non-approved bundled font families instead of merely leaving them unused.

## Chosen Approach

### 1. Split font ownership cleanly by surface

The terminal renderer and the Slint shell should keep separate font contracts:

- Terminal renderer owns terminal text.
- Slint UI owns shell chrome, dialogs, sidebar, titlebar, menus, and settings surfaces.

This keeps the product direction explicit instead of relying on accidental fallback behavior.

### 2. Make `Sarasa Term SC` the only terminal text family

All terminal text entry points should converge on `Sarasa Term SC`:

- shared backend defaults
- bitmap atlas renderer
- Windows native DirectWrite path
- Windows fallback discovery for non-emoji text

The only functional fallback that remains should be color emoji fallback, because removing it would risk tofu squares or degraded emoji rendering.

### 3. Move all shell UI typography to bundled `JetBrains Maple Mono`

The UI shell should import bundled `JetBrains Maple Mono` assets and expose them through the shared Slint typography contract so the whole shell changes together:

- app window default font
- popup/menu surfaces
- shared component typography

This avoids piecemeal font overrides and keeps the shell visually consistent.

### 4. Delete retired bundled font families

After the terminal/UI contracts are rewritten, remove the old bundled font families and all references to them:

- `assets/fonts/JetBrainsMono`
- `assets/fonts/CascadiaMono`
- `assets/fonts/SarasaUiSC`
- `assets/fonts/Fusion-JetBrainsMapleMono`

Also update build scripts, packaging scripts, tests, and docs so the repository describes only the approved font story.

## Architecture

### Terminal path

Update the Rust-side terminal font contract so all normal terminal text resolves to `Sarasa Term SC`.

This includes:

- `src/app/terminal_font/backend.rs`
- `src/app/terminal_atlas.rs`
- `src/app/terminal_font/windows_dwrite.rs`
- `src/app/terminal_font/windows_fallback.rs`
- related renderer/source-contract tests

The atlas path and Windows native path should describe the same primary family so the screenshot mismatch does not come back through a second rendering path.

### UI path

Replace the current Slint UI family contract with bundled `JetBrains Maple Mono`:

- add `JetBrains Maple Mono` assets plus license text under `assets/fonts`
- update `ui/theme/typography.slint`
- update `ui/app-window.slint`
- update popup/menu typography that does not inherit the root `Window` defaults automatically
- update UI typography tests and packaging/license checks

### Asset and packaging path

The repository should only bundle the approved font families after this change:

- `Sarasa Term SC`
- `JetBrains Maple Mono`

Build/packaging/license staging should be rewritten accordingly so packaged artifacts no longer include licenses for removed font families.

## Risks and Mitigations

### Risk 1: `JetBrains Maple Mono` is not yet vendored in the repository

Mitigation:

- add the required `JetBrains Maple Mono` font files as part of the change
- add the corresponding license file
- update source-contract tests first so missing assets fail clearly

### Risk 2: terminal private-use / Nerd Font glyphs may depend on historical assets

The repo contains tests around private-use terminal glyph rendering, so blindly deleting historical font assets could break icon rendering.

Mitigation:

- audit the actual glyph path before deletion
- if a Sarasa-family variant is still required for private-use glyph coverage, keep only the minimal Sarasa-family asset needed
- do not keep JetBrains/Maple/Cascadia around just to preserve the old mixed-family contract

### Risk 3: emoji rendering regresses if all fallback is removed

Mitigation:

- keep emoji-specific system fallback in the terminal
- do not treat emoji fallback as a reason to keep multiple primary text families

### Risk 4: packaging/tests/docs drift from the new contract

Mitigation:

- update tests, build scripts, and docs in the same change
- keep font-asset removal and font-contract changes in the same implementation plan so stale references fail fast

## Testing Strategy

1. Add/adjust failing source-contract tests for the new terminal/UI font contracts.
2. Add/adjust asset and packaging tests so removed font families are no longer expected.
3. Implement the Rust terminal font changes.
4. Implement the Slint UI `JetBrains Maple Mono` changes.
5. Remove retired bundled font families and clean all references.
6. Run focused typography/font contract tests.
7. Run `cargo check`.
8. Run Windows packaging verification with `./build-win-x64.sh`.

## Success Criteria

This change is successful when all of the following are true:

1. Terminal body text resolves to `Sarasa Term SC` instead of the current JetBrains-led mixed path.
2. The desktop shell UI defaults to bundled `JetBrains Maple Mono`.
3. The repository no longer bundles the retired JetBrains/Cascadia/SarasaUi/Fusion font families.
4. Emoji still renders via the allowed fallback path instead of regressing into missing-glyph output.
5. Build scripts, tests, docs, and packaged license staging all match the new font contract.

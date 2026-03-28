# Windows Skia Mainline and Terminal Font TDD Spec

Date: 2026-03-28
Scope: follow-up test-driven-development input for the Windows Skia mainline packaging change and Sarasa lazy terminal font registration.

## Implemented Surface

### Runtime Profile and Renderer Selection

- `src/app/runtime_profile.rs`
  - `AppBuildFlavor`
    - `Development`
    - `WindowsMainline`
    - `WindowsSoftwareCompat`
  - `RendererMode`
    - `Software`
    - `SkiaSoftware`
  - `AppRuntimeProfile`
    - `development()`
    - `mainline()`
    - `software_compat()`
    - `packaged()`
    - `forced_backend()`
    - `forced_renderer()`
    - `selector_label()`

- `src/main.rs`
  - `select_runtime_profile()` now resolves through `AppRuntimeProfile::packaged()`.
  - `apply_renderer_selector()` now routes `forced_backend()` and `forced_renderer()` into `slint::BackendSelector`.

- `src/app/bootstrap.rs`
  - `startup_failure_message()` now renders the selected `selector_label()` instead of a hard-coded `winit-software` string.

### Packaging Entry Points

- `build-win-x64.sh`
  - Windows Skia mainline wrapper
  - exports:
    - `CARGO_NO_DEFAULT_FEATURES=1`
    - `CARGO_FEATURES=slint-renderer-skia`
    - `MICA_TERM_BUILD_FLAVOR=windows-mainline`
    - `MICA_TERM_PACKAGE_RENDERER=skia-software`
    - `PACKAGE_FLAVOR_SUFFIX=-skia`

- `build-win-x64-software.sh`
  - Windows software compatibility wrapper
  - exports:
    - `CARGO_NO_DEFAULT_FEATURES=1`
    - `CARGO_FEATURES=slint-renderer-software`
    - `MICA_TERM_BUILD_FLAVOR=windows-software-compat`
    - `MICA_TERM_PACKAGE_RENDERER=software`
    - `PACKAGE_FLAVOR_SUFFIX=-software`

- `build-desktop.sh`
  - shared package skeleton
  - package naming now depends on `PACKAGE_FLAVOR_SUFFIX`

- `build-release.sh`
  - Linux leg stays on current default path
  - Windows GNU leg explicitly routes through the Skia mainline packaging env

### Terminal Font Registration

- `src/app/terminal_font.rs`
  - `ensure_terminal_font_registered() -> Result<(), String>`
  - embeds `ui/fonts/SarasaTermSCNerd-Regular.ttf`
  - uses `slint::fontique_07::shared_collection()`
  - uses `OnceLock<Result<(), String>>` so registration only runs once per process

- `src/app/bootstrap.rs`
  - `sync_workspace_session_state()` now calls `crate::app::terminal_font::ensure_terminal_font_registered()` when `workspace_session_host_mode() == "terminal"`

- `ui/shell/terminal-session-host.slint`
  - `terminal-font-family` is now:
    - `"Sarasa Term SC Nerd, Iosevka Term, Cascadia Mono, Consolas, monospace"`

- `ui/app-window.slint`
  - still imports only `IosevkaTerm-Regular.ttf`
  - does not reintroduce `SarasaTermSCNerd-Regular.ttf` into the startup `.slint` import path

## Slint Callback and UI Contract Notes

`TerminalSessionHost` still exposes the existing callback surface and the new font strategy must remain compatible with it:

- `text-input(string)`
- `key-input(string, bool, bool, bool)`
- `surface-resize-requested(int, int)`
- `copy-selection-requested(int, int, int, int)`
- `paste-requested()`
- `scroll-requested(int, int, int, bool, bool, bool)`
- `scroll-thumb-drag-requested(float)`
- `scroll-jump-requested(float)`
- `mouse-input(string, string, int, int, bool, bool, bool)`

The font change is intentionally data-contract-only on the Slint side. No callback signatures changed.

## Existing Test Coverage Added or Updated

- `tests/runtime_profile.rs`
- `tests/bootstrap_profile_smoke.rs`
- `tests/panic_logging.rs`
- `tests/logging_runtime.rs`
- `tests/build_win_x64_script_smoke.sh`
- `tests/build_win_x64_software_script_smoke.sh`
- `tests/build_win_x64_skia_script_smoke.sh`
- `tests/build_release_script_smoke.sh`
- `tests/startup_font_memory_regression.rs`
- `tests/terminal_font_registration_smoke.rs`
- `tests/workspace_tabs_spec.rs`
- `tests/ssh_terminal_interaction_spec.rs`
- `tests/window_theme_contract_smoke.sh`

## Edge Cases and Risks

- Compile-time env mismatch:
  - if `MICA_TERM_BUILD_FLAVOR` and `MICA_TERM_PACKAGE_RENDERER` do not form a recognized pair, `AppRuntimeProfile::packaged()` falls back to `Development + Software`
  - this is safe for development, but a packaging wrapper typo silently degrades to the software path

- Lazy font registration failure:
  - `OnceLock<Result<(), String>>` memoizes the first result
  - if the first registration attempt fails, later terminal openings reuse the same error state in-process

- Terminal host timing:
  - Sarasa is only registered when host mode becomes `terminal`
  - welcome/error states intentionally do not preload the larger font

- Packaging artifact naming:
  - downstream scripts must expect `-skia` and `-software` suffixes for Windows wrapper outputs

- Vendored backend contract:
  - `[patch.crates-io]` must continue to point at `vendor/i-slint-backend-winit`
  - `tests/slint_backend_patch_contract_smoke.sh` remains the protection against regressing the Windows partial-visibility fix

## Verification Evidence from This Implementation Pass

These commands were run and passed on the current worktree state:

- `cargo test --test runtime_profile --test bootstrap_profile_smoke --test panic_logging --test logging_runtime -q`
- `cargo test --test startup_font_memory_regression --test terminal_font_registration_smoke -q`
- `cargo test --test workspace_tabs_spec --test ssh_terminal_interaction_spec -q`
- `bash tests/build_win_x64_script_smoke.sh`
- `bash tests/build_win_x64_software_script_smoke.sh`
- `bash tests/build_win_x64_skia_script_smoke.sh`
- `bash tests/build_release_script_smoke.sh`
- `bash tests/window_theme_contract_smoke.sh`
- `bash tests/slint_backend_patch_contract_smoke.sh`
- `cargo check --workspace`
- `cargo clippy --workspace -- -D warnings`

Residue scan passed for:

- `build-win-x64-skia.sh`
- `windows-skia-experimental`
- `renderer_name("software".into())`
- `Mica Term failed to initialize winit-software`

## Current Out-of-Scope Repository Failure

`cargo test -q` does not fully pass on this branch yet because of an unrelated existing assertion in:

- `tests/assets_modal_smoke.rs`
  - failing test: `ssh_modal_no_longer_renders_dead_connection_options_group`
  - current mismatch: expected `label: "Upstream SSH Connection"`

This failure is outside the Windows Skia mainline / terminal font scope and should be handled as a separate task before branch-finalization or merge.

## Recommended Next TDD Focus

1. Add explicit test coverage for all valid `packaged()` env combinations and at least one malformed env pair.
2. Add a tighter runtime-oriented test around `ensure_terminal_font_registered()` so the one-time registration path is exercised, not only source contracts.
3. Add a packaged artifact smoke test that checks actual Windows archive names include `-skia` and `-software`.
4. Add a memory-regression test strategy for startup vs first terminal activation if a stable measurement harness becomes available.

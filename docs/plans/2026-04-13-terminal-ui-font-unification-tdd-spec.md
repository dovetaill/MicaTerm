# Terminal UI Font Unification TDD Spec

## Scope

This document records the final implementation contract for the `2026-04-13-terminal-ui-font-unification` slice.

Goal:

- unify the Slint shell UI onto bundled `MiSans`
- unify the shared terminal text contract onto bundled `Sarasa Term SC`
- remove retired bundled font families from runtime, packaging, and repository assets

## Core Structs

- `AppTypography` in `ui/theme/typography.slint`
  - Slint global typography contract for shell UI surfaces
  - exposes `ui-font-family`, `ui-font-weight-regular`, and `ui-font-weight-semibold`
- `FontRequest` in `src/app/terminal_font/backend.rs`
  - shared terminal font request contract
  - `FontRequest::windows_default()` now resolves to `Sarasa Term SC`
- `LoadedFont` in `src/app/terminal_font/backend.rs`
  - runtime-loaded terminal font plus metrics and render profile
  - used by both bitmap and Windows native terminal paths
- `DirectWriteFontSystem` in `src/app/terminal_font/windows_dwrite.rs`
  - Windows-native terminal font loader, shaper, rasterizer, and fallback-chain discoverer
  - bundled primary face is now `assets/fonts/SarasaTermSC/SarasaTermSC-Regular.ttf`
- `WindowsFontFallbackResolver` in `src/app/terminal_font/windows_fallback.rs`
  - resolves fallback families for emoji and symbol text
  - keeps plain CJK/body text on the Sarasa primary family
- `TerminalAtlasRenderer` in `src/app/terminal_atlas.rs`
  - bitmap terminal atlas renderer
  - bundled mono face is now `Sarasa Term SC`
- `WindowsNativePresenter` in `src/app/terminal_presenter.rs`
  - Windows retained-native presenter
  - creates `FontRequest::windows_default()` and reloads font metrics on raster-scale changes
- `WindowsDirectWriteTextRendererState` in `src/app/terminal_renderer/platform/windows.rs`
  - tracks the DirectWrite in-memory bundled font path for retained-native rendering
  - bundled in-memory bytes now point only at `SarasaTermSC`

## Traits And Interface Contracts

- `FontSystem` in `src/app/terminal_font/backend.rs`
  - contract for `load_font`, shaping, glyph rasterization, fallback-face discovery, and color glyph rasterization
  - the shared primary family exposed through this contract is now `Sarasa Term SC`
- `TerminalPresenter` in `src/app/terminal_presenter.rs`
  - contract for `set_raster_scale`, `present`, and `default_cell_size`
  - Windows native presenter and bitmap presenter now consume the same Sarasa-first terminal typography contract
- packaging contract in `build-desktop.sh`
  - `stage_bundled_font_licenses()` stages only:
    - `licenses/fonts/MiSans/LICENSE.txt`
    - `licenses/fonts/SarasaTermSC/LICENSE.txt`
  - retired bundles are no longer copied into packaged Windows artifacts

## Slint Callbacks, Global State, And Bindings

- `ui/app-window.slint` imports:
  - `../assets/fonts/MiSans/MiSans-Regular.ttf`
  - `../assets/fonts/MiSans/MiSans-Semibold.ttf`
- `AppWindow` binds:
  - `default-font-family: AppTypography.ui-font-family`
  - `default-font-weight: AppTypography.ui-font-weight-regular`
- this feature does not add new Slint callbacks
- this feature relies on declarative global binding rather than imperative UI mutation
- `workspace-session-device-scale-factor` remains the bridge from shell layout state into terminal presentation scale, and it indirectly controls terminal font reload timing through presenter sync

## Tokio Task, Channel, And Actor Relationships

- this feature does not introduce a new Tokio task, channel, or actor
- font-family selection and bundled font loading remain synchronous inside the presenter and renderer path
- existing async SSH/session/runtime flows still feed `ShellViewModel` through pre-existing runtime channels
- once shell state is synchronized on the UI thread, `bootstrap.rs` calls `ensure_workspace_terminal_presenter(window, profile, scale_factor)` and then `host.set_raster_scale(scale_factor)`
- because font loading stays synchronous here, this feature avoids adding a second async font-loading lifecycle that could race the UI thread

## State Flow

1. Slint shell startup loads `MiSans` through `ui/app-window.slint`.
2. `AppTypography` publishes the shell UI typography contract to the full window tree.
3. Terminal presenters request the shared terminal font through `FontRequest::default()` or `FontRequest::windows_default()`.
4. `TerminalAtlasRenderer` loads bundled `Sarasa Term SC` for the bitmap path.
5. `DirectWriteFontSystem` loads bundled `Sarasa Term SC` first, then resolves emoji and symbol fallbacks only when text content requires them.
6. `WindowsDirectWriteTextRendererState` keeps the in-memory bundled Sarasa bytes available so native DirectWrite faces stay on repository-owned font data.
7. On DPI or scale changes, `WindowsNativePresenter::reload_loaded_font_for_scale()` reloads metrics and clears shaped-row/frame caches before the next present.
8. During packaging, `build-desktop.sh` stages only `MiSans` and `SarasaTermSC` license files into the final Windows artifact.

## Key Error Handling Strategies

- bundled font loading failures return `anyhow::Result` errors in:
  - `TerminalAtlasRenderer::with_emoji_renderer()`
  - `DirectWriteFontSystem::new()`
  - `DirectWriteFontSystem::ensure_face_for_family()`
- Windows native raster-scale reload failures are logged in `WindowsNativePresenter::set_raster_scale()` and the presenter keeps the previous loaded font state instead of crashing the UI
- DirectWrite bundled in-memory font setup in `platform/windows.rs` fails closed by returning `None`, which allows higher-level code to keep fallback behavior instead of dereferencing incomplete native state
- packaging fails fast in `build-desktop.sh` if required bundled license files are missing, preventing silent shipment of incomplete attribution bundles

## Edge Cases

- Tokio channel blockage or message backlog
  - this feature adds no new channel, but delayed session-state delivery can still postpone `workspace-session-device-scale-factor` sync and leave one frame rendered with stale terminal metrics
- incorrect UI-thread update timing
  - if scale-factor propagation reaches the presenter after a frame present, bitmap/native cell metrics can drift from Slint layout measurements for one update cycle
- data race or shared-state inconsistency
  - `WindowsNativePresenter` must clear `previous_frame`, `previous_shaped_rows`, and `shaped_row_cache` when scale changes, or glyph caches can remain keyed to stale font metrics
- resource release ordering
  - `WindowsDirectWriteTextRendererState` must keep the in-memory font loader and font-file handles alive for at least as long as the DirectWrite faces that reference bundled Sarasa bytes
- async task cancellation or dangling callbacks after window close
  - this feature avoids new async font callbacks; if bundled font loading ever moves off-thread later, the callback path must guard against closed `AppWindow` handles before pushing UI updates
- Slint model updates drifting from the real data source
  - `workspace_session_device_scale_factor`, terminal cell width, and terminal cell height must be updated as a coherent set or shell overlays can disagree with the actual rendered glyph grid
- packaged artifact drift
  - future packaging edits must keep retired font licenses out of `licenses/fonts/`, or downstream artifact validation will regress even if runtime rendering still works

## Recommended Follow-up Tests

- unit tests for `WindowsFontFallbackResolver`
  - plain ASCII and CJK text should stay on `Sarasa Term SC`
  - emoji and symbol text should append only the expected fallback families
- unit tests for `FontRequest::windows_default()`
  - assert the family and size contract for Windows remains Sarasa-first
- integration tests for `WindowsNativePresenter::set_raster_scale()`
  - verify cache reset and loaded-font reload behavior across multiple scale changes
- packaging smoke tests
  - assert Windows zip contents contain only `MiSans/LICENSE.txt` and `SarasaTermSC/LICENSE.txt`
  - assert retired font license directories never reappear
- UI typography smoke tests
  - assert `AppWindow` keeps importing bundled `MiSans` faces
  - assert `AppTypography` remains the sole default shell font contract
- renderer contract tests
  - assert bitmap atlas and retained-native DirectWrite paths both continue using bundled `Sarasa Term SC`

## Final Verification Snapshot

- passed focused font suite from Task 5:
  - `cargo test --test ui_typography_defaults_spec --test terminal_font_registration_smoke --test windows_terminal_typography_defaults_spec --test startup_font_memory_regression --test windows_directwrite_font_chain_spec --test terminal_atlas_renderer_spec --test runtime_profile -q`
- passed compile verification:
  - `cargo check`
- passed Windows packaging verification:
  - `./build-win-x64.sh`
- packaged Windows artifact now stages only:
  - `licenses/fonts/MiSans/LICENSE.txt`
  - `licenses/fonts/SarasaTermSC/LICENSE.txt`

# Mica Term

Project planning is in `docs/plans/`.

- Overall style design: `docs/plans/2026-03-10-overall-style-design.md`
- Overall style implementation plan: `docs/plans/2026-03-10-overall-style-implementation-plan.md`
- Overall style verification: `docs/plans/2026-03-10-overall-style-verification.md`

## Icon Assets

- Source vectors: `assets/icons/`
- Export script: `scripts/export-icons.sh`
- Windows icon: `assets/icons/windows/mica-term.ico`

## Mainline Build Entry Points

- `./build-release.sh`
  - Runs the Linux x64 release path and the Windows GNU software release path for `x86_64-unknown-linux-gnu` and `x86_64-pc-windows-gnu`
  - Default mode: `MODE=fail-fast`
  - Optional mode: `MODE=best-effort`

- `./build-desktop.sh`
  - Default target: `x86_64-unknown-linux-gnu`
  - Output: `dist/mica-term-x86_64-unknown-linux-gnu-release.tar.gz`
- `TARGET=aarch64-unknown-linux-gnu ./build-desktop.sh`
  - Linux ARM64 build on Linux hosts with a GNU cross-linker
- `TARGET=x86_64-apple-darwin ./build-desktop.sh`
  - macOS Intel build on macOS hosts
- `TARGET=aarch64-apple-darwin ./build-desktop.sh`
  - macOS Apple Silicon build on macOS hosts
- `TARGET=aarch64-pc-windows-msvc ./build-desktop.sh`
  - Windows ARM64 build on Windows MSVC environments
- `./build-win-x64.sh`
  - Windows Skia mainline wrapper
  - Default target: `x86_64-pc-windows-msvc`
  - Host requirement: Windows MSVC shell / Git Bash, or Linux + `cargo-xwin` + `clang`
  - Default parallelism: auto-detects jobs when `BUILD_JOBS` is unset
  - Explicit parallel override examples:
    - `BUILD_JOBS=32 ./build-win-x64.sh`
    - `BUILD_JOBS=$(nproc) ./build-win-x64.sh`
  - Outputs:
    `dist/mica-term-x86_64-pc-windows-msvc-release-skia.zip`
- `./build-win-x64-software.sh`
  - Windows software compatibility wrapper
  - Default target: `x86_64-pc-windows-gnu`
  - Override target: `TARGET=x86_64-pc-windows-msvc ./build-win-x64-software.sh`
  - Outputs:
    `dist/mica-term-x86_64-pc-windows-gnu-release-software.zip`
    `dist/mica-term-x86_64-pc-windows-msvc-release-software.zip`

Notes:

- Cargo already builds in parallel by default.
- `BUILD_JOBS` is the repo wrapper's explicit override knob and maps directly to `cargo --jobs <N>`.
- `./build-win-x64.sh` now auto-detects a default job count when `BUILD_JOBS` is unset, probing `nproc`, then `getconf _NPROCESSORS_ONLN`, then `NUMBER_OF_PROCESSORS`.
- If `BUILD_JOBS` is unset, `./build-desktop.sh` and `./build-win-x64-software.sh` keep the previous behavior and do not append `--jobs`.
- If you already set `CARGO_BUILD_JOBS`, the repo entrypoints still recommend using `BUILD_JOBS` as the wrapper-level override.
- Shared build logs now announce `phase 1/3`, `phase 2/3`, and `phase 3/3` so the final root-crate compile/link stretch is easier to distinguish from the earlier parallel dependency fan-out.
- `./build-win-x64.sh` defaults to the Windows MSVC Skia mainline package on `x86_64-pc-windows-msvc`; it is not the GNU entrypoint.
- `./build-win-x64.sh` packages the Windows Skia route as `winit-skia` on `x86_64-pc-windows-msvc`.
- `./build-win-x64-software.sh` packages the Windows compatibility route as `winit-software`.
- `./build-win-x64-software.sh` remains the default Windows GNU wrapper entrypoint.
- Generic development builds stay on the default packaged fallback unless a wrapper injects build flavor and renderer environment variables.
- `./build-release.sh` remains the aggregate Linux x64 + Windows GNU release entrypoint, with the Windows leg routed through `./build-win-x64-software.sh` because `rust-skia` does not ship `x86_64-pc-windows-gnu` Skia binaries.
- `[patch.crates-io]` in `Cargo.toml` still points to the vendored `i-slint-backend-winit` backend so the Windows partial-visibility fix stays active.
- `assets/fonts/JetBrainsMapleMono/` is the bundled shell UI family, and `ui/app-window.slint` imports `JetBrains Maple Mono` directly for the Slint desktop shell.
- `assets/fonts/SarasaTermSCNerd/SarasaTermSCNerd-Regular.ttf` is the bundled `Sarasa Term SC Nerd` terminal face shared by the Rust atlas and Windows DirectWrite paths.
- `src/app/terminal_atlas.rs` uses `ab_glyph` for lazy glyph loading and rasterization, avoiding the heavier pre-expanded `fontdue` path for the terminal font.
- Terminal body text stays on the bundled Sarasa mono atlas path, while emoji-presenting clusters use system color emoji fonts and are composited into the same atlas surface as RGBA sprites.
- Windows expects `Segoe UI Emoji` for terminal body color emoji rendering, and Linux expects an installed color emoji family such as `Noto Color Emoji`.
- If no preferred system color emoji font is available, or if emoji rasterization fails, the terminal atlas falls back to a visible replacement glyph and emits a diagnostic warning instead of silently painting transparent cells.
- `ui/shell/terminal-session-host.slint` renders the terminal body through a single atlas-backed image surface; Slint keeps cursor, selection, scrollbar, and context-menu overlays only.

## Terminal Renderer Migration Status

- Current terminal core status:
  - WezTerm-backed terminal core remains the shipped default today
  - Rio remains an architectural reference rather than migrated runtime code
- Windows-first native renderer:
  - packaged `windows-mainline` builds ship the retained-native child HWND presenter as the live Windows terminal path
  - packaged `windows-software-compat` builds keep the software host renderer, but the visible terminal path is still retained-native
  - the retired same-HWND DC overlay path is no longer part of the retained-native design
  - if native presenter setup fails, runtime falls back to the bitmap presenter instead of leaving the terminal blank
  - `app.terminal` diagnostics log the requested render mode, active presenter mode, and fallback transitions during packaged bring-up
- Platform support matrix:
  - Windows mainline: retained-native presenter
  - Windows software compatibility: retained-native presenter on the software host renderer
  - Linux/macOS: bitmap presenter today; Linux/macOS native renderer follow-up work is still pending
- Migration scope today:
  - text shaping and frame preparation are split behind the presenter boundary
  - Slint still owns the terminal host, cursor, selection, scrollbar, and input routing
  - Linux/macOS native renderer work remains follow-up, not part of the current release slice

Archive formats:

- Linux and macOS targets produce `dist/<app>-<target>-<profile>.tar.gz`
- Windows wrapper targets produce `dist/<app>-<target>-<profile>-skia.zip` or `dist/<app>-<target>-<profile>-software.zip`

Prerequisites by target:

- Linux x64:
  - installed Rust target: `rustup target add x86_64-unknown-linux-gnu`
- Linux ARM64:
  - installed Rust target: `rustup target add aarch64-unknown-linux-gnu`
  - available linker: `aarch64-linux-gnu-gcc`
  - override supported via `CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER`
- macOS Intel / Apple Silicon:
  - installed Rust target: `rustup target add x86_64-apple-darwin` or `rustup target add aarch64-apple-darwin`
  - must be built from a macOS host
- Windows GNU x64:
  - installed Rust target: `rustup target add x86_64-pc-windows-gnu`
  - available linker: `x86_64-w64-mingw32-gcc`
  - available assembler: `nasm`
  - override supported via `CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER`
  - used by `./build-win-x64-software.sh` and `./build-release.sh`
- Windows MSVC x64 / ARM64:
  - installed Rust target: `rustup target add x86_64-pc-windows-msvc` or `rustup target add aarch64-pc-windows-msvc`
  - must be built from a Windows MSVC shell or Git Bash environment
  - required for `./build-win-x64.sh`

## Windows Logging

To keep logs next to `mica-term.exe` on Windows, create an empty `.mica-term-portable`
file in the packaged app directory before launching the app.

PowerShell example:

```powershell
cd .\dist\mica-term-x86_64-pc-windows-msvc-release-skia
ni .mica-term-portable -ItemType File -Force
$env:MICA_TERM_LOG = "debug"
$env:MICA_TERM_MEMORY_DIAGNOSTICS = "1"
.\mica-term.exe
```

Expected output location:

- portable mode: `logs/system-error.log.YYYY-MM-DD`
- standard mode without `.mica-term-portable`: `%LOCALAPPDATA%\MicaTerm\MicaTerm\logs\`

Notes:

- `MICA_TERM_LOG=debug` enables `ui.theme` and `app.window` diagnostics.
- `MICA_TERM_MEMORY_DIAGNOSTICS=1` enables opt-in terminal memory diagnostics for session close, cache shrink, and large-output trim investigations.
- Without `MICA_TERM_LOG=debug`, only error-level events are persisted.
- Windows builds use daily log rotation, so the file name includes the current date.
- Terminal memory entries are written under the `app.memory` target with events like
  `startup-snapshot`, `session-close`, `close-shrink`, `idle-shrink`, `trim-request`, and `trim-executed`.
- `startup-snapshot` captures the current `working_set_bytes`, `private_usage_bytes`, and
  `pagefile_usage_bytes` near startup so later field runs can compare private/commit behavior
  instead of relying on working-set motion alone.
- `session-close` captures `before_session_count`, `after_session_count`,
  `before_runtime_control_count`, `after_runtime_control_count`,
  `terminal_memory_release_succeeded`, and `runtime_disconnect_succeeded` so field runs can tell
  whether session close really released session/runtime state before later surface-clear or
  no-surface idle shrink events run.
- The shrink events now also surface bitmap-atlas retained-state counters such as
  `cache_before_bitmap_sprite_cache_entries`, `cache_before_bitmap_row_hash_entries`, and
  `cache_before_bitmap_surface_bytes` so software/fallback runs can tell whether the atlas path
  actually dropped sprite and backing-pixel state instead of only clearing native glyph caches.
- `trim-request` / `trim-executed` and the shrink events should be interpreted together: if only
  `working_set_bytes` falls while `private_usage_bytes` and `pagefile_usage_bytes` stay flat, the
  process mainly shed resident pages rather than truly releasing committed private memory.
- The packaged memory baseline matrix, counter checklist, and renderer/path capture rules live in
  `docs/plans/2026-06-09-memory-footprint-reduction/verification.md`.
- After reproducing, you can filter just the memory diagnostics with
  `Select-String -Path .\logs\system-error.log* -Pattern "app.memory","startup-snapshot","session-close","close-shrink","idle-shrink","trim-request","trim-executed"`.

## Asset Persistence

Asset data does not resolve relative to the working directory. Mica Term uses the same
application root strategy for logs and persisted console assets, then stores the asset catalog
under the root `data/` directory as `<root>/data/assets.redb`.

- portable mode with `.mica-term-portable`: both logging and asset data resolve relative to the
  executable directory, so a packaged Windows app keeps `logs/` and `data/assets.redb` next to
  `mica-term.exe`
- standard mode without `.mica-term-portable`: the root moves to the platform local data
  directory instead of the working directory; on Windows this means
  `%LOCALAPPDATA%\\MicaTerm\\MicaTerm\\data\\assets.redb` for assets and
  `%LOCALAPPDATA%\\MicaTerm\\MicaTerm\\logs\\` for logs

Notes:

- `.mica-term-portable` affects the logging root and the asset data root at the same time.
- Launching the app from a different shell or working directory does not change where
  `assets.redb` is stored.

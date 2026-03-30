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
  - Host requirement: Windows MSVC shell or Git Bash environment
  - Linux-host alternative: `./build-win-x64-software.sh`
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

- `./build-win-x64.sh` packages the Windows Skia route as `winit-skia-software` on `x86_64-pc-windows-msvc`.
- `./build-win-x64-software.sh` packages the Windows compatibility route as `winit-software`.
- Generic development builds stay on the default packaged fallback unless a wrapper injects build flavor and renderer environment variables.
- `./build-release.sh` remains the aggregate Linux x64 + Windows GNU release entrypoint, with the Windows leg routed through `./build-win-x64-software.sh` because `rust-skia` does not ship `x86_64-pc-windows-gnu` Skia binaries.
- `[patch.crates-io]` in `Cargo.toml` still points to the vendored `i-slint-backend-winit` backend so the Windows partial-visibility fix stays active.
- `ui/fonts/SarasaTermSCNerd-Regular.ttf` is the only bundled terminal face and is owned directly by the Rust atlas renderer.
- `src/app/terminal_atlas.rs` uses `ab_glyph` for lazy glyph loading and rasterization, avoiding the heavier pre-expanded `fontdue` path for the terminal font.
- `ui/shell/terminal-session-host.slint` renders the terminal body through a single atlas-backed image surface; Slint keeps cursor, selection, scrollbar, and context-menu overlays only.

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
.\mica-term.exe
```

Expected output location:

- portable mode: `logs/system-error.log.YYYY-MM-DD`
- standard mode without `.mica-term-portable`: `%LOCALAPPDATA%\MicaTerm\MicaTerm\logs\`

Notes:

- `MICA_TERM_LOG=debug` enables `ui.theme` and `app.window` diagnostics.
- Without `MICA_TERM_LOG=debug`, only error-level events are persisted.
- Windows builds use daily log rotation, so the file name includes the current date.

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

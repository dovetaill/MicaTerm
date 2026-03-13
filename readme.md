# Mica Term

Project planning is in `docs/plans/`.

- Overall style design: `docs/plans/2026-03-10-overall-style-design.md`
- Overall style implementation plan: `docs/plans/2026-03-10-overall-style-implementation-plan.md`
- Overall style verification: `docs/plans/2026-03-10-overall-style-verification.md`

## Icon Assets

- Source vectors: `assets/icons/`
- Export script: `scripts/export-icons.sh`
- Windows icon: `assets/icons/windows/mica-term.ico`

## Formal Release

Debian formal release aggregator:

- `./build-release.sh`
  - Runs the formal Debian release path for `x86_64-unknown-linux-gnu` and `x86_64-pc-windows-gnu`
  - Default mode: `MODE=fail-fast`
  - Optional mode: `MODE=best-effort`

Per-target formal entrypoints:

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
  - Compatibility wrapper for the formal `x86_64-pc-windows-gnu` package
  - Output: `dist/mica-term-x86_64-pc-windows-gnu-release.zip`

## FemtoVG WGPU Experimental

Experimental renderer entrypoints are wrapper-only and do not change `./build-release.sh`.

- `./build-linux-x64-femtovg-wgpu.sh`
  - Linux experimental package for `x86_64-unknown-linux-gnu`
  - Uses `--no-default-features --features femtovg-wgpu-experimental`
  - Keeps executable name as `mica-term`
  - Produces `dist/mica-term-femtovg-wgpu-experimental-x86_64-unknown-linux-gnu-release.tar.gz`
- `./build-win-x64-femtovg-wgpu.sh`
  - Windows experimental package for `x86_64-pc-windows-msvc`
  - Uses `--no-default-features --features femtovg-wgpu-experimental`
  - Keeps executable name as `mica-term`
  - Produces `dist/mica-term-femtovg-wgpu-experimental-x86_64-pc-windows-msvc-release.zip`
- `./build-win-x64-gnu-femtovg-wgpu.sh`
  - Linux-host Windows GNU experimental package for `x86_64-pc-windows-gnu`
  - Uses `--no-default-features --features femtovg-wgpu-experimental`
  - Keeps executable name as `mica-term`
  - Produces `dist/mica-term-femtovg-wgpu-experimental-x86_64-pc-windows-gnu-release.zip`

Notes:

- Both wrappers stage and archive under `mica-term-femtovg-wgpu-experimental-*`.
- The runtime profile is internal to the app and must lock `winit + femtovg-wgpu + wgpu-28`.
- `./build-win-x64-femtovg-wgpu.sh` remains the Windows-host MSVC experimental path.
- Experimental packages are intentionally separate from the formal Debian release aggregator.
- `./build-release.sh` remains the formal release path and must not expose the experimental wrapper entrypoints.

## Try / Future Renderer Exploration

- `docs/plans/try-winit-femtovg-wgpu.md`
  - Kept as a try document only, not part of the current formal or experimental default route

Archive formats:

- Linux and macOS targets produce `dist/<app>-<target>-<profile>.tar.gz`
- Windows targets produce `dist/<app>-<target>-<profile>.zip`

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
  - override supported via `CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER`
- Windows MSVC x64 / ARM64:
  - installed Rust target: `rustup target add x86_64-pc-windows-msvc` or `rustup target add aarch64-pc-windows-msvc`
  - must be built from a Windows MSVC shell or Git Bash environment

## Windows Logging

To keep logs next to `mica-term.exe` on Windows, create an empty `.mica-term-portable`
file in the packaged app directory before launching the app.

PowerShell example:

```powershell
cd .\dist\mica-term-x86_64-pc-windows-gnu-release
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

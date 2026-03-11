# Mica Term

Project planning is in `docs/plans/`.

- Overall style design: `docs/plans/2026-03-10-overall-style-design.md`
- Overall style implementation plan: `docs/plans/2026-03-10-overall-style-implementation-plan.md`
- Overall style verification: `docs/plans/2026-03-10-overall-style-verification.md`

## Icon Assets

- Source vectors: `assets/icons/`
- Export script: `scripts/export-icons.sh`
- Windows icon: `assets/icons/windows/mica-term.ico`

## Desktop Build

Primary entrypoint:

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

Compatibility wrapper:

- `./build-win-x64.sh`
  - Default target: `x86_64-pc-windows-gnu`
  - Output: `dist/mica-term-x86_64-pc-windows-gnu-release.zip`
- `TARGET=x86_64-pc-windows-msvc ./build-win-x64.sh`
  - Windows x64 MSVC build on Windows MSVC environments
- `./build-win-x64-skia.sh`
  - Experimental Windows x64 MSVC build with Slint Skia renderer enabled
  - Output: `dist/mica-term-x86_64-pc-windows-msvc-release.zip`

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

Experimental renderer overrides:

- `CARGO_FEATURES=windows-skia-experimental ./build-win-x64.sh`
  - Compiles the Windows package with Slint Skia renderer support enabled
- `./build-win-x64-skia.sh`
  - Convenience wrapper for the same experimental Windows Skia build on a Windows MSVC shell
- `CARGO_NO_DEFAULT_FEATURES=1`
  - Optional advanced override if you want to drop the default software renderer feature during a custom build

Current limitation:

- `windows-skia-experimental` is not currently viable on the Linux -> `x86_64-pc-windows-gnu` cross-build path in this repo.
- The upstream `rust-skia` download step for `x86_64-pc-windows-gnu` falls back to a full Skia source build, which requires a Windows MSVC / VC toolchain.
- Use a Windows MSVC shell with `TARGET=x86_64-pc-windows-msvc` for this experiment.

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

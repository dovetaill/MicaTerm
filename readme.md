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

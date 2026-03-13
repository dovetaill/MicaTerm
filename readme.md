# Mica Term

Project planning is in `docs/plans/`.

- Overall style design: `docs/plans/2026-03-10-overall-style-design.md`
- Overall style implementation plan: `docs/plans/2026-03-10-overall-style-implementation-plan.md`
- Overall style verification: `docs/plans/2026-03-10-overall-style-verification.md`
- Windows FemtoVG WGPU DX12 retrospective:
  `docs/plans/2026-03-13-windows-femtovg-wgpu-dx12-retrospective.md`

## Icon Assets

- Source vectors: `assets/icons/`
- Export script: `scripts/export-icons.sh`
- Windows icon: `assets/icons/windows/mica-term.ico`

## Mainline Build Entry Points

- `./build-release.sh`
  - Runs the mainline GPU release path for `x86_64-unknown-linux-gnu` and `x86_64-pc-windows-gnu`
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
  - Single Windows x64 wrapper
  - Default target: `x86_64-pc-windows-gnu`
  - Override target: `TARGET=x86_64-pc-windows-msvc ./build-win-x64.sh`
  - Outputs:
    `dist/mica-term-x86_64-pc-windows-gnu-release.zip`
    `dist/mica-term-x86_64-pc-windows-msvc-release.zip`

- `./build-linux-x64-femtovg-wgpu.sh`
  - Linux x64 mainline convenience wrapper for `x86_64-unknown-linux-gnu`
  - Produces `dist/mica-term-x86_64-unknown-linux-gnu-release.tar.gz`

Notes:

- All build entrypoints now resolve to the same runtime route: `winit + femtovg-wgpu + wgpu-28`.
- The runtime profile is internal to the app and is always locked to that GPU renderer.
- `./build-release.sh` remains the aggregate Linux x64 + Windows GNU release entrypoint.

## Windows FemtoVG WGPU Note

- On Windows, the mainline `femtovg-wgpu` route now explicitly requests `wgpu::Backends::DX12`.
- This is intentional and not a cosmetic tweak. The visual corruption issue still reproduced on the
  tested RX550 system after `transparent_window=false`, `present_mode=Fifo`, and `alpha_mode=Opaque`
  were already in place, but it stopped once the runtime actually initialized WGPU with `backend=Dx12`
  instead of `backend=Vulkan`.
- If you need to verify the live path, run with `MICA_TRACE_RENDER_PIPELINE=1` and confirm both of
  these log lines are present:
  - `femtovg renderer received requested graphics api ... requested_backends=Some(Backends(DX12))`
  - `wgpu adapter initialized for femtovg renderer backend=Dx12`
- The complete investigation and timeline are recorded in
  `docs/plans/2026-03-13-windows-femtovg-wgpu-dx12-retrospective.md`.

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

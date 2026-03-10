# Mica Term

Project planning is in `docs/plans/`.

- Overall style design: `docs/plans/2026-03-10-overall-style-design.md`
- Overall style implementation plan: `docs/plans/2026-03-10-overall-style-implementation-plan.md`
- Overall style verification: `docs/plans/2026-03-10-overall-style-verification.md`

## Windows Build

- `./build-win-x64.sh`
  - Default target: `x86_64-pc-windows-gnu`
  - Output: `dist/mica-term-x86_64-pc-windows-gnu-release.zip`
- `TARGET=x86_64-pc-windows-msvc ./build-win-x64.sh`
  - For Windows MSVC environments only
- GNU cross build prerequisite on Linux:
  - installed Rust target: `rustup target add x86_64-pc-windows-gnu`
  - available linker: `x86_64-w64-mingw32-gcc`

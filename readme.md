<div align="center">
  <p>
    <strong>English</strong> |
    <a href="readme.zh-CN.md">简体中文</a> |
    <a href="readme.ja.md">日本語</a>
  </p>

  <img src="assets/icons/mica-term-app.svg" width="112" alt="Mica Term logo">

  <h1>Mica Term</h1>

  <p><strong>A focused desktop workspace for terminals, remote servers, and the tools around them.</strong></p>

  <p>
    <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/Rust-2024-000000?style=flat-square&logo=rust" alt="Rust 2024"></a>
    <a href="https://slint.dev/"><img src="https://img.shields.io/badge/UI-Slint%201.15-2379F4?style=flat-square" alt="Slint 1.15"></a>
    <img src="https://img.shields.io/badge/platform-Windows%20%7C%20Linux%20%7C%20macOS-4F5D75?style=flat-square" alt="Windows, Linux, and macOS">
    <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-E6B450?style=flat-square" alt="MIT License"></a>
  </p>
</div>

Mica Term brings a modern terminal, SSH connections, SFTP file management, reusable snippets, credentials, and encrypted configuration sync into one native desktop application. It is built for people who move between local shells and remote machines throughout the day and want one quiet, fast place to manage that work.

> [!IMPORTANT]
> Mica Term is under active development. The current package version is `0.1.0`, so behavior, storage formats, and interfaces may change before the first stable release.

## Highlights

| Area | What Mica Term provides |
| --- | --- |
| Terminal | A WezTerm-backed terminal core, tabbed sessions, Unicode shaping, color emoji, themes, selection, URL opening, command blocks, and a native Windows presentation path. |
| SSH | Saved connections, password and SSH-key authentication, known-host verification, connection progress, SOCKS5/HTTP proxies, and SSH jump hosts. |
| SFTP | A terminal-aware file browser with navigation history, current-directory following, upload/download queues, conflict handling, resumable transfers, and workspace expansion. |
| Workspace | Quick launch, session tabs, reconnect and clone actions, command palette, safer multiline paste confirmation, and a dedicated transfer center. |
| Assets | Searchable trees for SSH connections, snippets, snippet packages, identities, and SSH keys, backed by an embedded local database. |
| Secure sync | Encrypted vault snapshots, merge and recovery flows, conflict inboxes, and provider support for Git repositories, GitHub Gists, Gitee Gists, and S3-compatible storage. The GitLab Snippet connector is in progress. |

## Why Mica Term

- **One remote-work surface.** Keep terminal sessions, remote files, connection profiles, snippets, and credentials in the same workflow.
- **Native desktop performance.** The application is written in Rust and uses Slint with platform-aware rendering paths rather than a browser runtime.
- **Terminal text that holds up.** The rendering stack includes Unicode segmentation, shaping, font fallback, custom grid glyphs, and color emoji support.
- **Security-conscious storage.** Secrets integrate with the operating-system keyring, while synchronized vault data is protected with Argon2 key derivation and ChaCha20-Poly1305 encryption.
- **Portable when needed.** Packaged Windows builds can opt into portable data and logging beside the executable with a `.mica-term-portable` marker.

## Technology Stack

| Layer | Technology |
| --- | --- |
| Language | Rust 2024 edition |
| Desktop UI | Slint 1.15, Winit, software and Skia renderers |
| Terminal | WezTerm terminal/surface forks, Termwiz |
| Text and graphics | RustyBuzz, Swash, FontDB, DirectWrite on Windows |
| Async runtime | Tokio |
| Remote access | Russh, Russh SFTP |
| Local persistence | Redb, Bincode, Serde |
| Secrets and crypto | OS keyring, Argon2, ChaCha20-Poly1305, Zeroize |
| Sync and networking | Git2, Gix, Reqwest, OAuth 2.0, AWS SDK for S3 |
| Diagnostics | Tracing with rotating logs and crash records |

## Architecture at a Glance

```text
Slint views
    |
Shell and view-model projection
    |
Application services
    |-- Terminal core and renderer
    |-- SSH sessions and connection routing
    |-- SFTP browser and transfer engine
    |-- Assets, keychain, and local persistence
    `-- Encrypted vault and remote providers
```

The UI remains declarative, while Rust owns state, validation, asynchronous work, persistence, and platform integration. This keeps rendering concerns separate from terminal and remote-session behavior.

## Getting Started

### Prerequisites

- A current stable Rust toolchain with Cargo
- Platform build tools for your target
- On Debian/Ubuntu, `pkg-config` and `libwayland-dev` for the native Linux UI stack

See [apt-packages.md](apt-packages.md) for the repository's current Linux build dependencies and cross-compilation notes.

### Run from Source

```bash
git clone https://github.com/dovetaill/MicaTerm.git
cd MicaTerm
cargo run
```

### Verify the Project

```bash
cargo test
```

### Build Packages

```bash
# Linux x64 release archive
./build-desktop.sh

# Windows x64 MSVC + Skia package
./build-win-x64.sh

# Aggregate Linux x64 and Windows GNU/software packages
./build-release.sh
```

`build-desktop.sh` also supports Linux ARM64, macOS Intel and Apple Silicon, and Windows GNU/MSVC targets through the `TARGET` environment variable. Run `./build-desktop.sh --help` for the complete matrix and required host tools.

## Project Layout

```text
src/app/        Terminal, SSH, SFTP, vault, persistence, and platform services
src/shell/      Workspace state, projections, navigation, and UI orchestration
src/theme/      Runtime theme definitions
ui/             Slint views, components, shell, and design tokens
assets/         Brand assets, application icons, and bundled fonts
tests/          Behavioral, regression, integration, and UI contract tests
scripts/        Development, verification, and asset tooling
```

## Development Notes

- The default feature set uses the Slint software renderer and the native terminal renderer.
- Windows mainline packages use the Skia host renderer and retained native terminal presentation.
- Linux and macOS currently use the bitmap terminal presenter.
- Build output is written to `dist/`; local Cargo output stays in `target/`.
- Runtime diagnostics can be enabled with `MICA_TERM_LOG=debug`.

For detailed troubleshooting, see [troubleshooting-notes.md](troubleshooting-notes.md). The current manual terminal smoke checklist lives in [docs/terminal-tui-smoke-checklist.md](docs/terminal-tui-smoke-checklist.md).

## Contributing

Issues and pull requests are welcome. Please keep changes focused, add tests in proportion to behavioral risk, and run the relevant Rust and shell contract tests before submitting a change.

## License

Mica Term is distributed under the [MIT License](LICENSE).

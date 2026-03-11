#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TARGET="${TARGET:-x86_64-unknown-linux-gnu}"
PROFILE="${PROFILE:-release}"
APP_NAME="${APP_NAME:-$(awk -F'"' '/^name = / { print $2; exit }' "$ROOT_DIR/Cargo.toml")}"
BIN_NAME="${BIN_NAME:-$APP_NAME}"
DIST_DIR="${DIST_DIR:-$ROOT_DIR/dist}"

usage() {
  cat <<EOF
Usage: $(basename "$0") [--help]

Build and package the desktop binary for this project into dist/.

Defaults:
  TARGET=$TARGET
  PROFILE=$PROFILE
  APP_NAME=$APP_NAME
  DIST_DIR=$DIST_DIR

Supported targets:
  x86_64-unknown-linux-gnu   Linux x64 build on Linux hosts
  aarch64-unknown-linux-gnu  Linux ARM64 build on Linux hosts with a GNU cross-linker
  x86_64-apple-darwin        macOS Intel build on macOS hosts
  aarch64-apple-darwin       macOS Apple Silicon build on macOS hosts
  x86_64-pc-windows-gnu      Windows x64 GNU build with MinGW-w64
  x86_64-pc-windows-msvc     Windows x64 MSVC build on Windows MSVC hosts
  aarch64-pc-windows-msvc    Windows ARM64 MSVC build on Windows MSVC hosts

Environment overrides:
  TARGET=<target triple>
  PROFILE=release|debug
  APP_NAME=<package name>
  BIN_NAME=<binary file name without extension>
  DIST_DIR=<output directory>
  CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=<gnu linker path>
  CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=<linux arm64 linker path>

Output:
  dist/<app>-<target>-<profile>.tar.gz
  dist/<app>-<target>-<profile>.zip
EOF
}

fail() {
  echo "error: $*" >&2
  exit 1
}

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || fail "missing required command: $1"
}

require_uname() {
  local expected="$1"
  local actual
  actual="$(uname -s)"
  case "$expected" in
    linux)
      [[ "$actual" == "Linux" ]] || fail "target '$TARGET' must be built from a Linux host."
      ;;
    darwin)
      [[ "$actual" == "Darwin" ]] || fail "target '$TARGET' must be built from a macOS host."
      ;;
    windows-msvc)
      case "$actual" in
        MINGW*|MSYS*|CYGWIN*)
          ;;
        *)
          fail "target '$TARGET' must be built from a Windows MSVC shell or Git Bash environment."
          ;;
      esac
      ;;
    *)
      fail "unknown host requirement '$expected'"
      ;;
  esac
}

if [[ "${1:-}" == "--help" ]]; then
  usage
  exit 0
fi

if [[ $# -ne 0 ]]; then
  fail "unknown arguments: $*"
fi

[[ -f "$ROOT_DIR/Cargo.toml" ]] || fail "Cargo.toml not found in $ROOT_DIR"

require_cmd cargo
require_cmd rustup

case "$PROFILE" in
  release)
    PROFILE_ARGS=(--release)
    ;;
  debug)
    PROFILE_ARGS=()
    ;;
  *)
    fail "unsupported PROFILE '$PROFILE' (expected release or debug)"
    ;;
esac

if ! rustup target list --installed | grep -qx "$TARGET"; then
  fail "Rust target '$TARGET' is not installed. Run: rustup target add $TARGET"
fi

case "$TARGET" in
  x86_64-unknown-linux-gnu)
    require_uname linux
    require_cmd tar
    require_cmd gzip
    BIN_SUFFIX=""
    ARCHIVE_SUFFIX=".tar.gz"
    ;;
  aarch64-unknown-linux-gnu)
    require_uname linux
    require_cmd tar
    require_cmd gzip
    GNU_LINKER="${CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER:-aarch64-linux-gnu-gcc}"
    command -v "$GNU_LINKER" >/dev/null 2>&1 || fail \
      "Linux ARM64 target requires linker '$GNU_LINKER'. Install aarch64 GNU tools or set CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER."
    export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER="$GNU_LINKER"
    BIN_SUFFIX=""
    ARCHIVE_SUFFIX=".tar.gz"
    ;;
  x86_64-apple-darwin|aarch64-apple-darwin)
    require_uname darwin
    require_cmd tar
    require_cmd gzip
    BIN_SUFFIX=""
    ARCHIVE_SUFFIX=".tar.gz"
    ;;
  x86_64-pc-windows-gnu)
    require_cmd zip
    GNU_LINKER="${CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER:-x86_64-w64-mingw32-gcc}"
    command -v "$GNU_LINKER" >/dev/null 2>&1 || fail \
      "Windows GNU target requires linker '$GNU_LINKER'. Install MinGW-w64 or set CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER."
    export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER="$GNU_LINKER"
    BIN_SUFFIX=".exe"
    ARCHIVE_SUFFIX=".zip"
    ;;
  x86_64-pc-windows-msvc|aarch64-pc-windows-msvc)
    require_uname windows-msvc
    require_cmd zip
    BIN_SUFFIX=".exe"
    ARCHIVE_SUFFIX=".zip"
    ;;
  *)
    fail "unsupported TARGET '$TARGET'"
    ;;
esac

STAGE_DIR="$DIST_DIR/${APP_NAME}-${TARGET}-${PROFILE}"
ARCHIVE_PATH="$DIST_DIR/${APP_NAME}-${TARGET}-${PROFILE}${ARCHIVE_SUFFIX}"
BIN_PATH="$ROOT_DIR/target/$TARGET/$PROFILE/$BIN_NAME$BIN_SUFFIX"

echo "==> Building $BIN_NAME for $TARGET ($PROFILE)"
cargo build "${PROFILE_ARGS[@]}" --target "$TARGET" --locked

[[ -f "$BIN_PATH" ]] || fail "expected binary not found: $BIN_PATH"

echo "==> Staging package in $STAGE_DIR"
mkdir -p "$DIST_DIR"
rm -rf "$STAGE_DIR"
mkdir -p "$STAGE_DIR"
cp "$BIN_PATH" "$STAGE_DIR/"

if [[ -f "$ROOT_DIR/readme.md" ]]; then
  cp "$ROOT_DIR/readme.md" "$STAGE_DIR/README.md"
fi

echo "==> Creating archive $ARCHIVE_PATH"
rm -f "$ARCHIVE_PATH"

case "$ARCHIVE_SUFFIX" in
  .zip)
    (
      cd "$DIST_DIR"
      zip -rq "$(basename "$ARCHIVE_PATH")" "$(basename "$STAGE_DIR")"
    )
    ;;
  .tar.gz)
    tar -C "$DIST_DIR" -czf "$ARCHIVE_PATH" "$(basename "$STAGE_DIR")"
    ;;
  *)
    fail "unsupported archive suffix '$ARCHIVE_SUFFIX'"
    ;;
esac

echo "==> Done"
echo "Archive: $ARCHIVE_PATH"

#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TARGET="${TARGET:-x86_64-pc-windows-gnu}"
PROFILE="${PROFILE:-release}"
APP_NAME="${APP_NAME:-$(awk -F'"' '/^name = / { print $2; exit }' "$ROOT_DIR/Cargo.toml")}"
BIN_NAME="${BIN_NAME:-$APP_NAME}"
DIST_DIR="${DIST_DIR:-$ROOT_DIR/dist}"

usage() {
  cat <<EOF
Usage: $(basename "$0") [--help]

Build and package the Windows binary for this project into dist/.

Defaults:
  TARGET=$TARGET
  PROFILE=$PROFILE
  APP_NAME=$APP_NAME
  DIST_DIR=$DIST_DIR

Supported targets:
  x86_64-pc-windows-gnu   Linux-friendly cross build with MinGW-w64 linker
  x86_64-pc-windows-msvc  Windows-only MSVC build

Environment overrides:
  TARGET=<target triple>
  PROFILE=release|debug
  APP_NAME=<package name>
  BIN_NAME=<binary file name without .exe>
  DIST_DIR=<output directory>
  CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=<gnu linker path>

Output:
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
require_cmd zip

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
  x86_64-pc-windows-gnu)
    GNU_LINKER="${CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER:-x86_64-w64-mingw32-gcc}"
    command -v "$GNU_LINKER" >/dev/null 2>&1 || fail \
      "GNU target requires linker '$GNU_LINKER'. Install MinGW-w64 or set CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER."
    export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER="$GNU_LINKER"
    ;;
  x86_64-pc-windows-msvc)
    case "$(uname -s)" in
      MINGW*|MSYS*|CYGWIN*)
        ;;
      *)
        fail "MSVC target must be built from a Windows MSVC shell or Git Bash environment."
        ;;
    esac
    ;;
  *)
    fail "unsupported TARGET '$TARGET'"
    ;;
esac

STAGE_DIR="$DIST_DIR/${APP_NAME}-${TARGET}-${PROFILE}"
ZIP_PATH="$DIST_DIR/${APP_NAME}-${TARGET}-${PROFILE}.zip"
EXE_PATH="$ROOT_DIR/target/$TARGET/$PROFILE/$BIN_NAME.exe"
ICON_PATH="$ROOT_DIR/assets/icons/windows/mica-term.ico"

echo "==> Building $BIN_NAME for $TARGET ($PROFILE)"
cargo build "${PROFILE_ARGS[@]}" --target "$TARGET" --locked

[[ -f "$EXE_PATH" ]] || fail "expected binary not found: $EXE_PATH"

echo "==> Staging package in $STAGE_DIR"
rm -rf "$STAGE_DIR"
mkdir -p "$STAGE_DIR"
cp "$EXE_PATH" "$STAGE_DIR/"

if [[ -f "$ICON_PATH" ]]; then
  cp "$ICON_PATH" "$STAGE_DIR/"
fi

if [[ -f "$ROOT_DIR/readme.md" ]]; then
  cp "$ROOT_DIR/readme.md" "$STAGE_DIR/README.md"
fi

echo "==> Creating archive $ZIP_PATH"
rm -f "$ZIP_PATH"
(
  cd "$DIST_DIR"
  zip -rq "$(basename "$ZIP_PATH")" "$(basename "$STAGE_DIR")"
)

echo "==> Done"
echo "Archive: $ZIP_PATH"

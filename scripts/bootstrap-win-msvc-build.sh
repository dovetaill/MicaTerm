#!/usr/bin/env bash
# Prepares the Linux-host Windows MSVC build toolchain expected by build-win-x64.sh.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET="${TARGET:-x86_64-pc-windows-msvc}"
AUTO_INSTALL=0

usage() {
  cat <<'EOF'
Usage: ./scripts/bootstrap-win-msvc-build.sh [--help] [--install]

Prepare the Linux-host Windows MSVC build prerequisites used by ./build-win-x64.sh.

Checks:
  - cargo install cargo-xwin
  - rustup target add x86_64-pc-windows-msvc
  - clang or clang-19

If clang is missing, install Debian/Ubuntu host packages first:
  ./install-apt-packages.sh

Then build the GPU package with:
  ./build-win-x64.sh

Options:
  --install  install cargo-xwin and the Rust target when they are missing
EOF
}

probe_command() {
  local candidate
  for candidate in "$@"; do
    if command -v "$candidate" >/dev/null 2>&1; then
      echo "$candidate"
      return 0
    fi
  done
  return 1
}

if [[ "${1:-}" == "--help" ]]; then
  usage
  exit 0
fi

if [[ "${1:-}" == "--install" ]]; then
  AUTO_INSTALL=1
elif [[ $# -ne 0 ]]; then
  echo "error: unknown arguments: $*" >&2
  exit 1
fi

echo "==> Checking Linux-host Windows MSVC build prerequisites"

if cargo xwin --version >/dev/null 2>&1; then
  echo "  - cargo-xwin: installed"
else
  echo "  - cargo-xwin: missing"
  if [[ "$AUTO_INSTALL" -eq 1 ]]; then
    cargo install cargo-xwin
  else
    echo "    run: cargo install cargo-xwin"
  fi
fi

if rustup target list --installed | grep -qx "$TARGET"; then
  echo "  - rustup target $TARGET: installed"
else
  echo "  - rustup target $TARGET: missing"
  if [[ "$AUTO_INSTALL" -eq 1 ]]; then
    rustup target add "$TARGET"
  else
    echo "    run: rustup target add $TARGET"
  fi
fi

if CLANG_CMD="$(probe_command clang-19 clang)"; then
  echo "  - clang: using $CLANG_CMD"
else
  echo "  - clang: missing"
  echo "    run: ./install-apt-packages.sh"
fi

echo
echo "Next step:"
echo "  TARGET=$TARGET $ROOT_DIR/build-win-x64.sh"

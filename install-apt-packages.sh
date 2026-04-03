#!/usr/bin/env bash
# Installs the native toolchain packages expected by Linux and Windows cross-build workflows.

set -euo pipefail

APT_PACKAGES=(
  gcc-mingw-w64-x86-64-posix
  nasm
  llvm-19
  clang-19
  zip
  libwayland-dev
  pkg-config
)

describe_package() {
  # Keep the human-readable inventory next to the machine-readable package list so contributors can
  # understand why each dependency exists before installing the full build toolchain.
  case "$1" in
    gcc-mingw-w64-x86-64-posix)
      echo "provides x86_64-w64-mingw32-gcc for Windows GNU builds"
      ;;
    nasm)
      echo "provides the assembler required by aws-lc-sys on the Windows GNU build path"
      ;;
    llvm-19)
      echo "provides llvm resource compiler tooling for Windows MSVC validation"
      ;;
    clang-19)
      echo "provides clang for Windows MSVC cross-target validation"
      ;;
    zip)
      echo "provides zip packaging used by build-desktop.sh for Windows artifacts"
      ;;
    libwayland-dev)
      echo "provides wayland-client.pc needed by the Slint/winit Linux host build path"
      ;;
    pkg-config)
      echo "helps native dependency discovery such as wayland-client.pc on Debian/Ubuntu"
      ;;
    *)
      echo "no description available"
      ;;
  esac
}

usage() {
  cat <<'EOF'
Usage: ./install-apt-packages.sh [--help]

Interactive apt installer for the current Mica Term Windows build chain.

Packages that will be offered for installation:
  - gcc-mingw-w64-x86-64-posix
  - nasm
  - llvm-19
  - clang-19
  - zip
  - libwayland-dev
  - pkg-config

Behavior:
  1. Print the package list and purpose
  2. Ask for a single confirmation
  3. Run apt-get update
  4. Run apt-get install -y ...
  5. Print installation status and command probes
EOF
}

choose_apt_runner() {
  if [[ "${EUID:-$(id -u)}" -eq 0 ]]; then
    echo "apt-get"
    return
  fi

  if command -v sudo >/dev/null 2>&1; then
    echo "sudo apt-get"
    return
  fi

  echo "error: root privileges are required and sudo is not available." >&2
  exit 1
}

print_package_list() {
  echo "Packages that will be installed:"
  for package in "${APT_PACKAGES[@]}"; do
    echo "  - $package: $(describe_package "$package")"
  done
}

print_installation_status() {
  echo
  echo "Installation status:"
  for package in "${APT_PACKAGES[@]}"; do
    if dpkg-query -W -f='${Status} ${Version}\n' "$package" >/tmp/mica-term-apt-status.$$ 2>/dev/null; then
      echo "  - $package: $(cat /tmp/mica-term-apt-status.$$)"
    else
      echo "  - $package: not installed"
    fi
  done
  rm -f /tmp/mica-term-apt-status.$$
}

probe_command() {
  local label="$1"
  shift
  local candidate
  for candidate in "$@"; do
    if command -v "$candidate" >/dev/null 2>&1; then
      local version_line
      version_line="$("$candidate" --version 2>/dev/null | head -n 1 || true)"
      echo "  - $label: $candidate (${version_line:-version probe unavailable})"
      return
    fi
  done

  echo "  - $label: unavailable"
}

print_command_probes() {
  echo
  echo "Command probes:"
  probe_command "MinGW linker" x86_64-w64-mingw32-gcc
  probe_command "NASM" nasm
  probe_command "Clang" clang-19 clang
  probe_command "LLVM resource compiler" llvm-rc-19 llvm-rc
  probe_command "zip" zip

  if command -v pkg-config >/dev/null 2>&1; then
    local wayland_version
    wayland_version="$(pkg-config --modversion wayland-client 2>/dev/null || true)"
    if [[ -n "$wayland_version" ]]; then
      echo "  - wayland-client.pc: found ($wayland_version)"
    else
      echo "  - wayland-client.pc: not resolved by pkg-config"
    fi
  else
    echo "  - pkg-config: unavailable"
  fi
}

print_follow_up_notes() {
  cat <<'EOF'

Follow-up reminders:
  - cargo install cargo-xwin
  - rustup target add x86_64-pc-windows-gnu
  - rustup target add x86_64-pc-windows-msvc
  - verify cargo and rustup are available in PATH
  - run ./scripts/bootstrap-win-msvc-build.sh before ./build-win-x64.sh on Linux hosts
  - see ./apt-packages.md for the current dependency inventory
EOF
}

if [[ "${1:-}" == "--help" ]]; then
  usage
  exit 0
fi

if [[ $# -ne 0 ]]; then
  echo "error: unknown arguments: $*" >&2
  exit 1
fi

print_package_list
echo
read -r -p "Type y to run apt-get update && apt-get install -y ${APT_PACKAGES[*]}: " CONFIRM

if [[ "$CONFIRM" != "y" ]]; then
  echo "Installation cancelled."
  exit 0
fi

APT_RUNNER="$(choose_apt_runner)"
echo
echo "Running: $APT_RUNNER update"
eval "$APT_RUNNER update"
echo "Running: $APT_RUNNER install -y ${APT_PACKAGES[*]}"
eval "$APT_RUNNER install -y ${APT_PACKAGES[*]}"

print_installation_status
print_command_probes
print_follow_up_notes

#!/usr/bin/env bash
# Packages the current target binary into dist/ with a minimal distributable layout.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TARGET="${TARGET:-x86_64-unknown-linux-gnu}"
PROFILE="${PROFILE:-release}"
APP_NAME="${APP_NAME:-$(awk -F'"' '/^name = / { print $2; exit }' "$ROOT_DIR/Cargo.toml")}"
BIN_NAME="${BIN_NAME:-$APP_NAME}"
DIST_DIR="${DIST_DIR:-$ROOT_DIR/dist}"
PACKAGE_FLAVOR_SUFFIX="${PACKAGE_FLAVOR_SUFFIX:-}"
ARCHIVE_STEM="${APP_NAME}-${TARGET}-${PROFILE}${PACKAGE_FLAVOR_SUFFIX}"

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
  x86_64-pc-windows-gnu      Windows x64 GNU build with MinGW-w64 and nasm
  x86_64-pc-windows-msvc     Windows x64 MSVC build on Windows MSVC hosts or Linux hosts with cargo-xwin
  aarch64-pc-windows-msvc    Windows ARM64 MSVC build on Windows MSVC hosts or Linux hosts with cargo-xwin

Environment overrides:
  TARGET=<target triple>
  PROFILE=release|debug
  APP_NAME=<package name>
  BIN_NAME=<binary file name without extension>
  DIST_DIR=<output directory>
  CARGO_FEATURES=<space or comma separated cargo features>
  CARGO_NO_DEFAULT_FEATURES=1
  MICA_TERM_PACKAGE_PORTABLE=1  create .mica-term-portable in staged Windows packages
  CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=<gnu linker path>
  CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=<linux arm64 linker path>

Windows GNU prerequisites on Linux hosts:
  x86_64-w64-mingw32-gcc
  nasm

Windows MSVC prerequisites on Linux hosts:
  cargo-xwin
  clang or clang-19
  lld-link or lld-link-19 (provided by lld)
  llvm-ar or llvm-ar-19 (provided by llvm-19)
  llvm-lib or llvm-lib-19 (provided by llvm-19)
  llvm-rc or llvm-rc-19 (provided by llvm-19)
  rustup target add x86_64-pc-windows-msvc
  rustup target add aarch64-pc-windows-msvc

Output:
  dist/<app>-<target>-<profile><package flavor suffix>.tar.gz
  dist/<app>-<target>-<profile><package flavor suffix>.zip
EOF
}

fail() {
  echo "error: $*" >&2
  exit 1
}

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || fail "missing required command: $1"
}

require_cargo_xwin() {
  cargo xwin --version >/dev/null 2>&1 || \
    fail "Linux-host Windows MSVC packaging requires cargo-xwin. Install it with: cargo install cargo-xwin"
}

WINDOWS_MSVC_TOOL_SHIM_DIR=""
WINDOWS_MSVC_LIB_SHIM_DIR=""

choose_clang_command() {
  local candidate
  for candidate in clang-19 clang; do
    if command -v "$candidate" >/dev/null 2>&1; then
      echo "$candidate"
      return 0
    fi
  done

  fail "Linux-host Windows MSVC packaging requires clang or clang-19. Install it via ./install-apt-packages.sh"
}

choose_llvm_lib_command() {
  local candidate
  for candidate in llvm-lib-19 llvm-lib; do
    if command -v "$candidate" >/dev/null 2>&1; then
      echo "$candidate"
      return 0
    fi
  done

  fail "Linux-host Windows MSVC packaging requires llvm-lib or llvm-lib-19. Install llvm-19 via ./install-apt-packages.sh"
}

choose_llvm_rc_command() {
  local candidate
  for candidate in llvm-rc-19 llvm-rc; do
    if command -v "$candidate" >/dev/null 2>&1; then
      echo "$candidate"
      return 0
    fi
  done

  fail "Linux-host Windows MSVC packaging requires llvm-rc or llvm-rc-19. Install llvm-19 via ./install-apt-packages.sh"
}

choose_llvm_ar_command() {
  local candidate
  for candidate in llvm-ar-19 llvm-ar; do
    if command -v "$candidate" >/dev/null 2>&1; then
      echo "$candidate"
      return 0
    fi
  done

  fail "Linux-host Windows MSVC packaging requires llvm-ar or llvm-ar-19. Install llvm-19 via ./install-apt-packages.sh"
}

choose_lld_link_command() {
  local candidate
  for candidate in lld-link-19 lld-link; do
    if command -v "$candidate" >/dev/null 2>&1; then
      echo "$candidate"
      return 0
    fi
  done

  fail "Linux-host Windows MSVC packaging requires lld-link or lld-link-19. Install lld via ./install-apt-packages.sh"
}

cleanup_stale_cargo_xwin_symlinks() {
  local cargo_xwin_cache
  cargo_xwin_cache="$(windows_msvc_cargo_xwin_cache_dir)"

  if [[ -L "$cargo_xwin_cache/clang-cl" && ! -e "$cargo_xwin_cache/clang-cl" ]]; then
    rm -f "$cargo_xwin_cache/clang-cl"
  fi
}

windows_msvc_cargo_xwin_cache_dir() {
  echo "${XWIN_CACHE_DIR:-${XDG_CACHE_HOME:-$HOME/.cache}/cargo-xwin}"
}

windows_msvc_xwin_arch() {
  case "$1" in
    x86_64-pc-windows-msvc)
      echo "x86_64"
      ;;
    aarch64-pc-windows-msvc)
      echo "aarch64"
      ;;
    *)
      fail "unsupported Windows MSVC target '$1' for xwin cache lookup"
      ;;
  esac
}

windows_msvc_xwin_um_lib_dir() {
  local arch="$1"
  echo "$(windows_msvc_cargo_xwin_cache_dir)/xwin/sdk/lib/um/$arch"
}

choose_windows_msvc_import_lib_path() {
  local lib_dir="$1" requested_name="$2" candidate path

  if [[ -f "$lib_dir/$requested_name" ]]; then
    echo "$lib_dir/$requested_name"
    return 0
  fi

  for candidate in "${requested_name,,}" "AdvAPI32.Lib" "ADVAPI32.lib"; do
    if [[ -f "$lib_dir/$candidate" ]]; then
      echo "$lib_dir/$candidate"
      return 0
    fi
  done

  path="$(find "$lib_dir" -maxdepth 1 -type f -iname "$requested_name" | head -n1 || true)"
  if [[ -n "$path" ]]; then
    echo "$path"
    return 0
  fi

  fail "missing Windows import library '$requested_name' in $lib_dir. Refresh the cargo-xwin cache."
}

setup_linux_windows_msvc_tool_shims() {
  local clang_cmd clang_path lld_link_cmd lld_link_path llvm_ar_cmd llvm_ar_path
  local llvm_lib_cmd llvm_lib_path llvm_rc_cmd llvm_rc_path

  clang_cmd="$(choose_clang_command)"
  lld_link_cmd="$(choose_lld_link_command)"
  llvm_ar_cmd="$(choose_llvm_ar_command)"
  llvm_lib_cmd="$(choose_llvm_lib_command)"
  llvm_rc_cmd="$(choose_llvm_rc_command)"
  clang_path="$(command -v "$clang_cmd")"
  lld_link_path="$(command -v "$lld_link_cmd")"
  llvm_ar_path="$(command -v "$llvm_ar_cmd")"
  llvm_lib_path="$(command -v "$llvm_lib_cmd")"
  llvm_rc_path="$(command -v "$llvm_rc_cmd")"

  WINDOWS_MSVC_TOOL_SHIM_DIR="$ROOT_DIR/target/cargo-xwin-tools"
  mkdir -p "$WINDOWS_MSVC_TOOL_SHIM_DIR"
  cleanup_stale_cargo_xwin_symlinks

  # cargo-xwin hard-codes the unversioned tool names on Linux, so provide a
  # stable PATH shim when the distro ships only versioned LLVM binaries.
  ln -sf "$clang_path" "$WINDOWS_MSVC_TOOL_SHIM_DIR/clang"
  ln -sf "$llvm_lib_path" "$WINDOWS_MSVC_TOOL_SHIM_DIR/llvm-lib"
  ln -sf "$llvm_rc_path" "$WINDOWS_MSVC_TOOL_SHIM_DIR/llvm-rc"

  cat > "$WINDOWS_MSVC_TOOL_SHIM_DIR/lld-link" <<EOF
#!/usr/bin/env bash
set -euo pipefail

args=()
for arg in "\$@"; do
  if [[ "\${MICA_TERM_WINDOWS_MSVC_STRIP_ICU_IMPORTS:-0}" == "1" ]]; then
    case "\$arg" in
      */registry/src/*/windows_x86_64_msvc-*/lib/windows.*.lib)
        relative_path="\${arg#*/registry/src/}"
        patched_path="${ROOT_DIR}/target/cargo-xwin-patched-registry/\$relative_path"
        if [[ ! -f "\$patched_path" ]]; then
          mkdir -p "\$(dirname "\$patched_path")"
          cp "\$arg" "\$patched_path"
        fi
        while read -r member_count member_name; do
          [[ -n "\$member_name" ]] || continue
          # The bundled Windows metadata import archives expose ICU import DLL
          # thunks that collide with Skia's statically linked ICU on Linux-host
          # MSVC builds, so strip every matching import member from the copied
          # archive before handing it to lld-link.
          for ((i = 0; i < member_count; i++)); do
            "${llvm_ar_path}" dN 1 "\$patched_path" "\$member_name"
          done
        done < <("${llvm_ar_path}" t "\$patched_path" | grep -E '^icu.*\\.dll$' | sort | uniq -c || true)
        args+=("\$patched_path")
        continue
        ;;
    esac
  fi
  args+=("\$arg")
done

exec "${lld_link_path}" "\${args[@]}"
EOF
  chmod +x "$WINDOWS_MSVC_TOOL_SHIM_DIR/lld-link"
  export PATH="$WINDOWS_MSVC_TOOL_SHIM_DIR:$PATH"
}

setup_linux_windows_msvc_library_shims() {
  local target="$1" xwin_arch xwin_um_lib_dir advapi32_path

  xwin_arch="$(windows_msvc_xwin_arch "$target")"
  xwin_um_lib_dir="$(windows_msvc_xwin_um_lib_dir "$xwin_arch")"
  [[ -d "$xwin_um_lib_dir" ]] || fail \
    "missing cargo-xwin Windows SDK libraries at $xwin_um_lib_dir. Refresh the cargo-xwin cache."

  advapi32_path="$(choose_windows_msvc_import_lib_path "$xwin_um_lib_dir" "Advapi32.lib")"
  WINDOWS_MSVC_LIB_SHIM_DIR="$ROOT_DIR/target/cargo-xwin-libs"
  mkdir -p "$WINDOWS_MSVC_LIB_SHIM_DIR"

  # Skia asks lld-link for the mixed-case import lib name used on Windows, so
  # provide that spelling on case-sensitive Linux filesystems.
  ln -sf "$advapi32_path" "$WINDOWS_MSVC_LIB_SHIM_DIR/Advapi32.lib"

  if [[ -n "${LIB:-}" ]]; then
    export LIB="$WINDOWS_MSVC_LIB_SHIM_DIR;$LIB"
  else
    export LIB="$WINDOWS_MSVC_LIB_SHIM_DIR"
  fi
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

CARGO_BUILD_CMD=(cargo build)

USES_WINDOWS_SKIA=0
if [[ "${MICA_TERM_PACKAGE_RENDERER:-}" == "skia" || "${MICA_TERM_PACKAGE_RENDERER:-}" == "skia-software" ]]; then
  USES_WINDOWS_SKIA=1
fi
if [[ "${CARGO_FEATURES:-}" == *"slint-renderer-skia"* ]]; then
  USES_WINDOWS_SKIA=1
fi

if [[ "$TARGET" == "x86_64-pc-windows-gnu" && "$USES_WINDOWS_SKIA" -eq 1 ]]; then
  fail "Skia Windows GNU packaging is unsupported by rust-skia upstream. Use ./build-win-x64-software.sh for Linux-host Windows packages, or switch to x86_64-pc-windows-msvc via cargo-xwin on Linux or a Windows MSVC shell."
fi

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
    require_cmd nasm
    GNU_LINKER="${CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER:-x86_64-w64-mingw32-gcc}"
    command -v "$GNU_LINKER" >/dev/null 2>&1 || fail \
      "Windows GNU target requires linker '$GNU_LINKER'. Install MinGW-w64 or set CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER."
    export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER="$GNU_LINKER"
    BIN_SUFFIX=".exe"
    ARCHIVE_SUFFIX=".zip"
    ;;
  x86_64-pc-windows-msvc|aarch64-pc-windows-msvc)
    require_cmd zip
    case "$(uname -s)" in
      MINGW*|MSYS*|CYGWIN*)
        ;;
      Linux)
        require_cargo_xwin
        setup_linux_windows_msvc_tool_shims
        setup_linux_windows_msvc_library_shims "$TARGET"
        if [[ "$USES_WINDOWS_SKIA" -eq 1 ]]; then
          export MICA_TERM_WINDOWS_MSVC_STRIP_ICU_IMPORTS=1
        fi
        export CC=clang
        CARGO_BUILD_CMD=(cargo xwin build)
        ;;
      *)
        fail "target '$TARGET' must be built from a Windows MSVC shell/Git Bash environment or from a Linux host with cargo-xwin + clang."
        ;;
    esac
    BIN_SUFFIX=".exe"
    ARCHIVE_SUFFIX=".zip"
    ;;
  *)
    fail "unsupported TARGET '$TARGET'"
    ;;
esac

STAGE_DIR="$DIST_DIR/$ARCHIVE_STEM"
ARCHIVE_PATH="$DIST_DIR/${ARCHIVE_STEM}${ARCHIVE_SUFFIX}"
BIN_PATH="$ROOT_DIR/target/$TARGET/$PROFILE/$BIN_NAME$BIN_SUFFIX"

echo "==> Building $BIN_NAME for $TARGET ($PROFILE)"
CARGO_BUILD_ARGS=("${PROFILE_ARGS[@]}" --target "$TARGET" --locked)

if [[ "${CARGO_NO_DEFAULT_FEATURES:-0}" == "1" ]]; then
  CARGO_BUILD_ARGS+=(--no-default-features)
fi

if [[ -n "${CARGO_FEATURES:-}" ]]; then
  CARGO_BUILD_ARGS+=(--features "$CARGO_FEATURES")
fi

"${CARGO_BUILD_CMD[@]}" "${CARGO_BUILD_ARGS[@]}"

[[ -f "$BIN_PATH" ]] || fail "expected binary not found: $BIN_PATH"

echo "==> Staging package in $STAGE_DIR"
# Stage a minimal distributable layout first, then archive that directory so downstream release
# tooling can inspect the unpacked tree or the final artifact interchangeably.
mkdir -p "$DIST_DIR"
rm -rf "$STAGE_DIR"
mkdir -p "$STAGE_DIR"
cp "$BIN_PATH" "$STAGE_DIR/"

if [[ "$TARGET" == *windows* && "${MICA_TERM_PACKAGE_PORTABLE:-0}" == "1" ]]; then
  : > "$STAGE_DIR/.mica-term-portable"
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

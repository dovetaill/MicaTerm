#!/usr/bin/env bash
# Ensures Linux-host Windows MSVC packaging can shim versioned LLVM tools into
# the unversioned command names cargo-xwin expects.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT_SOURCE="$ROOT_DIR/build-desktop.sh"
WINDOWS_WRAPPER="$ROOT_DIR/build-win-x64.sh"
DESIGN_DOC="$ROOT_DIR/docs/plans/2026-04-04-windows-terminal-text-rendering-design.md"
PLAN_DOC="$ROOT_DIR/docs/plans/2026-04-04-windows-terminal-text-rendering-implementation-plan.md"

grep -F 'export MICA_TERM_PACKAGE_TERMINAL_RENDERER="native"' "$WINDOWS_WRAPPER" >/dev/null
grep -F 'export MICA_TERM_PACKAGE_NATIVE_PRESENT_PATH="event-loop"' "$WINDOWS_WRAPPER" >/dev/null
grep -F 'export MICA_TERM_EXPECTED_TEXT_RENDERER_PATH="directwrite-d2d"' "$WINDOWS_WRAPPER" >/dev/null
grep -F 'export MICA_TERM_TEXT_RENDERER_FALLBACK_PATH="bitmap-mask-compat"' "$WINDOWS_WRAPPER" >/dev/null
grep -F 'expected primary text path: directwrite-d2d' "$WINDOWS_WRAPPER" >/dev/null
grep -F 'export MICA_TERM_VERIFICATION_DPI_SCALE_MATRIX="100,125,150"' "$WINDOWS_WRAPPER" >/dev/null
grep -F 'export MICA_TERM_VERIFICATION_FONT_PX_MATRIX="12,13,14,15"' "$WINDOWS_WRAPPER" >/dev/null
grep -F 'Implementation closure note' "$DESIGN_DOC" >/dev/null
grep -F 'Diagnostics ship as trace/log hooks only' "$DESIGN_DOC" >/dev/null
grep -F 'Verification hooks:' "$PLAN_DOC" >/dev/null
grep -F 'directwrite-d2d' "$PLAN_DOC" >/dev/null
grep -F '`build-win-x64.sh` exports the authoritative Windows validation matrix' "$PLAN_DOC" >/dev/null

TMP_ROOT="$(mktemp -d)"
cleanup() {
  rm -rf "$TMP_ROOT"
}
trap cleanup EXIT

PROJECT_DIR="$TMP_ROOT/project"
FAKE_BIN="$TMP_ROOT/bin"
mkdir -p \
  "$PROJECT_DIR/assets/icons/windows" \
  "$PROJECT_DIR/assets/fonts/MiSans" \
  "$PROJECT_DIR/assets/fonts/SarasaTermSC" \
  "$FAKE_BIN"

cp "$SCRIPT_SOURCE" "$PROJECT_DIR/build-desktop.sh"

cat <<'TOML' > "$PROJECT_DIR/Cargo.toml"
[package]
name = "mica-term"
version = "0.1.0"
edition = "2021"
TOML

printf '# stub readme\n' > "$PROJECT_DIR/readme.md"
printf 'icon\n' > "$PROJECT_DIR/assets/icons/windows/mica-term.ico"
printf 'misans license\n' > "$PROJECT_DIR/assets/fonts/MiSans/LICENSE.txt"
printf 'sarasa term license\n' > "$PROJECT_DIR/assets/fonts/SarasaTermSC/LICENSE.txt"

cat <<'EOF_CARGO' > "$FAKE_BIN/cargo"
#!/usr/bin/env bash
set -euo pipefail

case "${1:-}" in
  xwin)
    shift
    case "${1:-}" in
      --version)
        echo 'cargo-xwin-xwin 0.21.4'
        exit 0
        ;;
      build)
        shift
        cache_root="${XDG_CACHE_HOME:-$HOME/.cache}"
        clang_cl_symlink="$cache_root/cargo-xwin/clang-cl"
        advapi_shim="$FAKE_PROJECT_DIR/target/cargo-xwin-libs/Advapi32.lib"

        if [[ -L "$clang_cl_symlink" && ! -e "$clang_cl_symlink" ]]; then
          echo "stale cargo-xwin clang-cl symlink: $clang_cl_symlink" >&2
          exit 1
        fi
        if [[ ! -L "$advapi_shim" ]]; then
          echo "expected Advapi32 shim symlink at $advapi_shim" >&2
          exit 1
        fi
        if [[ "$(readlink "$advapi_shim")" != "$cache_root/cargo-xwin/xwin/sdk/lib/um/x86_64/advapi32.lib" ]]; then
          echo "unexpected Advapi32 shim target: $(readlink "$advapi_shim")" >&2
          exit 1
        fi

        clang_target="$(readlink -f "$(command -v clang)")"
        llvm_lib_target="$(readlink -f "$(command -v llvm-lib)")"
        llvm_rc_target="$(readlink -f "$(command -v llvm-rc)")"
        lld_link_wrapper="$(command -v lld-link)"
        patched_windows_lib="$FAKE_PROJECT_DIR/target/cargo-xwin-patched-registry/index.crates.io-test/windows_x86_64_msvc-0.52.6/lib/windows.0.52.0.lib"
        source_windows_lib="$HOME/.cargo/registry/src/index.crates.io-test/windows_x86_64_msvc-0.52.6/lib/windows.0.52.0.lib"

        [[ "$(basename "$clang_target")" == "clang-19" ]] || {
          echo "expected clang shim to resolve to clang-19, got $clang_target" >&2
          exit 1
        }
        [[ "$(basename "$llvm_lib_target")" == "llvm-lib-19" ]] || {
          echo "expected llvm-lib shim to resolve to llvm-lib-19, got $llvm_lib_target" >&2
          exit 1
        }
        [[ "$(basename "$llvm_rc_target")" == "llvm-rc-19" ]] || {
          echo "expected llvm-rc shim to resolve to llvm-rc-19, got $llvm_rc_target" >&2
          exit 1
        }
        [[ "$lld_link_wrapper" == "$FAKE_PROJECT_DIR/target/cargo-xwin-tools/lld-link" ]] || {
          echo "expected lld-link wrapper at $FAKE_PROJECT_DIR/target/cargo-xwin-tools/lld-link, got $lld_link_wrapper" >&2
          exit 1
        }
        lld-link "$source_windows_lib"
        [[ -f "$patched_windows_lib" ]] || {
          echo "expected patched Windows import lib at $patched_windows_lib" >&2
          exit 1
        }
        if grep -Fxq 'icu.dll' "$patched_windows_lib"; then
          echo "patched Windows import lib should have icu.dll removed" >&2
          exit 1
        fi

        target=''
        profile='debug'
        while [[ $# -gt 0 ]]; do
          case "$1" in
            --target)
              target="$2"
              shift 2
              ;;
            --release)
              profile='release'
              shift
              ;;
            *)
              shift
              ;;
          esac
        done

        [[ -n "$target" ]] || {
          echo 'missing --target in fake cargo build invocation' >&2
          exit 1
        }

        mkdir -p "$(dirname "$clang_cl_symlink")"
        ln -sfn "$(command -v clang)" "$clang_cl_symlink"
        mkdir -p "$FAKE_PROJECT_DIR/target/$target/$profile"
        printf 'stub exe\n' > "$FAKE_PROJECT_DIR/target/$target/$profile/mica-term.exe"
        exit 0
        ;;
    esac
    ;;
esac

echo "unexpected cargo invocation: $*" >&2
exit 1
EOF_CARGO

cat <<'EOF_RUSTUP' > "$FAKE_BIN/rustup"
#!/usr/bin/env bash
set -euo pipefail

if [[ "$*" == 'target list --installed' ]]; then
  echo 'x86_64-pc-windows-msvc'
  exit 0
fi

echo "unexpected rustup invocation: $*" >&2
exit 1
EOF_RUSTUP

cat <<'EOF_ZIP' > "$FAKE_BIN/zip"
#!/usr/bin/env bash
set -euo pipefail

archive=''
for arg in "$@"; do
  if [[ "$arg" != -* ]]; then
    archive="$arg"
    break
  fi
done

[[ -n "$archive" ]] || {
  echo 'missing archive path for fake zip' >&2
  exit 1
}

touch "$PWD/$archive"
EOF_ZIP

for tool in clang llvm-lib llvm-rc; do
  cat <<'EOF_BAD' > "$FAKE_BIN/$tool"
#!/usr/bin/env bash
set -euo pipefail
echo "unexpected direct invocation of unshimmed tool: $(basename "$0")" >&2
exit 99
EOF_BAD
done

for tool in clang-19 llvm-lib-19 llvm-rc-19; do
  cat <<'EOF_GOOD' > "$FAKE_BIN/$tool"
#!/usr/bin/env bash
set -euo pipefail
exit 0
EOF_GOOD
done

cat <<'EOF_LLVM_AR' > "$FAKE_BIN/llvm-ar-19"
#!/usr/bin/env bash
set -euo pipefail

mode="${1:-}"
count=''

case "$mode" in
  dN)
    count="${2:-}"
    archive="${3:-}"
    member="${4:-}"
    ;;
  *)
    archive="${2:-}"
    member="${3:-}"
    ;;
esac

case "$mode" in
  t)
    cat "$archive"
    ;;
  d|dN)
    tmp="$(mktemp)"
    grep -Fxv "$member" "$archive" > "$tmp" || true
    mv "$tmp" "$archive"
    ;;
  *)
    echo "unexpected fake llvm-ar invocation: $*" >&2
    exit 1
    ;;
esac
EOF_LLVM_AR

cat <<'EOF_LLD' > "$FAKE_BIN/lld-link-19"
#!/usr/bin/env bash
set -euo pipefail

expected="${FAKE_EXPECTED_PATCHED_WINDOWS_LIB:-}"
[[ -n "$expected" ]] || {
  echo 'missing FAKE_EXPECTED_PATCHED_WINDOWS_LIB for fake lld-link-19' >&2
  exit 1
}

for arg in "$@"; do
  if [[ "$arg" == "$expected" ]]; then
    if grep -Fxq 'icu.dll' "$arg"; then
      echo "fake lld-link-19 received unpatched ICU import archive: $arg" >&2
      exit 1
    fi
    exit 0
  fi
done

echo "fake lld-link-19 did not receive patched Windows import lib: $expected" >&2
exit 1
EOF_LLD

chmod +x "$FAKE_BIN"/*

mkdir -p "$TMP_ROOT/home/.cache/cargo-xwin/xwin/sdk/lib/um/x86_64"
printf 'stub advapi32\n' > "$TMP_ROOT/home/.cache/cargo-xwin/xwin/sdk/lib/um/x86_64/advapi32.lib"
mkdir -p "$TMP_ROOT/home/.cargo/registry/src/index.crates.io-test/windows_x86_64_msvc-0.52.6/lib"
printf 'icu.dll\nkernel32.dll\n' > "$TMP_ROOT/home/.cargo/registry/src/index.crates.io-test/windows_x86_64_msvc-0.52.6/lib/windows.0.52.0.lib"

PATH="$FAKE_BIN:/usr/bin:/bin" \
FAKE_PROJECT_DIR="$PROJECT_DIR" \
FAKE_EXPECTED_PATCHED_WINDOWS_LIB="$PROJECT_DIR/target/cargo-xwin-patched-registry/index.crates.io-test/windows_x86_64_msvc-0.52.6/lib/windows.0.52.0.lib" \
HOME="$TMP_ROOT/home" \
TARGET=x86_64-pc-windows-msvc \
PROFILE=debug \
MICA_TERM_PACKAGE_RENDERER=skia \
DIST_DIR="$PROJECT_DIR/out" \
bash "$PROJECT_DIR/build-desktop.sh"

PATH="$FAKE_BIN:/usr/bin:/bin" \
FAKE_PROJECT_DIR="$PROJECT_DIR" \
FAKE_EXPECTED_PATCHED_WINDOWS_LIB="$PROJECT_DIR/target/cargo-xwin-patched-registry/index.crates.io-test/windows_x86_64_msvc-0.52.6/lib/windows.0.52.0.lib" \
HOME="$TMP_ROOT/home" \
TARGET=x86_64-pc-windows-msvc \
PROFILE=debug \
MICA_TERM_PACKAGE_RENDERER=skia \
DIST_DIR="$PROJECT_DIR/out" \
bash "$PROJECT_DIR/build-desktop.sh"

[[ -f "$PROJECT_DIR/out/mica-term-x86_64-pc-windows-msvc-debug.zip" ]]
[[ -f "$PROJECT_DIR/out/mica-term-x86_64-pc-windows-msvc-debug/mica-term.exe" ]]
[[ -f "$PROJECT_DIR/out/mica-term-x86_64-pc-windows-msvc-debug/licenses/fonts/MiSans/LICENSE.txt" ]]
[[ -f "$PROJECT_DIR/out/mica-term-x86_64-pc-windows-msvc-debug/licenses/fonts/SarasaTermSC/LICENSE.txt" ]]
[[ ! -e "$PROJECT_DIR/out/mica-term-x86_64-pc-windows-msvc-debug/README.md" ]]
[[ ! -e "$PROJECT_DIR/out/mica-term-x86_64-pc-windows-msvc-debug/OFL.txt" ]]
[[ ! -e "$PROJECT_DIR/out/mica-term-x86_64-pc-windows-msvc-debug/mica-term.ico" ]]

#!/usr/bin/env bash
# Thin wrapper around the Windows x64 Skia GPU mainline package used by CI and manual packaging.
# Packaged terminal subsystem: retained-native-surface.
# Packaged Skia builds now target the same-host-window retained-native path
# and keep notifier-driven host redraw replay as the primary present contract.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
APP_NAME="${APP_NAME:-$(awk -F'\"' '/^name = / { print $2; exit }' "$ROOT_DIR/Cargo.toml")}"
PROFILE="${PROFILE:-release}"
DIST_DIR="${DIST_DIR:-$ROOT_DIR/dist}"
PUBLISH_DIR="${PUBLISH_DIR:-/var/www/html}"

detect_default_build_jobs() {
  local detected_jobs="" detected_source=""

  if [[ -n "${BUILD_JOBS:-}" ]]; then
    unset MICA_TERM_BUILD_JOBS_AUTO || true
    unset MICA_TERM_BUILD_JOBS_SOURCE || true
    return 0
  fi

  unset MICA_TERM_BUILD_JOBS_AUTO || true
  unset MICA_TERM_BUILD_JOBS_SOURCE || true

  if command -v nproc >/dev/null 2>&1; then
    detected_jobs="$(nproc 2>/dev/null || true)"
    detected_source="nproc"
  elif command -v getconf >/dev/null 2>&1; then
    detected_jobs="$(getconf _NPROCESSORS_ONLN 2>/dev/null || true)"
    detected_source="getconf"
  elif [[ -n "${NUMBER_OF_PROCESSORS:-}" ]]; then
    detected_jobs="${NUMBER_OF_PROCESSORS:-}"
    detected_source="NUMBER_OF_PROCESSORS"
  fi

  if [[ "$detected_jobs" =~ ^[1-9][0-9]*$ ]]; then
    export BUILD_JOBS="$detected_jobs"
    export MICA_TERM_BUILD_JOBS_AUTO=1
    export MICA_TERM_BUILD_JOBS_SOURCE="$detected_source"
  fi
}

usage() {
  cat <<'EOF'
Usage: ./build-win-x64.sh [--help]

Windows Skia GPU wrapper.
Direct3D-first native terminal renderer package with retained-native presenter default.

Default target:
  x86_64-pc-windows-msvc
  supported hosts: Windows MSVC shell/Git Bash or Linux + cargo-xwin + clang
  packaged renderer: winit-skia
  packaged terminal renderer: native
  live Windows terminal path: retained-native host surface
  packaged native present path: rendering-notifier
  expected primary text path: directwrite-d2d
  compatibility text fallback path: bitmap-mask-compat
  verification matrix: DPI 100% | 125% | 150%; font sizes 12px | 13px | 14px | 15px
  runtime fallback chain: winit-skia+d3d -> winit-skia-software -> winit-software

Linux-host Windows GNU package:
  ./build-win-x64-software.sh

Outputs:
  dist/mica-term-x86_64-pc-windows-msvc-release-skia.zip
  /var/www/html/mica-term-x86_64-pc-windows-msvc-release-skia.zip

Default parallelism:
  auto-detects parallel jobs for this wrapper when BUILD_JOBS is unset
  probe order: nproc -> getconf _NPROCESSORS_ONLN -> NUMBER_OF_PROCESSORS

Parallel override:
  BUILD_JOBS=<positive integer> ./build-win-x64.sh
EOF
}

if [[ "${1:-}" == "--help" ]]; then
  usage
  exit 0
fi

TARGET="${TARGET:-x86_64-pc-windows-msvc}"

if [[ "$TARGET" == "x86_64-pc-windows-gnu" ]]; then
  cat >&2 <<'EOF'
error: rust-skia does not ship Windows GNU Skia binaries for x86_64-pc-windows-gnu.
Use ./build-win-x64-software.sh for GNU/software compatibility packages, or run TARGET=x86_64-pc-windows-msvc ./build-win-x64.sh for the MSVC GPU package.
EOF
  exit 1
fi

echo "==> Windows wrapper target: $TARGET"
export TARGET
export CARGO_NO_DEFAULT_FEATURES=1
export CARGO_FEATURES="slint-renderer-skia,terminal-native-renderer"
export MICA_TERM_BUILD_FLAVOR="windows-mainline"
export MICA_TERM_PACKAGE_RENDERER="skia"
export MICA_TERM_PACKAGE_TERMINAL_RENDERER="native"
export MICA_TERM_PACKAGE_NATIVE_PRESENT_PATH="rendering-notifier"
export MICA_TERM_EXPECTED_TEXT_RENDERER_PATH="directwrite-d2d"
export MICA_TERM_TEXT_RENDERER_FALLBACK_PATH="bitmap-mask-compat"
export MICA_TERM_VERIFICATION_DPI_SCALE_MATRIX="100,125,150"
export MICA_TERM_VERIFICATION_FONT_PX_MATRIX="12,13,14,15"
export MICA_TERM_PACKAGE_PORTABLE=1
export PACKAGE_FLAVOR_SUFFIX="-skia"

ARCHIVE_STEM="${APP_NAME}-${TARGET}-${PROFILE}${PACKAGE_FLAVOR_SUFFIX}"
ARCHIVE_PATH="$DIST_DIR/${ARCHIVE_STEM}.zip"

echo "==> Text renderer path: ${MICA_TERM_EXPECTED_TEXT_RENDERER_PATH} -> ${MICA_TERM_TEXT_RENDERER_FALLBACK_PATH}"
echo "==> Verification matrix: DPI ${MICA_TERM_VERIFICATION_DPI_SCALE_MATRIX}; font px ${MICA_TERM_VERIFICATION_FONT_PX_MATRIX}"

detect_default_build_jobs

publish_archive() {
  local source_archive="$1"
  local publish_dir="$2"
  local publish_archive="$publish_dir/$(basename "$source_archive")"
  local host_os

  host_os="$(uname -s)"
  if [[ "$host_os" != "Linux" && "$publish_dir" == "/var/www/html" ]]; then
    echo "==> Publish step skipped on $host_os host: $publish_archive"
    return 0
  fi

  [[ -f "$source_archive" ]] || {
    echo "error: expected archive not found: $source_archive" >&2
    exit 1
  }

  mkdir -p "$publish_dir"
  rm -f "$publish_archive"
  cp "$source_archive" "$publish_archive"
  echo "==> Published archive: $publish_archive"
}

"$ROOT_DIR/build-desktop.sh" "$@"
publish_archive "$ARCHIVE_PATH" "$PUBLISH_DIR"

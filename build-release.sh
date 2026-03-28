#!/usr/bin/env bash
# Aggregates the Linux software mainline and Windows Skia mainline packages and reports failures consistently.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MODE="${MODE:-fail-fast}"

usage() {
  cat <<EOF
Usage: $(basename "$0") [--help]

Mainline Linux software + Windows Skia release aggregator.

Modes:
  MODE=fail-fast   Stop on first failure (default)
  MODE=best-effort Continue both targets and report summary

Mainline targets:
  x86_64-unknown-linux-gnu
  x86_64-pc-windows-gnu

Windows x64 packages default to winit-skia-software.
EOF
}

fail() {
  echo "error: $*" >&2
  exit 1
}

run_target() {
  local target="$1"

  echo "==> Mainline release target: $target"
  case "$target" in
    x86_64-unknown-linux-gnu)
      TARGET="$target" "$ROOT_DIR/build-desktop.sh"
      ;;
    x86_64-pc-windows-gnu)
      TARGET="$target" \
      CARGO_NO_DEFAULT_FEATURES=1 \
      CARGO_FEATURES="slint-renderer-skia" \
      MICA_TERM_BUILD_FLAVOR="windows-mainline" \
      MICA_TERM_PACKAGE_RENDERER="skia-software" \
      PACKAGE_FLAVOR_SUFFIX="-skia" \
      "$ROOT_DIR/build-desktop.sh"
      ;;
    *)
      fail "unsupported release target '$target'"
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

case "$MODE" in
  fail-fast|best-effort)
    ;;
  *)
    fail "unsupported MODE '$MODE' (expected fail-fast or best-effort)"
    ;;
esac

mainline_targets=(
  "x86_64-unknown-linux-gnu"
  "x86_64-pc-windows-gnu"
)

failures=0
results=()

for target in "${mainline_targets[@]}"; do
  if run_target "$target"; then
    results+=("ok $target")
  else
    results+=("failed $target")
    failures=$((failures + 1))

    if [[ "$MODE" == "fail-fast" ]]; then
      exit 1
    fi
  fi
done

echo "==> Mainline release summary ($MODE)"
for result in "${results[@]}"; do
  echo "   $result"
done

if [[ "$failures" -ne 0 ]]; then
  exit 1
fi

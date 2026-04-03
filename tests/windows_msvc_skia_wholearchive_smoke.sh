#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if rg -n 'WHOLEARCHIVE' "$ROOT_DIR/build.rs" >/dev/null; then
  echo 'build.rs should not use /WHOLEARCHIVE for the Windows MSVC Skia ICU workaround anymore' >&2
  exit 1
fi

if rg -n 'rustc-link-arg-bin=mica-term=.*skunicode_(core|icu)\\.lib' "$ROOT_DIR/build.rs" >/dev/null; then
  echo 'build.rs should not inject manual Skia ICU link args; the lld-link shim handles the Linux-host MSVC collision now' >&2
  exit 1
fi

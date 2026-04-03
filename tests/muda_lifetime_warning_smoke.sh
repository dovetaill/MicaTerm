#!/usr/bin/env bash
# Fails if the vendored `muda` crate still emits the lifetime warning on
# Windows-targeted cargo checks.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

check_target() {
  local target="$1"
  local output

  output="$(cargo check -p muda --target "$target" --quiet 2>&1)"

  if grep -F "mismatched_lifetime_syntaxes" <<<"$output" >/dev/null; then
    echo "unexpected lifetime warning while checking target $target" >&2
    echo "$output" >&2
    exit 1
  fi
}

check_target x86_64-pc-windows-msvc
check_target x86_64-pc-windows-gnu

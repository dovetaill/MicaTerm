#!/usr/bin/env bash
# Confirms the old experimental split wrapper no longer exists now that Skia is the mainline path.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LEGACY_SKIA_SPLIT_NAME='build-win-x64'
LEGACY_SKIA_SPLIT_SUFFIX='-skia.sh'
SCRIPT_PATH="$ROOT_DIR/${LEGACY_SKIA_SPLIT_NAME}${LEGACY_SKIA_SPLIT_SUFFIX}"
MAINLINE_WRAPPER="$ROOT_DIR/build-win-x64.sh"
SOFTWARE_WRAPPER="$ROOT_DIR/build-win-x64-software.sh"
LEGACY_SKIA_EXPERIMENTAL='windows-skia'
LEGACY_SKIA_EXPERIMENTAL_SUFFIX='-experimental'
LEGACY_SKIA_LABEL='Skia'
LEGACY_SKIA_LABEL_SUFFIX=' Experimental'

if [[ -f "$SCRIPT_PATH" ]]; then
  echo "unexpected skia build script remains: $SCRIPT_PATH" >&2
  exit 1
fi

[[ -f "$MAINLINE_WRAPPER" ]]
[[ -f "$SOFTWARE_WRAPPER" ]]

if rg -n "${LEGACY_SKIA_EXPERIMENTAL}${LEGACY_SKIA_EXPERIMENTAL_SUFFIX}|${LEGACY_SKIA_SPLIT_NAME}${LEGACY_SKIA_SPLIT_SUFFIX//./\\.}|${LEGACY_SKIA_LABEL}${LEGACY_SKIA_LABEL_SUFFIX}" \
  "$MAINLINE_WRAPPER" "$SOFTWARE_WRAPPER" "$ROOT_DIR/build-release.sh" >/dev/null; then
  echo "unexpected experimental skia split reference remains in packaging scripts" >&2
  exit 1
fi

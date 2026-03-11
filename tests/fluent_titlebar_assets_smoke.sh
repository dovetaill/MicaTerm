#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

for file in \
  assets/icons/fluent/menu-20-regular.svg \
  assets/icons/fluent/menu-20-filled.svg \
  assets/icons/fluent/navigation-24-regular.svg \
  assets/icons/fluent/dark-theme-20-regular.svg \
  assets/icons/fluent/weather-sunny-20-regular.svg \
  assets/icons/fluent/panel-right-20-regular.svg \
  assets/icons/fluent/panel-right-20-filled.svg \
  assets/icons/fluent/pin-20-regular.svg \
  assets/icons/fluent/pin-off-20-regular.svg \
  assets/icons/fluent/subtract-20-regular.svg \
  assets/icons/fluent/maximize-20-regular.svg \
  assets/icons/fluent/restore-20-regular.svg \
  assets/icons/fluent/dismiss-20-regular.svg
do
  [[ -f "$ROOT_DIR/$file" ]] || {
    echo "missing $file" >&2
    exit 1
  }
done

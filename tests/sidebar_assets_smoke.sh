#!/usr/bin/env bash
# Ensures sidebar asset files needed by the current shell exist.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

for file in \
  assets/icons/fluent/folder-20-regular.svg \
  assets/icons/fluent/folder-open-20-regular.svg \
  assets/icons/fluent/window-console-20-regular.svg \
  assets/icons/fluent/document-code-16-regular.svg \
  assets/icons/fluent/key-multiple-20-regular.svg \
  assets/icons/fluent/search-20-regular.svg \
  assets/icons/fluent/arrow-expand-all-20-regular.svg \
  assets/icons/fluent/arrow-collapse-all-20-regular.svg \
  assets/icons/fluent/list-20-regular.svg \
  assets/icons/fluent/branch-20-regular.svg \
  assets/icons/fluent/add-20-regular.svg \
  assets/icons/fluent/chevron-down-20-regular.svg
do
  [[ -f "$ROOT_DIR/$file" ]] || {
    echo "missing $file" >&2
    exit 1
  }
done

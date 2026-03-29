#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
mapper="${repo_root}/src/app/assets_catalog/mapper.rs"
bootstrap="${repo_root}/src/app/bootstrap.rs"

rg -n "pub fn asset_trees_to_catalog" "${mapper}"
rg -n "pub fn catalog_to_asset_trees" "${mapper}"
rg -n "replace_snippet_asset_tree" "${bootstrap}"

if rg -n "asset_tree_to_catalog\\(state\\.console_asset_tree\\(\\)\\)" "${bootstrap}"; then
    echo "bootstrap still persists only the console asset tree" >&2
    exit 1
fi

if rg -n "replace_console_asset_tree\\(catalog_to_asset_tree\\(" "${bootstrap}"; then
    echo "bootstrap still loads only the console asset tree from persisted catalog" >&2
    exit 1
fi

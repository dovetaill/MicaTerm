#!/usr/bin/env bash
# Guards ShellViewModel staying in the root file while domain impls split into submodules.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VIEW_MODEL_ROOT="$ROOT_DIR/src/shell/view_model.rs"

[[ -f "$VIEW_MODEL_ROOT" ]] || {
  echo "missing src/shell/view_model.rs" >&2
  exit 1
}

[[ ! -f "$ROOT_DIR/src/shell/view_model/mod.rs" ]] || {
  echo "src/shell/view_model.rs must remain the stable root module" >&2
  exit 1
}

grep -F 'pub struct ShellViewModel {' "$VIEW_MODEL_ROOT" >/dev/null

for module in projection workspace quick_launch assets keychain sftp validation; do
  [[ -f "$ROOT_DIR/src/shell/view_model/${module}.rs" ]] || {
    echo "missing src/shell/view_model/${module}.rs" >&2
    exit 1
  }
  grep -F "mod ${module};" "$VIEW_MODEL_ROOT" >/dev/null
done

grep -F 'pub fn visible_console_asset_rows(&self) -> Vec<VisibleAssetRow>' \
  "$ROOT_DIR/src/shell/view_model/projection.rs" >/dev/null
grep -F 'pub fn workspace_tabs(&self) -> &[WorkspaceTab]' \
  "$ROOT_DIR/src/shell/view_model/workspace.rs" >/dev/null
grep -F 'pub fn set_quick_launch_search_query(&mut self, query: String)' \
  "$ROOT_DIR/src/shell/view_model/quick_launch.rs" >/dev/null
grep -F 'pub fn select_sidebar_destination(&mut self, destination: SidebarDestination)' \
  "$ROOT_DIR/src/shell/view_model/assets.rs" >/dev/null
grep -F 'pub fn keychain_catalog(&self) -> &KeychainCatalog' \
  "$ROOT_DIR/src/shell/view_model/keychain.rs" >/dev/null
grep -F 'pub fn open_sftp_panel(&mut self)' \
  "$ROOT_DIR/src/shell/view_model/sftp.rs" >/dev/null
grep -F 'pub fn asset_create_modal_can_confirm(&self) -> bool' \
  "$ROOT_DIR/src/shell/view_model/validation.rs" >/dev/null

if grep -F 'pub fn visible_console_asset_rows(&self) -> Vec<VisibleAssetRow>' "$VIEW_MODEL_ROOT" >/dev/null; then
  echo "projection impls must move out of src/shell/view_model.rs" >&2
  exit 1
fi

if grep -F 'pub fn workspace_tabs(&self) -> &[WorkspaceTab]' "$VIEW_MODEL_ROOT" >/dev/null; then
  echo "workspace impls must move out of src/shell/view_model.rs" >&2
  exit 1
fi

if grep -F 'pub fn set_quick_launch_search_query(&mut self, query: String)' "$VIEW_MODEL_ROOT" >/dev/null; then
  echo "quick launch impls must move out of src/shell/view_model.rs" >&2
  exit 1
fi

if grep -F 'pub fn select_sidebar_destination(&mut self, destination: SidebarDestination)' "$VIEW_MODEL_ROOT" >/dev/null; then
  echo "assets impls must move out of src/shell/view_model.rs" >&2
  exit 1
fi

if grep -F 'pub fn keychain_catalog(&self) -> &KeychainCatalog' "$VIEW_MODEL_ROOT" >/dev/null; then
  echo "keychain impls must move out of src/shell/view_model.rs" >&2
  exit 1
fi

if grep -F 'pub fn open_sftp_panel(&mut self)' "$VIEW_MODEL_ROOT" >/dev/null; then
  echo "sftp impls must move out of src/shell/view_model.rs" >&2
  exit 1
fi

if grep -F 'pub fn asset_create_modal_can_confirm(&self) -> bool' "$VIEW_MODEL_ROOT" >/dev/null; then
  echo "validation impls must move out of src/shell/view_model.rs" >&2
  exit 1
fi

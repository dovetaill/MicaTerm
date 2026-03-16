# Assets Sidebar Toolbar TDD Spec

Date: 2026-03-16
Scope: assets sidebar toolbar shell contract, Slint interaction wiring, popup menu shell, placeholder state rendering
Status: implementation complete, ready for test-driven follow-up

## Source Inputs

- Design: `docs/plans/2026-03-16-assets-sidebar-toolbar-design.md`
- Implementation Plan: `docs/plans/2026-03-16-assets-sidebar-toolbar-implementation-plan.md`
- Verification: `verification.md` -> `Assets Sidebar Toolbar Verification`

## Core Rust Surfaces

### `src/shell/assets.rs`

- `AssetViewMode`
  - Variants: `Tree`, `Flat`
  - Methods:
    - `id() -> &'static str`
    - `toggle() -> Self`
- `AssetCreateAction`
  - Variants: `NewFolder`, `NewSshConnection`
  - Method:
    - `id() -> &'static str`

### `src/shell/view_model.rs`

- `ShellViewModel`
  - Added fields:
    - `asset_view_mode: AssetViewMode`
    - `asset_search_expanded: bool`
    - `asset_search_query: String`
    - `asset_create_menu_open: bool`
    - `asset_tree_fully_expanded: bool`
  - Added methods:
    - `toggle_asset_view_mode()`
    - `toggle_asset_search()`
    - `set_asset_search_query(String)`
    - `collapse_asset_search_if_empty()`
    - `toggle_asset_tree_expansion()`
    - `toggle_asset_create_menu()`
    - `close_asset_create_menu()`
- No new Rust trait was introduced in this feature.

### `src/app/bootstrap.rs`

- `sync_assets_toolbar_state(window, state)`
  - Syncs Rust state into Slint window properties:
    - `asset_view_mode`
    - `asset_search_expanded`
    - `assets_search_query`
    - `asset_create_menu_open`
    - `asset_tree_fully_expanded`
- Added callback bindings:
  - `toggle_assets_search_requested`
  - `assets_search_query_changed`
  - `collapse_assets_search_requested`
  - `toggle_assets_view_mode_requested`
  - `toggle_assets_tree_expansion_requested`
  - `toggle_assets_create_menu_requested`
  - `close_assets_create_menu_requested`
  - `assets_create_action_selected`

## Core Slint Surfaces

### `ui/app-window.slint`

- Added window properties:
  - `assets-search-query`
  - `asset-search-expanded`
  - `asset-create-menu-open`
  - `asset-tree-fully-expanded`
  - `asset-view-mode`
- Added callbacks:
  - `toggle-assets-search-requested()`
  - `assets-search-query-changed(string)`
  - `collapse-assets-search-requested()`
  - `toggle-assets-view-mode-requested()`
  - `toggle-assets-tree-expansion-requested()`
  - `toggle-assets-create-menu-requested()`
  - `close-assets-create-menu-requested()`
  - `assets-create-action-selected(string)`

### `ui/shell/sidebar.slint`

- Pass-through layer for all assets toolbar properties and callbacks from `AppWindow` to `AssetsSidebar`.

### `ui/shell/assets-sidebar.slint`

- Header shell:
  - title `资产列表`
  - `search-button`
  - `tree-expansion-button`
  - `view-mode-button`
  - `create-button`
- Search row shell:
  - visible when `asset-search-expanded`
  - `TextInput`
  - `edited` -> `assets-search-query-changed`
  - `changed has-focus` -> `collapse-assets-search-requested`
- Popup menu shell:
  - `AssetsCreateMenu`
  - `changed asset-create-menu-open` controls `show()/close()`
- Placeholder proof rendering:
  - `Tree + expanded` -> `Console Tree — Expanded`
  - `Tree + collapsed` -> `Console Tree — Collapsed`
  - `Flat` -> `Console Flat List`
  - empty search -> `Hosts, recent sessions, favorites`
  - non-empty search -> `Filter: <query>`

### New Components

- `ui/components/sidebar-toolbar-icon-button.slint`
  - lightweight toolbar icon button, 28x28
- `ui/components/assets-create-menu.slint`
  - `PopupWindow` menu shell
  - actions:
    - `New Folder`
    - `New SSH Connection`

## Existing Automated Coverage

- Rust:
  - `tests/assets_sidebar_toolbar_spec.rs`
  - `tests/assets_sidebar_toolbar_smoke.rs`
  - `tests/shell_view_model.rs`
- Shell smoke:
  - `tests/sidebar_assets_smoke.sh`
  - `tests/sidebar_ui_contract_smoke.sh`
  - `tests/assets_sidebar_toolbar_ui_contract_smoke.sh`
  - `tests/shell_layout_ui_contract_smoke.sh`

## Required Next-Stage TDD Focus

### 1. Search Focus And Collapse Behavior

- Verify focus enters the `TextInput` after pressing the search button.
- Verify empty query + blur collapses the row.
- Verify non-empty query + blur keeps the row open.
- Verify repeated search-button clicks do not clear existing query accidentally.

### 2. View Mode And Expansion Invariants

- Verify `tree` -> `flat` toggle preserves prior tree expansion state without mutating it.
- Verify `flat` mode keeps the expansion button disabled and `toggle_asset_tree_expansion()` no-op.
- Verify switching back to `tree` restores the previous expansion indicator.

### 3. Create Menu Lifecycle

- Verify `asset_create_menu_open` and `PopupWindow` visibility remain synchronized on:
  - button click
  - outside click
  - explicit close callback
  - menu action selection
- Verify action ids remain exactly:
  - `new-folder`
  - `new-ssh-connection`

### 4. Placeholder Contract

- Verify placeholder text remains tied to state, so future refactors cannot silently break proof rendering.
- Once real asset models land, replace placeholder assertions with rendered list-state assertions.

## Edge Cases And Risks

- `flat` mode must never mutate real tree expansion state.
- Search collapse currently depends on focus loss and empty query only; future popup/menu overlap can regress this if focus is stolen unexpectedly.
- `asset_create_menu_open` is the source of truth for menu visibility; future direct `PopupWindow.close()` calls must still feed back into Rust state.
- Current implementation does not yet use `ModelRc` for assets data because no real asset tree/list exists.
- Current implementation does not yet use Tokio, channels, or background actors.
  - When real SSH/SFTP/asset data arrives from async tasks, UI mutations must be marshalled onto the Slint UI thread with `slint::invoke_from_event_loop`.
  - Background workers must not touch Slint component state directly.
  - If channel-based asset updates are introduced, test for queue backpressure and stale-state overwrite behavior.

## Renderer / Tooling Notes

- `tests/titlebar_render_spec.rs` is now compiled out under `slint-renderer-femtovg-wgpu`, because the repository no longer exposes a software renderer feature path.
- Keep future tests aligned with the current renderer strategy instead of assuming `software_renderer` is available.

## Suggested Next Verification Commands

```bash
cargo test --test assets_sidebar_toolbar_spec --test assets_sidebar_toolbar_smoke -q
bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh
cargo check --workspace
cargo clippy --workspace -- -D warnings
```

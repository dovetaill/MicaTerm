# Assets Sidebar Toolbar Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 在不接入真实 terminal / SSH / SFTP runtime 的前提下，为左侧 `AssetsSidebar` 落地已确认的顶部 `Toolbar`，接入搜索展开行、`tree / flat` 模式切换、树展开态切换，以及 `Create` 自绘下拉菜单，并由 Rust 状态驱动。

**Architecture:** 保持现有 `AppWindow -> Sidebar -> AssetsSidebar` 的 shell 结构不变，把 toolbar 视为 `AssetsSidebar` 的通用 header 骨架，但首轮只在 `console` panel 激活其动作。Rust 侧新增资产区状态模型与回调处理，Slint 侧负责 header、搜索行、菜单和占位内容渲染；搜索框采用 `inline collapsible row`，`Create` 继续复用当前自绘 `PopupWindow` 语言。

**Tech Stack:** Rust, Cargo, Slint 1.15.1, `PopupWindow`, `TextInput`, `ModelRc` / `VecModel`, Fluent SVG assets, `i-slint-backend-testing`, shell smoke scripts, `cargo fmt`, `cargo check`, `cargo test`, `cargo clippy`

---

## Execution Notes

- 设计输入固定为 `docs/plans/2026-03-16-assets-sidebar-toolbar-design.md`，实现时不得偏离已确认组合：`D0B D1B D2A D3A D4A D5A`。
- 每个任务都先走 `@superpowers:test-driven-development`：先写失败测试或失败 smoke，再写最小实现，再跑通过。
- 如果 `Slint TextInput` focus 丢失、`PopupWindow` 锚定、或 callback 链路出现异常，不允许猜测，立即切换到 `@superpowers:systematic-debugging`。
- 本轮不实现真实资产树、真实搜索过滤、创建向导、SSH 表单提交、SFTP 数据模型；只交付 shell 层 contract、交互和 placeholder。
- 计划默认在从 `/home/wwwroot/mica-term` 派生的独立 worktree 中执行；若继续在当前工作区执行，也必须将改动范围严格限制在本计划列出的文件。
- `ShellViewModel` 当前是 `Copy` 结构体；由于本轮要引入 `search_query` 字符串状态，必须移除 `Copy` 并只保留 `Clone`，不要在实现时遗漏。

### Target Snapshot

完成后应满足以下用户可见结果：

- `AssetsSidebar` 顶部出现 `资产列表` toolbar
- toolbar 右侧包含：
  - 搜索按钮
  - 展开全部 / 收起全部按钮
  - 平铺 / 树形切换按钮
  - `Create` 按钮
- 点击搜索按钮后，toolbar 下方展开一行搜索输入框
- 搜索框为空且失去焦点时自动收起
- 搜索框非空时失去焦点不会自动收起
- `tree` 模式下，展开态按钮根据当前状态在“展开全部 / 收起全部”之间切换
- `flat` 模式下，展开态按钮 disabled
- `Create` 打开自绘菜单，包含 `New Folder` 与 `New SSH Connection`
- 当前 `console` panel 的 placeholder 文本能根据 `tree / flat` 与搜索状态体现差异，证明状态已真正接入

### Out of Scope

- `wezterm-term` / `termwiz` / `russh` / `russh-sftp`
- 真实资产树节点与持久化 schema
- 真实搜索过滤算法
- 创建文件夹 / SSH 连接的表单与业务落库
- tooltip、快捷键、键盘导航的完整无障碍适配

## Task 1: 建立资产区 Toolbar 状态契约与尺寸常量

**Files:**
- Create: `src/shell/assets.rs`
- Modify: `src/shell/mod.rs`
- Modify: `src/shell/view_model.rs`
- Modify: `src/shell/metrics.rs`
- Create: `tests/assets_sidebar_toolbar_spec.rs`
- Modify: `tests/shell_view_model.rs`
- Modify: `tests/window_shell.rs`

**Step 1: Write the failing tests**

创建 `tests/assets_sidebar_toolbar_spec.rs`：

```rust
use mica_term::shell::assets::{AssetCreateAction, AssetViewMode};
use mica_term::shell::view_model::ShellViewModel;

#[test]
fn asset_view_mode_defaults_to_tree() {
    let view_model = ShellViewModel::default();

    assert_eq!(view_model.asset_view_mode, AssetViewMode::Tree);
    assert!(!view_model.asset_search_expanded);
    assert!(view_model.asset_search_query.is_empty());
    assert!(!view_model.asset_create_menu_open);
    assert!(!view_model.asset_tree_fully_expanded);
}

#[test]
fn toggling_asset_view_mode_flips_between_tree_and_flat() {
    let mut view_model = ShellViewModel::default();

    view_model.toggle_asset_view_mode();
    assert_eq!(view_model.asset_view_mode, AssetViewMode::Flat);

    view_model.toggle_asset_view_mode();
    assert_eq!(view_model.asset_view_mode, AssetViewMode::Tree);
}

#[test]
fn collapsing_empty_search_hides_search_row() {
    let mut view_model = ShellViewModel::default();

    view_model.toggle_asset_search();
    assert!(view_model.asset_search_expanded);

    view_model.collapse_asset_search_if_empty();
    assert!(!view_model.asset_search_expanded);
}

#[test]
fn non_empty_search_stays_open_when_focus_leaves() {
    let mut view_model = ShellViewModel::default();

    view_model.toggle_asset_search();
    view_model.set_asset_search_query("prod".into());
    view_model.collapse_asset_search_if_empty();

    assert!(view_model.asset_search_expanded);
    assert_eq!(view_model.asset_search_query, "prod");
}

#[test]
fn flat_mode_disables_tree_expansion_toggle() {
    let mut view_model = ShellViewModel::default();

    view_model.toggle_asset_view_mode();
    assert_eq!(view_model.asset_view_mode, AssetViewMode::Flat);

    view_model.toggle_asset_tree_expansion();
    assert!(!view_model.asset_tree_fully_expanded);
}

#[test]
fn create_menu_toggles_and_actions_are_named() {
    let mut view_model = ShellViewModel::default();

    view_model.toggle_asset_create_menu();
    assert!(view_model.asset_create_menu_open);

    assert_eq!(AssetCreateAction::NewFolder.id(), "new-folder");
    assert_eq!(AssetCreateAction::NewSshConnection.id(), "new-ssh-connection");
}
```

修改 `tests/shell_view_model.rs`，补当前壳层默认状态：

```rust
use mica_term::shell::assets::AssetViewMode;

#[test]
fn shell_view_model_starts_with_assets_toolbar_defaults() {
    let view_model = ShellViewModel::default();

    assert_eq!(view_model.asset_view_mode, AssetViewMode::Tree);
    assert!(!view_model.asset_search_expanded);
    assert!(view_model.asset_search_query.is_empty());
    assert!(!view_model.asset_create_menu_open);
}
```

修改 `tests/window_shell.rs`，补 toolbar 尺寸预算：

```rust
#[test]
fn assets_toolbar_metrics_match_the_design_budget() {
    assert_eq!(ShellMetrics::ASSETS_TOOLBAR_HEIGHT, 44);
    assert_eq!(ShellMetrics::ASSETS_TOOLBAR_BUTTON_SIZE, 28);
    assert_eq!(ShellMetrics::ASSETS_SEARCH_ROW_HEIGHT, 40);
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --test assets_sidebar_toolbar_spec --test shell_view_model --test window_shell -q`  
Expected: FAIL，报错应包括 `could not find shell::assets`、`no field asset_view_mode`、`no method toggle_asset_search`、`ASSETS_TOOLBAR_HEIGHT not found` 等。

**Step 3: Write minimal implementation**

创建 `src/shell/assets.rs`：

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetViewMode {
    Tree,
    Flat,
}

impl AssetViewMode {
    pub fn id(self) -> &'static str {
        match self {
            Self::Tree => "tree",
            Self::Flat => "flat",
        }
    }

    pub fn toggle(self) -> Self {
        match self {
            Self::Tree => Self::Flat,
            Self::Flat => Self::Tree,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetCreateAction {
    NewFolder,
    NewSshConnection,
}

impl AssetCreateAction {
    pub fn id(self) -> &'static str {
        match self {
            Self::NewFolder => "new-folder",
            Self::NewSshConnection => "new-ssh-connection",
        }
    }
}
```

修改 `src/shell/mod.rs`：

```rust
pub mod assets;
pub mod layout;
pub mod metrics;
pub mod sidebar;
pub mod signature;
pub mod view_model;
```

修改 `src/shell/view_model.rs`：

```rust
use crate::shell::assets::AssetViewMode;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellViewModel {
    pub show_welcome: bool,
    pub show_right_panel: bool,
    pub show_global_menu: bool,
    pub show_assets_sidebar: bool,
    pub active_sidebar_destination: SidebarDestination,
    pub is_window_active: bool,
    pub theme_mode: ThemeMode,
    pub is_always_on_top: bool,
    pub asset_view_mode: AssetViewMode,
    pub asset_search_expanded: bool,
    pub asset_search_query: String,
    pub asset_create_menu_open: bool,
    pub asset_tree_fully_expanded: bool,
    window_placement: WindowPlacementKind,
}

impl Default for ShellViewModel {
    fn default() -> Self {
        Self {
            show_welcome: true,
            show_right_panel: false,
            show_global_menu: false,
            show_assets_sidebar: true,
            active_sidebar_destination: SidebarDestination::Console,
            is_window_active: true,
            theme_mode: ThemeMode::Dark,
            is_always_on_top: false,
            asset_view_mode: AssetViewMode::Tree,
            asset_search_expanded: false,
            asset_search_query: String::new(),
            asset_create_menu_open: false,
            asset_tree_fully_expanded: false,
            window_placement: WindowPlacementKind::Restored,
        }
    }
}
```

补充最小方法：

```rust
pub fn toggle_asset_view_mode(&mut self) {
    self.asset_view_mode = self.asset_view_mode.toggle();
}

pub fn toggle_asset_search(&mut self) {
    self.asset_search_expanded = true;
}

pub fn set_asset_search_query(&mut self, query: String) {
    self.asset_search_query = query;
}

pub fn collapse_asset_search_if_empty(&mut self) {
    if self.asset_search_query.is_empty() {
        self.asset_search_expanded = false;
    }
}

pub fn toggle_asset_tree_expansion(&mut self) {
    if self.asset_view_mode == AssetViewMode::Tree {
        self.asset_tree_fully_expanded = !self.asset_tree_fully_expanded;
    }
}

pub fn toggle_asset_create_menu(&mut self) {
    self.asset_create_menu_open = !self.asset_create_menu_open;
}
```

修改 `src/shell/metrics.rs`：

```rust
pub const ASSETS_TOOLBAR_HEIGHT: u32 = 44;
pub const ASSETS_TOOLBAR_BUTTON_SIZE: u32 = 28;
pub const ASSETS_SEARCH_ROW_HEIGHT: u32 = 40;
```

**Step 4: Run tests to verify they pass**

Run: `cargo test --test assets_sidebar_toolbar_spec --test shell_view_model --test window_shell -q`  
Expected: PASS。

**Step 5: Commit**

```bash
git add src/shell/assets.rs src/shell/mod.rs src/shell/view_model.rs src/shell/metrics.rs tests/assets_sidebar_toolbar_spec.rs tests/shell_view_model.rs tests/window_shell.rs
git commit -m "feat: add assets sidebar toolbar state contract"
```

## Task 2: 添加 Toolbar 图标资源与资源 smoke 契约

**Files:**
- Create: `assets/icons/fluent/search-20-regular.svg`
- Create: `assets/icons/fluent/arrow-expand-all-20-regular.svg`
- Create: `assets/icons/fluent/arrow-collapse-all-20-regular.svg`
- Create: `assets/icons/fluent/list-20-regular.svg`
- Create: `assets/icons/fluent/branch-20-regular.svg`
- Create: `assets/icons/fluent/add-20-regular.svg`
- Create: `assets/icons/fluent/chevron-down-20-regular.svg`
- Modify: `tests/sidebar_assets_smoke.sh`

**Step 1: Write the failing smoke assertions**

修改 `tests/sidebar_assets_smoke.sh`，追加：

```bash
check_file "$ROOT_DIR/assets/icons/fluent/search-20-regular.svg"
check_file "$ROOT_DIR/assets/icons/fluent/arrow-expand-all-20-regular.svg"
check_file "$ROOT_DIR/assets/icons/fluent/arrow-collapse-all-20-regular.svg"
check_file "$ROOT_DIR/assets/icons/fluent/list-20-regular.svg"
check_file "$ROOT_DIR/assets/icons/fluent/branch-20-regular.svg"
check_file "$ROOT_DIR/assets/icons/fluent/add-20-regular.svg"
check_file "$ROOT_DIR/assets/icons/fluent/chevron-down-20-regular.svg"
```

如果当前脚本没有 `check_file`，沿用现有 `[[ -f ... ]]` 风格逐条补断言。

**Step 2: Run smoke to verify it fails**

Run: `bash tests/sidebar_assets_smoke.sh`  
Expected: FAIL，提示缺失新的 toolbar icon 资源。

**Step 3: Add the assets**

把官方 Fluent SVG 导出到上述确切路径，保持与现有仓库一致的资源命名风格：

- `search-20-regular.svg`
- `arrow-expand-all-20-regular.svg`
- `arrow-collapse-all-20-regular.svg`
- `list-20-regular.svg`
- `branch-20-regular.svg`
- `add-20-regular.svg`
- `chevron-down-20-regular.svg`

要求：

- 保持 `viewBox` 正确
- 使用 `currentColor` 或与现有 Fluent SVG 相同的着色模式
- 不引入额外内联 style 噪音

**Step 4: Run smoke to verify it passes**

Run: `bash tests/sidebar_assets_smoke.sh`  
Expected: PASS。

**Step 5: Commit**

```bash
git add assets/icons/fluent/search-20-regular.svg assets/icons/fluent/arrow-expand-all-20-regular.svg assets/icons/fluent/arrow-collapse-all-20-regular.svg assets/icons/fluent/list-20-regular.svg assets/icons/fluent/branch-20-regular.svg assets/icons/fluent/add-20-regular.svg assets/icons/fluent/chevron-down-20-regular.svg tests/sidebar_assets_smoke.sh
git commit -m "feat: add assets sidebar toolbar icons"
```

## Task 3: 建立 Slint Toolbar 结构契约与搜索行骨架

**Files:**
- Create: `ui/components/sidebar-toolbar-icon-button.slint`
- Modify: `ui/shell/assets-sidebar.slint`
- Modify: `ui/shell/sidebar.slint`
- Modify: `ui/app-window.slint`
- Create: `tests/assets_sidebar_toolbar_ui_contract_smoke.sh`

**Step 1: Write the failing UI contract smoke**

创建 `tests/assets_sidebar_toolbar_ui_contract_smoke.sh`：

```bash
#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ASSETS="$ROOT_DIR/ui/shell/assets-sidebar.slint"
SIDEBAR="$ROOT_DIR/ui/shell/sidebar.slint"
APP_WINDOW="$ROOT_DIR/ui/app-window.slint"
BUTTON="$ROOT_DIR/ui/components/sidebar-toolbar-icon-button.slint"

grep -F 'export component SidebarToolbarIconButton' "$BUTTON" >/dev/null
grep -F 'Text { text: "资产列表";' "$ASSETS" >/dev/null
grep -F 'search-button := SidebarToolbarIconButton' "$ASSETS" >/dev/null
grep -F 'tree-expansion-button := SidebarToolbarIconButton' "$ASSETS" >/dev/null
grep -F 'view-mode-button := SidebarToolbarIconButton' "$ASSETS" >/dev/null
grep -F 'create-button' "$ASSETS" >/dev/null
grep -F 'if root.asset-search-expanded : Rectangle' "$ASSETS" >/dev/null
grep -F 'in property <string> assets-search-query' "$SIDEBAR" >/dev/null
grep -F 'callback assets-search-query-changed(string);' "$APP_WINDOW" >/dev/null
grep -F 'callback toggle-assets-view-mode-requested();' "$APP_WINDOW" >/dev/null
grep -F 'callback toggle-assets-tree-expansion-requested();' "$APP_WINDOW" >/dev/null
grep -F 'callback toggle-assets-create-menu-requested();' "$APP_WINDOW" >/dev/null
```

**Step 2: Run smoke to verify it fails**

Run: `bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh`  
Expected: FAIL，提示缺少 toolbar 结构、搜索行或 callback/property。

**Step 3: Build the Slint contract surface**

创建 `ui/components/sidebar-toolbar-icon-button.slint`：

```slint
import { ThemeTokens } from "../theme/tokens.slint";

export component SidebarToolbarIconButton inherits Rectangle {
    in property <image> icon-source;
    in property <image> active-icon-source;
    in property <bool> active: false;
    in property <bool> enabled: true;
    callback clicked;

    width: 28px;
    height: 28px;
    border-radius: 6px;
    border-width: root.active ? 1px : 0px;
    border-color: ThemeTokens.shell-stroke;
    background: !root.enabled
        ? transparent
        : touch.pressed
            ? ThemeTokens.panel-tint
            : (touch.has-hover || root.active)
                ? ThemeTokens.command-tint
                : transparent;

    // 保持与 TitlebarIconButton 相同的 Fluent 视觉语言，但不引入 tooltip 依赖
}
```

修改 `ui/app-window.slint`，新增确切属性和回调：

```slint
in-out property <string> assets-search-query: "";
in-out property <bool> asset-search-expanded: false;
in-out property <bool> asset-create-menu-open: false;
in-out property <bool> asset-tree-fully-expanded: false;
in-out property <string> asset-view-mode: "tree";

callback toggle-assets-search-requested();
callback assets-search-query-changed(string);
callback collapse-assets-search-requested();
callback toggle-assets-view-mode-requested();
callback toggle-assets-tree-expansion-requested();
callback toggle-assets-create-menu-requested();
callback close-assets-create-menu-requested();
callback assets-create-action-selected(string);
```

把这些属性和回调逐级透传到 `Sidebar` 和 `AssetsSidebar`。

修改 `ui/shell/assets-sidebar.slint`，先落结构骨架：

```slint
header := Rectangle {
    height: 44px;

    Text { text: "资产列表"; }
    search-button := SidebarToolbarIconButton { }
    tree-expansion-button := SidebarToolbarIconButton { }
    view-mode-button := SidebarToolbarIconButton { }
    create-button := Rectangle { }
}

if root.asset-search-expanded : Rectangle {
    height: 40px;
    // Step 4 再塞 TextInput，先把结构和 property 接上
}
```

**Step 4: Run smoke to verify it passes**

Run: `bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh`  
Expected: PASS。

**Step 5: Commit**

```bash
git add ui/components/sidebar-toolbar-icon-button.slint ui/shell/assets-sidebar.slint ui/shell/sidebar.slint ui/app-window.slint tests/assets_sidebar_toolbar_ui_contract_smoke.sh
git commit -m "feat: scaffold assets sidebar toolbar ui contract"
```

## Task 4: 接入 Rust callback 链路与搜索/视图/展开态交互

**Files:**
- Modify: `src/app/bootstrap.rs`
- Modify: `ui/shell/assets-sidebar.slint`
- Create: `tests/assets_sidebar_toolbar_smoke.rs`

**Step 1: Write the failing interaction tests**

创建 `tests/assets_sidebar_toolbar_smoke.rs`：

```rust
use mica_term::AppWindow;
use mica_term::app::bootstrap::bind_top_status_bar_with_store;
use mica_term::shell::assets::AssetViewMode;
use slint::ComponentHandle;

#[test]
fn bootstrap_initializes_assets_toolbar_defaults() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    assert_eq!(app.get_asset_view_mode().as_str(), "tree");
    assert!(!app.get_asset_search_expanded());
    assert_eq!(app.get_assets_search_query().as_str(), "");
    assert!(!app.get_asset_create_menu_open());
    assert!(!app.get_asset_tree_fully_expanded());
}

#[test]
fn search_toggle_and_query_binding_round_trip() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_toggle_assets_search_requested();
    assert!(app.get_asset_search_expanded());

    app.invoke_assets_search_query_changed("prod".into());
    assert_eq!(app.get_assets_search_query().as_str(), "prod");

    app.invoke_collapse_assets_search_requested();
    assert!(app.get_asset_search_expanded());

    app.invoke_assets_search_query_changed("".into());
    app.invoke_collapse_assets_search_requested();
    assert!(!app.get_asset_search_expanded());
}

#[test]
fn view_mode_toggle_and_tree_expansion_follow_the_contract() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_toggle_assets_tree_expansion_requested();
    assert!(app.get_asset_tree_fully_expanded());

    app.invoke_toggle_assets_view_mode_requested();
    assert_eq!(app.get_asset_view_mode().as_str(), "flat");

    app.invoke_toggle_assets_tree_expansion_requested();
    assert!(app.get_asset_tree_fully_expanded());
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --test assets_sidebar_toolbar_smoke -q`  
Expected: FAIL，报错应包括缺少 `get_asset_view_mode`、`invoke_toggle_assets_search_requested`、`invoke_assets_search_query_changed` 等。

**Step 3: Wire the callbacks in Rust**

修改 `src/app/bootstrap.rs`，补同步函数：

```rust
fn sync_assets_toolbar_state(window: &AppWindow, state: &ShellViewModel) {
    window.set_asset_view_mode(state.asset_view_mode.id().into());
    window.set_asset_search_expanded(state.asset_search_expanded);
    window.set_assets_search_query(state.asset_search_query.clone().into());
    window.set_asset_create_menu_open(state.asset_create_menu_open);
    window.set_asset_tree_fully_expanded(state.asset_tree_fully_expanded);
}
```

在 `sync_sidebar_state` 或新的 `sync_assets_sidebar_state` 中调用它。

补 callbacks：

```rust
window.on_toggle_assets_search_requested(move || {
    let window = handle.unwrap();
    let mut state = state.borrow_mut();
    state.toggle_asset_search();
    sync_assets_toolbar_state(&window, &state);
});

window.on_assets_search_query_changed(move |query| {
    let window = handle.unwrap();
    let mut state = state.borrow_mut();
    state.set_asset_search_query(query.to_string());
    sync_assets_toolbar_state(&window, &state);
});

window.on_collapse_assets_search_requested(move || {
    let window = handle.unwrap();
    let mut state = state.borrow_mut();
    state.collapse_asset_search_if_empty();
    sync_assets_toolbar_state(&window, &state);
});

window.on_toggle_assets_view_mode_requested(move || {
    let window = handle.unwrap();
    let mut state = state.borrow_mut();
    state.toggle_asset_view_mode();
    sync_assets_toolbar_state(&window, &state);
});

window.on_toggle_assets_tree_expansion_requested(move || {
    let window = handle.unwrap();
    let mut state = state.borrow_mut();
    state.toggle_asset_tree_expansion();
    sync_assets_toolbar_state(&window, &state);
});
```

**Step 4: Fill the Slint search row and mode binding**

在 `ui/shell/assets-sidebar.slint` 中把搜索行补成可工作的最小版本：

```slint
if root.asset-search-expanded : Rectangle {
    height: 40px;
    border-width: 1px;
    border-color: ThemeTokens.shell-stroke;
    background: ThemeTokens.command-tint;

    search-input := TextInput {
        text <=> root.assets-search-query;
        accepted => { }
        edited => {
            root.assets-search-query-changed(self.text);
        }
        has-focus-changed => {
            if !self.has-focus {
                root.collapse-assets-search-requested();
            }
        }
    }
}
```

让按钮状态绑定到当前 property：

```slint
tree-expansion-button.active: root.asset-tree-fully-expanded;
tree-expansion-button.enabled: root.asset-view-mode == "tree";
view-mode-button.active: root.asset-view-mode == "flat";
```

**Step 5: Run tests to verify they pass**

Run: `cargo test --test assets_sidebar_toolbar_smoke -q`  
Expected: PASS。

**Step 6: Commit**

```bash
git add src/app/bootstrap.rs ui/shell/assets-sidebar.slint tests/assets_sidebar_toolbar_smoke.rs
git commit -m "feat: wire assets sidebar toolbar interactions"
```

## Task 5: 落地 Create 自绘菜单与 console placeholder 差异渲染

**Files:**
- Create: `ui/components/assets-create-menu.slint`
- Modify: `ui/shell/assets-sidebar.slint`
- Modify: `src/app/bootstrap.rs`
- Modify: `src/shell/view_model.rs`
- Modify: `tests/assets_sidebar_toolbar_smoke.rs`
- Modify: `tests/assets_sidebar_toolbar_ui_contract_smoke.sh`

**Step 1: Extend the failing tests**

在 `tests/assets_sidebar_toolbar_smoke.rs` 追加：

```rust
#[test]
fn create_menu_toggle_and_close_round_trip() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_toggle_assets_create_menu_requested();
    assert!(app.get_asset_create_menu_open());

    app.invoke_close_assets_create_menu_requested();
    assert!(!app.get_asset_create_menu_open());
}
```

在 `tests/assets_sidebar_toolbar_ui_contract_smoke.sh` 追加：

```bash
MENU="$ROOT_DIR/ui/components/assets-create-menu.slint"
grep -F 'export component AssetsCreateMenu inherits PopupWindow' "$MENU" >/dev/null
grep -F 'label: "New Folder"' "$MENU" >/dev/null
grep -F 'label: "New SSH Connection"' "$MENU" >/dev/null
grep -F 'close-policy: PopupClosePolicy.close-on-click' "$MENU" >/dev/null
grep -F 'asset-view-mode == "tree"' "$ASSETS" >/dev/null
grep -F 'asset-view-mode == "flat"' "$ASSETS" >/dev/null
```

**Step 2: Run tests to verify they fail**

Run: `bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh && cargo test --test assets_sidebar_toolbar_smoke -q`  
Expected: FAIL，提示缺少 `AssetsCreateMenu`、缺少 `invoke_toggle_assets_create_menu_requested`，或缺少 tree/flat placeholder 差异渲染。

**Step 3: Implement the menu and menu callbacks**

创建 `ui/components/assets-create-menu.slint`：

```slint
import { ThemeTokens } from "../theme/tokens.slint";

export component AssetsCreateMenu inherits PopupWindow {
    callback new-folder-selected;
    callback new-ssh-connection-selected;
    callback close-requested;

    width: 196px;
    height: 88px;
    close-policy: PopupClosePolicy.close-on-click;

    // 样式沿用 TitlebarMenu 的圆角、描边、panel-tint 语言
}
```

菜单项固定为：

- `New Folder`
- `New SSH Connection`

修改 `src/shell/view_model.rs`，补：

```rust
pub fn close_asset_create_menu(&mut self) {
    self.asset_create_menu_open = false;
}
```

修改 `src/app/bootstrap.rs`，补：

```rust
window.on_toggle_assets_create_menu_requested(move || {
    let window = handle.unwrap();
    let mut state = state.borrow_mut();
    state.toggle_asset_create_menu();
    sync_assets_toolbar_state(&window, &state);
});

window.on_close_assets_create_menu_requested(move || {
    let window = handle.unwrap();
    let mut state = state.borrow_mut();
    state.close_asset_create_menu();
    sync_assets_toolbar_state(&window, &state);
});

window.on_assets_create_action_selected(move |action_id| {
    let window = handle.unwrap();
    let mut state = state.borrow_mut();
    state.close_asset_create_menu();
    sync_assets_toolbar_state(&window, &state);

    tracing::info!(target: "ui.assets", action = %action_id, "assets create action selected");
});
```

**Step 4: Make the console placeholder visibly react to state**

在 `ui/shell/assets-sidebar.slint` 的 `console` placeholder 中加入最小差异渲染：

```slint
Text {
    text: root.asset-view-mode == "tree"
        ? (root.asset-tree-fully-expanded ? "Console Tree — Expanded" : "Console Tree — Collapsed")
        : "Console Flat List";
}

Text {
    text: root.assets-search-query == ""
        ? "Hosts, recent sessions, favorites"
        : "Filter: " + root.assets-search-query;
}
```

目的不是做最终 UI，而是证明状态真的驱动到界面。

**Step 5: Run tests to verify they pass**

Run: `bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh && cargo test --test assets_sidebar_toolbar_smoke -q`  
Expected: PASS。

**Step 6: Commit**

```bash
git add ui/components/assets-create-menu.slint ui/shell/assets-sidebar.slint src/app/bootstrap.rs src/shell/view_model.rs tests/assets_sidebar_toolbar_smoke.rs tests/assets_sidebar_toolbar_ui_contract_smoke.sh
git commit -m "feat: add assets sidebar create menu and placeholder state rendering"
```

## Task 6: 全量回归验证与整理

**Files:**
- Modify: `verification.md`
- Optionally Modify: `docs/plans/2026-03-16-assets-sidebar-toolbar-design.md`

**Step 1: Format the code**

Run: `cargo fmt`  
Expected: 无错误输出。

**Step 2: Run focused Rust tests**

Run:

```bash
cargo test --test assets_sidebar_toolbar_spec --test assets_sidebar_toolbar_smoke --test shell_view_model --test sidebar_navigation_spec --test sidebar_navigation_smoke --test top_status_bar_smoke -q
```

Expected: PASS。

**Step 3: Run focused shell smoke scripts**

Run:

```bash
bash tests/sidebar_assets_smoke.sh
bash tests/sidebar_ui_contract_smoke.sh
bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh
bash tests/shell_layout_ui_contract_smoke.sh
```

Expected: PASS。

**Step 4: Run compile validation**

Run: `cargo check`  
Expected: PASS。

**Step 5: Run lint**

Run: `cargo clippy --all-targets --all-features -- -D warnings`  
Expected: PASS。若现有仓库已有非本轮警告，先记录再决定是否局部豁免；不要悄悄降标准。

**Step 6: Update verification notes**

在 `verification.md` 记录：

- 新增 toolbar contract
- 搜索行展开/收起规则
- `flat` 模式下展开按钮 disabled
- `Create` 自绘菜单存在且项数正确
- 执行的命令与结果

**Step 7: Final commit**

```bash
git add verification.md
git commit -m "test: verify assets sidebar toolbar implementation"
```

## Task Ordering Summary

严格按以下顺序执行，不要并行跳步：

1. 状态契约与 metrics
2. 图标资源
3. Slint 结构 contract
4. Rust callback 链路与搜索/视图/展开态
5. Create 菜单与 placeholder 差异渲染
6. 全量验证

## Common Failure Modes

### 1. 忘记移除 `ShellViewModel` 的 `Copy`

症状：

- 编译器在 `String` 字段加入后报 derive 错误

处理：

- 保留 `Clone`，移除 `Copy`
- 检查是否有依赖 `Copy` 语义的调用点

### 2. 搜索框“点击外部收起”误实现为全局鼠标拦截

症状：

- 点击菜单、点击列表、点击输入框边缘会异常收起

处理：

- 首轮使用 `TextInput` focus-lost 作为收起主触发
- 仅在 query 为空时 collapse

### 3. `flat` 模式下错误改写树展开状态

症状：

- 切到 `flat` 再点“展开全部”会污染 tree 的真实状态

处理：

- `toggle_asset_tree_expansion()` 在 `flat` 模式下直接 no-op

### 4. `Create` 菜单没有正确关闭

症状：

- 菜单项点击后仍残留
- 再次点击按钮状态不同步

处理：

- 所有菜单出口都走 `close_asset_create_menu()`
- `PopupWindow` 的 `close-requested` 和 Rust 状态要双向同步

## Handoff

计划完成后，执行者必须先确认：

- design 文档仍是最新
- 当前分支/工作区没有无关脏改动
- 可以按 TDD 顺序逐任务落地

Plan complete and saved to `docs/plans/2026-03-16-assets-sidebar-toolbar-implementation-plan.md`. Two execution options:

**1. Subagent-Driven (this session)** - I dispatch fresh subagent per task, review between tasks, fast iteration

**2. Parallel Session (separate)** - Open new session with executing-plans, batch execution with checkpoints

Which approach?

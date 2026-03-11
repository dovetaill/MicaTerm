# Sidebar Navigation Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 在不触碰 SSH / SFTP / terminal runtime 的前提下，把当前空的左侧 `Sidebar` 落地为已确认的双层 `Activity Bar + Assets Sidebar` 导航骨架，接入 `Folder / Folder Open` 开关、`Window Console`、`Snippets`、`Keychain` 三个一级模块，以及 Rust 驱动的导航状态模型。

**Architecture:** 保持现有 `AppWindow -> Sidebar -> WelcomeView / RightPanel` 的 shell 结构不变，但把现有单层 `48px` 左栏恢复为双层左区：固定 `48px Activity Bar` 负责一级导航，按需展开的 `256px Assets Sidebar` 负责模块内容。Rust 侧新增 `SidebarDestination` 和扩展后的 `ShellViewModel` 作为唯一状态源，`bootstrap` 负责把状态同步到 Slint 属性与 `ModelRc` 导航模型；Slint 只负责声明式渲染、点击回调和轻量转场。

**Tech Stack:** Rust, Cargo, Slint 1.15.1, `ModelRc` / `VecModel`, Fluent SVG assets, `i-slint-backend-testing`, shell smoke scripts, `cargo fmt`, `cargo test`, `cargo check`, `cargo clippy`

---

## Execution Notes

- 设计输入固定为 `docs/plans/2026-03-11-sidebar-navigation-design.md`，实现时不得偏离已确认选型：`1B + 2A + 3B + 4A + 5A + 6B + 7A + 8A`。
- 每个任务都先用 `@superpowers:test-driven-development`：先写失败测试或失败 smoke，再写最小实现，再跑通过。
- 如果 `Slint ModelRc`、数组属性、回调联动出现异常，不允许猜测，立即切换到 `@superpowers:systematic-debugging`。
- 本轮只交付左侧导航骨架与内容占位，不顺手实现真实 terminal、SFTP、snippet 编辑器或 keychain 存储。
- 尽量复用现有 `ThemeTokens`、`ShellMetrics`、`TitlebarIconButton` 风格语言，但不要把 titlebar tooltip 逻辑硬塞进 sidebar。
- 计划默认在从 `/home/wwwroot/mica-term` 派生的独立 worktree 中执行；若继续在当前工作区执行，也必须将改动范围严格限制在本计划列出的文件。

### Target Snapshot

完成后应满足以下用户可见结果：

- 左侧固定保留 `48px Activity Bar`
- 默认显示 `Folder Open`
- 点击 `Folder Open` 后折叠 `Assets Sidebar` 并切换为 `Folder`
- `Window Console`、`Snippets`、`Keychain` 是一级导航项
- 点击一级导航时自动展开 `Assets Sidebar`
- `Assets Sidebar` 根据当前 destination 显示对应占位内容
- 导航项由 Rust 侧模型驱动，而不是在 `.slint` 中写死三个按钮

### Out of Scope

- `wezterm-term` / `termwiz` 接入
- `russh` 会话连接
- Snippet 执行与变量模板
- Keychain 持久化与加密
- Transfers / Tunnels / Settings / Logs 的可见 UI

## Task 1: 建立 Sidebar 状态契约与尺寸常量

**Files:**
- Create: `src/shell/sidebar.rs`
- Modify: `src/shell/mod.rs`
- Modify: `src/shell/view_model.rs`
- Modify: `src/shell/metrics.rs`
- Create: `tests/sidebar_navigation_spec.rs`
- Modify: `tests/shell_view_model.rs`
- Modify: `tests/window_shell.rs`

**Step 1: Write the failing tests**

创建 `tests/sidebar_navigation_spec.rs`：

```rust
use mica_term::shell::sidebar::{SidebarDestination, sidebar_destinations};
use mica_term::shell::view_model::ShellViewModel;

#[test]
fn sidebar_destinations_match_the_approved_order() {
    assert_eq!(
        sidebar_destinations(),
        &[
            SidebarDestination::Console,
            SidebarDestination::Snippets,
            SidebarDestination::Keychain,
        ]
    );
}

#[test]
fn shell_view_model_starts_with_console_selected_and_assets_sidebar_open() {
    let view_model = ShellViewModel::default();

    assert!(view_model.show_assets_sidebar);
    assert_eq!(view_model.active_sidebar_destination, SidebarDestination::Console);
}

#[test]
fn toggling_assets_sidebar_keeps_current_destination() {
    let mut view_model = ShellViewModel::default();

    view_model.select_sidebar_destination(SidebarDestination::Snippets);
    view_model.toggle_assets_sidebar();

    assert!(!view_model.show_assets_sidebar);
    assert_eq!(view_model.active_sidebar_destination, SidebarDestination::Snippets);
}

#[test]
fn selecting_sidebar_destination_auto_expands_assets_sidebar() {
    let mut view_model = ShellViewModel::default();

    view_model.toggle_assets_sidebar();
    assert!(!view_model.show_assets_sidebar);

    view_model.select_sidebar_destination(SidebarDestination::Keychain);

    assert!(view_model.show_assets_sidebar);
    assert_eq!(view_model.active_sidebar_destination, SidebarDestination::Keychain);
}
```

修改 `tests/shell_view_model.rs`，补充现有壳层状态契约：

```rust
use mica_term::shell::sidebar::SidebarDestination;

#[test]
fn shell_view_model_starts_in_welcome_mode_with_right_panel_hidden() {
    let view_model = ShellViewModel::default();
    assert!(view_model.show_welcome);
    assert!(!view_model.show_right_panel);
    assert!(view_model.show_assets_sidebar);
    assert_eq!(view_model.active_sidebar_destination, SidebarDestination::Console);
}
```

修改 `tests/window_shell.rs`，补 sidebar 尺寸预算：

```rust
#[test]
fn sidebar_metrics_match_the_navigation_design() {
    assert_eq!(ShellMetrics::ACTIVITY_BAR_WIDTH, 48);
    assert_eq!(ShellMetrics::ASSETS_SIDEBAR_WIDTH, 256);
    assert_eq!(ShellMetrics::ACTIVITY_BAR_BUTTON_SIZE, 36);
    assert_eq!(ShellMetrics::ACTIVITY_BAR_ICON_SIZE, 20);
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --test sidebar_navigation_spec --test shell_view_model --test window_shell -q`  
Expected: FAIL，报错应包括 `could not find shell::sidebar`、`no field show_assets_sidebar`、`no method select_sidebar_destination`、`ACTIVITY_BAR_BUTTON_SIZE not found` 等。

**Step 3: Write minimal implementation**

创建 `src/shell/sidebar.rs`：

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarDestination {
    Console,
    Snippets,
    Keychain,
}

impl SidebarDestination {
    pub fn id(self) -> &'static str {
        match self {
            Self::Console => "console",
            Self::Snippets => "snippets",
            Self::Keychain => "keychain",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::Console => "Window Console",
            Self::Snippets => "Snippets",
            Self::Keychain => "Keychain",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "console" => Some(Self::Console),
            "snippets" => Some(Self::Snippets),
            "keychain" => Some(Self::Keychain),
            _ => None,
        }
    }
}

pub fn sidebar_destinations() -> &'static [SidebarDestination] {
    &[
        SidebarDestination::Console,
        SidebarDestination::Snippets,
        SidebarDestination::Keychain,
    ]
}
```

修改 `src/shell/mod.rs`：

```rust
pub mod metrics;
pub mod sidebar;
pub mod signature;
pub mod view_model;
```

修改 `src/shell/view_model.rs`：

```rust
use crate::shell::sidebar::SidebarDestination;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShellViewModel {
    pub show_welcome: bool,
    pub show_right_panel: bool,
    pub show_global_menu: bool,
    pub show_assets_sidebar: bool,
    pub active_sidebar_destination: SidebarDestination,
    pub is_window_maximized: bool,
    pub is_window_active: bool,
    pub theme_mode: ThemeMode,
    pub is_always_on_top: bool,
}

impl Default for ShellViewModel {
    fn default() -> Self {
        Self {
            show_welcome: true,
            show_right_panel: false,
            show_global_menu: false,
            show_assets_sidebar: true,
            active_sidebar_destination: SidebarDestination::Console,
            is_window_maximized: false,
            is_window_active: true,
            theme_mode: ThemeMode::Dark,
            is_always_on_top: false,
        }
    }
}

impl ShellViewModel {
    pub fn toggle_assets_sidebar(&mut self) {
        self.show_assets_sidebar = !self.show_assets_sidebar;
    }

    pub fn select_sidebar_destination(&mut self, destination: SidebarDestination) {
        self.active_sidebar_destination = destination;
        self.show_assets_sidebar = true;
    }
}
```

修改 `src/shell/metrics.rs`，补齐 sidebar 细化尺寸：

```rust
pub const ACTIVITY_BAR_BUTTON_SIZE: u32 = 36;
pub const ACTIVITY_BAR_ICON_SIZE: u32 = 20;
pub const ACTIVITY_BAR_DIVIDER_WIDTH: u32 = 1;
pub const ACTIVITY_BAR_DIVIDER_HEIGHT: u32 = 20;
pub const ASSETS_SIDEBAR_HEADER_HEIGHT: u32 = 44;
pub const ASSETS_SIDEBAR_SECTION_GAP: u32 = 12;
```

**Step 4: Run tests to verify they pass**

Run: `cargo test --test sidebar_navigation_spec --test shell_view_model --test window_shell -q`  
Expected: PASS，证明 sidebar destination 顺序、默认状态与尺寸常量已经稳定。

**Step 5: Commit**

```bash
git add src/shell/sidebar.rs src/shell/mod.rs src/shell/view_model.rs src/shell/metrics.rs \
  tests/sidebar_navigation_spec.rs tests/shell_view_model.rs tests/window_shell.rs
git commit -m "feat: define sidebar navigation state contract"
```

## Task 2: Vendor Sidebar Fluent Icons 并加 smoke 覆盖

**Files:**
- Create: `assets/icons/fluent/folder-20-regular.svg`
- Create: `assets/icons/fluent/folder-open-20-regular.svg`
- Create: `assets/icons/fluent/window-console-20-regular.svg`
- Create: `assets/icons/fluent/document-code-16-regular.svg`
- Create: `assets/icons/fluent/key-multiple-20-regular.svg`
- Create: `tests/sidebar_assets_smoke.sh`

**Step 1: Write the failing smoke test**

创建 `tests/sidebar_assets_smoke.sh`：

```bash
#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

for file in \
  assets/icons/fluent/folder-20-regular.svg \
  assets/icons/fluent/folder-open-20-regular.svg \
  assets/icons/fluent/window-console-20-regular.svg \
  assets/icons/fluent/document-code-16-regular.svg \
  assets/icons/fluent/key-multiple-20-regular.svg
do
  [[ -f "$ROOT_DIR/$file" ]] || {
    echo "missing $file" >&2
    exit 1
  }
done
```

**Step 2: Run smoke test to verify it fails**

Run: `bash tests/sidebar_assets_smoke.sh`  
Expected: FAIL，至少报出一个 `missing assets/icons/fluent/...`。

**Step 3: Vendor the exact SVG assets**

将 Fluent 官方 SVG 以稳定文件名复制进仓库，保持下列文件名不变：

- `folder-20-regular.svg`
- `folder-open-20-regular.svg`
- `window-console-20-regular.svg`
- `document-code-16-regular.svg`
- `key-multiple-20-regular.svg`

要求：

- 不做运行时下载
- 不引入 icon font
- 保持 SVG 原始 `viewBox`
- 如有必要，只做最小文件名规范化，不重绘图标

**Step 4: Run smoke test to verify it passes**

Run: `bash tests/sidebar_assets_smoke.sh`  
Expected: PASS，所有 sidebar 所需 Fluent 资源都已在本地仓库存在。

**Step 5: Commit**

```bash
git add assets/icons/fluent/folder-20-regular.svg \
  assets/icons/fluent/folder-open-20-regular.svg \
  assets/icons/fluent/window-console-20-regular.svg \
  assets/icons/fluent/document-code-16-regular.svg \
  assets/icons/fluent/key-multiple-20-regular.svg \
  tests/sidebar_assets_smoke.sh
git commit -m "feat: add fluent sidebar navigation assets"
```

## Task 3: 搭建 Slint 双层 Sidebar 组件树与 UI contract smoke

**Files:**
- Create: `ui/components/sidebar-nav-button.slint`
- Create: `ui/shell/assets-sidebar.slint`
- Modify: `ui/shell/sidebar.slint`
- Modify: `ui/app-window.slint`
- Create: `tests/sidebar_ui_contract_smoke.sh`

**Step 1: Write the failing UI contract smoke**

创建 `tests/sidebar_ui_contract_smoke.sh`：

```bash
#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SIDEBAR="$ROOT_DIR/ui/shell/sidebar.slint"
ASSETS="$ROOT_DIR/ui/shell/assets-sidebar.slint"
APP_WINDOW="$ROOT_DIR/ui/app-window.slint"
BUTTON="$ROOT_DIR/ui/components/sidebar-nav-button.slint"

[[ -f "$BUTTON" ]] || {
  echo "missing ui/components/sidebar-nav-button.slint" >&2
  exit 1
}

[[ -f "$ASSETS" ]] || {
  echo "missing ui/shell/assets-sidebar.slint" >&2
  exit 1
}

grep -F 'export struct SidebarNavItem' "$SIDEBAR" >/dev/null
grep -F 'in property <[SidebarNavItem]> items;' "$SIDEBAR" >/dev/null
grep -F 'in property <bool> show-assets-sidebar: true;' "$SIDEBAR" >/dev/null
grep -F 'callback toggle-assets-sidebar-requested();' "$APP_WINDOW" >/dev/null
grep -F 'callback sidebar-destination-selected(string);' "$APP_WINDOW" >/dev/null
grep -F 'Sidebar {' "$APP_WINDOW" >/dev/null
grep -F 'show-assets-sidebar: root.show-assets-sidebar;' "$APP_WINDOW" >/dev/null
grep -F 'active-sidebar-destination: root.active-sidebar-destination;' "$APP_WINDOW" >/dev/null
grep -F 'Folder Open' "$ASSETS" >/dev/null || true
grep -F 'Window Console' "$ASSETS" >/dev/null
grep -F 'Snippets' "$ASSETS" >/dev/null
grep -F 'Keychain' "$ASSETS" >/dev/null
grep -F 'folder-open-20-regular.svg' "$BUTTON" >/dev/null
grep -F 'window-console-20-regular.svg' "$BUTTON" >/dev/null
grep -F 'document-code-16-regular.svg' "$BUTTON" >/dev/null
grep -F 'key-multiple-20-regular.svg' "$BUTTON" >/dev/null
```

**Step 2: Run smoke test to verify it fails**

Run: `bash tests/sidebar_ui_contract_smoke.sh`  
Expected: FAIL，缺少 `sidebar-nav-button.slint`、`assets-sidebar.slint`，以及新的 `AppWindow` 属性 / 回调。

**Step 3: Write minimal UI structure**

创建 `ui/components/sidebar-nav-button.slint`：

```slint
import { ThemeTokens } from "../theme/tokens.slint";

export component SidebarNavButton inherits Rectangle {
    in property <string> item-id;
    in property <string> label;
    in property <bool> active: false;
    in property <bool> sidebar-open: true;
    callback clicked(string);

    private property <image> folder-icon: @image-url("../../assets/icons/fluent/folder-20-regular.svg");
    private property <image> folder-open-icon: @image-url("../../assets/icons/fluent/folder-open-20-regular.svg");
    private property <image> console-icon: @image-url("../../assets/icons/fluent/window-console-20-regular.svg");
    private property <image> snippets-icon: @image-url("../../assets/icons/fluent/document-code-16-regular.svg");
    private property <image> keychain-icon: @image-url("../../assets/icons/fluent/key-multiple-20-regular.svg");

    private property <image> displayed-icon:
        item-id == "sidebar-toggle"
            ? (sidebar-open ? folder-open-icon : folder-icon)
            : item-id == "console"
                ? console-icon
                : item-id == "snippets"
                    ? snippets-icon
                    : keychain-icon;

    width: 36px;
    height: 36px;
    border-radius: 8px;
    background: touch.pressed
        ? ThemeTokens.panel-tint
        : (touch.has-hover || root.active)
            ? ThemeTokens.shell-surface
            : transparent;

    Image {
        x: (parent.width - self.width) / 2;
        y: (parent.height - self.height) / 2;
        width: 20px;
        height: 20px;
        source: root.displayed-icon;
        image-fit: contain;
        colorize: ThemeTokens.text-primary;
    }

    touch := TouchArea {
        clicked => {
            root.clicked(root.item-id);
        }
    }
}
```

创建 `ui/shell/assets-sidebar.slint`：

```slint
import { ThemeTokens } from "../theme/tokens.slint";

export component AssetsSidebar inherits Rectangle {
    in property <bool> expanded: true;
    in property <string> active-panel: "console";

    width: expanded ? 256px : 0px;
    visible: expanded;
    clip: true;
    background: ThemeTokens.shell-surface;
    border-width: 1px;
    border-color: ThemeTokens.shell-stroke;

    if root.active-panel == "console" : VerticalLayout {
        Text { text: "Window Console"; color: ThemeTokens.text-primary; }
        Text { text: "Hosts, recent sessions, favorites"; color: ThemeTokens.text-primary; }
    }

    if root.active-panel == "snippets" : VerticalLayout {
        Text { text: "Snippets"; color: ThemeTokens.text-primary; }
        Text { text: "Groups, favorites, templates"; color: ThemeTokens.text-primary; }
    }

    if root.active-panel == "keychain" : VerticalLayout {
        Text { text: "Keychain"; color: ThemeTokens.text-primary; }
        Text { text: "Accounts, identities, SSH keys"; color: ThemeTokens.text-primary; }
    }
}
```

修改 `ui/shell/sidebar.slint`：

```slint
import { ThemeTokens } from "../theme/tokens.slint";
import { AssetsSidebar } from "assets-sidebar.slint";
import { SidebarNavButton } from "../components/sidebar-nav-button.slint";

export struct SidebarNavItem {
    id: string,
    label: string,
    active: bool,
}

export component Sidebar inherits Rectangle {
    in property <[SidebarNavItem]> items;
    in property <bool> show-assets-sidebar: true;
    in property <string> active-sidebar-destination: "console";
    callback toggle-assets-sidebar-requested();
    callback destination-selected(string);

    background: transparent;

    HorizontalLayout {
        spacing: 0px;

        activity-bar := Rectangle {
            width: 48px;
            background: ThemeTokens.shell-surface;
            border-width: 1px;
            border-color: ThemeTokens.shell-stroke;

            VerticalLayout {
                padding-top: 6px;
                padding-left: 6px;
                padding-right: 6px;
                spacing: 8px;

                toggle-button := SidebarNavButton {
                    item-id: "sidebar-toggle";
                    label: "Toggle Sidebar";
                    sidebar-open: root.show-assets-sidebar;
                    clicked(item-id) => {
                        root.toggle-assets-sidebar-requested();
                    }
                }

                divider-line := Rectangle {
                    width: parent.width - 12px;
                    height: 1px;
                    background: ThemeTokens.shell-stroke;
                }

                for item in root.items : SidebarNavButton {
                    item-id: item.id;
                    label: item.label;
                    active: item.active;
                    sidebar-open: root.show-assets-sidebar;
                    clicked(item-id) => {
                        root.destination-selected(item-id);
                    }
                }
            }
        }

        AssetsSidebar {
            expanded: root.show-assets-sidebar;
            active-panel: root.active-sidebar-destination;
        }
    }
}
```

修改 `ui/app-window.slint`，增加 sidebar 状态透传：

```slint
import { Sidebar, SidebarNavItem } from "shell/sidebar.slint";

in-out property <bool> show-assets-sidebar: true;
in-out property <string> active-sidebar-destination: "console";
in-out property <[SidebarNavItem]> sidebar-items;

callback toggle-assets-sidebar-requested();
callback sidebar-destination-selected(string);

Sidebar {
    items: root.sidebar-items;
    show-assets-sidebar: root.show-assets-sidebar;
    active-sidebar-destination: root.active-sidebar-destination;

    toggle-assets-sidebar-requested => {
        root.toggle-assets-sidebar-requested();
    }

    destination-selected(id) => {
        root.sidebar-destination-selected(id);
    }
}
```

**Step 4: Run smoke test to verify it passes**

Run: `bash tests/sidebar_ui_contract_smoke.sh`  
Expected: PASS，证明双层 sidebar 组件树、顶层属性与图标依赖都已经存在。

**Step 5: Commit**

```bash
git add ui/components/sidebar-nav-button.slint ui/shell/assets-sidebar.slint \
  ui/shell/sidebar.slint ui/app-window.slint tests/sidebar_ui_contract_smoke.sh
git commit -m "feat: add dual-pane sidebar slint layout"
```

## Task 4: 在 Bootstrap 中接入 sidebar 状态同步与导航模型

**Files:**
- Modify: `src/app/bootstrap.rs`
- Modify: `src/shell/sidebar.rs`
- Create: `tests/sidebar_navigation_smoke.rs`
- Modify: `tests/top_status_bar_smoke.rs`

**Step 1: Write the failing tests**

创建 `tests/sidebar_navigation_smoke.rs`：

```rust
use std::fs;

use mica_term::AppWindow;
use mica_term::app::bootstrap::bind_top_status_bar_with_store;
use mica_term::app::ui_preferences::UiPreferencesStore;

#[test]
fn bootstrap_initializes_sidebar_defaults() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let temp_path = std::env::temp_dir()
        .join("mica-term")
        .join("tests")
        .join("sidebar-defaults.json");
    let _ = fs::remove_file(&temp_path);

    bind_top_status_bar_with_store(&app, Some(UiPreferencesStore::new(temp_path.clone())));

    assert!(app.get_show_assets_sidebar());
    assert_eq!(app.get_active_sidebar_destination().as_str(), "console");

    let _ = fs::remove_file(temp_path);
}

#[test]
fn bootstrap_toggles_assets_sidebar_without_losing_destination() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let temp_path = std::env::temp_dir()
        .join("mica-term")
        .join("tests")
        .join("sidebar-toggle.json");
    let _ = fs::remove_file(&temp_path);

    bind_top_status_bar_with_store(&app, Some(UiPreferencesStore::new(temp_path.clone())));

    app.invoke_sidebar_destination_selected("snippets".into());
    assert_eq!(app.get_active_sidebar_destination().as_str(), "snippets");

    app.invoke_toggle_assets_sidebar_requested();
    assert!(!app.get_show_assets_sidebar());
    assert_eq!(app.get_active_sidebar_destination().as_str(), "snippets");

    let _ = fs::remove_file(temp_path);
}

#[test]
fn selecting_destination_auto_expands_assets_sidebar() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let temp_path = std::env::temp_dir()
        .join("mica-term")
        .join("tests")
        .join("sidebar-select.json");
    let _ = fs::remove_file(&temp_path);

    bind_top_status_bar_with_store(&app, Some(UiPreferencesStore::new(temp_path.clone())));

    app.invoke_toggle_assets_sidebar_requested();
    assert!(!app.get_show_assets_sidebar());

    app.invoke_sidebar_destination_selected("keychain".into());
    assert!(app.get_show_assets_sidebar());
    assert_eq!(app.get_active_sidebar_destination().as_str(), "keychain");

    let _ = fs::remove_file(temp_path);
}
```

修改 `tests/top_status_bar_smoke.rs`，在现有绑定测试里加一条 sidebar 初始断言：

```rust
assert!(app.get_show_assets_sidebar());
assert_eq!(app.get_active_sidebar_destination().as_str(), "console");
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --test sidebar_navigation_smoke --test top_status_bar_smoke -q`  
Expected: FAIL，报错应包括缺少 `show_assets_sidebar` / `active_sidebar_destination` getter，或者缺少 `invoke_sidebar_destination_selected`、`invoke_toggle_assets_sidebar_requested`。

**Step 3: Wire bootstrap and model sync**

修改 `src/shell/sidebar.rs`，补一个构造 UI 导航模型的 helper：

```rust
use slint::SharedString;

use crate::SidebarNavItem;
use crate::shell::view_model::ShellViewModel;

pub fn sidebar_items_for(state: &ShellViewModel) -> Vec<SidebarNavItem> {
    sidebar_destinations()
        .iter()
        .map(|destination| SidebarNavItem {
            id: SharedString::from(destination.id()),
            label: SharedString::from(destination.title()),
            active: *destination == state.active_sidebar_destination,
        })
        .collect()
}
```

修改 `src/app/bootstrap.rs`，新增 sidebar 同步函数并挂载回调：

```rust
use slint::{ModelRc, VecModel};

use crate::shell::sidebar::{SidebarDestination, sidebar_items_for};

fn sync_sidebar_state(window: &AppWindow, state: &ShellViewModel) {
    window.set_show_assets_sidebar(state.show_assets_sidebar);
    window.set_active_sidebar_destination(state.active_sidebar_destination.id().into());
    window.set_sidebar_items(ModelRc::new(VecModel::from(sidebar_items_for(state))));
}

fn sync_shell_state(
    window: &AppWindow,
    state: &ShellViewModel,
    effects: &dyn PlatformWindowEffects,
) {
    sync_top_status_bar_state(window, state, effects);
    sync_sidebar_state(window, state);
}
```

在初始化时调用：

```rust
sync_shell_state(window, &view_model.borrow(), effects.as_ref());
```

添加 toggle 回调：

```rust
let state = Rc::clone(&view_model);
let handle = window.as_weak();
let effects_ref = Rc::clone(&effects);
window.on_toggle_assets_sidebar_requested(move || {
    let window = handle.unwrap();
    let mut state = state.borrow_mut();
    state.toggle_assets_sidebar();
    sync_shell_state(&window, &state, effects_ref.as_ref());
});

let state = Rc::clone(&view_model);
let handle = window.as_weak();
let effects_ref = Rc::clone(&effects);
window.on_sidebar_destination_selected(move |destination_id| {
    let window = handle.unwrap();
    let mut state = state.borrow_mut();
    let destination = SidebarDestination::from_id(destination_id.as_str())
        .unwrap_or(SidebarDestination::Console);
    state.select_sidebar_destination(destination);
    sync_shell_state(&window, &state, effects_ref.as_ref());
});
```

注意：

- 不要重命名现有 `bind_top_status_bar*` 公共 API，本轮只扩展其职责
- 由于 sidebar item 只有 3 个，允许每次状态变化时重建 `VecModel`
- 不要把 `show_assets_sidebar` 写入 `UiPreferencesStore`，本轮不新增持久化范围

**Step 4: Run tests to verify they pass**

Run: `cargo test --test sidebar_navigation_smoke --test top_status_bar_smoke -q`  
Expected: PASS，证明 bootstrap 已经初始化 sidebar 默认状态、切换 destination 时自动展开、折叠时不丢选中态。

**Step 5: Commit**

```bash
git add src/app/bootstrap.rs src/shell/sidebar.rs \
  tests/sidebar_navigation_smoke.rs tests/top_status_bar_smoke.rs
git commit -m "feat: bind sidebar navigation state into app window"
```

## Task 5: 完成集成验证与清理

**Files:**
- No source changes expected unless verification exposes defects

**Step 1: Run formatting**

Run: `cargo fmt --all`  
Expected: 命令成功退出；若有格式差异，检查 diff 仅限本计划列出的文件。

**Step 2: Run targeted Rust tests**

Run:

```bash
cargo test --test sidebar_navigation_spec --test shell_view_model \
  --test sidebar_navigation_smoke --test top_status_bar_smoke --test window_shell -q
```

Expected: PASS，且不引入 titlebar/window shell 既有回归。

**Step 3: Run sidebar smoke scripts**

Run:

```bash
bash tests/sidebar_assets_smoke.sh
bash tests/sidebar_ui_contract_smoke.sh
```

Expected: PASS，证明 SVG 资源、Slint 结构、顶层 callback/property 契约都已满足。

**Step 4: Run repository-level sanity checks**

Run:

```bash
cargo check -q
cargo clippy --all-targets -- -D warnings
```

Expected: PASS；如果 `clippy` 失败，只修本轮 sidebar 改动引入的问题，不顺手清仓库历史告警。

**Step 5: Commit final verification-safe state**

```bash
git add src/app/bootstrap.rs src/shell/sidebar.rs src/shell/view_model.rs src/shell/metrics.rs \
  ui/components/sidebar-nav-button.slint ui/shell/assets-sidebar.slint ui/shell/sidebar.slint \
  ui/app-window.slint tests/sidebar_navigation_spec.rs tests/sidebar_navigation_smoke.rs \
  tests/sidebar_assets_smoke.sh tests/sidebar_ui_contract_smoke.sh \
  tests/shell_view_model.rs tests/top_status_bar_smoke.rs tests/window_shell.rs \
  assets/icons/fluent/folder-20-regular.svg assets/icons/fluent/folder-open-20-regular.svg \
  assets/icons/fluent/window-console-20-regular.svg assets/icons/fluent/document-code-16-regular.svg \
  assets/icons/fluent/key-multiple-20-regular.svg
git commit -m "feat: implement sidebar navigation shell"
```

## Verification Checklist

- [ ] `Activity Bar` 固定为 `48px`
- [ ] `Assets Sidebar` 默认展开且宽度为 `256px`
- [ ] 默认 destination 是 `Window Console`
- [ ] `Folder Open` 点击后仅折叠 `Assets Sidebar`，不清空当前 destination
- [ ] 折叠状态下点击 `Snippets` 或 `Keychain` 会自动展开
- [ ] 顶层 `AppWindow` 已暴露 `show-assets-sidebar`、`active-sidebar-destination`、`sidebar-items`
- [ ] 导航项由 Rust 侧模型生成
- [ ] SVG 图标均来自本地 vendored Fluent assets
- [ ] 不引入 terminal runtime、SFTP runtime 或 keychain 存储实现

## Rollback Notes

- 如果 Task 3 的 `SidebarNavButton` 过于复杂，允许先不做 tooltip，只保留点击与 active 态
- 如果 `ModelRc` 绑定在 Task 4 出现不稳定行为，允许先使用“重建 `VecModel` 再整体 set 回窗口”的方式，不要过早引入自定义 `Model`
- 如果 `cargo clippy` 暴露仓库历史告警，本轮只修 sidebar 新增代码路径，不做仓库级 cleanup


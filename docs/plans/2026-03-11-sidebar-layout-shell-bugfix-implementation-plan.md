# Sidebar Layout Shell Bugfix Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 修复当前 `Sidebar` 引入后的 shell 布局回归，使默认打开、恢复窗口和最大化三种状态下都能正确填满窗口 client area，同时落实 `Activity Bar -> Assets Sidebar -> RightPanel -> Main Workspace` 的响应式优先级。

**Architecture:** 以 Rust 侧显式控制 restored window size 作为窗口尺寸真源，并新增纯 Rust 的 `shell layout policy` 负责计算在给定逻辑宽度下的有效布局状态。Slint 侧将 `AppWindow` 重构为明确的 `ShellFrame / ShellBody` 结构，由 `ShellBody` 独占剩余高度，`Sidebar`、主区和 `RightPanel` 只消费 Rust 下发的有效状态与宽度契约；最后通过几何契约测试而不是截图来防止回归。

**Tech Stack:** Rust, Cargo, Slint 1.15.1, `slint::Window`, `i-slint-backend-testing`, shell smoke scripts, `cargo fmt`, `cargo test`, `cargo check`, `cargo clippy`

---

## Execution Notes

- 设计输入固定为 `docs/plans/2026-03-11-sidebar-layout-shell-bugfix-design.md`，实现时不得偏离已确认选型：`1B + 2B + 3B + 4A`。
- 每个任务都先用 `@superpowers:test-driven-development`：先写失败测试，再写最小实现，再跑通过。
- 如果 `Slint` 的窗口尺寸同步、`changed width/height`、或测试 backend 的几何行为与预期不一致，不允许猜测，立即切换到 `@superpowers:systematic-debugging`。
- 本轮只修 shell 壳层和布局契约，不顺手实现 terminal runtime、SSH、SFTP、Snippets 管理器或 Keychain 真实业务。
- 响应式“优先级折叠”必须区分“用户请求状态”和“当前有效显示状态”，不要在缩窗时直接抹掉用户 intent。
- 计划默认在独立 worktree 执行；若继续在当前工作区执行，也必须把改动限制在本计划列出的文件中。

### Target Snapshot

完成后应满足以下用户可见结果：

- 启动时窗口恢复为 `1440x900` 的设计默认尺寸
- 默认打开不再出现下半区缺失
- 最大化后 `Titlebar` 保持固定高度，`ShellBody` 跟随窗口增长
- `Activity Bar` 永远固定 `48px`
- 宽度不足时先折叠 `Assets Sidebar`，再隐藏 `RightPanel`
- 主区始终拿到剩余宽度，并且高度吃满 `Window - Titlebar`
- 现有 sidebar 导航状态与 tooltip 行为不退化

### Out of Scope

- `wezterm-term` / `termwiz` 接入
- `russh` / `SFTP` 连接逻辑
- Snippets / Keychain 的真实业务内容
- 窗口位置与上次尺寸持久化
- 基于截图的 golden test 基础设施

## Task 1: 建立窗口尺寸常量与响应式布局策略

**Files:**
- Create: `src/shell/layout.rs`
- Modify: `src/shell/mod.rs`
- Modify: `src/shell/metrics.rs`
- Create: `tests/shell_layout_policy.rs`
- Modify: `tests/window_shell.rs`
- Modify: `tests/bootstrap_smoke.rs`

**Step 1: Write the failing tests**

创建 `tests/shell_layout_policy.rs`：

```rust
use mica_term::shell::layout::{ShellLayoutInput, resolve_shell_layout};
use mica_term::shell::metrics::ShellMetrics;

#[test]
fn layout_policy_keeps_full_shell_when_width_budget_is_sufficient() {
    let layout = resolve_shell_layout(ShellLayoutInput {
        window_width: ShellMetrics::WINDOW_DEFAULT_WIDTH,
        request_assets_sidebar: true,
        request_right_panel: true,
    });

    assert!(layout.show_assets_sidebar);
    assert!(layout.show_right_panel);
    assert!(layout.main_workspace_width >= ShellMetrics::MAIN_WORKSPACE_MIN_WIDTH);
}

#[test]
fn layout_policy_collapses_assets_sidebar_before_right_panel() {
    let collapse_assets = resolve_shell_layout(ShellLayoutInput {
        window_width: ShellMetrics::FULL_LAYOUT_MIN_WIDTH - 1,
        request_assets_sidebar: true,
        request_right_panel: true,
    });
    assert!(!collapse_assets.show_assets_sidebar);
    assert!(collapse_assets.show_right_panel);

    let collapse_right = resolve_shell_layout(ShellLayoutInput {
        window_width: ShellMetrics::RIGHT_PANEL_ONLY_MIN_WIDTH - 1,
        request_assets_sidebar: true,
        request_right_panel: true,
    });
    assert!(!collapse_right.show_assets_sidebar);
    assert!(!collapse_right.show_right_panel);
}

#[test]
fn layout_policy_preserves_requested_state_when_regions_are_not_requested() {
    let layout = resolve_shell_layout(ShellLayoutInput {
        window_width: ShellMetrics::WINDOW_DEFAULT_WIDTH,
        request_assets_sidebar: false,
        request_right_panel: false,
    });

    assert!(!layout.show_assets_sidebar);
    assert!(!layout.show_right_panel);
}
```

修改 `tests/window_shell.rs`，补布局预算断言：

```rust
#[test]
fn shell_layout_metrics_match_the_layout_bugfix_budget() {
    assert_eq!(ShellMetrics::WINDOW_DEFAULT_WIDTH, 1440);
    assert_eq!(ShellMetrics::WINDOW_DEFAULT_HEIGHT, 900);
    assert_eq!(ShellMetrics::ACTIVITY_BAR_WIDTH, 48);
    assert_eq!(ShellMetrics::ASSETS_SIDEBAR_WIDTH, 256);
    assert_eq!(ShellMetrics::RIGHT_PANEL_WIDTH, 392);
    assert_eq!(ShellMetrics::MAIN_WORKSPACE_MIN_WIDTH, 640);
}
```

修改 `tests/bootstrap_smoke.rs`，让 `default_window_size()` 成为 metrics 的投影：

```rust
use mica_term::shell::metrics::ShellMetrics;

#[test]
fn bootstrap_exposes_shell_default_window_budget() {
    assert_eq!(app_title(), "Mica Term");
    assert_eq!(
        default_window_size(),
        (
            ShellMetrics::WINDOW_DEFAULT_WIDTH,
            ShellMetrics::WINDOW_DEFAULT_HEIGHT,
        )
    );
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --test shell_layout_policy --test window_shell --test bootstrap_smoke -q`  
Expected: FAIL，报错应包括 `could not find shell::layout`、`WINDOW_DEFAULT_WIDTH not found`、`MAIN_WORKSPACE_MIN_WIDTH not found` 等。

**Step 3: Write the minimal implementation**

创建 `src/shell/layout.rs`：

```rust
use crate::shell::metrics::ShellMetrics;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShellLayoutInput {
    pub window_width: u32,
    pub request_assets_sidebar: bool,
    pub request_right_panel: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShellLayoutDecision {
    pub show_assets_sidebar: bool,
    pub show_right_panel: bool,
    pub main_workspace_width: u32,
}

pub fn resolve_shell_layout(input: ShellLayoutInput) -> ShellLayoutDecision {
    let show_assets_sidebar =
        input.request_assets_sidebar && input.window_width >= ShellMetrics::FULL_LAYOUT_MIN_WIDTH;

    let right_panel_threshold = if show_assets_sidebar {
        ShellMetrics::FULL_LAYOUT_MIN_WIDTH
    } else {
        ShellMetrics::RIGHT_PANEL_ONLY_MIN_WIDTH
    };

    let show_right_panel =
        input.request_right_panel && input.window_width >= right_panel_threshold;

    let occupied = ShellMetrics::ACTIVITY_BAR_WIDTH
        + if show_assets_sidebar {
            ShellMetrics::ASSETS_SIDEBAR_WIDTH
        } else {
            0
        }
        + if show_right_panel {
            ShellMetrics::RIGHT_PANEL_WIDTH
        } else {
            0
        };

    ShellLayoutDecision {
        show_assets_sidebar,
        show_right_panel,
        main_workspace_width: input.window_width.saturating_sub(occupied),
    }
}
```

修改 `src/shell/mod.rs`：

```rust
pub mod layout;
pub mod metrics;
pub mod sidebar;
pub mod signature;
pub mod view_model;
```

修改 `src/shell/metrics.rs`，补窗口与主区预算：

```rust
pub const WINDOW_DEFAULT_WIDTH: u32 = 1440;
pub const WINDOW_DEFAULT_HEIGHT: u32 = 900;
pub const WINDOW_MIN_HEIGHT: u32 = 640;
pub const MAIN_WORKSPACE_MIN_WIDTH: u32 = 640;
pub const FULL_LAYOUT_MIN_WIDTH: u32 =
    ACTIVITY_BAR_WIDTH + ASSETS_SIDEBAR_WIDTH + MAIN_WORKSPACE_MIN_WIDTH + RIGHT_PANEL_WIDTH;
pub const RIGHT_PANEL_ONLY_MIN_WIDTH: u32 =
    ACTIVITY_BAR_WIDTH + MAIN_WORKSPACE_MIN_WIDTH + RIGHT_PANEL_WIDTH;
pub const WINDOW_MIN_WIDTH: u32 = ACTIVITY_BAR_WIDTH + MAIN_WORKSPACE_MIN_WIDTH;
```

修改 `src/app/bootstrap.rs`：

```rust
pub fn default_window_size() -> (u32, u32) {
    (
        ShellMetrics::WINDOW_DEFAULT_WIDTH,
        ShellMetrics::WINDOW_DEFAULT_HEIGHT,
    )
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test --test shell_layout_policy --test window_shell --test bootstrap_smoke -q`  
Expected: PASS，证明窗口尺寸与响应式预算已经有稳定常量和纯 Rust 策略函数。

**Step 5: Commit**

```bash
git add src/shell/layout.rs src/shell/mod.rs src/shell/metrics.rs src/app/bootstrap.rs \
  tests/shell_layout_policy.rs tests/window_shell.rs tests/bootstrap_smoke.rs
git commit -m "feat: add shell layout policy budget"
```

## Task 2: 接管 runtime window size，并分离请求状态与有效显示状态

**Files:**
- Modify: `src/app/windowing.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `src/shell/view_model.rs`
- Modify: `ui/app-window.slint`
- Modify: `tests/top_status_bar_smoke.rs`
- Modify: `tests/sidebar_navigation_smoke.rs`

**Step 1: Write the failing tests**

修改 `tests/top_status_bar_smoke.rs`，验证绑定后窗口尺寸会被同步到默认 restored size：

```rust
use slint::{ComponentHandle, PhysicalSize};

#[test]
fn bootstrap_applies_default_restored_size_before_run() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    app.window().set_size(PhysicalSize::new(800, 500));

    bind_top_status_bar_with_store(&app, None);

    let size = app.window().size();
    assert_eq!((size.width, size.height), (1440, 900));
}
```

修改 `tests/sidebar_navigation_smoke.rs`，验证“请求状态”与“有效状态”分离：

```rust
use slint::{ComponentHandle, PhysicalSize};

#[test]
fn narrow_width_preserves_requested_right_panel_but_hides_it_effectively() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_toggle_right_panel_requested();
    app.window().set_size(PhysicalSize::new(1000, 900));
    app.invoke_shell_layout_invalidated(1000.0, 900.0);

    assert!(app.get_show_right_panel());
    assert!(!app.get_effective_show_assets_sidebar());
    assert!(!app.get_effective_show_right_panel());
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --test top_status_bar_smoke --test sidebar_navigation_smoke -q`  
Expected: FAIL，报错应包括 `no method invoke_shell_layout_invalidated`、`no method get_effective_show_assets_sidebar`、绑定后窗口尺寸仍为 `800x500` 等。

**Step 3: Write the minimal implementation**

修改 `src/shell/view_model.rs`，明确“请求状态”只代表用户 intent：

```rust
impl ShellViewModel {
    pub fn requested_assets_sidebar(&self) -> bool {
        self.show_assets_sidebar
    }

    pub fn requested_right_panel(&self) -> bool {
        self.show_right_panel
    }
}
```

修改 `src/app/windowing.rs`，新增窗口尺寸同步助手：

```rust
use slint::{ComponentHandle, PhysicalSize};

pub fn apply_restored_window_size<C: ComponentHandle>(component: &C, size: (u32, u32)) {
    component
        .window()
        .set_size(PhysicalSize::new(size.0, size.1));
}
```

修改 `ui/app-window.slint`，新增有效布局属性和尺寸失效回调：

```slint
in-out property <bool> effective-show-assets-sidebar: true;
in-out property <bool> effective-show-right-panel: false;
callback shell-layout-invalidated(length, length);

changed width => {
    root.shell-layout-invalidated(self.width, self.height);
}

changed height => {
    root.shell-layout-invalidated(self.width, self.height);
}
```

修改 `src/app/bootstrap.rs`，在 bind 阶段显式设置 restored size，并在统一的 `sync_shell_layout()` 中计算有效显示状态：

```rust
use crate::shell::layout::{ShellLayoutInput, resolve_shell_layout};
use crate::shell::metrics::ShellMetrics;

fn sync_shell_layout(window: &AppWindow, state: &ShellViewModel, logical_width: u32) {
    let layout = resolve_shell_layout(ShellLayoutInput {
        window_width: logical_width.max(ShellMetrics::WINDOW_MIN_WIDTH),
        request_assets_sidebar: state.requested_assets_sidebar(),
        request_right_panel: state.requested_right_panel(),
    });

    window.set_effective_show_assets_sidebar(layout.show_assets_sidebar);
    window.set_effective_show_right_panel(layout.show_right_panel);
}

pub fn bind_top_status_bar_with_store_and_effects(...) {
    // create window...
    apply_restored_window_size(window, default_window_size());
    sync_shell_state(window, &view_model.borrow(), effects.as_ref());
    sync_shell_layout(window, &view_model.borrow(), ShellMetrics::WINDOW_DEFAULT_WIDTH);

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_shell_layout_invalidated(move |width, _height| {
        let window = handle.unwrap();
        let state = state.borrow();
        sync_shell_layout(&window, &state, width as u32);
    });
}
```

注意：所有现有 `toggle-right-panel`、`toggle-assets-sidebar`、`select-sidebar-destination` 回调在更新请求状态后，都要补一次 `sync_shell_layout(...)`，不要只更新旧属性。

**Step 4: Run tests to verify they pass**

Run: `cargo test --test top_status_bar_smoke --test sidebar_navigation_smoke -q`  
Expected: PASS，说明默认尺寸会在运行时真正应用，同时有效可见性能够随宽度变化。

**Step 5: Commit**

```bash
git add src/app/windowing.rs src/app/bootstrap.rs src/shell/view_model.rs ui/app-window.slint \
  tests/top_status_bar_smoke.rs tests/sidebar_navigation_smoke.rs
git commit -m "fix: sync runtime shell window size"
```

## Task 3: 重构 Slint 壳层为 `ShellFrame / ShellBody`

**Files:**
- Modify: `ui/app-window.slint`
- Modify: `ui/shell/sidebar.slint`
- Modify: `ui/shell/right-panel.slint`
- Modify: `ui/welcome/welcome-view.slint`
- Create: `tests/shell_layout_ui_contract_smoke.sh`
- Modify: `tests/sidebar_ui_contract_smoke.sh`

**Step 1: Write the failing UI contract smoke**

创建 `tests/shell_layout_ui_contract_smoke.sh`：

```bash
#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_WINDOW="$ROOT_DIR/ui/app-window.slint"
SIDEBAR="$ROOT_DIR/ui/shell/sidebar.slint"
RIGHT_PANEL="$ROOT_DIR/ui/shell/right-panel.slint"
WELCOME="$ROOT_DIR/ui/welcome/welcome-view.slint"

grep -F 'shell-frame := Rectangle' "$APP_WINDOW" >/dev/null
grep -F 'body-host := Rectangle' "$APP_WINDOW" >/dev/null
grep -F 'vertical-stretch: 1;' "$APP_WINDOW" >/dev/null
grep -F 'shell-body := HorizontalLayout' "$APP_WINDOW" >/dev/null
grep -F 'effective-show-assets-sidebar: root.effective-show-assets-sidebar;' "$APP_WINDOW" >/dev/null
grep -F 'expanded: root.effective-show-right-panel;' "$APP_WINDOW" >/dev/null
grep -F 'visible: root.expanded;' "$RIGHT_PANEL" >/dev/null
grep -F 'VerticalLayout {' "$WELCOME" >/dev/null
grep -F 'activity-bar := Rectangle' "$SIDEBAR" >/dev/null
```

修改 `tests/sidebar_ui_contract_smoke.sh`，把 sidebar 展开绑定从请求态切到有效态：

```bash
grep -F 'show-assets-sidebar: root.effective-show-assets-sidebar;' "$APP_WINDOW" >/dev/null
```

**Step 2: Run smoke to verify it fails**

Run: `bash tests/shell_layout_ui_contract_smoke.sh && bash tests/sidebar_ui_contract_smoke.sh`  
Expected: FAIL，说明新的 `ShellFrame / ShellBody` 层级与有效态绑定尚未落地。

**Step 3: Write the minimal implementation**

修改 `ui/app-window.slint`，把壳层重组为明确的 frame/body：

```slint
shell-frame := Rectangle {
    border-radius: 14px;
    border-width: 1px;
    border-color: ThemeTokens.shell-stroke;
    background: ThemeTokens.shell-surface;

    VerticalLayout {
        spacing: 0px;

        titlebar := Titlebar { ... }

        body-host := Rectangle {
            vertical-stretch: 1;
            background: transparent;

            shell-body := HorizontalLayout {
                width: parent.width;
                height: parent.height;
                spacing: 0px;

                sidebar := Sidebar {
                    items: root.sidebar-items;
                    show-assets-sidebar: root.effective-show-assets-sidebar;
                    active-sidebar-destination: root.active-sidebar-destination;
                }

                main-workspace := Rectangle {
                    horizontal-stretch: 1;
                    background: ThemeTokens.terminal-surface;

                    VerticalLayout {
                        spacing: 0px;

                        TabBar {}

                        content-host := Rectangle {
                            vertical-stretch: 1;
                            background: transparent;

                            WelcomeView {}
                        }
                    }
                }

                right-panel := RightPanel {
                    expanded: root.effective-show-right-panel;
                }
            }
        }
    }
}
```

修改 `ui/shell/right-panel.slint`，确保 panel 自身不把 body 高度锁死：

```slint
export component RightPanel inherits Rectangle {
    in property <bool> expanded: false;

    width: root.expanded ? 392px : 0px;
    visible: root.expanded;
    clip: true;
    background: ThemeTokens.panel-tint;
}
```

修改 `ui/welcome/welcome-view.slint`，从绝对定位文本改成声明式布局容器：

```slint
export component WelcomeView inherits Rectangle {
    background: transparent;

    VerticalLayout {
        padding-top: 32px;
        padding-left: 24px;
        padding-right: 24px;
        spacing: 8px;

        Text { text: "Welcome to Mica Term"; font-size: 28px; }
        Text { text: "Command-first SSH and SFTP workspace"; }
        Rectangle { vertical-stretch: 1; background: transparent; }
    }
}
```

**Step 4: Run smoke to verify it passes**

Run: `bash tests/shell_layout_ui_contract_smoke.sh && bash tests/sidebar_ui_contract_smoke.sh`  
Expected: PASS，说明新的壳层层级和有效态绑定已经落地。

**Step 5: Commit**

```bash
git add ui/app-window.slint ui/shell/sidebar.slint ui/shell/right-panel.slint ui/welcome/welcome-view.slint \
  tests/shell_layout_ui_contract_smoke.sh tests/sidebar_ui_contract_smoke.sh
git commit -m "fix: refactor shell frame body layout"
```

## Task 4: 暴露布局诊断属性并补齐几何契约测试

**Files:**
- Modify: `ui/app-window.slint`
- Modify: `ui/shell/sidebar.slint`
- Modify: `ui/shell/right-panel.slint`
- Create: `tests/window_geometry_spec.rs`
- Modify: `tests/window_shell.rs`

**Step 1: Write the failing geometry tests**

创建 `tests/window_geometry_spec.rs`：

```rust
use mica_term::AppWindow;
use mica_term::app::bootstrap::bind_top_status_bar_with_store;
use mica_term::shell::metrics::ShellMetrics;
use slint::{ComponentHandle, PhysicalSize};

#[test]
fn shell_body_height_matches_window_height_minus_titlebar() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    let body_height = app.get_layout_shell_body_height() as u32;
    let titlebar_height = app.get_layout_titlebar_height() as u32;
    assert_eq!(titlebar_height, ShellMetrics::TITLEBAR_HEIGHT);
    assert_eq!(body_height, ShellMetrics::WINDOW_DEFAULT_HEIGHT - ShellMetrics::TITLEBAR_HEIGHT);
}

#[test]
fn larger_window_expands_shell_body_instead_of_leaving_blank_space() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.window().set_size(PhysicalSize::new(1600, 1000));
    app.invoke_shell_layout_invalidated(1600.0, 1000.0);

    assert_eq!(app.get_layout_shell_body_height() as u32, 1000 - ShellMetrics::TITLEBAR_HEIGHT);
}

#[test]
fn collapse_order_matches_design_under_narrow_widths() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);
    app.invoke_toggle_right_panel_requested();

    app.window().set_size(PhysicalSize::new(1335, 900));
    app.invoke_shell_layout_invalidated(1335.0, 900.0);
    assert_eq!(app.get_layout_assets_sidebar_width() as u32, 0);
    assert_eq!(app.get_layout_right_panel_width() as u32, ShellMetrics::RIGHT_PANEL_WIDTH);

    app.window().set_size(PhysicalSize::new(1079, 900));
    app.invoke_shell_layout_invalidated(1079.0, 900.0);
    assert_eq!(app.get_layout_right_panel_width() as u32, 0);
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --test window_geometry_spec -q`  
Expected: FAIL，报错应包括 `get_layout_shell_body_height not found`、`get_layout_assets_sidebar_width not found` 等。

**Step 3: Write the minimal implementation**

修改 `ui/app-window.slint`，暴露稳定的只读布局诊断属性：

```slint
out property <length> layout-titlebar-height: titlebar.height;
out property <length> layout-shell-body-height: body-host.height;
out property <length> layout-main-workspace-width: main-workspace.width;
out property <length> layout-right-panel-width: right-panel.width;
```

修改 `ui/shell/sidebar.slint`，为关键宽度增加输出：

```slint
out property <length> activity-bar-width: activity-bar.width;
out property <length> assets-sidebar-width: root.show-assets-sidebar ? 256px : 0px;
```

然后回到 `ui/app-window.slint`，把 sidebar 的诊断属性上抛：

```slint
out property <length> layout-activity-bar-width: sidebar.activity-bar-width;
out property <length> layout-assets-sidebar-width: sidebar.assets-sidebar-width;
```

如果 `assets-sidebar-width` 需要更准确地绑定实际组件宽度，就在 `ui/shell/sidebar.slint` 给 `AssetsSidebar` 命名后直接绑定其 `width`，不要在根组件重新手算。

**Step 4: Run tests to verify they pass**

Run: `cargo test --test window_geometry_spec -q`  
Expected: PASS，证明默认尺寸、最大化替代场景和窄窗折叠顺序都能被自动验证。

**Step 5: Final verification**

Run:

```bash
cargo fmt --check
cargo test --test shell_layout_policy --test top_status_bar_smoke --test sidebar_navigation_smoke --test window_geometry_spec --test window_shell -q
bash tests/shell_layout_ui_contract_smoke.sh
bash tests/sidebar_ui_contract_smoke.sh
cargo check -q
cargo clippy --all-targets -- -D warnings
```

Expected:

- `cargo fmt --check` 返回 0
- 所有目标测试通过
- 两个 shell smoke 返回 0
- `cargo check` 和 `cargo clippy` 无错误

**Step 6: Commit**

```bash
git add ui/app-window.slint ui/shell/sidebar.slint ui/shell/right-panel.slint \
  tests/window_geometry_spec.rs tests/window_shell.rs
git commit -m "test: add shell geometry contracts"
```


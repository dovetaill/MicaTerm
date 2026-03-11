# Top Status Bar Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 在现有 `Rust + Slint` frameless shell 基座上实现已确认的顶部状态栏方案, 交付五段式标题栏、自绘窗口控制按钮、原生拖拽/双击最大化、设置下拉菜单和右侧侧栏开关联动。

**Architecture:** 顶栏实现分为三层。Slint 负责标题栏结构、视觉和事件发出; Rust `ShellViewModel` 负责顶栏可测试状态; `windowing` 适配层负责窗口命令与 `winit` 原生拖拽桥接。把“可单测的状态/命令”与“需要最终人工 smoke 的原生窗口行为”分离, 避免把平台细节散落到 UI 组件内部。

**Tech Stack:** Rust, Cargo, Slint 1.15.1, `slint::winit_030`, Tokio, `cargo test`, `cargo fmt`, `cargo clippy`

---

**Execution Notes**

- 使用 @superpowers:test-driven-development 执行每个任务, 先写失败测试, 再写最小实现。
- 如果 `Slint` 编译错误、`PopupWindow` 行为或 `winit` 拖拽接入不明确, 立即切换到 @superpowers:systematic-debugging, 不要猜。
- 完成后必须使用 @superpowers:verification-before-completion, 先给出命令输出证据, 再声称功能完成。
- 执行范围仅限顶部状态栏和其状态/窗口桥接, 不改 SSH / SFTP / terminal session 逻辑。
- 当前 `ui/shell/titlebar.slint` 中的 `CommandEntry` 不属于本轮范围, 本计划默认将其从标题栏主结构中移除, 为主拖拽区让位。
- 执行此计划时优先在独立 worktree 中完成, 根目录应基于 `/home/wwwroot/mica-term`。

### Task 1: 扩展 `ShellViewModel` 承载顶栏状态

**Files:**
- Modify: `src/shell/view_model.rs`
- Modify: `tests/shell_view_model.rs`

**Step 1: Write the failing test**

修改 `tests/shell_view_model.rs`, 在现有测试后追加:

```rust
#[test]
fn shell_view_model_tracks_top_status_bar_state() {
    let mut view_model = ShellViewModel::default();

    assert!(view_model.show_welcome);
    assert!(!view_model.show_right_panel);
    assert!(!view_model.show_settings_menu);
    assert!(!view_model.is_window_maximized);
    assert!(view_model.is_window_active);

    view_model.toggle_right_panel();
    assert!(view_model.show_right_panel);

    view_model.toggle_settings_menu();
    assert!(view_model.show_settings_menu);

    view_model.close_settings_menu();
    assert!(!view_model.show_settings_menu);

    view_model.set_window_maximized(true);
    assert!(view_model.is_window_maximized);

    view_model.set_window_active(false);
    assert!(!view_model.is_window_active);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test shell_view_model -q`  
Expected: FAIL with missing fields such as `show_settings_menu` / `is_window_maximized` or missing methods such as `toggle_right_panel`.

**Step 3: Write minimal implementation**

修改 `src/shell/view_model.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShellViewModel {
    pub show_welcome: bool,
    pub show_right_panel: bool,
    pub show_settings_menu: bool,
    pub is_window_maximized: bool,
    pub is_window_active: bool,
}

impl Default for ShellViewModel {
    fn default() -> Self {
        Self {
            show_welcome: true,
            show_right_panel: false,
            show_settings_menu: false,
            is_window_maximized: false,
            is_window_active: true,
        }
    }
}

impl ShellViewModel {
    pub fn toggle_right_panel(&mut self) {
        self.show_right_panel = !self.show_right_panel;
    }

    pub fn toggle_settings_menu(&mut self) {
        self.show_settings_menu = !self.show_settings_menu;
    }

    pub fn close_settings_menu(&mut self) {
        self.show_settings_menu = false;
    }

    pub fn set_window_maximized(&mut self, value: bool) {
        self.is_window_maximized = value;
    }

    pub fn set_window_active(&mut self, value: bool) {
        self.is_window_active = value;
    }
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test --test shell_view_model -q`  
Expected: PASS with `2 passed; 0 failed`.

**Step 5: Commit**

```bash
git add src/shell/view_model.rs tests/shell_view_model.rs
git commit -m "feat: add shell chrome state model"
```

### Task 2: 引入窗口命令适配层与 `winit` 原生拖拽入口

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/app/windowing.rs`
- Modify: `tests/window_shell.rs`

**Step 1: Write the failing test**

修改 `tests/window_shell.rs`, 在现有测试后追加:

```rust
use mica_term::app::windowing::{next_maximize_state, window_command_spec};

#[test]
fn top_status_bar_window_commands_match_the_approved_strategy() {
    let spec = window_command_spec();

    assert!(spec.uses_winit_drag);
    assert!(spec.self_drawn_controls);
    assert!(spec.supports_double_click_maximize);

    assert!(next_maximize_state(false));
    assert!(!next_maximize_state(true));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test window_shell -q`  
Expected: FAIL with unresolved items `window_command_spec` and `next_maximize_state`.

**Step 3: Write minimal implementation**

先修改 `Cargo.toml`, 给 `slint` 增加 `winit` 访问能力:

```toml
slint = { version = "1.15.1", default-features = false, features = ["std", "backend-winit", "renderer-software", "compat-1-2", "unstable-winit-030"] }
```

然后修改 `src/app/windowing.rs`:

```rust
use anyhow::{Result, anyhow};
use slint::Window;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowCommandSpec {
    pub uses_winit_drag: bool,
    pub self_drawn_controls: bool,
    pub supports_double_click_maximize: bool,
}

pub fn window_command_spec() -> WindowCommandSpec {
    WindowCommandSpec {
        uses_winit_drag: true,
        self_drawn_controls: true,
        supports_double_click_maximize: true,
    }
}

pub fn next_maximize_state(is_maximized: bool) -> bool {
    !is_maximized
}

#[derive(Clone)]
pub struct WindowController {
    window: Window,
}

impl WindowController {
    pub fn new(window: Window) -> Self {
        Self { window }
    }

    pub fn minimize(&self) {
        self.window.set_minimized(true);
    }

    pub fn toggle_maximize(&self, current: bool) -> bool {
        let next = next_maximize_state(current);
        self.window.set_maximized(next);
        next
    }

    pub fn close(&self) -> Result<()> {
        self.window.hide().map_err(|err| anyhow!(err.to_string()))
    }

    pub fn drag(&self) -> Result<()> {
        use slint::winit_030::{WinitWindowAccessor, winit};

        let mut drag_result = Ok(());
        self.window.with_winit_window(|window: &winit::window::Window| {
            drag_result = window
                .drag_window()
                .map_err(|err| anyhow!(err.to_string()));
        });
        drag_result
    }
}
```

保留现有 `WindowAppearance` / `MaterialKind` 不动。

**Step 4: Run test to verify it passes**

Run: `cargo test --test window_shell -q`  
Expected: PASS with `3 passed; 0 failed`.

**Step 5: Commit**

```bash
git add Cargo.toml src/app/windowing.rs tests/window_shell.rs
git commit -m "feat: add top status bar window controller"
```

### Task 3: 重建 Slint 顶栏组件为五段式布局

**Files:**
- Create: `ui/components/titlebar-icon-button.slint`
- Create: `ui/components/window-control-button.slint`
- Create: `ui/components/titlebar-menu.slint`
- Modify: `ui/shell/titlebar.slint`
- Modify: `ui/app-window.slint`
- Modify: `ui/shell/right-panel.slint`
- Modify: `src/shell/metrics.rs`
- Create: `tests/titlebar_layout_spec.rs`

**Step 1: Write the failing test**

创建 `tests/titlebar_layout_spec.rs`:

```rust
use mica_term::shell::metrics::ShellMetrics;

#[test]
fn top_status_bar_layout_preserves_drag_and_action_budget() {
    assert_eq!(ShellMetrics::TITLEBAR_HEIGHT, 48);
    assert_eq!(ShellMetrics::TITLEBAR_ACTIONS_WIDTH, 120);
    assert_eq!(ShellMetrics::TITLEBAR_WINDOW_CONTROL_WIDTH, 138);
    assert_eq!(ShellMetrics::TITLEBAR_MIN_DRAG_WIDTH, 96);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test titlebar_layout_spec -q`  
Expected: FAIL with missing associated constants on `ShellMetrics`.

**Step 3: Write minimal implementation**

先扩展 `src/shell/metrics.rs`:

```rust
impl ShellMetrics {
    pub const TITLEBAR_ACTIONS_WIDTH: u32 = 120;
    pub const TITLEBAR_WINDOW_CONTROL_WIDTH: u32 = 138;
    pub const TITLEBAR_MIN_DRAG_WIDTH: u32 = 96;
}
```

创建 `ui/components/titlebar-icon-button.slint`:

```slint
import { ThemeTokens } from "../theme/tokens.slint";

export component TitlebarIconButton inherits Rectangle {
    in property <string> label;
    callback clicked;

    width: 32px;
    height: 32px;
    border-radius: 8px;
    background: touch.pressed ? ThemeTokens.panel-tint : touch.has-hover ? ThemeTokens.shell-surface : transparent;

    Text {
        text: root.label;
        color: ThemeTokens.text-primary;
        horizontal-alignment: center;
        vertical-alignment: center;
    }

    touch := TouchArea {
        clicked => { root.clicked(); }
    }
}
```

创建 `ui/components/window-control-button.slint`:

```slint
import { ThemeTokens } from "../theme/tokens.slint";

export component WindowControlButton inherits Rectangle {
    in property <string> glyph;
    in property <bool> danger: false;
    callback clicked;

    width: 46px;
    height: 32px;
    background: touch.pressed
        ? (root.danger ? #b91c1c : ThemeTokens.panel-tint)
        : touch.has-hover
            ? (root.danger ? #dc2626 : ThemeTokens.shell-surface)
            : transparent;

    Text {
        text: root.glyph;
        color: ThemeTokens.text-primary;
        horizontal-alignment: center;
        vertical-alignment: center;
    }

    touch := TouchArea {
        clicked => { root.clicked(); }
    }
}
```

创建 `ui/components/titlebar-menu.slint`:

```slint
import { ThemeTokens } from "../theme/tokens.slint";

export component TitlebarMenu inherits Rectangle {
    callback settings-selected;
    callback appearance-selected;
    callback close-requested;

    width: 196px;
    height: 132px;
    border-radius: 12px;
    border-width: 1px;
    border-color: ThemeTokens.shell-stroke;
    background: ThemeTokens.panel-tint;
}
```

然后把 `ui/shell/titlebar.slint` 重构为五段式:

- 左侧品牌区: icon / app name / global menu button
- 主拖拽区: 空白 `Rectangle + TouchArea`
- 右侧动作区: 状态位占位 / 右侧面板按钮 / 设置按钮
- 最小拖拽安全区: 固定最小宽度空白区
- 窗口控制区: `minimize / maximize / close`

关键要求:

- 删除当前内嵌的 `CommandEntry`
- `Titlebar` 对外暴露这些 callback:
  - `drag-requested`
  - `drag-double-clicked`
  - `toggle-right-panel-requested`
  - `toggle-settings-menu-requested`
  - `close-settings-menu-requested`
  - `minimize-requested`
  - `maximize-toggle-requested`
  - `close-requested`
- `Titlebar` 对外暴露这些 property:
  - `show-right-panel`
  - `show-settings-menu`
  - `is-window-maximized`
  - `is-window-active`

同时修改 `ui/app-window.slint`:

- 为 `AppWindow` 增加同名 `in-out property`
- `RightPanel` 根据 `show-right-panel` 控制 `visible`
- `Titlebar` 与 `AppWindow` 根属性双向对齐

**Step 4: Run test to verify it passes**

Run: `cargo test --test titlebar_layout_spec -q`  
Expected: PASS with `1 passed; 0 failed`.

Then run a compile-focused check: `cargo test --test bootstrap_smoke -q`  
Expected: PASS, confirming the new Slint component tree still compiles.

**Step 5: Commit**

```bash
git add ui/components/titlebar-icon-button.slint ui/components/window-control-button.slint ui/components/titlebar-menu.slint ui/shell/titlebar.slint ui/app-window.slint ui/shell/right-panel.slint src/shell/metrics.rs tests/titlebar_layout_spec.rs
git commit -m "feat: build five-zone top status bar layout"
```

### Task 4: 绑定 `AppWindow` 顶栏回调到 Rust 状态与窗口命令

**Files:**
- Modify: `src/app/bootstrap.rs`
- Modify: `ui/app-window.slint`
- Modify: `ui/shell/titlebar.slint`
- Create: `tests/top_status_bar_smoke.rs`

**Step 1: Write the failing test**

创建 `tests/top_status_bar_smoke.rs`:

```rust
use mica_term::AppWindow;

#[test]
fn app_window_exposes_top_status_bar_state_contract() {
    let app = AppWindow::new().unwrap();

    assert!(!app.get_show_right_panel());
    assert!(!app.get_show_settings_menu());
    assert!(!app.get_is_window_maximized());
    assert!(app.get_is_window_active());
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test top_status_bar_smoke -q`  
Expected: FAIL with missing generated getters such as `get_show_settings_menu`.

**Step 3: Write minimal implementation**

修改 `ui/app-window.slint`, 在根组件上增加:

```slint
in-out property <bool> show-right-panel: false;
in-out property <bool> show-settings-menu: false;
in-out property <bool> is-window-maximized: false;
in-out property <bool> is-window-active: true;

callback drag-requested();
callback drag-double-clicked();
callback minimize-requested();
callback maximize-toggle-requested();
callback close-requested();
callback toggle-right-panel-requested();
callback toggle-settings-menu-requested();
callback close-settings-menu-requested();
```

将这些 callback 透传到 `Titlebar`:

```slint
Titlebar {
    show-right-panel: root.show-right-panel;
    show-settings-menu: root.show-settings-menu;
    is-window-maximized: root.is-window-maximized;
    is-window-active: root.is-window-active;

    drag-requested => { root.drag-requested(); }
    drag-double-clicked => { root.drag-double-clicked(); }
    minimize-requested => { root.minimize-requested(); }
    maximize-toggle-requested => { root.maximize-toggle-requested(); }
    close-requested => { root.close-requested(); }
    toggle-right-panel-requested => { root.toggle-right-panel-requested(); }
    toggle-settings-menu-requested => { root.toggle-settings-menu-requested(); }
    close-settings-menu-requested => { root.close-settings-menu-requested(); }
}
```

修改 `src/app/bootstrap.rs`, 用 `Rc<RefCell<ShellViewModel>>` 统一托管状态, 用 `WindowController` 执行窗口命令:

```rust
use std::cell::RefCell;
use std::rc::Rc;

use anyhow::Result;
use slint::ComponentHandle;

use crate::AppWindow;
use crate::app::windowing::WindowController;
use crate::shell::view_model::ShellViewModel;

pub fn run() -> Result<()> {
    let window = AppWindow::new()?;
    let view_model = Rc::new(RefCell::new(ShellViewModel::default()));
    let controller = WindowController::new(window.window());

    {
        let state = view_model.borrow();
        window.set_show_right_panel(state.show_right_panel);
        window.set_show_settings_menu(state.show_settings_menu);
        window.set_is_window_maximized(state.is_window_maximized);
        window.set_is_window_active(state.is_window_active);
    }

    let state = view_model.clone();
    let handle = window.as_weak();
    window.on_toggle_right_panel_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.toggle_right_panel();
        window.set_show_right_panel(state.show_right_panel);
    });

    let state = view_model.clone();
    let handle = window.as_weak();
    window.on_toggle_settings_menu_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.toggle_settings_menu();
        window.set_show_settings_menu(state.show_settings_menu);
    });

    let state = view_model.clone();
    let handle = window.as_weak();
    window.on_close_settings_menu_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.close_settings_menu();
        window.set_show_settings_menu(state.show_settings_menu);
    });

    let handle = window.as_weak();
    window.on_minimize_requested(move || {
        controller.minimize();
        let _ = handle;
    });

    let state = view_model.clone();
    let handle = window.as_weak();
    window.on_maximize_toggle_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let next = controller.toggle_maximize(state.is_window_maximized);
        state.set_window_maximized(next);
        window.set_is_window_maximized(next);
    });

    window.on_close_requested(move || {
        let _ = controller.close();
    });

    window.on_drag_requested(move || {
        let _ = controller.drag();
    });

    let state = view_model.clone();
    let handle = window.as_weak();
    window.on_drag_double_clicked(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let next = controller.toggle_maximize(state.is_window_maximized);
        state.set_window_maximized(next);
        window.set_is_window_maximized(next);
    });

    window.run()?;
    Ok(())
}
```

如果 `Window::on_close_requested` 与当前 `AppWindow` 事件命名冲突, 保持策略不变, 只调整具体函数名到 Slint 生成 API 的真实名称。

**Step 4: Run test to verify it passes**

Run: `cargo test --test top_status_bar_smoke -q`  
Expected: PASS with `1 passed; 0 failed`.

Then run: `cargo test --test shell_view_model --test window_shell --test titlebar_layout_spec --test top_status_bar_smoke -q`  
Expected: PASS with all targeted top-status-bar tests green.

**Step 5: Commit**

```bash
git add src/app/bootstrap.rs ui/app-window.slint ui/shell/titlebar.slint tests/top_status_bar_smoke.rs
git commit -m "feat: wire top status bar callbacks and state"
```

### Task 5: 完整验证与收尾

**Files:**
- Modify: `docs/plans/2026-03-10-top-status-bar-design.md` (only if implementation decisions diverged)
- Create: `docs/plans/2026-03-10-top-status-bar-verification.md` (optional, if you want a permanent verification record)

**Step 1: Run formatting and static checks**

Run: `cargo fmt --all --check`  
Expected: PASS with no diff.

Run: `cargo clippy --all-targets --all-features -- -D warnings`  
Expected: PASS with no warnings.

**Step 2: Run the full automated test suite**

Run: `cargo test -q`  
Expected: PASS with all existing and newly added tests green.

**Step 3: Run desktop smoke verification**

On a desktop session, run: `cargo run`  
Expected:

- 窗口使用新的五段式顶栏启动
- 中间拖拽区可以拖动窗口
- 双击拖拽区可最大化 / 还原
- `Minimize / Maximize / Close` 三个按钮全部工作
- 设置按钮点击后出现下拉菜单
- 点击菜单外部区域可关闭菜单
- 按 `Esc` 可关闭菜单
- 右侧侧栏按钮可以展开 / 收起 `RightPanel`

**Step 4: Run Windows-target acceptance smoke**

在 Windows 11 环境执行最终验收:

- 拖动窗口到屏幕边缘时行为自然
- 双击最大化/还原手感符合系统习惯
- `Close` 按钮 hover / pressed 态明显区别于另外两个按钮
- 窄窗口时仍保留最小拖拽安全区

如需要打包验证, 再运行: `./build-win-x64.sh`

**Step 5: Commit**

```bash
git add docs/plans/2026-03-10-top-status-bar-design.md docs/plans/2026-03-10-top-status-bar-verification.md
git commit -m "docs: capture top status bar verification"
```

## 出现偏差时的处理原则

- 如果 `PopupWindow` 的状态驱动方式与计划中的 callback 契约稍有差异, 允许调整具体回调命名, 但必须保留“Slint 承载弹层 + Rust 同步状态”的总体路线。
- 如果 `WindowController::close()` 需要使用不同于 `window.hide()` 的关闭方式, 允许替换底层实现, 但禁止把 close 逻辑直接塞回 `titlebar.slint`。
- 如果 `slint::winit_030` 的 API 与文档示例存在细微差异, 允许在适配层内调整导入和调用写法, 但禁止让 `winit` 类型泄漏到 `ShellViewModel` 或 Slint 组件接口。

Plan complete and saved to `docs/plans/2026-03-10-top-status-bar-implementation-plan.md`.

# Top Status Bar Style Bugfix2 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 在不改变现有 `Rust + Slint` frameless shell 基座的前提下，完成 `top status bar style bugfix2`，实现左侧固定 `Navigation` 入口、顶部专用 `Navigation-Led Logotype`、右侧 `theme -> panel-toggle -> divider -> pin -> min -> maximize/restore -> close` 顺序、`theme/pin` 可持久化状态、以及稳定的 shared tooltip 体验。

**Architecture:** 继续沿用现有分层：`AppWindow` 承载窗口属性与回调桥接，`Titlebar` 负责顶栏结构与交互，`TitlebarIconButton` / `WindowControlButton` / `TitlebarTooltip` 承载通用视觉，`ShellViewModel` 持有可测试 UI 状态，新增 `UiPreferences` 负责最轻量本地持久化。`theme` 通过 `ThemeTokens.dark-mode` 闭环生效，`pin` 优先走 Slint `Window` 的 `always-on-top` 属性；实现阶段不改 SSH / SFTP / terminal 主逻辑，不重写窗口系统。

**Tech Stack:** Rust, Cargo, Slint 1.15.1, `PopupWindow`, `serde`, `serde_json`, `directories`, shell smoke scripts, `cargo fmt`, `cargo check`, `cargo test`

---

## Execution Notes

- 设计输入文档固定为 `docs/plans/2026-03-11-top-status-bar-style-bugfix2-design.md`，实现时不得偏离已确认决策。
- 使用 `@superpowers:test-driven-development` 执行每个任务：先写失败测试或失败 smoke，再写最小实现，再跑通过。
- 使用 `@superpowers:verification-before-completion` 做最终验证，不允许“先宣称完成、后补验证”。
- `theme` 与 `pin` 的持久化统一使用一个轻量 `ui-preferences.json`，放在标准 app config 目录，不引入数据库。
- `TitlebarMenu` 本轮不重做信息架构；只保留现有菜单组件和左侧弹出锚点，不扩张到设置页设计。
- `Workspace` 与 `SSH` 必须彻底从顶栏源码中删除，不能改名保留。

## Task 1: 新增 UI Preferences 与顶栏状态模型

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/app/mod.rs`
- Create: `src/app/ui_preferences.rs`
- Modify: `src/theme/spec.rs`
- Modify: `src/shell/view_model.rs`
- Create: `tests/ui_preferences.rs`
- Modify: `tests/shell_view_model.rs`

**Step 1: Write the failing tests**

创建 `tests/ui_preferences.rs`，定义最小契约：

```rust
use mica_term::app::ui_preferences::{UiPreferences, UiPreferencesStore};
use mica_term::theme::ThemeMode;

#[test]
fn ui_preferences_default_to_dark_and_not_pinned() {
    let prefs = UiPreferences::default();
    assert_eq!(prefs.theme_mode, ThemeMode::Dark);
    assert!(!prefs.always_on_top);
}

#[test]
fn ui_preferences_roundtrip_theme_and_pin_state() {
    let temp_path = std::env::temp_dir()
        .join("mica-term")
        .join("tests")
        .join("ui-preferences-roundtrip.json");

    let store = UiPreferencesStore::new(temp_path.clone());
    let prefs = UiPreferences {
        theme_mode: ThemeMode::Light,
        always_on_top: true,
    };

    store.save(&prefs).unwrap();
    let loaded = store.load_or_default().unwrap();

    assert_eq!(loaded, prefs);
    let _ = std::fs::remove_file(temp_path);
}
```

修改 `tests/shell_view_model.rs`，把顶栏状态测试升级为：

```rust
use mica_term::theme::ThemeMode;

#[test]
fn shell_view_model_tracks_titlebar_theme_and_pin_state() {
    let mut view_model = ShellViewModel::default();

    assert_eq!(view_model.theme_mode, ThemeMode::Dark);
    assert!(!view_model.is_always_on_top);

    view_model.toggle_theme_mode();
    assert_eq!(view_model.theme_mode, ThemeMode::Light);

    view_model.toggle_always_on_top();
    assert!(view_model.is_always_on_top);
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --test ui_preferences --test shell_view_model -q`  
Expected: FAIL with missing module/items such as `ui_preferences`, `theme_mode`, `is_always_on_top`, `toggle_theme_mode`, or `toggle_always_on_top`.

**Step 3: Write minimal implementation**

在 `Cargo.toml` 增加依赖：

```toml
directories = "5"
serde_json = "1"
```

在 `src/theme/spec.rs` 为 `ThemeMode` 增加序列化与切换能力：

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeMode {
    Dark,
    Light,
}

impl ThemeMode {
    pub fn toggled(self) -> Self {
        match self {
            Self::Dark => Self::Light,
            Self::Light => Self::Dark,
        }
    }
}
```

创建 `src/app/ui_preferences.rs`：

```rust
use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

use crate::theme::ThemeMode;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UiPreferences {
    pub theme_mode: ThemeMode,
    pub always_on_top: bool,
}

impl Default for UiPreferences {
    fn default() -> Self {
        Self {
            theme_mode: ThemeMode::Dark,
            always_on_top: false,
        }
    }
}

pub struct UiPreferencesStore {
    path: PathBuf,
}

impl UiPreferencesStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn for_app() -> Result<Self> {
        let dirs = ProjectDirs::from("dev", "MicaTerm", "MicaTerm")
            .context("project directories are unavailable")?;
        Ok(Self::new(dirs.config_dir().join("ui-preferences.json")))
    }

    pub fn load_or_default(&self) -> Result<UiPreferences> {
        if !self.path.exists() {
            return Ok(UiPreferences::default());
        }
        let content = fs::read_to_string(&self.path)?;
        Ok(serde_json::from_str(&content)?)
    }

    pub fn save(&self, prefs: &UiPreferences) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(prefs)?;
        fs::write(&self.path, content)?;
        Ok(())
    }
}
```

在 `src/shell/view_model.rs` 增加：

```rust
use crate::theme::ThemeMode;

pub struct ShellViewModel {
    pub show_welcome: bool,
    pub show_right_panel: bool,
    pub show_global_menu: bool,
    pub is_window_maximized: bool,
    pub is_window_active: bool,
    pub theme_mode: ThemeMode,
    pub is_always_on_top: bool,
}

impl ShellViewModel {
    pub fn toggle_theme_mode(&mut self) {
        self.theme_mode = self.theme_mode.toggled();
    }

    pub fn toggle_always_on_top(&mut self) {
        self.is_always_on_top = !self.is_always_on_top;
    }
}
```

补一个状态转换，避免 bootstrap 手工拼装偏好结构：

```rust
impl From<&ShellViewModel> for UiPreferences {
    fn from(value: &ShellViewModel) -> Self {
        Self {
            theme_mode: value.theme_mode,
            always_on_top: value.is_always_on_top,
        }
    }
}
```

并在 `src/app/mod.rs` 导出 `ui_preferences` 模块。

**Step 4: Run tests to verify they pass**

Run: `cargo test --test ui_preferences --test shell_view_model -q`  
Expected: PASS, 且顶栏新增状态与轻量持久化模型已经成立。

**Step 5: Commit**

```bash
git add Cargo.toml src/app/mod.rs src/app/ui_preferences.rs src/theme/spec.rs src/shell/view_model.rs tests/ui_preferences.rs tests/shell_view_model.rs
git commit -m "feat: add persistent titlebar ui preferences"
```

## Task 2: 把 Theme / Pin 状态桥接到 AppWindow 与 Bootstrap

**Files:**
- Modify: `ui/app-window.slint`
- Modify: `src/app/bootstrap.rs`
- Modify: `src/app/windowing.rs`
- Modify: `tests/top_status_bar_smoke.rs`
- Modify: `tests/window_shell.rs`

**Step 1: Write the failing tests**

修改 `tests/top_status_bar_smoke.rs`，增加主题与置顶状态回调契约：

```rust
#[test]
fn bootstrap_binds_theme_and_pin_callbacks_to_window_state() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();

    app.set_dark_mode(false);
    app.set_is_window_always_on_top(true);

    bind_top_status_bar(&app);

    assert!(app.get_dark_mode());
    assert!(!app.get_is_window_always_on_top());

    app.invoke_toggle_theme_mode_requested();
    assert!(!app.get_dark_mode());

    app.invoke_toggle_window_always_on_top_requested();
    assert!(app.get_is_window_always_on_top());
}
```

修改 `tests/window_shell.rs`，增加窗口能力策略断言：

```rust
#[test]
fn top_status_bar_window_commands_support_topmost_state() {
    let spec = window_command_spec();
    assert!(spec.supports_always_on_top);
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --test top_status_bar_smoke --test window_shell -q`  
Expected: FAIL with missing properties/callbacks such as `is_window_always_on_top`, `toggle_theme_mode_requested`, `toggle_window_always_on_top_requested`, or `supports_always_on_top`.

**Step 3: Write minimal implementation**

在 `ui/app-window.slint` 中新增状态与回调，并完成 Window / ThemeTokens 绑定：

```slint
export component AppWindow inherits Window {
    in-out property <bool> dark-mode: true;
    in-out property <bool> is-window-always-on-top: false;

    callback toggle-theme-mode-requested();
    callback toggle-window-always-on-top-requested();

    always-on-top: root.is-window-always-on-top;
    ThemeTokens.dark-mode: root.dark-mode;
}
```

在 `src/app/windowing.rs` 中扩展 `WindowCommandSpec`：

```rust
pub struct WindowCommandSpec {
    pub uses_winit_drag: bool,
    pub self_drawn_controls: bool,
    pub supports_double_click_maximize: bool,
    pub supports_always_on_top: bool,
}
```

在 `src/app/bootstrap.rs` 中：

- 创建 `UiPreferencesStore::for_app()`
- 启动时加载 `UiPreferences`
- 用其初始化 `ShellViewModel`
- 增加一个私有 helper，例如 `bind_top_status_bar_with_store(window, store)`，让测试可以注入临时 store，而 `bind_top_status_bar()` 继续走真实 app store
- 在 `sync_top_status_bar_state()` 中同步：
  - `window.set_dark_mode(state.theme_mode == ThemeMode::Dark);`
  - `window.set_is_window_always_on_top(state.is_always_on_top);`
- 新增两个回调：

```rust
window.on_toggle_theme_mode_requested(move || {
    state.toggle_theme_mode();
    window.set_dark_mode(state.theme_mode == ThemeMode::Dark);
    let _ = store.save(&UiPreferences::from(&*state));
});

window.on_toggle_window_always_on_top_requested(move || {
    state.toggle_always_on_top();
    window.set_is_window_always_on_top(state.is_always_on_top);
    let _ = store.save(&UiPreferences::from(&*state));
});
```

要求：

- 读取失败时回退默认值，不阻塞启动
- 保存失败时记录错误但不阻塞交互
- `bind_top_status_bar()` 保持外部签名不变，必要时内部调用私有 helper

**Step 4: Run tests to verify they pass**

Run: `cargo test --test top_status_bar_smoke --test window_shell -q`  
Expected: PASS, 并且 `theme/pin` 状态已经能从 bootstrap 驱动到窗口属性。

**Step 5: Commit**

```bash
git add ui/app-window.slint src/app/bootstrap.rs src/app/windowing.rs tests/top_status_bar_smoke.rs tests/window_shell.rs
git commit -m "feat: bridge titlebar theme and pin state to window"
```

## Task 3: 新增 Header Logotype 与缺失的 Fluent 图标资产

**Files:**
- Create: `assets/icons/mica-term-header-logotype.svg`
- Create: `assets/icons/fluent/navigation-24-regular.svg`
- Create: `assets/icons/fluent/dark-theme-20-regular.svg`
- Create: `assets/icons/fluent/weather-sunny-20-regular.svg`
- Create: `assets/icons/fluent/pin-20-regular.svg`
- Create: `assets/icons/fluent/pin-off-20-regular.svg`
- Modify: `tests/fluent_titlebar_assets_smoke.sh`
- Modify: `tests/icon_svg_assets_smoke.sh`

**Step 1: Write the failing asset smoke**

扩展 `tests/fluent_titlebar_assets_smoke.sh`：

```bash
for file in \
  assets/icons/fluent/navigation-24-regular.svg \
  assets/icons/fluent/dark-theme-20-regular.svg \
  assets/icons/fluent/weather-sunny-20-regular.svg \
  assets/icons/fluent/pin-20-regular.svg \
  assets/icons/fluent/pin-off-20-regular.svg
do
  [[ -f "$ROOT_DIR/$file" ]] || {
    echo "missing $file" >&2
    exit 1
  }
done
```

扩展 `tests/icon_svg_assets_smoke.sh`：

```bash
check_file "$ROOT_DIR/assets/icons/mica-term-header-logotype.svg" 'viewBox='
grep -F 'currentColor' "$ROOT_DIR/assets/icons/mica-term-header-logotype.svg" >/dev/null
```

**Step 2: Run smoke to verify it fails**

Run: `bash tests/fluent_titlebar_assets_smoke.sh`  
Expected: FAIL with `missing assets/icons/fluent/...`

Run: `bash tests/icon_svg_assets_smoke.sh`  
Expected: FAIL with missing `mica-term-header-logotype.svg`.

**Step 3: Write minimal implementation**

约束如下：

- `mica-term-header-logotype.svg` 是顶部专用横向字标，不复用旧 `M` 徽标
- 字标使用 `currentColor`，便于顶栏按主题统一染色
- 新字标宽高比适合 `48px` 顶栏，不引入第二强调色
- Fluent 资产统一 vendoring 到本地 `assets/icons/fluent`
- `theme` 用 `dark-theme` / `weather-sunny`
- `pin` 用 `pin` / `pin-off`
- 左侧 `Navigation` 使用 `24 regular`

最小 SVG 结构示例：

```svg
<svg viewBox="0 0 420 96" fill="none" xmlns="http://www.w3.org/2000/svg">
  <path d="..." fill="currentColor"/>
</svg>
```

**Step 4: Run smoke to verify it passes**

Run: `bash tests/fluent_titlebar_assets_smoke.sh`  
Expected: PASS

Run: `bash tests/icon_svg_assets_smoke.sh`  
Expected: PASS

**Step 5: Commit**

```bash
git add assets/icons/mica-term-header-logotype.svg assets/icons/fluent tests/fluent_titlebar_assets_smoke.sh tests/icon_svg_assets_smoke.sh
git commit -m "feat: add titlebar logotype and utility icon assets"
```

## Task 4: 扩展按钮组件尺寸与标题栏布局度量

**Files:**
- Modify: `src/shell/metrics.rs`
- Modify: `tests/titlebar_layout_spec.rs`
- Modify: `ui/components/titlebar-icon-button.slint`
- Modify: `ui/components/window-control-button.slint`

**Step 1: Write the failing layout test**

修改 `tests/titlebar_layout_spec.rs`，把标题栏预算升级为 bugfix2 目标：

```rust
#[test]
fn top_status_bar_layout_matches_bugfix2_budget() {
    assert_eq!(ShellMetrics::TITLEBAR_HEIGHT, 48);
    assert_eq!(ShellMetrics::TITLEBAR_NAV_WIDTH, 44);
    assert_eq!(ShellMetrics::TITLEBAR_BRAND_WIDTH, 188);
    assert_eq!(ShellMetrics::TITLEBAR_TOOL_BUTTON_SIZE, 36);
    assert_eq!(ShellMetrics::TITLEBAR_TOOL_ICON_SIZE, 20);
    assert!(ShellMetrics::TITLEBAR_UTILITY_WIDTH >= 136);
    assert_eq!(ShellMetrics::TITLEBAR_WINDOW_CONTROL_WIDTH, 138);
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --test titlebar_layout_spec -q`  
Expected: FAIL with missing constants such as `TITLEBAR_NAV_WIDTH`, `TITLEBAR_TOOL_BUTTON_SIZE`, or `TITLEBAR_TOOL_ICON_SIZE`.

**Step 3: Write minimal implementation**

在 `src/shell/metrics.rs` 增加新度量：

```rust
pub struct ShellMetrics;

impl ShellMetrics {
    pub const TITLEBAR_HEIGHT: u32 = 48;
    pub const TITLEBAR_NAV_WIDTH: u32 = 44;
    pub const TITLEBAR_BRAND_WIDTH: u32 = 188;
    pub const TITLEBAR_UTILITY_WIDTH: u32 = 136;
    pub const TITLEBAR_WINDOW_CONTROL_WIDTH: u32 = 138;
    pub const TITLEBAR_MIN_DRAG_WIDTH: u32 = 96;
    pub const TITLEBAR_TOOL_BUTTON_SIZE: u32 = 36;
    pub const TITLEBAR_TOOL_ICON_SIZE: u32 = 20;
}
```

同步扩展组件：

`ui/components/titlebar-icon-button.slint`

```slint
width: 36px;
height: 36px;

Image {
    width: 20px;
    height: 20px;
}
```

`ui/components/window-control-button.slint`

```slint
height: 36px;

Image {
    width: 20px;
    height: 20px;
}
```

要求：

- 保留 shared tooltip 回调接口
- 保留 `danger` 状态
- 保留 `active-icon-source` 的回退逻辑
- 不新增不必要的新组件

**Step 4: Run tests to verify they pass**

Run: `cargo test --test titlebar_layout_spec -q`  
Expected: PASS

**Step 5: Commit**

```bash
git add src/shell/metrics.rs tests/titlebar_layout_spec.rs ui/components/titlebar-icon-button.slint ui/components/window-control-button.slint
git commit -m "refactor: enlarge titlebar button metrics for bugfix2"
```

## Task 5: 重构 Titlebar 到最终布局与交互顺序

**Files:**
- Modify: `ui/shell/titlebar.slint`
- Modify: `tests/top_status_bar_ui_contract_smoke.sh`
- Modify: `tests/top_status_bar_smoke.rs`

**Step 1: Write the failing UI contract smoke**

把 `tests/top_status_bar_ui_contract_smoke.sh` 升级为 bugfix2 源码契约：

```bash
grep -F 'nav-button := TitlebarIconButton' "$TITLEBAR" >/dev/null
grep -F 'theme-button := TitlebarIconButton' "$TITLEBAR" >/dev/null
grep -F 'panel-toggle-button := TitlebarIconButton' "$TITLEBAR" >/dev/null
grep -F 'pin-button := TitlebarIconButton' "$TITLEBAR" >/dev/null
grep -F 'brand-logotype := Image' "$TITLEBAR" >/dev/null
grep -F 'navigation-24-regular.svg' "$TITLEBAR" >/dev/null
grep -F 'mica-term-header-logotype.svg' "$TITLEBAR" >/dev/null
grep -F 'dark-theme-20-regular.svg' "$TITLEBAR" >/dev/null
grep -F 'weather-sunny-20-regular.svg' "$TITLEBAR" >/dev/null
grep -F 'pin-20-regular.svg' "$TITLEBAR" >/dev/null
grep -F 'pin-off-20-regular.svg' "$TITLEBAR" >/dev/null
grep -F 'divider-line := Rectangle' "$TITLEBAR" >/dev/null
grep -F 'tooltip-text: "Switch to dark mode"' "$TITLEBAR" >/dev/null
grep -F 'tooltip-text: "Switch to light mode"' "$TITLEBAR" >/dev/null
grep -F 'tooltip-text: "Pin window on top"' "$TITLEBAR" >/dev/null
grep -F 'tooltip-text: "Unpin window from top"' "$TITLEBAR" >/dev/null
! grep -F 'text: "Workspace"' "$TITLEBAR" >/dev/null
! grep -F 'text: "SSH"' "$TITLEBAR" >/dev/null
```

同时修改 `tests/top_status_bar_smoke.rs`，加入新回调断言：

```rust
app.invoke_toggle_theme_mode_requested();
assert!(!app.get_dark_mode());

app.invoke_toggle_window_always_on_top_requested();
assert!(app.get_is_window_always_on_top());
```

**Step 2: Run smoke/tests to verify they fail**

Run: `bash tests/top_status_bar_ui_contract_smoke.sh`  
Expected: FAIL because current `titlebar.slint` 仍然包含 `Workspace` / `SSH`，且不存在 `nav-button/theme-button/pin-button`。

Run: `cargo test --test top_status_bar_smoke -q`  
Expected: FAIL until new titlebar callback wiring is complete.

**Step 3: Write minimal implementation**

在 `ui/shell/titlebar.slint` 中完成最终布局：

```slint
export component Titlebar inherits Rectangle {
    in property <bool> dark-mode: true;
    in property <bool> is-window-always-on-top: false;

    callback toggle-theme-mode-requested;
    callback toggle-window-always-on-top-requested;

    content := HorizontalLayout {
        nav-zone := Rectangle {
            width: 44px;
            nav-button := TitlebarIconButton {
                icon-source: root.navigation-icon;
                active: root.show-global-menu;
                tooltip-text: "Open menu";
            }
        }

        brand-zone := Rectangle {
            width: 188px;
            brand-logotype := Image {
                source: @image-url("../../assets/icons/mica-term-header-logotype.svg");
            }
        }

        drag-zone := Rectangle { }

        utility-zone := Rectangle {
            theme-button := TitlebarIconButton {
                icon-source: root.dark-mode ? root.dark-theme-icon : root.weather-sunny-icon;
                tooltip-text: root.dark-mode ? "Switch to light mode" : "Switch to dark mode";
            }

            panel-toggle-button := TitlebarIconButton { ... }

            divider-line := Rectangle {
                width: 1px;
                background: ThemeTokens.shell-stroke;
            }

            pin-button := TitlebarIconButton {
                icon-source: root.is-window-always-on-top ? root.pin-icon : root.pin-off-icon;
                tooltip-text: root.is-window-always-on-top ? "Unpin window from top" : "Pin window on top";
            }
        }

        window-controls := Rectangle { ... }
    }
}
```

明确要求：

- 左侧 `Navigation` 按钮必须固定在最左
- 顶栏品牌必须改用 `Image + mica-term-header-logotype.svg`
- 删除 `Workspace`
- 删除 `SSH`
- 保留 shared tooltip 架构，继续使用单一 `TitlebarTooltip`
- global menu 锚点从 `nav-button.absolute-position` 计算
- `theme` / `pin` 使用 `TitlebarIconButton`
- `panel-toggle` 保留现有 active/filled 语义
- `divider` 使用自绘 `Rectangle`，不能使用 SVG divider 图标
- `TitlebarMenu` 内容本轮不做重构

在 `ui/app-window.slint` 中把新增属性与回调继续透传到 `Titlebar`。

**Step 4: Run verification for this task**

Run: `bash tests/top_status_bar_ui_contract_smoke.sh`  
Expected: PASS

Run: `cargo test --test top_status_bar_smoke -q`  
Expected: PASS

**Step 5: Commit**

```bash
git add ui/shell/titlebar.slint ui/app-window.slint tests/top_status_bar_ui_contract_smoke.sh tests/top_status_bar_smoke.rs
git commit -m "feat: rebuild titlebar layout for bugfix2"
```

## Task 6: 最终验证并更新验证记录

**Files:**
- Modify: `verification.md`

**Step 1: Format the code**

Run: `cargo fmt --all`  
Expected: command exits successfully with no formatting errors.

**Step 2: Run compile verification**

Run: `cargo check`  
Expected: PASS

**Step 3: Run tests and smoke scripts**

Run: `cargo test -q`  
Expected: PASS

Run: `bash tests/fluent_titlebar_assets_smoke.sh`  
Expected: PASS

Run: `bash tests/icon_svg_assets_smoke.sh`  
Expected: PASS

Run: `bash tests/top_status_bar_ui_contract_smoke.sh`  
Expected: PASS

**Step 4: Update verification evidence**

把以下内容写入 `verification.md`：

- 本轮对应设计文档与实现计划路径
- 执行过的命令清单
- 自动化结果
- Windows 11 GUI 手工检查清单：
  - `Navigation` 固定最左
  - 顶栏品牌为新 logotype
  - `Workspace` / `SSH` 不再出现
  - `theme -> panel-toggle -> divider -> pin -> min -> maximize/restore -> close` 顺序正确
  - `theme` 点击后深浅主题立即切换且重启后保留
  - `pin` 点击后窗口置顶状态立即生效且重启后保留
  - 所有按钮 hover 有 tooltip
  - `maximize / restore` 图标在状态切换时正确变化

**Step 5: Commit**

```bash
git add verification.md
git commit -m "docs: capture bugfix2 verification evidence"
```

## Expected End State

- 左侧为固定 `Navigation` 入口 + 顶部专用 `Navigation-Led Logotype`
- 中间是纯拖拽区，不再显示 `Workspace`
- 右侧严格为 `theme -> panel-toggle -> divider -> pin -> min -> maximize/restore -> close`
- 顶栏彻底删除 `SSH`
- `theme` 与 `pin` 均为可持久化状态
- `ThemeTokens.dark-mode` 真正参与 UI 主题切换
- `always-on-top` 在 Windows 11 上真实生效
- 所有图标按钮共享一个短延迟 tooltip popup

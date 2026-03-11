# Theme Toggle Window Appearance Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 在保留当前 `Rust + Slint` frameless/transparent shell 结构的前提下，修复 `Light / Dark` 切换时窗口超出屏幕区域出现不完全切换的问题，并把主题切换升级为 `Slint 内容层 + 原生窗口外壳层` 的双层同步。

**Architecture:** 保持现有 `AppWindow -> bootstrap -> WindowController` 主骨架不变，新增一个很薄的 `PlatformWindowEffects` 抽象层承接“原生窗口 theme / backdrop / redraw”职责。`ThemeMode` 继续作为唯一主题源，`bootstrap` 在初始化和主题切换时同时更新 Slint token 与原生窗口外观；Windows 首先通过 `winit::Window::set_theme(...) + window_vibrancy::apply_tabbed(...) + request_redraw()` 落地真实实现，其他平台先提供 `no-op` 降级。

**Tech Stack:** Rust, Cargo, Slint 1.15.1, `unstable-winit-030`, `window-vibrancy 0.7.1`, `serde`, shell smoke scripts, `cargo fmt`, `cargo check`, `cargo test`, `cargo clippy`

---

## Execution Notes

- 设计输入文档固定为 `docs/plans/2026-03-11-theme-toggle-window-appearance-design.md`，实现时不得偏离已确认决策。
- 使用 `@superpowers:test-driven-development` 执行每个任务：先写失败测试或失败 smoke，再写最小实现，再跑通过。
- 如果出现“透明窗口 + 原生 backdrop”相关的异常行为，不允许猜测，必须切换到 `@superpowers:systematic-debugging`。
- 在完成所有任务前，不要顺手改动 `Titlebar` 信息架构、Tooltip 方案或 Welcome 界面样式；本轮只处理主题切换与窗口外壳同步。
- 计划默认在从 `/home/wwwroot/mica-term` 派生的独立 worktree 中执行；如果继续在当前工作区执行，也必须限制改动范围只覆盖本计划列出的文件。

### Task 1: 建立原生窗口外观请求模型与平台抽象

**Files:**
- Create: `src/app/window_effects.rs`
- Modify: `src/app/mod.rs`
- Create: `tests/window_effects.rs`
- Modify: `tests/window_shell.rs`

**Step 1: Write the failing tests**

创建 `tests/window_effects.rs`，定义纯请求模型契约：

```rust
use mica_term::app::window_effects::{
    BackdropApplyStatus, BackdropPreference, NativeWindowTheme,
    WindowAppearanceSyncReport, build_native_window_appearance_request,
};
use mica_term::app::windowing::window_appearance;
use mica_term::theme::ThemeMode;

#[test]
fn dark_theme_maps_to_dark_native_theme_and_alt_mica_backdrop() {
    let request = build_native_window_appearance_request(ThemeMode::Dark, window_appearance());

    assert_eq!(request.theme, NativeWindowTheme::Dark);
    assert_eq!(request.backdrop, BackdropPreference::MicaAlt);
    assert!(request.request_redraw);
}

#[test]
fn light_theme_maps_to_light_native_theme_and_alt_mica_backdrop() {
    let request = build_native_window_appearance_request(ThemeMode::Light, window_appearance());

    assert_eq!(request.theme, NativeWindowTheme::Light);
    assert_eq!(request.backdrop, BackdropPreference::MicaAlt);
    assert!(request.request_redraw);
}

#[test]
fn skipped_sync_report_is_explicit() {
    let report = WindowAppearanceSyncReport::skipped();

    assert!(!report.theme_applied);
    assert_eq!(report.backdrop_status, BackdropApplyStatus::Skipped);
    assert!(!report.redraw_requested);
}
```

修改 `tests/window_shell.rs`，补一个稳定规格，确保现有窗口壳层元数据没有被本轮改歪：

```rust
use mica_term::app::window_effects::{BackdropPreference, build_native_window_appearance_request};
use mica_term::theme::ThemeMode;

#[test]
fn window_shell_prefers_alt_mica_backdrop_for_both_themes() {
    let appearance = window_appearance();

    let dark = build_native_window_appearance_request(ThemeMode::Dark, appearance);
    let light = build_native_window_appearance_request(ThemeMode::Light, appearance);

    assert_eq!(dark.backdrop, BackdropPreference::MicaAlt);
    assert_eq!(light.backdrop, BackdropPreference::MicaAlt);
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --test window_effects --test window_shell -q`  
Expected: FAIL with unresolved items such as `window_effects`, `build_native_window_appearance_request`, `NativeWindowTheme`, or `WindowAppearanceSyncReport::skipped`.

**Step 3: Write minimal implementation**

创建 `src/app/window_effects.rs`：

```rust
use crate::AppWindow;
use crate::app::windowing::{MaterialKind, WindowAppearance};
use crate::theme::ThemeMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeWindowTheme {
    Dark,
    Light,
}

impl NativeWindowTheme {
    pub fn is_dark(self) -> bool {
        matches!(self, Self::Dark)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackdropPreference {
    None,
    Mica,
    MicaAlt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeWindowAppearanceRequest {
    pub theme: NativeWindowTheme,
    pub backdrop: BackdropPreference,
    pub request_redraw: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackdropApplyStatus {
    Applied,
    Skipped,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowAppearanceSyncReport {
    pub theme_applied: bool,
    pub backdrop_status: BackdropApplyStatus,
    pub redraw_requested: bool,
}

impl WindowAppearanceSyncReport {
    pub fn skipped() -> Self {
        Self {
            theme_applied: false,
            backdrop_status: BackdropApplyStatus::Skipped,
            redraw_requested: false,
        }
    }
}

pub fn build_native_window_appearance_request(
    mode: ThemeMode,
    appearance: WindowAppearance,
) -> NativeWindowAppearanceRequest {
    let theme = match mode {
        ThemeMode::Dark => NativeWindowTheme::Dark,
        ThemeMode::Light => NativeWindowTheme::Light,
    };

    let backdrop = match appearance.material {
        MaterialKind::MicaAlt => BackdropPreference::MicaAlt,
    };

    NativeWindowAppearanceRequest {
        theme,
        backdrop,
        request_redraw: true,
    }
}

pub trait PlatformWindowEffects {
    fn apply_to_app_window(
        &self,
        window: &AppWindow,
        request: &NativeWindowAppearanceRequest,
    ) -> WindowAppearanceSyncReport;
}

#[derive(Default)]
pub struct NoopWindowEffects;

impl PlatformWindowEffects for NoopWindowEffects {
    fn apply_to_app_window(
        &self,
        _window: &AppWindow,
        _request: &NativeWindowAppearanceRequest,
    ) -> WindowAppearanceSyncReport {
        WindowAppearanceSyncReport::skipped()
    }
}
```

修改 `src/app/mod.rs`：

```rust
pub mod bootstrap;
pub mod ui_preferences;
pub mod window_effects;
pub mod windowing;
```

**Step 4: Run tests to verify they pass**

Run: `cargo test --test window_effects --test window_shell -q`  
Expected: PASS，且新的请求模型已经能稳定表达 `ThemeMode -> Native Theme + Mica Alt + Redraw`。

**Step 5: Commit**

```bash
git add src/app/mod.rs src/app/window_effects.rs tests/window_effects.rs tests/window_shell.rs
git commit -m "feat: add native window appearance request model"
```

### Task 2: 让 Bootstrap 在初始化和切换时调用原生窗口外观桥接

**Files:**
- Modify: `src/app/bootstrap.rs`
- Modify: `tests/top_status_bar_smoke.rs`
- Modify: `src/app/window_effects.rs`

**Step 1: Write the failing test**

修改 `tests/top_status_bar_smoke.rs`，新增一个 recording fake，验证“首次绑定”和“点击主题切换”都会触发原生窗口外观同步：

```rust
use std::cell::RefCell;
use std::rc::Rc;

use mica_term::AppWindow;
use mica_term::app::bootstrap::bind_top_status_bar_with_store_and_effects;
use mica_term::app::ui_preferences::UiPreferencesStore;
use mica_term::app::window_effects::{
    BackdropApplyStatus, NativeWindowAppearanceRequest, NativeWindowTheme,
    PlatformWindowEffects, WindowAppearanceSyncReport,
};

#[derive(Clone)]
struct RecordingWindowEffects {
    requests: Rc<RefCell<Vec<NativeWindowAppearanceRequest>>>,
}

impl RecordingWindowEffects {
    fn new(requests: Rc<RefCell<Vec<NativeWindowAppearanceRequest>>>) -> Self {
        Self { requests }
    }
}

impl PlatformWindowEffects for RecordingWindowEffects {
    fn apply_to_app_window(
        &self,
        _window: &AppWindow,
        request: &NativeWindowAppearanceRequest,
    ) -> WindowAppearanceSyncReport {
        self.requests.borrow_mut().push(*request);
        WindowAppearanceSyncReport {
            theme_applied: true,
            backdrop_status: BackdropApplyStatus::Applied,
            redraw_requested: request.request_redraw,
        }
    }
}

#[test]
fn bootstrap_syncs_native_window_effects_on_bind_and_theme_toggle() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let temp_path = std::env::temp_dir()
        .join("mica-term")
        .join("tests")
        .join("top-status-bar-window-effects.json");
    let _ = std::fs::remove_file(&temp_path);

    let requests = Rc::new(RefCell::new(Vec::new()));
    let effects = Rc::new(RecordingWindowEffects::new(Rc::clone(&requests)));

    bind_top_status_bar_with_store_and_effects(
        &app,
        Some(UiPreferencesStore::new(temp_path.clone())),
        effects,
    );

    {
        let requests = requests.borrow();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].theme, NativeWindowTheme::Dark);
        assert!(requests[0].request_redraw);
    }

    app.invoke_toggle_theme_mode_requested();

    {
        let requests = requests.borrow();
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[1].theme, NativeWindowTheme::Light);
        assert!(requests[1].request_redraw);
    }

    let _ = std::fs::remove_file(temp_path);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test top_status_bar_smoke -q`  
Expected: FAIL with missing symbol such as `bind_top_status_bar_with_store_and_effects`, or because `bootstrap` still only updates Slint state and does not call the injected effects bridge.

**Step 3: Write minimal implementation**

在 `src/app/window_effects.rs` 里先补默认 provider，后续 Task 3 再把 Windows 真实实现填进去：

```rust
use std::rc::Rc;

pub fn default_platform_window_effects() -> Rc<dyn PlatformWindowEffects> {
    Rc::new(NoopWindowEffects)
}
```

修改 `src/app/bootstrap.rs`，把原来的纯 Slint 主题切换升级为“Slint + 原生窗口外观”双通道：

```rust
use std::cell::RefCell;
use std::rc::Rc;

use crate::AppWindow;
use crate::app::ui_preferences::{UiPreferences, UiPreferencesStore};
use crate::app::window_effects::{
    PlatformWindowEffects, build_native_window_appearance_request,
    default_platform_window_effects,
};
use crate::app::windowing::{WindowController, window_appearance};
use crate::shell::view_model::ShellViewModel;
use crate::theme::ThemeMode;

fn sync_theme_and_window_effects(
    window: &AppWindow,
    state: &ShellViewModel,
    effects: &dyn PlatformWindowEffects,
) {
    window.set_dark_mode(state.theme_mode == ThemeMode::Dark);
    let request = build_native_window_appearance_request(state.theme_mode, window_appearance());
    let _ = effects.apply_to_app_window(window, &request);
}

fn sync_top_status_bar_state(
    window: &AppWindow,
    state: &ShellViewModel,
    effects: &dyn PlatformWindowEffects,
) {
    sync_theme_and_window_effects(window, state, effects);
    window.set_show_right_panel(state.show_right_panel);
    window.set_show_global_menu(state.show_global_menu);
    window.set_is_window_maximized(state.is_window_maximized);
    window.set_is_window_active(state.is_window_active);
    window.set_is_window_always_on_top(state.is_always_on_top);
}

pub fn bind_top_status_bar_with_store_and_effects(
    window: &AppWindow,
    store: Option<UiPreferencesStore>,
    effects: Rc<dyn PlatformWindowEffects>,
) {
    // 现有初始化逻辑保持不动
    // ...
    sync_top_status_bar_state(window, &view_model.borrow(), effects.as_ref());

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let store_ref = store.clone();
    let effects_ref = Rc::clone(&effects);
    window.on_toggle_theme_mode_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.toggle_theme_mode();
        sync_theme_and_window_effects(&window, &state, effects_ref.as_ref());
        save_ui_preferences(&store_ref, &state);
    });

    // 其他 callback 保持现有行为
}

pub fn bind_top_status_bar_with_store(window: &AppWindow, store: Option<UiPreferencesStore>) {
    bind_top_status_bar_with_store_and_effects(window, store, default_platform_window_effects());
}
```

注意：

- 只有“主题初始化”和“主题 toggle”需要调用原生窗口外观桥接
- `right panel / menu / pin / maximize` 不要误接入这个桥

**Step 4: Run test to verify it passes**

Run: `cargo test --test top_status_bar_smoke -q`  
Expected: PASS，并且 recording fake 能记录到 2 次请求：一次初始化 dark，一次切换后的 light。

**Step 5: Commit**

```bash
git add src/app/bootstrap.rs src/app/window_effects.rs tests/top_status_bar_smoke.rs
git commit -m "feat: bridge theme toggle to native window effects"
```

### Task 3: 实现 Windows 原生外观同步与源代码契约 smoke

**Files:**
- Modify: `src/app/window_effects.rs`
- Create: `tests/window_theme_contract_smoke.sh`
- Modify: `tests/window_effects.rs`

**Step 1: Write the failing smoke and test**

扩展 `tests/window_effects.rs`，增加默认 provider 的可用性断言：

```rust
use mica_term::app::window_effects::default_platform_window_effects;

#[test]
fn default_platform_window_effects_is_constructible() {
    let _ = default_platform_window_effects();
}
```

创建 `tests/window_theme_contract_smoke.sh`：

```bash
#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
FILE="$ROOT_DIR/src/app/window_effects.rs"

grep -F 'window.set_theme(Some(' "$FILE" >/dev/null
grep -F 'window.request_redraw();' "$FILE" >/dev/null
grep -F 'window_vibrancy::apply_tabbed' "$FILE" >/dev/null
grep -F '#[cfg(target_os = "windows")]' "$FILE" >/dev/null
grep -F 'NoopWindowEffects' "$FILE" >/dev/null
```

给脚本可执行权限：

```bash
chmod +x tests/window_theme_contract_smoke.sh
```

**Step 2: Run to verify it fails**

Run: `cargo test --test window_effects -q && bash tests/window_theme_contract_smoke.sh`  
Expected: Rust test may pass, but shell smoke FAIL，因为 `window_effects.rs` 里还没有真正的 Windows 实现，也没有 `set_theme/apply_tabbed/request_redraw`。

**Step 3: Write minimal implementation**

在 `src/app/window_effects.rs` 中把默认 provider 升级为“Windows 真实实现 + 非 Windows no-op”：

```rust
use std::rc::Rc;

pub fn default_platform_window_effects() -> Rc<dyn PlatformWindowEffects> {
    #[cfg(target_os = "windows")]
    {
        Rc::new(WindowsWindowEffects)
    }

    #[cfg(not(target_os = "windows"))]
    {
        Rc::new(NoopWindowEffects)
    }
}

#[cfg(target_os = "windows")]
#[derive(Default)]
pub struct WindowsWindowEffects;

#[cfg(target_os = "windows")]
impl PlatformWindowEffects for WindowsWindowEffects {
    fn apply_to_app_window(
        &self,
        app: &AppWindow,
        request: &NativeWindowAppearanceRequest,
    ) -> WindowAppearanceSyncReport {
        use slint::ComponentHandle;
        use slint::winit_030::{WinitWindowAccessor, winit};

        let mut theme_applied = false;
        let mut backdrop_status = BackdropApplyStatus::Skipped;
        let mut redraw_requested = false;

        let _ = app.window().with_winit_window(|window: &winit::window::Window| {
            let theme = match request.theme {
                NativeWindowTheme::Dark => winit::window::Theme::Dark,
                NativeWindowTheme::Light => winit::window::Theme::Light,
            };

            window.set_theme(Some(theme));
            theme_applied = true;

            backdrop_status = match request.backdrop {
                BackdropPreference::None => BackdropApplyStatus::Skipped,
                BackdropPreference::MicaAlt => {
                    match window_vibrancy::apply_tabbed(window, Some(request.theme.is_dark())) {
                        Ok(()) => BackdropApplyStatus::Applied,
                        Err(_) => BackdropApplyStatus::Failed,
                    }
                }
                BackdropPreference::Mica => {
                    match window_vibrancy::apply_mica(window, Some(request.theme.is_dark())) {
                        Ok(()) => BackdropApplyStatus::Applied,
                        Err(_) => BackdropApplyStatus::Failed,
                    }
                }
            };

            if request.request_redraw {
                window.request_redraw();
                redraw_requested = true;
            }
        });

        WindowAppearanceSyncReport {
            theme_applied,
            backdrop_status,
            redraw_requested,
        }
    }
}
```

注意：

- `MicaAlt` 对应 `window_vibrancy::apply_tabbed(...)`
- `ThemeMode` 和 `NativeWindowTheme` 的映射必须在进入 Windows API 前就完成
- backdrop 失败时不要 panic，返回 `BackdropApplyStatus::Failed` 即可

**Step 4: Run tests to verify they pass**

Run: `cargo test --test window_effects -q && bash tests/window_theme_contract_smoke.sh`  
Expected: PASS，且 smoke 能确认 `set_theme`、`apply_tabbed`、`request_redraw`、`cfg(target_os = "windows")`、`NoopWindowEffects` 全部存在。

**Step 5: Commit**

```bash
git add src/app/window_effects.rs tests/window_effects.rs tests/window_theme_contract_smoke.sh
git commit -m "feat: add windows native window appearance sync"
```

### Task 4: 收口验证文档并执行完整验证矩阵

**Files:**
- Modify: `verification.md`
- Reference: `docs/plans/2026-03-11-theme-toggle-window-appearance-design.md`
- Reference: `docs/plans/2026-03-11-theme-toggle-window-appearance-implementation-plan.md`
- Reference: `tests/window_theme_contract_smoke.sh`

**Step 1: Write the verification doc update**

将 `verification.md` 改为当前特性的验证报告模板：

```md
# Theme Toggle Window Appearance Verification

Date: <timestamp>

## Source Documents

- Design: `docs/plans/2026-03-11-theme-toggle-window-appearance-design.md`
- Implementation Plan: `docs/plans/2026-03-11-theme-toggle-window-appearance-implementation-plan.md`

## Commands Executed

- [x] `cargo fmt --all`
- [x] `cargo check --workspace`
- [x] `cargo test --test window_effects --test top_status_bar_smoke --test window_shell -q`
- [x] `bash tests/window_theme_contract_smoke.sh`
- [x] `cargo clippy --workspace -- -D warnings`

## GUI Smoke Status

- [ ] `cargo run`
- GUI smoke was not executed in this environment unless a desktop-capable Windows session is available.

## Windows 11 Manual Checklist

- [ ] `Dark -> Light -> Dark` 正常切换，窗口整体颜色一致
- [ ] 窗口底部超出屏幕时切换，超出区域不残留旧主题色
- [ ] 窗口左侧超出屏幕时切换，超出区域不残留旧主题色
- [ ] 窗口右侧超出屏幕时切换，超出区域不残留旧主题色
- [ ] 窗口顶部超出屏幕时切换，超出区域不残留旧主题色
- [ ] 最大化后切换主题，窗口外壳与内容区一致
- [ ] 还原后切换主题，窗口外壳与内容区一致
- [ ] 重启后主题持久化与窗口原生外观一致
- [ ] Windows 不支持 backdrop 或系统关闭透明效果时，应用能平稳降级
```

**Step 2: Run formatting and verification commands**

Run: `cargo fmt --all`  
Expected: PASS

Run: `cargo check --workspace`  
Expected: PASS

Run: `cargo test --test window_effects --test top_status_bar_smoke --test window_shell -q`  
Expected: PASS

Run: `bash tests/window_theme_contract_smoke.sh`  
Expected: PASS

Run: `cargo clippy --workspace -- -D warnings`  
Expected: PASS

**Step 3: Perform Windows 11 manual verification**

在真实 Windows 11 桌面环境中执行以下操作并把结果填回 `verification.md`：

1. 启动应用，确认初始主题与配置一致
2. 普通切换 `Dark -> Light -> Dark`
3. 将窗口底部拖出任务栏下方后切换主题
4. 将窗口左侧、右侧、顶部分别拖出屏幕边界后切换主题
5. 在最大化与还原状态各切换一次主题
6. 重启应用验证主题持久化与外壳一致性

**Step 4: Save verification report**

把自动验证结果和手动验证结论回填到 `verification.md`，不允许留空“已执行但没记录”状态。

**Step 5: Commit**

```bash
git add verification.md
git commit -m "docs: verify theme toggle window appearance sync"
```

## Final Verification Gate

只有在以下全部满足后，才允许宣称本轮实现完成：

- `cargo fmt --all` 通过
- `cargo check --workspace` 通过
- `cargo test --test window_effects --test top_status_bar_smoke --test window_shell -q` 通过
- `bash tests/window_theme_contract_smoke.sh` 通过
- `cargo clippy --workspace -- -D warnings` 通过
- Windows 11 手动清单中“窗口超出屏幕边界切换主题”的 4 个场景全部通过
- `verification.md` 已更新为本特性的验证报告

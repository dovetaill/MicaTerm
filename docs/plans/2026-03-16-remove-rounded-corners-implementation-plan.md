# Remove Rounded Corners Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 将当前 shell chrome 从“恢复态默认圆角、最大化时方角”的混合几何，收敛为全应用统一方角，并在 Windows 11 原生窗口层同步禁用顶层圆角。

**Architecture:** 保留 `WindowPlacementKind` 仅用于窗口布局与最大化判断，不再让它驱动 shell chrome 的圆角模式；Slint 层移除 `use-flat-window-chrome` 和一切 rounded/flat 切换语义，让 `AppWindow`、`Titlebar`、`RightPanel` 以及共享组件全部使用方角。Windows 原生层继续沿用 `window_effects.rs` 的统一外观同步路径，在同一处增加 `CornerPreference::DoNotRound`，避免 UI 层和 `HWND` 层出现不一致。

**Tech Stack:** Rust, Cargo, Slint 1.15.1, winit 0.30.13, `i-slint-backend-testing`, shell smoke scripts, `cargo fmt`, `cargo test`, `cargo check`, `cargo clippy`

---

## Execution Notes

- 我正在使用 `writing-plans` skill 来创建这份 implementation plan。
- 设计输入固定为 [2026-03-16-remove-rounded-corners-design.md](/home/wwwroot/mica-term/docs/plans/2026-03-16-remove-rounded-corners-design.md)。
- 最终选型固定为：`1B + 2B + 3C + 4A`。
- 执行时每个任务都先走 `@superpowers:test-driven-development`，先写失败测试或 smoke contract，再写最小实现。
- 最终收口前必须走 `@superpowers:verification-before-completion`，不要在没有新鲜验证输出的情况下声称完成。
- 本轮不接入 `wezterm-term` / `termwiz`，不修改 SSH/SFTP 逻辑，不引入新的 UI 组件，只收敛现有圆角实现。

## Target Snapshot

完成后应满足以下结果：

- `AppWindow` 不再暴露 `use-flat-window-chrome`。
- `WindowChromeMode`、`chrome_mode()`、`uses_flat_window_chrome()` 及其测试契约全部删除。
- `shell-frame`、`Titlebar`、`RightPanel` 和 `ui/` 下现有共享组件全部使用 `0px` 方角。
- Windows 原生窗口通过 `CornerPreference::DoNotRound` 禁用顶层圆角。
- 与历史圆角方案绑定的 smoke tests、命名、诊断字段、文档引用被清理到只剩新的方角基线。

## Out Of Scope

- `wezterm-term` / `termwiz` 接入
- SSH / SFTP 业务逻辑
- 新增 pane / tab / terminal 交互
- Theme token 体系重做
- Git 历史重写

## Task 1: 删除 shell chrome 圆角状态机

**Files:**
- Create: `tests/window_chrome_contract_smoke.sh`
- Modify: `src/app/window_state.rs`
- Modify: `src/shell/view_model.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `tests/window_state_spec.rs`
- Modify: `tests/shell_view_model.rs`
- Modify: `tests/top_status_bar_smoke.rs`
- Modify: `ui/app-window.slint`

**Step 1: Write the failing test**

新建 `tests/window_chrome_contract_smoke.sh`，先把“旧状态机符号必须消失”固化为 contract：

```bash
#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"

for unexpected in \
  'WindowChromeMode' \
  'chrome_mode(' \
  'uses_flat_window_chrome' \
  'set_use_flat_window_chrome' \
  'get_use_flat_window_chrome' \
  'use-flat-window-chrome'
do
  if rg -n --fixed-strings "$unexpected" \
    "$ROOT_DIR/src" "$ROOT_DIR/tests" "$ROOT_DIR/ui"
  then
    echo "unexpected rounded/flat chrome symbol remains: $unexpected" >&2
    exit 1
  fi
done
```

同时把 Rust 测试改成只验证 placement / maximize 语义，不再验证 chrome mode：

```rust
#[test]
fn restored_state_is_not_maximized() {
    assert!(!WindowPlacementKind::Restored.is_maximized());
}

#[test]
fn shell_view_model_tracks_window_placement_without_chrome_mode() {
    let mut view_model = ShellViewModel::default();

    assert_eq!(view_model.window_placement(), WindowPlacementKind::Restored);
    assert!(!view_model.is_window_maximized());

    view_model.set_window_placement(WindowPlacementKind::Maximized);
    assert_eq!(view_model.window_placement(), WindowPlacementKind::Maximized);
    assert!(view_model.is_window_maximized());
}
```

把 `tests/top_status_bar_smoke.rs` 中 `maximize_toggle_updates_flat_window_chrome_binding` 替换为不再涉及 `get_use_flat_window_chrome()` 的版本：

```rust
#[test]
fn maximize_toggle_updates_window_maximized_binding() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    assert!(!app.get_is_window_maximized());
    app.invoke_maximize_toggle_requested();
    assert!(app.get_is_window_maximized());
    app.invoke_drag_double_clicked();
    assert!(!app.get_is_window_maximized());
}
```

**Step 2: Run test to verify it fails**

Run: `bash tests/window_chrome_contract_smoke.sh`  
Expected: FAIL，当前仓库仍含 `WindowChromeMode`、`uses_flat_window_chrome`、`use-flat-window-chrome`。

Run: `cargo test --test window_state_spec --test shell_view_model --test top_status_bar_smoke -q`  
Expected: FAIL 或 compile error，测试已经不再接受旧的 chrome mode 路径。

**Step 3: Write minimal implementation**

在 `src/app/window_state.rs` 删除：

```rust
pub enum WindowChromeMode { ... }

impl WindowPlacementKind {
    pub fn chrome_mode(self) -> WindowChromeMode { ... }
}
```

保留：

```rust
impl WindowPlacementKind {
    pub fn is_maximized(self) -> bool {
        matches!(self, Self::Maximized)
    }
}
```

在 `src/shell/view_model.rs` 删除：

```rust
use crate::app::window_state::{WindowChromeMode, WindowPlacementKind};

pub fn uses_flat_window_chrome(&self) -> bool {
    matches!(self.window_placement.chrome_mode(), WindowChromeMode::Flat)
}
```

在 `src/app/bootstrap.rs` 删除：

```rust
window.set_use_flat_window_chrome(state.uses_flat_window_chrome());
```

在 `ui/app-window.slint` 删除：

```slint
in-out property <bool> use-flat-window-chrome: false;
```

**Step 4: Run test to verify it passes**

Run: `bash tests/window_chrome_contract_smoke.sh`  
Expected: PASS

Run: `cargo test --test window_state_spec --test shell_view_model --test top_status_bar_smoke -q`  
Expected: PASS

**Step 5: Commit**

```bash
git add tests/window_chrome_contract_smoke.sh src/app/window_state.rs \
  src/shell/view_model.rs src/app/bootstrap.rs tests/window_state_spec.rs \
  tests/shell_view_model.rs tests/top_status_bar_smoke.rs ui/app-window.slint
git commit -m "refactor: remove shell chrome mode state machine"
```

## Task 2: 将 `AppWindow` 外层几何固定为方角

**Files:**
- Modify: `ui/app-window.slint`
- Modify: `tests/window_geometry_spec.rs`
- Modify: `tests/top_status_bar_ui_contract_smoke.sh`

**Step 1: Write the failing test**

把 `tests/window_geometry_spec.rs` 的外层窗口壳断言改为“恢复态和最大化态都必须是 0 radius”：

```rust
#[test]
fn restored_window_uses_square_shell_frame() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);
    app.show().unwrap();

    assert_eq!(app.get_layout_shell_frame_radius() as u32, 0);
    assert_eq!(app.get_layout_titlebar_radius() as u32, 0);
    assert_eq!(app.get_layout_titlebar_border_width() as u32, 0);
}

#[test]
fn maximize_toggle_does_not_change_shell_frame_radius() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);
    app.show().unwrap();

    assert_eq!(app.get_layout_shell_frame_radius() as u32, 0);
    app.invoke_maximize_toggle_requested();
    assert_eq!(app.get_layout_shell_frame_radius() as u32, 0);
}
```

扩展 `tests/top_status_bar_ui_contract_smoke.sh`，明确禁止旧 ternary radius 表达式：

```bash
! grep -F 'use-flat-window-chrome' "$APP_WINDOW" >/dev/null
! grep -F 'root.use-flat-window-chrome ? 0px : 14px' "$APP_WINDOW" >/dev/null
grep -F 'border-radius: 0px;' "$APP_WINDOW" >/dev/null
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test window_geometry_spec -q`  
Expected: FAIL，当前 restored 状态仍导出 `layout_shell_frame_radius == 14`。

Run: `bash tests/top_status_bar_ui_contract_smoke.sh`  
Expected: FAIL，`ui/app-window.slint` 仍含旧 radius 切换语义。

**Step 3: Write minimal implementation**

在 `ui/app-window.slint` 把外层窗口壳固定为方角：

```slint
shell-frame := Rectangle {
    width: root.width;
    height: root.height;
    border-radius: 0px;
    border-width: 1px;
    border-color: ThemeTokens.shell-stroke;
    background: ThemeTokens.shell-surface;
}
```

如果 `chrome-host` 只剩“透传 width/height + clip”职责，保持它存在但把相关 radius 也固定为 `0px`：

```slint
chrome-host := Rectangle {
    width: parent.width;
    height: parent.height;
    border-radius: 0px;
    clip: true;
    background: transparent;
}
```

先不要在这一任务里删除 `chrome-host`，只做最小方角实现，降低风险。

**Step 4: Run test to verify it passes**

Run: `cargo test --test window_geometry_spec -q`  
Expected: PASS

Run: `bash tests/top_status_bar_ui_contract_smoke.sh`  
Expected: PASS

**Step 5: Commit**

```bash
git add ui/app-window.slint tests/window_geometry_spec.rs \
  tests/top_status_bar_ui_contract_smoke.sh
git commit -m "refactor: fix app window shell frame to square geometry"
```

## Task 3: 在 Windows 原生层同步 `DoNotRound`

**Files:**
- Modify: `src/app/window_effects.rs`
- Modify: `tests/window_effects.rs`
- Modify: `tests/top_status_bar_smoke.rs`
- Modify: `tests/window_theme_contract_smoke.sh`

**Step 1: Write the failing test**

在 `tests/window_effects.rs` 为 native appearance request 增加方角契约：

```rust
use mica_term::app::window_effects::{
    BackdropPreference, NativeWindowCornerPreference, NativeWindowTheme,
    build_native_window_appearance_request,
};

#[test]
fn dark_theme_maps_to_do_not_round_corner_preference() {
    let request = build_native_window_appearance_request(ThemeMode::Dark, window_appearance());
    assert_eq!(request.corner_preference, NativeWindowCornerPreference::DoNotRound);
}
```

在 `tests/top_status_bar_smoke.rs` 的 `RecordingWindowEffects` 相关测试中增加：

```rust
assert_eq!(
    requests[0].corner_preference,
    NativeWindowCornerPreference::DoNotRound
);
assert_eq!(
    requests[1].corner_preference,
    NativeWindowCornerPreference::DoNotRound
);
```

在 `tests/window_theme_contract_smoke.sh` 增加：

```bash
grep -F 'set_corner_preference' "$FILE" >/dev/null
grep -F 'CornerPreference::DoNotRound' "$FILE" >/dev/null
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test window_effects --test top_status_bar_smoke -q`  
Expected: FAIL，`corner_preference` 字段和枚举不存在。

Run: `bash tests/window_theme_contract_smoke.sh`  
Expected: FAIL，源码里还没有 `set_corner_preference`。

**Step 3: Write minimal implementation**

在 `src/app/window_effects.rs` 新增跨平台可测试的内部枚举：

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeWindowCornerPreference {
    Default,
    DoNotRound,
}
```

扩展 `NativeWindowAppearanceRequest`：

```rust
pub struct NativeWindowAppearanceRequest {
    pub theme: NativeWindowTheme,
    pub backdrop: BackdropPreference,
    pub corner_preference: NativeWindowCornerPreference,
    pub request_redraw: bool,
}
```

在 builder 中固定返回：

```rust
NativeWindowAppearanceRequest {
    theme,
    backdrop,
    corner_preference: NativeWindowCornerPreference::DoNotRound,
    request_redraw: true,
}
```

在 Windows 实现里映射到 `winit`：

```rust
use slint::winit_030::winit::platform::windows::{
    CornerPreference, WindowExtWindows,
};

let corner_preference = match request.corner_preference {
    NativeWindowCornerPreference::Default => CornerPreference::Default,
    NativeWindowCornerPreference::DoNotRound => CornerPreference::DoNotRound,
};

window.set_corner_preference(corner_preference);
```

**Step 4: Run test to verify it passes**

Run: `cargo test --test window_effects --test top_status_bar_smoke -q`  
Expected: PASS

Run: `bash tests/window_theme_contract_smoke.sh`  
Expected: PASS

**Step 5: Commit**

```bash
git add src/app/window_effects.rs tests/window_effects.rs \
  tests/top_status_bar_smoke.rs tests/window_theme_contract_smoke.sh
git commit -m "feat: disable native window corner rounding on windows"
```

## Task 4: 清零所有共享 UI 组件的圆角

**Files:**
- Create: `tests/square_component_contract_smoke.sh`
- Modify: `ui/components/status-pill.slint`
- Modify: `ui/components/sidebar-nav-button.slint`
- Modify: `ui/components/assets-create-menu.slint`
- Modify: `ui/components/command-palette.slint`
- Modify: `ui/components/active-tab.slint`
- Modify: `ui/components/titlebar-tooltip.slint`
- Modify: `ui/components/segmented-control.slint`
- Modify: `ui/components/sidebar-toolbar-icon-button.slint`
- Modify: `ui/components/command-entry.slint`
- Modify: `ui/components/titlebar-icon-button.slint`
- Modify: `ui/components/titlebar-menu.slint`
- Modify: `ui/shell/assets-sidebar.slint`
- Modify: `ui/shell/titlebar.slint`

**Step 1: Write the failing test**

新建 `tests/square_component_contract_smoke.sh`，对 `ui/` 目录启用全局方角约束：

```bash
#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"

if rg -nP 'border-radius:\s*(?!0px\b)' "$ROOT_DIR/ui"; then
  echo "unexpected rounded border-radius remains under ui/" >&2
  exit 1
fi
```

**Step 2: Run test to verify it fails**

Run: `bash tests/square_component_contract_smoke.sh`  
Expected: FAIL，并列出当前所有仍大于 `0px` 的 `border-radius` 文件。

**Step 3: Write minimal implementation**

把下列组件全部改为方角：

```slint
border-radius: 0px;
```

需要修改的现有位置包括但不限于：

- `ui/components/status-pill.slint`
- `ui/components/sidebar-nav-button.slint`
- `ui/components/assets-create-menu.slint`
- `ui/components/command-palette.slint`
- `ui/components/active-tab.slint`
- `ui/components/titlebar-tooltip.slint`
- `ui/components/segmented-control.slint`
- `ui/components/sidebar-toolbar-icon-button.slint`
- `ui/components/command-entry.slint`
- `ui/components/titlebar-icon-button.slint`
- `ui/components/titlebar-menu.slint`
- `ui/shell/assets-sidebar.slint`
- `ui/shell/titlebar.slint`

其中 `ui/shell/titlebar.slint` 里需要把局部 drag-zone 和 divider 也显式收敛为方角：

```slint
drag-zone := Rectangle {
    background: transparent;
    border-radius: 0px;
}

divider-line := Rectangle {
    ...
    border-radius: 0px;
}
```

`ui/shell/assets-sidebar.slint` 里的 `Create` 按钮也要同步去圆角：

```slint
create-button := Rectangle {
    width: 72px;
    height: 28px;
    border-radius: 0px;
    ...
}
```

**Step 4: Run test to verify it passes**

Run: `bash tests/square_component_contract_smoke.sh`  
Expected: PASS

Run: `bash tests/top_status_bar_ui_contract_smoke.sh`  
Expected: PASS

**Step 5: Commit**

```bash
git add tests/square_component_contract_smoke.sh ui/components/status-pill.slint \
  ui/components/sidebar-nav-button.slint ui/components/assets-create-menu.slint \
  ui/components/command-palette.slint ui/components/active-tab.slint \
  ui/components/titlebar-tooltip.slint ui/components/segmented-control.slint \
  ui/components/sidebar-toolbar-icon-button.slint ui/components/command-entry.slint \
  ui/components/titlebar-icon-button.slint ui/components/titlebar-menu.slint \
  ui/shell/assets-sidebar.slint ui/shell/titlebar.slint
git commit -m "refactor: square off shared shell components"
```

## Task 5: 清理失败尝试残留与冲突文档

**Files:**
- Delete: `docs/plans/2026-03-16-top-status-bar-corner-background-design.md`
- Delete: `docs/plans/2026-03-16-top-status-bar-corner-background-implementation-plan.md`
- Delete: `docs/plans/2026-03-16-top-status-bar-corner-background-tdd-spec.md`
- Modify: `verification.md`

**Step 1: Write the failing test**

先在 shell 中人工确认这些旧文档仍存在：

```bash
test -f docs/plans/2026-03-16-top-status-bar-corner-background-design.md
test -f docs/plans/2026-03-16-top-status-bar-corner-background-implementation-plan.md
test -f docs/plans/2026-03-16-top-status-bar-corner-background-tdd-spec.md
```

再追加一条 lightweight check，确保验证记录会引用新的设计/实现文档而不是旧链路：

```bash
grep -F '2026-03-16-remove-rounded-corners-design.md' verification.md >/dev/null
grep -F '2026-03-16-remove-rounded-corners-implementation-plan.md' verification.md >/dev/null
```

**Step 2: Run test to verify it fails**

Run: 上述 `test` 和 `grep` 命令  
Expected: 前三条存在性检查成功，后两条 `grep` 失败，说明旧文档仍在、新链路未写入验证记录。

**Step 3: Write minimal implementation**

删除已被新方案取代、且与当前执行方向冲突的最新旧文档：

```bash
rm -f docs/plans/2026-03-16-top-status-bar-corner-background-design.md
rm -f docs/plans/2026-03-16-top-status-bar-corner-background-implementation-plan.md
rm -f docs/plans/2026-03-16-top-status-bar-corner-background-tdd-spec.md
```

在 `verification.md` 追加新的计划引用和验证摘要模板，例如：

```md
- Design: docs/plans/2026-03-16-remove-rounded-corners-design.md
- Plan: docs/plans/2026-03-16-remove-rounded-corners-implementation-plan.md
- Contracts:
  - tests/window_chrome_contract_smoke.sh
  - tests/square_component_contract_smoke.sh
  - tests/top_status_bar_ui_contract_smoke.sh
  - tests/window_theme_contract_smoke.sh
```

**Step 4: Run test to verify it passes**

Run: 重复 Step 1 的 `test` 和 `grep` 命令  
Expected: 旧文档不存在，新验证记录可检出。

**Step 5: Commit**

```bash
git add verification.md
git rm docs/plans/2026-03-16-top-status-bar-corner-background-design.md \
  docs/plans/2026-03-16-top-status-bar-corner-background-implementation-plan.md \
  docs/plans/2026-03-16-top-status-bar-corner-background-tdd-spec.md
git commit -m "chore: remove superseded rounded-corner planning artifacts"
```

## Task 6: 全量验证与交付收口

**Files:**
- Modify: `verification.md`

**Step 1: Write the failing test**

这一任务不新增产品测试，直接使用前面所有 contract 与现有测试作为收口门槛。

**Step 2: Run verification to establish the baseline**

Run:

```bash
bash tests/window_chrome_contract_smoke.sh
bash tests/square_component_contract_smoke.sh
bash tests/top_status_bar_ui_contract_smoke.sh
bash tests/window_theme_contract_smoke.sh
cargo test --test window_state_spec --test shell_view_model --test window_effects \
  --test window_geometry_spec --test top_status_bar_smoke -q
cargo check
cargo clippy --all-targets --all-features -- -D warnings
```

Expected: 全部 PASS；若任一命令失败，返回对应任务继续修复，不进入最终提交。

**Step 3: Write the minimal implementation**

把最终验证结果追加到 `verification.md`，采用可审计格式记录命令与结论，例如：

```md
## 2026-03-16 Remove Rounded Corners

- `bash tests/window_chrome_contract_smoke.sh` => PASS
- `bash tests/square_component_contract_smoke.sh` => PASS
- `bash tests/top_status_bar_ui_contract_smoke.sh` => PASS
- `bash tests/window_theme_contract_smoke.sh` => PASS
- `cargo test --test window_state_spec --test shell_view_model --test window_effects --test window_geometry_spec --test top_status_bar_smoke -q` => PASS
- `cargo check` => PASS
- `cargo clippy --all-targets --all-features -- -D warnings` => PASS
```

**Step 4: Run verification again to confirm the record is accurate**

Run: 重复 Step 2 全部命令  
Expected: 仍然全部 PASS，且 `verification.md` 中的记录与刚执行结果一致。

**Step 5: Commit**

```bash
git add verification.md
git commit -m "docs: record remove-rounded-corners verification"
```

## Execution Handoff

Plan complete and saved to `docs/plans/2026-03-16-remove-rounded-corners-implementation-plan.md`. Two execution options:

**1. Subagent-Driven (this session)** - I dispatch fresh subagent per task, review between tasks, fast iteration

**2. Parallel Session (separate)** - Open new session with executing-plans, batch execution with checkpoints

Which approach?

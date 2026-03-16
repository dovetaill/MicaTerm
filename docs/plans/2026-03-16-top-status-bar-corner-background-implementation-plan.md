# Top Status Bar Corner Background Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 落实已确认的 “外层顺系统圆角、内部全部 flat” 壳层方案，消除顶部状态栏左右上角“里面圆、外面方”的观感问题，并将 `Titlebar / RightPanel / Workspace chrome` 收敛为稳定的 `flat internal chrome`。

**Architecture:** 保留 `AppWindow.shell-frame` 作为窗口级 outer geometry 的唯一 owner，继续让 `use-flat-window-chrome` 只驱动 `Restored => Rounded / Maximized|Snapped => Flat`。内部 `Titlebar` 和 `RightPanel` 不再持有完整 rounded card 几何，统一改为 flat docked chrome；如果 flat 内层在恢复态下会把颜色绘制到外层圆角之外，则在 `shell-frame` 内增加一个与外轮廓同半径的单一 clip host 作为几何 containment，而不是视觉补丁。

**Tech Stack:** Rust, Cargo, Slint 1.15.1, `i-slint-backend-testing`, `renderer-femtovg-wgpu`, shell smoke scripts, `cargo fmt`, `cargo test`, `cargo check`, `cargo clippy`

---

## Execution Notes

- 我正在使用 `writing-plans` skill 来创建这份 implementation plan。
- 设计输入固定为 [2026-03-16-top-status-bar-corner-background-design.md](/home/wwwroot/mica-term/docs/plans/2026-03-16-top-status-bar-corner-background-design.md)。
- 最终选型固定为：`1A + 2C + 3A + 4A`。
- 本轮不改 `wezterm-term` / `termwiz` / SSH / SFTP，不改业务交互，只处理 shell geometry ownership。
- 每个任务都先执行 `@superpowers:test-driven-development`：先写失败测试，再写最小实现，再跑通过。
- 如果 flat inner chrome 在恢复态下仍然露出方角背景，不要回退到 rounded header；应通过单一 `clip host` 约束内部绘制范围。
- `use-flat-window-chrome` 必须继续保留在 `AppWindow` 层，用于窗口级外轮廓切换；不要把这个状态重新传回 `Titlebar` 做内部圆角。

## Target Snapshot

完成后应满足以下结果：

- 恢复态下只有最外层窗口跟随系统圆角。
- `Titlebar` 不再是 rounded card，也不再持有完整 `1px` 外框。
- `RightPanel` 不再是 rounded card，而是 square docked pane。
- 左上角和右上角不再出现“里面圆、外面方”。
- `Maximized / Snapped` 状态下，outer geometry 与 inner chrome 都保持 flat。
- maximize button geometry、drag zone、resize band、Windows frame adapter 现有行为不退化。

## Out Of Scope

- `wezterm-term` / `termwiz` 接入
- SSH / SFTP 业务逻辑
- 新增导航、tab、pane splitter 交互
- 大规模 theme token 重做
- Win32 non-client area 全量接管

## Task Ordering

1. 先导出 geometry diagnostics，建立最终 flat contract 的可观察面。
2. 再把 `Titlebar` 从 rounded card 改成 flat chrome，并锁定“maximize 只影响 outer shell”。
3. 然后把内部绘制约束到外层窗口几何之内，防止恢复态下 flat 背景冲破圆角。
4. 再把 `RightPanel` 改成 square docked pane。
5. 最后补全 smoke、格式化、测试与验证记录。

## Task 1: 导出内部 chrome 几何诊断属性

**Files:**
- Modify: `ui/shell/titlebar.slint`
- Modify: `ui/shell/right-panel.slint`
- Modify: `ui/app-window.slint`
- Modify: `tests/window_geometry_spec.rs`

**Step 1: Write the failing test**

在 `tests/window_geometry_spec.rs` 追加一条能观察内部 chrome diagnostics 的测试：

```rust
#[test]
fn shell_exports_internal_chrome_geometry_contracts() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);
    app.invoke_toggle_right_panel_requested();
    app.show().unwrap();

    assert_eq!(app.get_layout_titlebar_border_width() as u32, 1);
    assert_eq!(app.get_layout_right_panel_radius() as u32, 14);
    assert_eq!(app.get_layout_right_panel_border_width() as u32, 1);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test window_geometry_spec -q`  
Expected: FAIL，缺少 `get_layout_titlebar_border_width`、`get_layout_right_panel_radius` 或 `get_layout_right_panel_border_width` getter。

**Step 3: Write minimal implementation**

在 `ui/shell/titlebar.slint` 导出：

```slint
out property <length> layout-radius: root.border-radius;
out property <length> layout-border-width: root.border-width;
```

在 `ui/shell/right-panel.slint` 导出：

```slint
out property <length> layout-radius: root.border-radius;
out property <length> layout-border-width: root.border-width;
```

在 `ui/app-window.slint` 上抛这些 diagnostics：

```slint
out property <length> layout-titlebar-border-width: titlebar.layout-border-width;
out property <length> layout-right-panel-radius: right-panel.layout-radius;
out property <length> layout-right-panel-border-width: right-panel.layout-border-width;
```

此任务只补可观察面，不改视觉语义。

**Step 4: Run test to verify it passes**

Run: `cargo test --test window_geometry_spec -q`  
Expected: PASS，新增 getter 可用，现有几何测试不退化。

**Step 5: Commit**

```bash
git add ui/shell/titlebar.slint ui/shell/right-panel.slint ui/app-window.slint \
  tests/window_geometry_spec.rs
git commit -m "test: export shell chrome geometry diagnostics"
```

## Task 2: 将 Titlebar 收敛为 flat internal chrome

**Files:**
- Modify: `ui/shell/titlebar.slint`
- Modify: `ui/app-window.slint`
- Modify: `tests/window_geometry_spec.rs`
- Modify: `tests/top_status_bar_smoke.rs`
- Modify: `tests/top_status_bar_ui_contract_smoke.sh`

**Step 1: Write the failing tests**

把 `tests/window_geometry_spec.rs` 中标题栏相关断言改成最终契约：

```rust
#[test]
fn restored_window_keeps_rounded_shell_frame_but_flattens_titlebar() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);
    app.show().unwrap();

    assert_eq!(app.get_layout_shell_frame_radius() as u32, 14);
    assert_eq!(app.get_layout_titlebar_radius() as u32, 0);
    assert_eq!(app.get_layout_titlebar_border_width() as u32, 0);
}

#[test]
fn flat_window_chrome_flattens_shell_frame_without_reintroducing_titlebar_card() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);
    app.invoke_maximize_toggle_requested();
    app.show().unwrap();

    assert_eq!(app.get_layout_shell_frame_radius() as u32, 0);
    assert_eq!(app.get_layout_titlebar_radius() as u32, 0);
    assert_eq!(app.get_layout_titlebar_border_width() as u32, 0);
}
```

在 `tests/top_status_bar_smoke.rs` 追加：

```rust
#[test]
fn maximize_toggle_only_changes_outer_shell_chrome() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    assert_eq!(app.get_layout_titlebar_radius() as u32, 0);
    assert_eq!(app.get_layout_titlebar_border_width() as u32, 0);

    app.set_use_flat_window_chrome(true);
    assert_eq!(app.get_layout_shell_frame_radius() as u32, 0);
    assert_eq!(app.get_layout_titlebar_radius() as u32, 0);
    assert_eq!(app.get_layout_titlebar_border_width() as u32, 0);

    app.set_use_flat_window_chrome(false);
    assert_eq!(app.get_layout_shell_frame_radius() as u32, 14);
    assert_eq!(app.get_layout_titlebar_radius() as u32, 0);
    assert_eq!(app.get_layout_titlebar_border_width() as u32, 0);
}
```

扩展 `tests/top_status_bar_ui_contract_smoke.sh`：

```bash
grep -F 'border-radius: 0px;' "$TITLEBAR" >/dev/null
grep -F 'border-width: 0px;' "$TITLEBAR" >/dev/null
! grep -F 'in property <bool> use-flat-window-chrome: false;' "$TITLEBAR" >/dev/null
! grep -F 'border-radius: root.use-flat-window-chrome ? 0px : 12px;' "$TITLEBAR" >/dev/null
! grep -F 'use-flat-window-chrome: root.use-flat-window-chrome;' "$APP_WINDOW" >/dev/null
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --test window_geometry_spec -q`  
Expected: FAIL，当前标题栏还是 `12px` 圆角和 `1px` 边框。

Run: `cargo test --test top_status_bar_smoke -q`  
Expected: FAIL，新增测试发现 titlebar 仍然随着窗口状态带 rounded card。

Run: `bash tests/top_status_bar_ui_contract_smoke.sh`  
Expected: FAIL，静态 contract 仍包含 `use-flat-window-chrome` 与旧圆角表达式。

**Step 3: Write minimal implementation**

在 `ui/shell/titlebar.slint`：

```slint
export component Titlebar inherits Rectangle {
    in property <bool> dark-mode: true;
    in property <bool> show-right-panel: false;
    in property <bool> show-global-menu: false;
    in property <bool> is-window-maximized: false;
    in property <bool> is-window-active: true;
    in property <bool> is-window-always-on-top: false;
    ...
    height: 48px;
    background: ThemeTokens.command-tint;
    border-radius: 0px;
    border-width: 0px;
    border-color: transparent;
}
```

在 `ui/app-window.slint` 停止把 `use-flat-window-chrome` 传给 `Titlebar`：

```slint
titlebar := Titlebar {
    x: 0px;
    y: 0px;
    width: parent.width;
    dark-mode: root.dark-mode;
    show-right-panel: root.show-right-panel;
    show-global-menu: root.show-global-menu;
    is-window-maximized: root.is-window-maximized;
    is-window-active: root.is-window-active;
    is-window-always-on-top: root.is-window-always-on-top;
    ...
}
```

保留 `AppWindow.use-flat-window-chrome`，但它只继续驱动 `shell-frame.border-radius`。

**Step 4: Run tests to verify they pass**

Run: `cargo test --test window_geometry_spec -q`  
Expected: PASS，恢复态只有 outer shell rounded，titlebar 始终 flat。

Run: `cargo test --test top_status_bar_smoke -q`  
Expected: PASS，maximize 只改变 outer shell。

Run: `bash tests/top_status_bar_ui_contract_smoke.sh`  
Expected: PASS，静态 contract 不再允许 rounded titlebar card 回流。

**Step 5: Commit**

```bash
git add ui/shell/titlebar.slint ui/app-window.slint \
  tests/window_geometry_spec.rs tests/top_status_bar_smoke.rs \
  tests/top_status_bar_ui_contract_smoke.sh
git commit -m "feat: flatten titlebar internal chrome"
```

## Task 3: 约束 flat internal chrome 到 outer shell geometry 内

**Files:**
- Modify: `ui/app-window.slint`
- Modify: `tests/shell_layout_ui_contract_smoke.sh`
- Modify: `tests/top_status_bar_ui_contract_smoke.sh`

**Step 1: Write the failing static contracts**

在 `tests/shell_layout_ui_contract_smoke.sh` 增加：

```bash
grep -F 'chrome-host := Rectangle' "$APP_WINDOW" >/dev/null
grep -F 'border-radius: parent.border-radius;' "$APP_WINDOW" >/dev/null
grep -F 'clip: true;' "$APP_WINDOW" >/dev/null
```

在 `tests/top_status_bar_ui_contract_smoke.sh` 增加：

```bash
grep -F 'chrome-host := Rectangle' "$APP_WINDOW" >/dev/null
grep -F 'titlebar := Titlebar {' "$APP_WINDOW" >/dev/null
```

**Step 2: Run scripts to verify they fail**

Run: `bash tests/shell_layout_ui_contract_smoke.sh`  
Expected: FAIL，当前 `AppWindow` 没有统一的 inner chrome containment host。

Run: `bash tests/top_status_bar_ui_contract_smoke.sh`  
Expected: FAIL，缺少 `chrome-host`。

**Step 3: Write minimal implementation**

在 `ui/app-window.slint` 中，把 `content-column` 改为一个几何 containment host：

```slint
shell-frame := Rectangle {
    width: root.width;
    height: root.height;
    border-radius: root.use-flat-window-chrome ? 0px : 14px;
    border-width: 1px;
    border-color: ThemeTokens.shell-stroke;
    background: ThemeTokens.shell-surface;

    chrome-host := Rectangle {
        width: parent.width;
        height: parent.height;
        border-radius: parent.border-radius;
        clip: true;
        background: transparent;

        titlebar := Titlebar { ... }
        body-host := Rectangle { ... }
    }
}
```

说明：

- `chrome-host` 是 outer geometry containment，不是视觉遮罩补丁。
- 不允许在这里重新引入独立圆角背景、角图片或透明角遮盖逻辑。

**Step 4: Run scripts to verify they pass**

Run: `bash tests/shell_layout_ui_contract_smoke.sh`  
Expected: PASS，内部 chrome 已经有统一 containment host。

Run: `bash tests/top_status_bar_ui_contract_smoke.sh`  
Expected: PASS。

**Step 5: Commit**

```bash
git add ui/app-window.slint \
  tests/shell_layout_ui_contract_smoke.sh tests/top_status_bar_ui_contract_smoke.sh
git commit -m "fix: constrain flat chrome to outer shell geometry"
```

## Task 4: 将 RightPanel 收敛为 square docked pane

**Files:**
- Modify: `ui/shell/right-panel.slint`
- Modify: `tests/window_geometry_spec.rs`
- Modify: `tests/shell_layout_ui_contract_smoke.sh`

**Step 1: Write the failing tests**

在 `tests/window_geometry_spec.rs` 追加：

```rust
#[test]
fn expanded_right_panel_is_flat_and_owns_no_full_card_border() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);
    app.invoke_toggle_right_panel_requested();
    app.show().unwrap();

    assert_eq!(app.get_layout_right_panel_radius() as u32, 0);
    assert_eq!(app.get_layout_right_panel_border_width() as u32, 0);
}
```

在 `tests/shell_layout_ui_contract_smoke.sh` 增加：

```bash
grep -F 'left-divider := Rectangle {' "$RIGHT_PANEL" >/dev/null
grep -F 'border-radius: 0px;' "$RIGHT_PANEL" >/dev/null
grep -F 'border-width: 0px;' "$RIGHT_PANEL" >/dev/null
! grep -F 'border-radius: 14px;' "$RIGHT_PANEL" >/dev/null
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --test window_geometry_spec -q`  
Expected: FAIL，当前 `RightPanel` 还是 rounded card。

Run: `bash tests/shell_layout_ui_contract_smoke.sh`  
Expected: FAIL，当前没有 left divider，且仍存在 `border-radius: 14px;`。

**Step 3: Write minimal implementation**

在 `ui/shell/right-panel.slint` 改为 docked pane：

```slint
export component RightPanel inherits Rectangle {
    in property <bool> expanded: false;
    out property <length> layout-radius: root.border-radius;
    out property <length> layout-border-width: root.border-width;

    width: root.expanded ? 392px : 0px;
    visible: root.expanded;
    clip: true;
    background: ThemeTokens.panel-tint;
    border-radius: 0px;
    border-width: 0px;
    border-color: transparent;

    left-divider := Rectangle {
        x: 0px;
        y: 0px;
        width: 1px;
        height: parent.height;
        background: ThemeTokens.shell-stroke;
    }

    SegmentedControl {
        x: 12px;
        y: 12px;
        width: parent.width - 24px;
        height: 36px;
    }
}
```

不要把 `RightPanel` 改成浮层，不要引入外侧圆角。

**Step 4: Run tests to verify they pass**

Run: `cargo test --test window_geometry_spec -q`  
Expected: PASS，right panel geometry diagnostics 归零。

Run: `bash tests/shell_layout_ui_contract_smoke.sh`  
Expected: PASS，right panel 已是 square docked pane。

**Step 5: Commit**

```bash
git add ui/shell/right-panel.slint tests/window_geometry_spec.rs \
  tests/shell_layout_ui_contract_smoke.sh
git commit -m "feat: flatten right panel docked chrome"
```

## Task 5: 完整验证与回归记录

**Files:**
- Modify: `verification.md`
- Modify: `docs/plans/2026-03-16-top-status-bar-corner-background-implementation-plan.md`

**Step 1: Run formatting and automated checks**

Run:

```bash
cargo fmt --check
cargo test --test window_geometry_spec -q
cargo test --test top_status_bar_smoke -q
bash tests/top_status_bar_ui_contract_smoke.sh
bash tests/shell_layout_ui_contract_smoke.sh
bash tests/windows_frame_contract_smoke.sh
cargo check -q
cargo clippy --workspace -- -D warnings
```

Expected:

- 全部 PASS
- `windows_frame_contract_smoke.sh` 不需要改动，证明 maximize hit-test/export contract 未退化

**Step 2: Run manual Windows verification**

在 Windows 真机按以下顺序检查：

1. 恢复态启动窗口，确认左上角和右上角不再出现“里面圆、外面方”。
2. 切换 dark/light theme，确认顶部两角都没有露底或双描边。
3. 打开 `RightPanel`，确认它是 docked square pane，而不是独立 rounded card。
4. 最大化窗口，确认 outer shell 与 inner chrome 都为 flat。
5. 贴靠窗口，确认无残留 rounded 角。
6. 使用 drag zone、maximize button、resize 边缘，确认行为与当前一致。

**Step 3: Record verification evidence**

在 `verification.md` 记录：

```md
## 2026-03-16 Top Status Bar Corner Background

- `cargo fmt --check`
- `cargo test --test window_geometry_spec -q`
- `cargo test --test top_status_bar_smoke -q`
- `bash tests/top_status_bar_ui_contract_smoke.sh`
- `bash tests/shell_layout_ui_contract_smoke.sh`
- `bash tests/windows_frame_contract_smoke.sh`
- `cargo check -q`
- `cargo clippy --workspace -- -D warnings`

Windows manual checks:
- restored: outer rounded, inner flat
- maximized: outer flat, inner flat
- right panel: square docked pane
- no square background visible behind top corners
```

**Step 4: Commit**

```bash
git add verification.md
git commit -m "test: verify flat internal chrome corner fix"
```

## Expected End State

- `ui/app-window.slint` 继续保留窗口级圆角切换，但内部通过单一 containment host 服从 outer geometry。
- `ui/shell/titlebar.slint` 不再保留 rounded card 语义，也不再依赖 `use-flat-window-chrome`。
- `ui/shell/right-panel.slint` 收敛为 square docked pane。
- `tests/window_geometry_spec.rs`、`tests/top_status_bar_smoke.rs`、相关 shell smoke 一起锁定“外层 rounded、内部 flat”的最终 contract。

## Rollback Rule

- 如果 flat internal chrome 在视觉上仍不成立，只允许调查 containment 是否失效。
- 不允许直接回滚到 rounded `Titlebar` card 或 rounded `RightPanel` card。
- 若必须调整，只能在“outer rounded + inner flat”前提下微调 divider、surface tint 或 containment 实现。

## Execution Artifacts

- Verification report: `verification.md`
- TDD handoff: `docs/plans/2026-03-16-top-status-bar-corner-background-tdd-spec.md`
- Execution workspace: `.worktrees/top-status-bar-corner-background`

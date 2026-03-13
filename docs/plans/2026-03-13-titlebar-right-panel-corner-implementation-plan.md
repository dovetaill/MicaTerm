# Titlebar / Right Panel Corner Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 在现有 `frameless + Mica` shell 中落实已确认的 `1A + 2A + 3A + 4A` 方案，让 `shell-frame` 成为唯一外轮廓 owner，同时把 `Titlebar` 和 `RightPanel` 收敛为 flat internal chrome，消除双边框与错误圆角。

**Architecture:** 保留 `AppWindow.shell-frame` 作为窗口外轮廓、窗口级圆角与窗口级描边的唯一来源。Slint 侧通过新增 geometry diagnostics、将 `Titlebar`/`RightPanel` 改为 flat pane，并在 `shell-frame` 内引入同半径的 `chrome-mask` 来约束内部 chrome；Rust 侧的 `window_placement -> use-flat-window-chrome` 映射继续只控制窗口级 outer geometry，不再驱动标题栏自身圆角。验证以 geometry contract、UI smoke contract 和现有 renderer/frame regression 为主。

**Tech Stack:** Rust, Cargo, Slint 1.15.1, `i-slint-backend-testing`, software renderer, shell smoke scripts, `cargo fmt`, `cargo test`, `cargo check`, `cargo clippy`

---

## Execution Notes

- 我正在使用 `writing-plans` skill 来创建这份 implementation plan。
- 设计输入固定为 `docs/plans/2026-03-13-titlebar-right-panel-corner-design.md`，执行时不得偏离已确认选型：`1A + 2A + 3A + 4A`。
- 每个任务都先执行 `@superpowers:test-driven-development`：先写失败测试，再写最小实现，再跑通过。
- 如果 `Slint` 的 `clip` 与 `border-radius` 在当前 renderer 下表现和预期不一致，立即切换到 `@superpowers:systematic-debugging`，不要猜测。
- 本轮只收敛 shell 几何 ownership，不改 terminal runtime、SSH / SFTP、tooltip 交互模型、窗口拖拽/resize 行为或主题 token。
- `ShellMetrics` 的宽高预算、`WindowPlacementKind -> WindowChromeMode` 映射、maximize button geometry export 必须保持兼容。
- 计划默认在独立 worktree 执行；若继续在当前工作区执行，也必须把修改范围限制在本计划列出的文件中。

## Target Snapshot

完成后应满足以下用户可见结果：

- `shell-frame` 仍然是唯一完整外框来源，恢复态半径保持 `14px`
- `Titlebar` 不再是 rounded card，而是 flat top chrome
- `RightPanel` 不再是 rounded card，而是 square docked pane
- `Titlebar` 与 `RightPanel` 根节点都不再持有完整 `1px` 外框
- `use-flat-window-chrome` 只控制窗口级 outer geometry，最大化/贴靠时只让 `shell-frame` 变平
- maximize button geometry、窗口拖拽、Windows frame adapter contract 不退化

## Task 1: 导出内部 chrome 几何诊断属性

**Files:**
- Modify: `ui/shell/titlebar.slint`
- Modify: `ui/shell/right-panel.slint`
- Modify: `ui/app-window.slint`
- Modify: `tests/window_geometry_spec.rs`

**Step 1: Write the failing test**

修改 `tests/window_geometry_spec.rs`，在现有测试后追加：

```rust
#[test]
fn shell_exports_internal_chrome_geometry_contracts() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);
    app.invoke_toggle_right_panel_requested();
    app.show().unwrap();

    assert_eq!(app.get_layout_titlebar_radius() as u32, 12);
    assert_eq!(app.get_layout_titlebar_border_width() as u32, 1);
    assert_eq!(app.get_layout_right_panel_radius() as u32, 14);
    assert_eq!(app.get_layout_right_panel_border_width() as u32, 1);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test window_geometry_spec -q`  
Expected: FAIL with missing getters such as `get_layout_titlebar_border_width`, `get_layout_right_panel_radius`, or `get_layout_right_panel_border_width`.

**Step 3: Write minimal implementation**

修改 `ui/shell/titlebar.slint`，为根节点导出 border 诊断：

```slint
out property <length> layout-radius: root.border-radius;
out property <length> layout-border-width: root.border-width;
```

修改 `ui/shell/right-panel.slint`，为右侧 pane 导出几何诊断：

```slint
out property <length> layout-radius: root.border-radius;
out property <length> layout-border-width: root.border-width;
```

修改 `ui/app-window.slint`，把这些诊断属性上抛到 `AppWindow`：

```slint
out property <length> layout-titlebar-border-width: titlebar.layout-border-width;
out property <length> layout-right-panel-radius: right-panel.layout-radius;
out property <length> layout-right-panel-border-width: right-panel.layout-border-width;
```

不要在本任务改视觉，只补 diagnostic surface，让后续 flat contract 可以被测试观察到。

**Step 4: Run test to verify it passes**

Run: `cargo test --test window_geometry_spec -q`  
Expected: PASS，新增测试通过，且现有 radius-related 测试仍保持绿色。

**Step 5: Commit**

```bash
git add ui/shell/titlebar.slint ui/shell/right-panel.slint ui/app-window.slint \
  tests/window_geometry_spec.rs
git commit -m "test: export shell chrome geometry diagnostics"
```

## Task 2: 将 Titlebar 收敛为 flat internal chrome

**Files:**
- Modify: `ui/app-window.slint`
- Modify: `ui/shell/titlebar.slint`
- Modify: `tests/window_geometry_spec.rs`
- Modify: `tests/top_status_bar_ui_contract_smoke.sh`

**Step 1: Write the failing tests**

先修改 `tests/window_geometry_spec.rs`，替换旧的标题栏圆角断言，并补一个 maximize 后的 flat contract：

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

然后修改 `tests/top_status_bar_ui_contract_smoke.sh`，追加新的静态 contract：

```bash
grep -F 'chrome-mask := Rectangle {' "$APP_WINDOW" >/dev/null
grep -F 'clip: true;' "$APP_WINDOW" >/dev/null
grep -F 'border-radius: 0px;' "$TITLEBAR" >/dev/null
grep -F 'border-width: 0px;' "$TITLEBAR" >/dev/null
! grep -F 'border-radius: root.use-flat-window-chrome ? 0px : 12px;' "$TITLEBAR" >/dev/null
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --test window_geometry_spec -q && bash tests/top_status_bar_ui_contract_smoke.sh`  
Expected: FAIL，`window_geometry_spec` 会看到标题栏仍是 `12px / 1px`，shell script 也会因为缺少 `chrome-mask` 或仍存在旧 rounded titlebar contract 而失败。

**Step 3: Write minimal implementation**

修改 `ui/shell/titlebar.slint`，把标题栏根节点改成 flat chrome：

```slint
height: 48px;
background: ThemeTokens.command-tint;
border-radius: 0px;
border-width: 0px;
border-color: transparent;
```

修改 `ui/app-window.slint`，在 `shell-frame` 内增加与外轮廓同半径的 `chrome-mask`，把 `Titlebar` 和 `body-host` 都挂到这个 mask 下，而不是直接画在 `shell-frame` 内：

```slint
shell-frame := Rectangle {
    width: root.width;
    height: root.height;
    border-radius: root.use-flat-window-chrome ? 0px : 14px;
    border-width: 1px;
    border-color: ThemeTokens.shell-stroke;
    background: ThemeTokens.shell-surface;

    chrome-mask := Rectangle {
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

实现时保持 `titlebar` 的布局、按钮、tooltip、drag zone、maximize button geometry export 不变，只改变 root geometry ownership。

**Step 4: Run tests to verify they pass**

Run: `cargo test --test window_geometry_spec -q && bash tests/top_status_bar_ui_contract_smoke.sh`  
Expected: PASS，说明窗口仍保留 outer radius，但标题栏已经不再拥有 rounded card 边框。

**Step 5: Commit**

```bash
git add ui/app-window.slint ui/shell/titlebar.slint \
  tests/window_geometry_spec.rs tests/top_status_bar_ui_contract_smoke.sh
git commit -m "feat: flatten titlebar chrome ownership"
```

## Task 3: 将 RightPanel 收敛为 square docked pane

**Files:**
- Modify: `ui/shell/right-panel.slint`
- Modify: `tests/window_geometry_spec.rs`
- Modify: `tests/shell_layout_ui_contract_smoke.sh`

**Step 1: Write the failing tests**

修改 `tests/window_geometry_spec.rs`，追加右侧 pane 的 flat contract：

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

修改 `tests/shell_layout_ui_contract_smoke.sh`，追加右侧 pane 的静态 contract：

```bash
grep -F 'left-divider := Rectangle {' "$RIGHT_PANEL" >/dev/null
grep -F 'border-radius: 0px;' "$RIGHT_PANEL" >/dev/null
grep -F 'border-width: 0px;' "$RIGHT_PANEL" >/dev/null
! grep -F 'border-radius: 14px;' "$RIGHT_PANEL" >/dev/null
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --test window_geometry_spec -q && bash tests/shell_layout_ui_contract_smoke.sh`  
Expected: FAIL，因为当前 `RightPanel` 还是 `14px` 圆角、`1px` 完整边框，并且没有明确的左侧 divider。

**Step 3: Write minimal implementation**

修改 `ui/shell/right-panel.slint`，把右侧 pane 改为 square docked pane，并用单独的左侧 divider 替代完整外框：

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

不要在本任务改变 `expanded` / width contract、right panel toggle 逻辑或内部控件模型。

**Step 4: Run tests to verify they pass**

Run: `cargo test --test window_geometry_spec -q && bash tests/shell_layout_ui_contract_smoke.sh`  
Expected: PASS，说明右侧 pane 已经变为 flat docked pane，且 seam ownership 从完整 card border 切换为单一 divider。

**Step 5: Commit**

```bash
git add ui/shell/right-panel.slint tests/window_geometry_spec.rs tests/shell_layout_ui_contract_smoke.sh
git commit -m "feat: flatten right panel shell seam"
```

## Task 4: 移除 Titlebar 上陈旧的 `use-flat-window-chrome` 依赖

**Files:**
- Modify: `ui/shell/titlebar.slint`
- Modify: `ui/app-window.slint`
- Modify: `tests/top_status_bar_smoke.rs`
- Modify: `tests/top_status_bar_ui_contract_smoke.sh`
- Modify: `tests/windows_frame_contract_smoke.sh`

**Step 1: Write the failing tests**

修改 `tests/top_status_bar_smoke.rs`，追加运行时回归测试：

```rust
#[test]
fn maximize_toggle_only_changes_outer_shell_chrome() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    assert_eq!(app.get_layout_titlebar_radius() as u32, 0);
    assert_eq!(app.get_layout_titlebar_border_width() as u32, 0);

    app.invoke_maximize_toggle_requested();
    assert_eq!(app.get_layout_shell_frame_radius() as u32, 0);
    assert_eq!(app.get_layout_titlebar_radius() as u32, 0);
    assert_eq!(app.get_layout_titlebar_border_width() as u32, 0);

    app.invoke_drag_double_clicked();
    assert_eq!(app.get_layout_shell_frame_radius() as u32, 14);
    assert_eq!(app.get_layout_titlebar_radius() as u32, 0);
    assert_eq!(app.get_layout_titlebar_border_width() as u32, 0);
}
```

修改 `tests/top_status_bar_ui_contract_smoke.sh`，要求删除陈旧 binding：

```bash
! grep -F 'in property <bool> use-flat-window-chrome: false;' "$TITLEBAR" >/dev/null
! grep -F 'use-flat-window-chrome: root.use-flat-window-chrome;' "$APP_WINDOW" >/dev/null
```

修改 `tests/windows_frame_contract_smoke.sh`，确保 frame adapter 继续只依赖 maximize button geometry，而不是标题栏自身 rounded card 语义：

```bash
! grep -F 'layout-titlebar-radius' "$FRAME_FILE" >/dev/null
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --test top_status_bar_smoke -q && bash tests/top_status_bar_ui_contract_smoke.sh && bash tests/windows_frame_contract_smoke.sh`  
Expected: FAIL，shell scripts 会先暴露 `Titlebar` 仍声明 `use-flat-window-chrome` 输入或 `AppWindow` 仍把该状态传给 `Titlebar`。

**Step 3: Write minimal implementation**

修改 `ui/shell/titlebar.slint`，删除已经失效的输入属性：

```slint
- in property <bool> use-flat-window-chrome: false;
```

修改 `ui/app-window.slint`，删除给 `Titlebar` 的旧 binding，只保留 `shell-frame` 对 `use-flat-window-chrome` 的使用：

```slint
titlebar := Titlebar {
    dark-mode: root.dark-mode;
    show-right-panel: root.show-right-panel;
    show-global-menu: root.show-global-menu;
    is-window-maximized: root.is-window-maximized;
    is-window-active: root.is-window-active;
    is-window-always-on-top: root.is-window-always-on-top;
    ...
}
```

不要改 `src/app/bootstrap.rs` 中的 `window.set_use_flat_window_chrome(...)`，因为窗口级 outer geometry 仍然需要这个状态。

**Step 4: Run tests to verify they pass**

Run: `cargo test --test top_status_bar_smoke -q && bash tests/top_status_bar_ui_contract_smoke.sh && bash tests/windows_frame_contract_smoke.sh`  
Expected: PASS，说明 flat-chrome state 已收敛到窗口 outer geometry，标题栏不会再被误绑定成“随窗口状态切换圆角”的内部 card。

**Step 5: Commit**

```bash
git add ui/shell/titlebar.slint ui/app-window.slint \
  tests/top_status_bar_smoke.rs tests/top_status_bar_ui_contract_smoke.sh \
  tests/windows_frame_contract_smoke.sh
git commit -m "refactor: scope flat chrome state to shell frame"
```

## Final Verification

按以下顺序执行完整验证：

1. `cargo fmt --all`
2. `cargo test --test window_geometry_spec --test top_status_bar_smoke --test titlebar_render_spec --test windows_frame_spec -q`
3. `bash tests/top_status_bar_ui_contract_smoke.sh`
4. `bash tests/shell_layout_ui_contract_smoke.sh`
5. `bash tests/windows_frame_contract_smoke.sh`
6. `cargo check --workspace -q`
7. `cargo clippy --workspace --all-targets -- -D warnings`

Expected:

- 所有 geometry / smoke / frame adapter 回归测试通过
- `shell-frame` 仍保持 restored rounded / maximized flat 语义
- `Titlebar` 与 `RightPanel` 根节点都不再持有 rounded card contract
- maximize button geometry export、window drag/resize 相关 contract 没有回归

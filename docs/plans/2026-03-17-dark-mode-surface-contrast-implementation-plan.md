# Mica Term Dark Mode Surface Contrast Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 基于已确认的 `1B + 2B + 3B + 4A` 方案，建立语义化 surface token ladder，修复暗色模式各模块对比度不足的问题，并让亮色模式沿用同一套 semantic mapping 完成对齐。

**Architecture:** 保持当前 `AppWindow -> Titlebar / Sidebar / Main Workspace / RightPanel` 布局、`MicaAlt` 窗口基础层、theme toggle 同步链路与 `femtovg-wgpu` 主线不变，只重做 surface hierarchy。先通过新的 `ThemeTokens` 语义层定义 `window/titlebar/activity/assets/workspace/inspector/divider/control` 阶梯，再把大面板与共享控件逐步迁移过去，最后删除旧的泛化 surface alias，并通过 smoke contract、window/theme regression、assets/sidebar regression 完成验证。

**Tech Stack:** Rust 2024, Slint 1.15.1, `femtovg-wgpu`, Bash smoke contracts, `i-slint-backend-testing`, `cargo test`, `cargo check`

---

## Execution Notes

- 我正在使用 `writing-plans` skill 来创建 implementation plan。
- 设计输入固定为 `docs/plans/2026-03-17-dark-mode-surface-contrast-design.md`；实施时不得偏离已确认方案：
  - `1B` semantic surface mapping
  - `2B` hybrid boundary expression
  - `3B` 仅 outer chrome / titlebar 保留 Mica 气质，内部 pane 使用稳定近不透明 neutral surface
  - `4A` `Titlebar` 最亮、`RightPanel` 次亮、`AssetsSidebar` 居中、`Activity Bar` 稍深、`Workspace` 最深
- 整个执行过程必须遵循 `@superpowers:test-driven-development`：先写失败 contract，再做最小实现，再跑通过。
- 如果 `Slint` 的颜色传播、alpha 合成或 hover/active 结果与预期不一致，立即切到 `@superpowers:systematic-debugging`，不要靠肉眼猜。
- 完成前必须执行 `@superpowers:verification-before-completion`，不能只看静态代码就宣称完成。
- 本轮不允许改动：
  - terminal runtime / `wezterm-term` / `termwiz`
  - SSH / SFTP
  - renderer 选择
  - 窗口几何与尺寸策略
  - Win32 `MicaAlt` / theme sync 链路

## Current Code Map

- `ui/theme/tokens.slint`
  - 当前只提供 `shell-surface / shell-stroke / command-tint / panel-tint / terminal-surface / accent / text-primary`
  - 还没有语义化 surface ladder
- `ui/shell/titlebar.slint`
  - 当前 `Titlebar` 背景使用 `ThemeTokens.command-tint`
- `ui/shell/sidebar.slint`
  - 当前 `Activity Bar` 使用 `ThemeTokens.shell-surface`
- `ui/shell/assets-sidebar.slint`
  - 当前 `AssetsSidebar` 使用 `ThemeTokens.shell-surface`
- `ui/app-window.slint`
  - 当前 `main-workspace` 使用 `ThemeTokens.terminal-surface`
- `ui/shell/right-panel.slint`
  - 当前 `RightPanel` 使用 `ThemeTokens.panel-tint`
- `ui/components/titlebar-icon-button.slint`
- `ui/components/window-control-button.slint`
- `ui/components/sidebar-nav-button.slint`
- `ui/components/sidebar-toolbar-icon-button.slint`
- `ui/components/titlebar-menu.slint`
- `ui/components/titlebar-tooltip.slint`
- `ui/components/assets-create-menu.slint`
- `ui/components/assets-toolbar-menu-row.slint`
- `ui/components/assets-search-popover.slint`
- `ui/components/segmented-control.slint`
- `ui/components/command-entry.slint`
- `ui/components/command-palette.slint`
- `ui/components/status-pill.slint`
  - 以上共享控件仍在使用旧的 `command-tint / panel-tint / shell-surface / shell-stroke`
- 现有可复用验证：
  - `tests/top_status_bar_ui_contract_smoke.sh`
  - `tests/assets_sidebar_toolbar_ui_contract_smoke.sh`
  - `tests/sidebar_ui_contract_smoke.sh`
  - `tests/shell_layout_ui_contract_smoke.sh`
  - `tests/window_theme_contract_smoke.sh`
  - `tests/window_shell.rs`
  - `tests/top_status_bar_smoke.rs`
  - `tests/window_effects.rs`
  - `tests/ui_preferences.rs`
  - `tests/assets_sidebar_toolbar_spec.rs`
  - `tests/assets_sidebar_toolbar_smoke.rs`

## Target Token Map

后续实现固定收敛到以下语义 token：

- `window-surface`
- `titlebar-surface`
- `activity-surface`
- `assets-surface`
- `workspace-surface`
- `inspector-surface`
- `divider-subtle`
- `divider-strong`
- `control-hover-surface`
- `control-active-surface`
- `accent`
- `text-primary`

建议起始值固定为：

```slint
out property <brush> window-surface: dark-mode ? #171a20 : #f4f6fa;
out property <brush> titlebar-surface: dark-mode ? #202734ee : #edf3fbea;
out property <brush> activity-surface: dark-mode ? #14181f : #eef2f7;
out property <brush> assets-surface: dark-mode ? #1a2029 : #f7f9fc;
out property <brush> workspace-surface: dark-mode ? #101419 : #ffffff;
out property <brush> inspector-surface: dark-mode ? #1e2632 : #e9eef7;
out property <brush> divider-subtle: dark-mode ? #ffffff14 : #0f172a12;
out property <brush> divider-strong: dark-mode ? #ffffff22 : #0f172a1e;
out property <brush> control-hover-surface: dark-mode ? #232c39 : #eef3fb;
out property <brush> control-active-surface: dark-mode ? #283240 : #e7edf7;
out property <brush> accent: #4ea1ff;
out property <brush> text-primary: dark-mode ? #f5f7fb : #101418;
```

原则：

- `Titlebar` 保持最亮 chrome
- `RightPanel` 次亮
- `AssetsSidebar` 介于 `RightPanel` 与 `Activity Bar` 之间
- `Activity Bar` 比 `AssetsSidebar` 更深
- `Workspace` 最深 / light mode 最接近纯白
- 所有大面板维持同一冷中性色家族，不允许品牌蓝化

## Task 1: Lock The Semantic Surface Contract Before Any UI Changes

**Files:**
- Create: `tests/theme_surface_contract_smoke.sh`
- Modify: `tests/window_shell.rs`

**Step 1: Write the failing shell contract**

新建 `tests/theme_surface_contract_smoke.sh`，先把已确认的 token 与一级面板映射锁成失败契约：

```bash
#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TOKENS="$ROOT_DIR/ui/theme/tokens.slint"
TITLEBAR="$ROOT_DIR/ui/shell/titlebar.slint"
SIDEBAR="$ROOT_DIR/ui/shell/sidebar.slint"
ASSETS="$ROOT_DIR/ui/shell/assets-sidebar.slint"
APP_WINDOW="$ROOT_DIR/ui/app-window.slint"
RIGHT_PANEL="$ROOT_DIR/ui/shell/right-panel.slint"

for token in \
  'out property <brush> window-surface:' \
  'out property <brush> titlebar-surface:' \
  'out property <brush> activity-surface:' \
  'out property <brush> assets-surface:' \
  'out property <brush> workspace-surface:' \
  'out property <brush> inspector-surface:' \
  'out property <brush> divider-subtle:' \
  'out property <brush> divider-strong:' \
  'out property <brush> control-hover-surface:' \
  'out property <brush> control-active-surface:'
do
  grep -F "$token" "$TOKENS" >/dev/null
done

grep -F 'background: ThemeTokens.titlebar-surface;' "$TITLEBAR" >/dev/null
grep -F 'background: ThemeTokens.activity-surface;' "$SIDEBAR" >/dev/null
grep -F 'background: ThemeTokens.assets-surface;' "$ASSETS" >/dev/null
grep -F 'background: ThemeTokens.workspace-surface;' "$APP_WINDOW" >/dev/null
grep -F 'background: ThemeTokens.inspector-surface;' "$RIGHT_PANEL" >/dev/null
grep -F 'border-color: ThemeTokens.divider-subtle;' "$APP_WINDOW" >/dev/null
grep -F 'background: ThemeTokens.divider-strong;' "$RIGHT_PANEL" >/dev/null
```

不要在这一阶段写任何 UI 实现，只写 contract。

**Step 2: Add a focused Rust contract to `tests/window_shell.rs`**

在 `tests/window_shell.rs` 追加一个只读取 `ui/theme/tokens.slint` 的源码契约：

```rust
#[test]
fn semantic_surface_tokens_define_dual_theme_ladder() {
    let content = std::fs::read_to_string("ui/theme/tokens.slint").unwrap();

    for token in [
        "window-surface",
        "titlebar-surface",
        "activity-surface",
        "assets-surface",
        "workspace-surface",
        "inspector-surface",
        "divider-subtle",
        "divider-strong",
        "control-hover-surface",
        "control-active-surface",
    ] {
        assert!(
            content.contains(token),
            "missing semantic token in ui/theme/tokens.slint: {token}"
        );
    }
}
```

**Step 3: Run the new contract to verify it fails**

Run:

```bash
bash tests/theme_surface_contract_smoke.sh
cargo test --test window_shell semantic_surface_tokens_define_dual_theme_ladder -- --nocapture
```

Expected:

- shell script FAIL，因为这些 token 和映射当前都不存在
- Rust test FAIL，因为 `ui/theme/tokens.slint` 还没有 semantic ladder

**Step 4: Do not implement yet**

这一 task 只负责写失败 contract。

**Step 5: Commit**

```bash
git add tests/theme_surface_contract_smoke.sh tests/window_shell.rs
git commit -m "test: lock semantic surface hierarchy contract"
```

## Task 2: Implement The Surface Ladder And Remap Primary Shell Regions

**Files:**
- Modify: `ui/theme/tokens.slint`
- Modify: `ui/shell/titlebar.slint`
- Modify: `ui/shell/sidebar.slint`
- Modify: `ui/shell/assets-sidebar.slint`
- Modify: `ui/app-window.slint`
- Modify: `ui/shell/right-panel.slint`
- Modify: `ui/shell/tabbar.slint`
- Test: `tests/theme_surface_contract_smoke.sh`
- Test: `tests/top_status_bar_ui_contract_smoke.sh`
- Test: `tests/sidebar_ui_contract_smoke.sh`
- Test: `tests/shell_layout_ui_contract_smoke.sh`

**Step 1: Implement the token ladder in `ui/theme/tokens.slint`**

把 `ui/theme/tokens.slint` 中旧的大面板 token 替换为语义 token；保留 `accent` 和 `text-primary`，不要引入额外 palette。

最小实现从下面这段开始：

```slint
export global ThemeTokens {
    in property <bool> dark-mode: true;

    out property <brush> window-surface: dark-mode ? #171a20 : #f4f6fa;
    out property <brush> titlebar-surface: dark-mode ? #202734ee : #edf3fbea;
    out property <brush> activity-surface: dark-mode ? #14181f : #eef2f7;
    out property <brush> assets-surface: dark-mode ? #1a2029 : #f7f9fc;
    out property <brush> workspace-surface: dark-mode ? #101419 : #ffffff;
    out property <brush> inspector-surface: dark-mode ? #1e2632 : #e9eef7;
    out property <brush> divider-subtle: dark-mode ? #ffffff14 : #0f172a12;
    out property <brush> divider-strong: dark-mode ? #ffffff22 : #0f172a1e;
    out property <brush> control-hover-surface: dark-mode ? #232c39 : #eef3fb;
    out property <brush> control-active-surface: dark-mode ? #283240 : #e7edf7;
    out property <brush> accent: #4ea1ff;
    out property <brush> text-primary: dark-mode ? #f5f7fb : #101418;
}
```

此任务先不要删除旧 token alias；如果迁移过程中需要短暂并存，可以保留到 Task 4 再统一清理。

**Step 2: Remap the five primary regions**

把大面板映射改成设计里确认的 hierarchy：

- `ui/app-window.slint`
  - `shell-frame.background -> ThemeTokens.window-surface`
  - `shell-frame.border-color -> ThemeTokens.divider-subtle`
  - `main-workspace.background -> ThemeTokens.workspace-surface`
- `ui/shell/titlebar.slint`
  - `background -> ThemeTokens.titlebar-surface`
  - `divider-line.background -> ThemeTokens.divider-subtle`
- `ui/shell/sidebar.slint`
  - `activity-bar.background -> ThemeTokens.activity-surface`
  - `activity-bar.border-color -> ThemeTokens.divider-subtle`
  - `divider-line.background -> ThemeTokens.divider-subtle`
- `ui/shell/assets-sidebar.slint`
  - `background -> ThemeTokens.assets-surface`
  - `border-color -> ThemeTokens.divider-subtle`
- `ui/shell/right-panel.slint`
  - `background -> ThemeTokens.inspector-surface`
  - `left-divider.background -> ThemeTokens.divider-strong`
- `ui/shell/tabbar.slint`
  - `border-color -> ThemeTokens.divider-subtle`

本任务只改“大面板与主 divider”，不要提前改按钮 hover/active。

**Step 3: Run targeted tests to verify the region remap passes**

Run:

```bash
bash tests/theme_surface_contract_smoke.sh
bash tests/top_status_bar_ui_contract_smoke.sh
bash tests/sidebar_ui_contract_smoke.sh
bash tests/shell_layout_ui_contract_smoke.sh
cargo test --test window_shell balanced_desktop_metrics_match_the_design_doc -- --nocapture
cargo test --test window_shell semantic_surface_tokens_define_dual_theme_ladder -- --nocapture
```

Expected:

- `theme_surface_contract_smoke.sh` PASS
- 现有 titlebar/sidebar/shell-layout contract 仍 PASS
- `window_shell` 现有 metrics / appearance 相关测试不退化

**Step 4: Commit**

```bash
git add ui/theme/tokens.slint ui/shell/titlebar.slint ui/shell/sidebar.slint \
  ui/shell/assets-sidebar.slint ui/app-window.slint ui/shell/right-panel.slint \
  ui/shell/tabbar.slint tests/theme_surface_contract_smoke.sh tests/window_shell.rs
git commit -m "feat: add semantic surface ladder for shell regions"
```

## Task 3: Migrate Shared Controls, Menus, And Popovers To The New Ladder

**Files:**
- Modify: `ui/components/titlebar-icon-button.slint`
- Modify: `ui/components/window-control-button.slint`
- Modify: `ui/components/sidebar-nav-button.slint`
- Modify: `ui/components/sidebar-toolbar-icon-button.slint`
- Modify: `ui/components/titlebar-menu.slint`
- Modify: `ui/components/titlebar-tooltip.slint`
- Modify: `ui/components/assets-create-menu.slint`
- Modify: `ui/components/assets-toolbar-menu-row.slint`
- Modify: `ui/components/assets-search-popover.slint`
- Modify: `ui/components/segmented-control.slint`
- Modify: `ui/components/command-entry.slint`
- Modify: `ui/components/command-palette.slint`
- Modify: `ui/components/status-pill.slint`
- Modify: `tests/top_status_bar_ui_contract_smoke.sh`
- Modify: `tests/assets_sidebar_toolbar_ui_contract_smoke.sh`
- Modify: `tests/sidebar_ui_contract_smoke.sh`

**Step 1: Extend existing UI contract scripts so they fail first**

在 `tests/top_status_bar_ui_contract_smoke.sh` 追加：

```bash
grep -F 'ThemeTokens.control-hover-surface' "$ROOT_DIR/ui/components/titlebar-icon-button.slint" >/dev/null
grep -F 'ThemeTokens.control-active-surface' "$ROOT_DIR/ui/components/titlebar-icon-button.slint" >/dev/null
grep -F 'ThemeTokens.control-hover-surface' "$ROOT_DIR/ui/components/window-control-button.slint" >/dev/null
grep -F 'ThemeTokens.control-active-surface' "$ROOT_DIR/ui/components/window-control-button.slint" >/dev/null
grep -F 'background: ThemeTokens.inspector-surface;' "$ROOT_DIR/ui/components/titlebar-menu.slint" >/dev/null
grep -F 'background: ThemeTokens.inspector-surface;' "$ROOT_DIR/ui/components/titlebar-tooltip.slint" >/dev/null
grep -F 'border-color: ThemeTokens.divider-strong;' "$ROOT_DIR/ui/components/titlebar-menu.slint" >/dev/null
```

在 `tests/assets_sidebar_toolbar_ui_contract_smoke.sh` 追加：

```bash
grep -F 'ThemeTokens.control-hover-surface' "$BUTTON" >/dev/null
grep -F 'ThemeTokens.control-active-surface' "$BUTTON" >/dev/null
grep -F 'ThemeTokens.control-hover-surface' "$ROW" >/dev/null
grep -F 'ThemeTokens.control-active-surface' "$ROW" >/dev/null
grep -F 'background: ThemeTokens.inspector-surface;' "$MENU" >/dev/null
grep -F 'background: ThemeTokens.inspector-surface;' "$SEARCH" >/dev/null
grep -F 'border-color: ThemeTokens.divider-strong;' "$MENU" >/dev/null
grep -F 'border-color: ThemeTokens.divider-subtle;' "$SEARCH" >/dev/null
```

在 `tests/sidebar_ui_contract_smoke.sh` 追加：

```bash
grep -F 'ThemeTokens.control-hover-surface' "$BUTTON" >/dev/null
grep -F 'ThemeTokens.control-active-surface' "$BUTTON" >/dev/null
```

**Step 2: Run the updated contract scripts to verify they fail**

Run:

```bash
bash tests/top_status_bar_ui_contract_smoke.sh
bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh
bash tests/sidebar_ui_contract_smoke.sh
```

Expected:

- FAIL，因为共享控件当前还在引用 `shell-surface / panel-tint / command-tint / shell-stroke`

**Step 3: Implement the minimal shared-control migration**

采用下面这套最小映射，不要再引入新的 menu/tooltip 专用 token：

- 所有 hover/active 按钮：
  - `touch.has-hover || root.active -> ThemeTokens.control-hover-surface`
  - `touch.pressed -> ThemeTokens.control-active-surface`
- 所有浮层容器：
  - `TitlebarMenu` / `AssetsCreateMenu` / `TitlebarTooltip` / `SegmentedControl` / `CommandEntry` / `CommandPalette` / `StatusPill` 优先用 `ThemeTokens.inspector-surface`
- Search 输入框容器：
  - `AssetsSearchPopover.field-frame.background -> ThemeTokens.inspector-surface`
  - `field-frame.border-color -> ThemeTokens.divider-subtle`
  - `glow-frame.border-color -> ThemeTokens.accent`
- 强边界：
  - menu / tooltip / create popover 外框优先 `ThemeTokens.divider-strong`

参考实现片段：

```slint
background: touch.pressed
    ? ThemeTokens.control-active-surface
    : (touch.has-hover || root.active)
        ? ThemeTokens.control-hover-surface
        : transparent;
```

```slint
background-layer := Rectangle {
    border-width: 1px;
    border-color: ThemeTokens.divider-strong;
    background: ThemeTokens.inspector-surface;
}
```

```slint
field-frame := Rectangle {
    border-width: 1px;
    border-color: ThemeTokens.divider-subtle;
    background: ThemeTokens.inspector-surface;
}
```

危险按钮（close button）的红色 danger 路径继续保留，不要把 danger 也替换成 neutral hover/active。

**Step 4: Run focused tests to verify the shared-control migration passes**

Run:

```bash
bash tests/top_status_bar_ui_contract_smoke.sh
bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh
bash tests/sidebar_ui_contract_smoke.sh
cargo test --test top_status_bar_smoke bootstrap_binds_top_status_bar_callbacks_to_window_state -- --nocapture
cargo test --test assets_sidebar_toolbar_spec -- --nocapture
cargo test --test assets_sidebar_toolbar_smoke -- --nocapture
```

Expected:

- 三个 UI contract smoke 全部 PASS
- 现有 titlebar 绑定、assets toolbar 状态语义、search / create menu 相关 smoke 全部 PASS

**Step 5: Commit**

```bash
git add ui/components/titlebar-icon-button.slint ui/components/window-control-button.slint \
  ui/components/sidebar-nav-button.slint ui/components/sidebar-toolbar-icon-button.slint \
  ui/components/titlebar-menu.slint ui/components/titlebar-tooltip.slint \
  ui/components/assets-create-menu.slint ui/components/assets-toolbar-menu-row.slint \
  ui/components/assets-search-popover.slint ui/components/segmented-control.slint \
  ui/components/command-entry.slint ui/components/command-palette.slint \
  ui/components/status-pill.slint tests/top_status_bar_ui_contract_smoke.sh \
  tests/assets_sidebar_toolbar_ui_contract_smoke.sh tests/sidebar_ui_contract_smoke.sh
git commit -m "fix: align shared chrome controls with semantic surface tokens"
```

## Task 4: Remove Obsolete Generic Surface Aliases, Lock Final Values, And Verify Everything

**Files:**
- Modify: `ui/theme/tokens.slint`
- Modify: `tests/theme_surface_contract_smoke.sh`
- Modify: `tests/window_shell.rs`
- Test: `tests/window_theme_contract_smoke.sh`
- Test: `tests/top_status_bar_smoke.rs`
- Test: `tests/window_effects.rs`
- Test: `tests/ui_preferences.rs`
- Test: `tests/assets_sidebar_toolbar_spec.rs`
- Test: `tests/assets_sidebar_toolbar_smoke.rs`
- Test: `tests/shell_layout_policy.rs`

**Step 1: Tighten the final contract so it fails before cleanup**

把 `tests/theme_surface_contract_smoke.sh` 再补两组最终约束：

```bash
if rg -n 'ThemeTokens\.(shell-surface|shell-stroke|command-tint|panel-tint|terminal-surface)' "$ROOT_DIR/ui" >/dev/null; then
  echo "obsolete generic surface token reference remains under ui/" >&2
  exit 1
fi

if rg -n 'out property <brush> (shell-surface|shell-stroke|command-tint|panel-tint|terminal-surface)' "$TOKENS" >/dev/null; then
  echo "obsolete generic surface token alias remains in ui/theme/tokens.slint" >&2
  exit 1
fi
```

再给 `tests/window_shell.rs` 加一个锁定最终值的源码契约：

```rust
#[test]
fn semantic_surface_tokens_lock_the_approved_dual_theme_values() {
    let content = std::fs::read_to_string("ui/theme/tokens.slint").unwrap();

    for line in [
        "out property <brush> window-surface: dark-mode ? #171a20 : #f4f6fa;",
        "out property <brush> titlebar-surface: dark-mode ? #202734ee : #edf3fbea;",
        "out property <brush> activity-surface: dark-mode ? #14181f : #eef2f7;",
        "out property <brush> assets-surface: dark-mode ? #1a2029 : #f7f9fc;",
        "out property <brush> workspace-surface: dark-mode ? #101419 : #ffffff;",
        "out property <brush> inspector-surface: dark-mode ? #1e2632 : #e9eef7;",
        "out property <brush> divider-subtle: dark-mode ? #ffffff14 : #0f172a12;",
        "out property <brush> divider-strong: dark-mode ? #ffffff22 : #0f172a1e;",
    ] {
        assert!(content.contains(line), "missing approved token value: {line}");
    }
}
```

**Step 2: Run the final contract to verify it fails before cleanup**

Run:

```bash
bash tests/theme_surface_contract_smoke.sh
cargo test --test window_shell semantic_surface_tokens_lock_the_approved_dual_theme_values -- --nocapture
```

Expected:

- FAIL，如果旧 alias 仍然残留
- FAIL，如果 token 值或 alpha 没有收敛到设计确认版本

**Step 3: Remove the obsolete aliases and normalize the final token values**

在 `ui/theme/tokens.slint` 中：

- 删除 `shell-surface`
- 删除 `shell-stroke`
- 删除 `command-tint`
- 删除 `panel-tint`
- 删除 `terminal-surface`
- 确认所有 UI 文件都只使用新的 semantic token
- 不新增第二套 light-mode 专用逻辑；所有 light values 只存在于同一组 token 的 `dark-mode ? ... : ...` 表达式里

**Step 4: Run the full verification suite**

Run:

```bash
bash tests/theme_surface_contract_smoke.sh
bash tests/top_status_bar_ui_contract_smoke.sh
bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh
bash tests/sidebar_ui_contract_smoke.sh
bash tests/shell_layout_ui_contract_smoke.sh
bash tests/window_theme_contract_smoke.sh
cargo test --test window_shell --test top_status_bar_smoke --test window_effects --test ui_preferences --test shell_layout_policy -q
cargo test --test assets_sidebar_toolbar_spec --test assets_sidebar_toolbar_smoke -q
cargo check -q
```

Expected:

- 所有 smoke contract PASS
- theme sync、window effects、ui preferences、layout policy、assets toolbar regression 全部 PASS
- `cargo check -q` PASS

**Step 5: Commit**

```bash
git add ui/theme/tokens.slint tests/theme_surface_contract_smoke.sh tests/window_shell.rs
git commit -m "refactor: retire generic shell surface aliases"
```

## Final Verification Checklist

- [ ] 暗色模式下 `Activity Bar` 与 `AssetsSidebar` 颜色台阶明确，不再共用同一层背景
- [ ] 暗色模式下 `Workspace` 是最深主内容层
- [ ] `Titlebar` 比 `RightPanel` 更像 chrome，但 `RightPanel` 仍具 inspector 提升感
- [ ] 亮色模式沿用同一语义映射，不存在第二套随意浅色逻辑
- [ ] 所有 hover / active / menu / tooltip / search field 都已迁移到 semantic token ladder
- [ ] `ThemeMode` 切换、`MicaAlt`、Win32 `corner_preference`、UI preferences、shell layout policy 不回归
- [ ] `ui/` 下不再残留 `ThemeTokens.shell-surface / shell-stroke / command-tint / panel-tint / terminal-surface`

## Handoff Commands

推荐实现顺序：

```bash
bash tests/theme_surface_contract_smoke.sh
cargo test --test window_shell semantic_surface_tokens_define_dual_theme_ladder -- --nocapture
# 实现 Task 2
bash tests/top_status_bar_ui_contract_smoke.sh
bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh
bash tests/sidebar_ui_contract_smoke.sh
# 实现 Task 3
bash tests/window_theme_contract_smoke.sh
cargo test --test window_shell --test top_status_bar_smoke --test window_effects --test ui_preferences --test shell_layout_policy -q
cargo test --test assets_sidebar_toolbar_spec --test assets_sidebar_toolbar_smoke -q
cargo check -q
```


# Assets Sidebar Toolbar Bugfix Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 按已确认的 `S1B + S2B + S3A + S4B + S5B` 方案，完成 `AssetsSidebar` 顶部工具区 bugfix：Search 改为 anchored overlay、空值 outside-click 收起、`Create` 改为纯 icon trigger、菜单几何收敛，并把 sidebar 宽度提升到 `288px`。

**Architecture:** 保持现有 `AppWindow -> Sidebar -> AssetsSidebar` 的壳层结构不变。Search 与 `Create` 都改为根窗口锚定 overlay 路线，其中 Search 采用根层可见组件而不是 `AssetsSidebar` 内联行，`Create` 继续使用根层 `PopupWindow` 但改为受根层 dismiss 逻辑控制。Rust `ShellViewModel` 继续作为 `asset_search_expanded`、`assets_search_query`、`asset_create_menu_open` 的唯一业务状态源，Slint 负责锚点几何、焦点与展示。

**Tech Stack:** Rust 2024, Slint 1.15.1, `PopupWindow`, `TextInput`, root overlay `TouchArea`, `i-slint-backend-testing`, shell smoke scripts, `cargo test`, `cargo check`

---

## Execution Notes

- 设计输入固定为 [2026-03-16-assets-sidebar-toolbar-bugfix-design.md](/home/wwwroot/mica-term/docs/plans/2026-03-16-assets-sidebar-toolbar-bugfix-design.md)，实现时不得回退到旧方案的 `inline search row + Create 文字按钮`。
- 每个任务都先走 `@superpowers:test-driven-development`：先写失败测试或失败 smoke，再做最小实现，再跑通过。
- 如果 Search overlay 的 click-away、焦点恢复、或 `PopupWindow` 几何行为与预期不一致，立即切到 `@superpowers:systematic-debugging`，不要猜。
- 本轮不接入真实 terminal / SSH / SFTP runtime，不实现真实搜索过滤、创建向导、树节点数据，也不改 renderer。
- 默认在独立 worktree 执行；如果继续在当前工作区执行，改动范围必须严格限制在本计划列出的文件。

## Current Code Map

- `ui/shell/assets-sidebar.slint`
  - 当前 Search 仍是 `if root.asset-search-expanded : Rectangle` 的内联行
  - 当前 `Create` 仍是 `Add + Create + Chevron` 的 104px 复合按钮
  - 当前 sidebar 宽度仍是 `256px`
- `ui/shell/sidebar.slint`
  - 已经具备 tooltip / create-menu anchor 的透传模式，可作为 search anchor 的直接参考
- `ui/app-window.slint`
  - 当前拥有根层 `assets-create-menu-overlay`
  - 适合新增 search overlay 与共享 dismiss layer
- `ui/components/assets-create-menu.slint`
  - 当前是 `PopupWindow`
  - 行布局已经是 `Image + Text`，但几何宽度与关闭策略仍需调整
- `src/shell/view_model.rs`
  - 已经具备 `asset_search_expanded`、`assets_search_query`、`asset_create_menu_open`
  - 已有 `collapse_asset_search_if_empty()`，这正是 `S2B` 的状态基础
- `src/shell/metrics.rs` 与 `src/shell/layout.rs`
  - 仍锁定 `ASSETS_SIDEBAR_WIDTH = 256`
- `tests/assets_sidebar_toolbar_ui_contract_smoke.sh`
  - 仍包含旧方案契约，需要改成 overlay / icon-only / 288px 路线

## Task 1: Raise Sidebar Width Contract To `288px`

**Files:**
- Modify: `src/shell/metrics.rs`
- Modify: `src/shell/layout.rs`
- Modify: `ui/shell/sidebar.slint`
- Modify: `ui/shell/assets-sidebar.slint`
- Modify: `tests/window_shell.rs`

**Step 1: Write the failing width assertions**

先在 `tests/window_shell.rs` 把现有 `256` 预算改成新设计的 `288`：

```rust
#[test]
fn balanced_desktop_metrics_match_the_design_doc() {
    assert_eq!(ShellMetrics::ASSETS_SIDEBAR_WIDTH, 288);
}

#[test]
fn sidebar_metrics_match_the_navigation_design() {
    assert_eq!(ShellMetrics::ASSETS_SIDEBAR_WIDTH, 288);
}

#[test]
fn shell_layout_metrics_match_the_layout_bugfix_budget() {
    assert_eq!(ShellMetrics::ASSETS_SIDEBAR_WIDTH, 288);
}
```

**Step 2: Run the focused test to verify it fails**

Run:

```bash
cargo test --test window_shell -- --nocapture
```

Expected: FAIL，断言仍然拿到 `256`。

**Step 3: Write the minimal width implementation**

同步更新四处宽度契约：

1. `src/shell/metrics.rs`

```rust
pub const ASSETS_SIDEBAR_WIDTH: u32 = 288;
```

2. `src/shell/layout.rs`

- 不新增分支逻辑
- 只让 `FULL_LAYOUT_MIN_WIDTH` 自然引用新的 `ASSETS_SIDEBAR_WIDTH`

3. `ui/shell/sidebar.slint`

```slint
width: 48px + (root.show-assets-sidebar ? 288px : 0px);
```

4. `ui/shell/assets-sidebar.slint`

```slint
width: expanded ? 288px : 0px;
```

**Step 4: Re-run the focused test**

Run:

```bash
cargo test --test window_shell -- --nocapture
```

Expected: PASS。

**Step 5: Commit**

```bash
git add src/shell/metrics.rs src/shell/layout.rs ui/shell/sidebar.slint ui/shell/assets-sidebar.slint tests/window_shell.rs
git commit -m "fix: widen assets sidebar budget"
```

## Task 2: Replace Inline Search Row With Root-Anchored Search Overlay

**Files:**
- Create: `ui/components/assets-search-popover.slint`
- Modify: `ui/shell/assets-sidebar.slint`
- Modify: `ui/shell/sidebar.slint`
- Modify: `ui/app-window.slint`
- Modify: `tests/assets_sidebar_toolbar_ui_contract_smoke.sh`
- Modify: `tests/assets_sidebar_toolbar_smoke.rs`

**Step 1: Add the failing UI contract and smoke tests**

先把 `tests/assets_sidebar_toolbar_ui_contract_smoke.sh` 改成 search overlay 路线：

```bash
grep -F 'import { AssetsSearchPopover } from "components/assets-search-popover.slint";' "$APP_WINDOW" >/dev/null
grep -F 'assets-search-overlay := AssetsSearchPopover {' "$APP_WINDOW" >/dev/null
grep -F 'out property <length> search-anchor-x' "$ASSETS" >/dev/null
grep -F 'out property <length> search-anchor-y' "$ASSETS" >/dev/null
grep -F 'out property <length> search-anchor-width' "$ASSETS" >/dev/null
grep -F 'out property <length> search-anchor-height' "$ASSETS" >/dev/null
grep -F 'out property <length> search-anchor-x' "$SIDEBAR" >/dev/null
grep -F 'out property <length> search-anchor-y' "$SIDEBAR" >/dev/null
grep -F 'out property <length> search-anchor-width' "$SIDEBAR" >/dev/null
grep -F 'out property <length> search-anchor-height' "$SIDEBAR" >/dev/null
! grep -F 'if root.asset-search-expanded : Rectangle' "$ASSETS" >/dev/null
```

在 `tests/assets_sidebar_toolbar_smoke.rs` 里新增根窗口 search anchor getter 检查：

```rust
#[test]
fn assets_search_anchor_is_exposed_at_root_window() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    assert!(app.get_layout_assets_search_anchor_width() > 0.0);
    assert!(app.get_layout_assets_search_anchor_height() > 0.0);
}
```

**Step 2: Run tests to verify they fail**

Run:

```bash
bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh
cargo test --test assets_sidebar_toolbar_smoke assets_search_anchor_is_exposed_at_root_window -- --nocapture
```

Expected:

- shell contract FAIL，因为当前仍然是 inline row
- Rust smoke FAIL，因为 `AppWindow` 还没有 search anchor getter

**Step 3: Write the minimal root-overlay implementation**

1. 创建 `ui/components/assets-search-popover.slint`

建议结构：

```slint
import { ThemeTokens } from "../theme/tokens.slint";

export component AssetsSearchPopover inherits Rectangle {
    in property <string> query: "";
    callback query-changed(string);

    width: 272px;
    height: 36px;
    border-width: 1px;
    border-color: ThemeTokens.shell-stroke;
    background: ThemeTokens.panel-tint;

    public function focus-input() {
        search-input.focus();
    }

    search-input := TextInput {
        x: 10px;
        y: 6px;
        width: parent.width - 20px;
        height: parent.height - 12px;
        text: root.query;
        edited => { root.query-changed(self.text); }
    }
}
```

2. `ui/shell/assets-sidebar.slint`

- 删除整个 inline search row
- 暴露 search button 几何：

```slint
out property <length> search-anchor-x: search-button.absolute-position.x;
out property <length> search-anchor-y: search-button.absolute-position.y;
out property <length> search-anchor-width: search-button.width;
out property <length> search-anchor-height: search-button.height;
```

3. `ui/shell/sidebar.slint`

- 透传 search anchor 四元组

4. `ui/app-window.slint`

- 新增 import：

```slint
import { AssetsSearchPopover } from "components/assets-search-popover.slint";
```

- 导出根窗口 layout getter：

```slint
out property <length> layout-assets-search-anchor-x: sidebar.search-anchor-x;
out property <length> layout-assets-search-anchor-y: sidebar.search-anchor-y;
out property <length> layout-assets-search-anchor-width: sidebar.search-anchor-width;
out property <length> layout-assets-search-anchor-height: sidebar.search-anchor-height;
```

- 根层挂一个唯一的 search overlay：

```slint
assets-search-overlay := AssetsSearchPopover {
    visible: root.asset-search-expanded;
    x: sidebar.search-anchor-x;
    y: sidebar.search-anchor-y + sidebar.search-anchor-height + 6px;
    query: root.assets-search-query;
    query-changed(value) => {
        root.assets-search-query-changed(value);
    }
}
```

**Step 4: Re-run the focused tests**

Run:

```bash
bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh
cargo test --test assets_sidebar_toolbar_smoke assets_search_anchor_is_exposed_at_root_window -- --nocapture
```

Expected: PASS。

**Step 5: Commit**

```bash
git add ui/components/assets-search-popover.slint ui/shell/assets-sidebar.slint ui/shell/sidebar.slint ui/app-window.slint tests/assets_sidebar_toolbar_ui_contract_smoke.sh tests/assets_sidebar_toolbar_smoke.rs
git commit -m "feat: host assets search as root overlay"
```

## Task 3: Add Shared Dismiss Layer And Enforce `S2B` Search Close Semantics

**Files:**
- Modify: `ui/app-window.slint`
- Modify: `ui/components/assets-search-popover.slint`
- Modify: `tests/assets_sidebar_toolbar_ui_contract_smoke.sh`
- Reference: `src/shell/view_model.rs`
- Reference: `tests/assets_sidebar_toolbar_spec.rs`
- Reference: `tests/assets_sidebar_toolbar_smoke.rs`

**Step 1: Add the failing contract for shared dismiss behavior**

在 `tests/assets_sidebar_toolbar_ui_contract_smoke.sh` 中追加：

```bash
grep -F 'overlay-dismiss-layer := TouchArea {' "$APP_WINDOW" >/dev/null
grep -F 'enabled: root.asset-search-expanded || root.asset-create-menu-open;' "$APP_WINDOW" >/dev/null
grep -F 'root.collapse-assets-search-requested();' "$APP_WINDOW" >/dev/null
grep -F 'root.close-assets-create-menu-requested();' "$APP_WINDOW" >/dev/null
grep -F 'public function focus-input()' "$SEARCH" >/dev/null
```

在脚本顶部补充：

```bash
SEARCH=ui/components/assets-search-popover.slint
```

`tests/assets_sidebar_toolbar_spec.rs` 与 `tests/assets_sidebar_toolbar_smoke.rs` 已经覆盖：

- 非空查询不会被 `collapse_asset_search_if_empty()` 收起
- 空查询会被收起

本任务不新增 Rust 行为测试，只复用现有测试做回归。

**Step 2: Run verification to prove current code is incomplete**

Run:

```bash
bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh
cargo test --test assets_sidebar_toolbar_spec -- --nocapture
cargo test --test assets_sidebar_toolbar_smoke search_toggle_and_query_binding_round_trip -- --nocapture
```

Expected:

- shell contract FAIL，因为还没有 shared dismiss layer / `focus-input()`
- 两个 Rust 测试 PASS，证明状态机基础已经具备

**Step 3: Write the minimal dismiss-layer implementation**

1. `ui/components/assets-search-popover.slint`

- 保留 `public function focus-input()`
- 不额外引入 close callback

2. `ui/app-window.slint`

- 在 body 之上、overlay 组件之下增加共享 dismiss 层：

```slint
overlay-dismiss-layer := TouchArea {
    x: 0px;
    y: titlebar.height;
    width: root.width;
    height: root.height - titlebar.height;
    enabled: root.asset-search-expanded || root.asset-create-menu-open;

    clicked => {
        root.collapse-assets-search-requested();
        root.close-assets-create-menu-requested();
    }
}
```

- 在 `changed asset-search-expanded =>` 中，展开时主动聚焦输入框：

```slint
changed asset-search-expanded => {
    if root.asset-search-expanded {
        assets-search-overlay.focus-input();
    }
}
```

说明：

- Search 的 `S2B` 语义由现有 `collapse_asset_search_if_empty()` 保证
- dismiss layer 不需要知道 query 内容
- 空 query 时：点击外部会关闭
- 非空 query 时：点击外部会调用 collapse，但 Rust 侧不会收起

**Step 4: Re-run verification**

Run:

```bash
bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh
cargo test --test assets_sidebar_toolbar_spec -- --nocapture
cargo test --test assets_sidebar_toolbar_smoke search_toggle_and_query_binding_round_trip -- --nocapture
```

Expected: 全部 PASS。

**Step 5: Commit**

```bash
git add ui/app-window.slint ui/components/assets-search-popover.slint tests/assets_sidebar_toolbar_ui_contract_smoke.sh
git commit -m "fix: add dismiss layer for assets overlays"
```

## Task 4: Convert `Create` To Icon-Only Trigger And Rework Menu Geometry

**Files:**
- Modify: `ui/shell/assets-sidebar.slint`
- Modify: `ui/components/assets-create-menu.slint`
- Modify: `ui/app-window.slint`
- Modify: `tests/assets_sidebar_toolbar_ui_contract_smoke.sh`
- Reference: `tests/assets_sidebar_toolbar_smoke.rs`

**Step 1: Add the failing UI contract**

在 `tests/assets_sidebar_toolbar_ui_contract_smoke.sh` 中，把旧的 `Create` 文本按钮契约替换为新设计：

```bash
grep -F 'create-button := SidebarToolbarIconButton {' "$ASSETS" >/dev/null
grep -F 'create-add-icon: @image-url("../../assets/icons/fluent/add-20-regular.svg")' "$ASSETS" >/dev/null
grep -F 'icon-source: root.create-add-icon;' "$ASSETS" >/dev/null
grep -F 'active: root.asset-create-menu-open;' "$ASSETS" >/dev/null
! grep -F 'text: "Create";' "$ASSETS" >/dev/null
grep -F 'width: 216px;' "$MENU" >/dev/null
grep -F 'close-policy: PopupClosePolicy.no-auto-close;' "$MENU" >/dev/null
grep -F 'x: sidebar.create-menu-anchor-x + sidebar.create-menu-anchor-width - self.width;' "$APP_WINDOW" >/dev/null
grep -F 'HorizontalLayout {' "$MENU" >/dev/null
grep -F 'spacing: 10px;' "$MENU" >/dev/null
```

说明：

- 本任务明确移除 `Create` 文字 trigger
- 菜单继续保留 `PopupWindow`
- 关闭不再依赖 popup 自己的 auto-close，而交给 Task 3 的 shared dismiss layer

**Step 2: Run the shell contract to verify it fails**

Run:

```bash
bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh
```

Expected: FAIL，当前仍然是复合文字按钮，菜单也还不是新的关闭策略和几何表达。

**Step 3: Write the minimal icon-only + geometry implementation**

1. `ui/shell/assets-sidebar.slint`

- 保留 `create-menu-anchor-*`
- 改为纯 icon trigger：

```slint
private property <image> create-add-icon: @image-url("../../assets/icons/fluent/add-20-regular.svg");

create-button := SidebarToolbarIconButton {
    icon-source: root.create-add-icon;
    active: root.asset-create-menu-open;
    clicked => {
        root.toggle-assets-create-menu-requested();
    }
}
```

2. `ui/components/assets-create-menu.slint`

- 菜单宽度改为更舒展的固定值，例如：

```slint
width: 216px;
close-policy: PopupClosePolicy.no-auto-close;
```

- 保持 `Image + Text` 行结构，但把 icon/text 对齐固定化：

```slint
HorizontalLayout {
    padding-left: 12px;
    padding-right: 12px;
    spacing: 10px;

    icon := Image {
        width: 16px;
        height: 16px;
        source <=> root.icon-source;
    }

    Text {
        text: root.label;
        vertical-alignment: center;
    }
}
```

3. `ui/app-window.slint`

- 把菜单 x 锚点改成右缘对齐：

```slint
x: sidebar.create-menu-anchor-x + sidebar.create-menu-anchor-width - self.width;
y: sidebar.create-menu-anchor-y + sidebar.create-menu-anchor-height + 6px;
```

不要在本任务加入额外 clamp；先验证 `288px + icon-only trigger` 是否已经足够稳定。

**Step 4: Re-run verification**

Run:

```bash
bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh
cargo test --test assets_sidebar_toolbar_smoke create_menu_toggle_and_close_round_trip -- --nocapture
```

Expected:

- shell contract PASS
- Rust smoke PASS，确保菜单状态回调未被破坏

**Step 5: Commit**

```bash
git add ui/shell/assets-sidebar.slint ui/components/assets-create-menu.slint ui/app-window.slint tests/assets_sidebar_toolbar_ui_contract_smoke.sh
git commit -m "fix: restyle assets create menu geometry"
```

## Task 5: Full Verification Gate

**Files:**
- Reference: `tests/assets_sidebar_toolbar_ui_contract_smoke.sh`
- Reference: `tests/assets_sidebar_toolbar_smoke.rs`
- Reference: `tests/assets_sidebar_toolbar_spec.rs`
- Reference: `tests/window_shell.rs`

**Step 1: Run the final shell contract**

Run:

```bash
bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh
```

Expected: PASS，无输出。

**Step 2: Run focused Rust tests**

Run:

```bash
cargo test --test assets_sidebar_toolbar_smoke -- --nocapture
cargo test --test assets_sidebar_toolbar_spec -- --nocapture
cargo test --test window_shell -- --nocapture
```

Expected:

- `assets_sidebar_toolbar_smoke`: PASS
- `assets_sidebar_toolbar_spec`: PASS
- `window_shell`: PASS

**Step 3: Run compile verification**

Run:

```bash
cargo check --workspace
```

Expected: PASS。

**Step 4: Only if verification fails, return to the owning task**

- width 预算或 layout fail: 回到 Task 1
- search overlay / dismiss fail: 回到 Task 2 或 Task 3
- create geometry / menu close fail: 回到 Task 4

**Step 5: Final commit only if Task 5 forced residual fixes**

如果 Task 5 只做验证且工作区已干净，不要追加空提交。

如果 Task 5 为修复残留问题产生了新改动，再执行：

```bash
git add ui/app-window.slint ui/shell/assets-sidebar.slint ui/shell/sidebar.slint ui/components/assets-search-popover.slint ui/components/assets-create-menu.slint src/shell/metrics.rs src/shell/layout.rs tests/assets_sidebar_toolbar_ui_contract_smoke.sh tests/assets_sidebar_toolbar_smoke.rs tests/window_shell.rs
git commit -m "fix: finalize assets toolbar overlay bugfix"
```

## Expected End State

- `AssetsSidebar` 宽度预算提升到 `288px`
- Search 不再以内联行形式存在于 `AssetsSidebar`
- Search 以根窗口 anchored overlay 形式出现
- 空 query outside-click 会收起 Search
- 非空 query outside-click 不会收起 Search
- `Create` 不再显示文字，只保留 icon trigger
- `Create` 菜单右缘与 trigger 右缘对齐，不再显得过窄
- Search 与 `Create` 都通过根层 dismiss layer 处理外部点击
- 不触碰 terminal runtime、renderer、SSH/SFTP 业务逻辑

## Execution Order

1. Task 1
2. Task 2
3. Task 3
4. Task 4
5. Task 5

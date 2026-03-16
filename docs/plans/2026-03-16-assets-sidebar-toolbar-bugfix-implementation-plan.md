# Assets Sidebar Toolbar Bugfix Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 修复 `AssetsSidebar` 顶部工具区，使 toolbar 图标正常显示、标题文案切换为 `Assets`、`Create` 变成 Fluent 复合按钮，并将 `Create` 菜单提升为 `AppWindow` 根层 overlay，菜单项显示 `icon + label`。

**Architecture:** 保持现有 `AppWindow -> Sidebar -> AssetsSidebar` 分层不变。继续让 Rust 只负责 `asset_create_menu_open` 这类业务状态，菜单锚点几何改为通过 Slint `out property` 从 `AssetsSidebar` 逐层上浮到 `AppWindow`，由根窗口拥有唯一的 `AssetsCreateMenu` host。图标体系继续复用本地 Fluent SVG `@image-url(...)`，不引入新图标管线，不触碰 renderer 或 terminal runtime。

**Tech Stack:** Rust 2024, Slint 1.15.1, 现有 `assets/icons/fluent/*.svg`, shell grep smoke tests, focused `cargo test`

---

## Guardrails

- 不修改 `src/main.rs`、`Cargo.toml`、renderer 选择或 terminal runtime 路径。
- 不处理圆角收敛；本计划严格执行 `F0`。
- 不新增字体图标、SVG 运行时加载器或 Rust 侧 icon registry。
- 最终只能保留一个 `AssetsCreateMenu` host，并且它必须归属于 `AppWindow`。
- 能复用现有模式时不新增通用组件；优先沿用 `tooltip-overlay` / `sidebar tooltip` 的根窗口 overlay 结构。

## Current Code Map

- `ui/shell/assets-sidebar.slint:40-194`
  - 当前拥有 header、三个 toolbar icon button、纯文字 `Create` 按钮，以及嵌套在本组件内的 `AssetsCreateMenu`。
- `ui/shell/sidebar.slint:11-214`
  - 当前已通过 `out property` 把 tooltip anchor 从子组件代理到根窗口，是本轮 create-menu anchor 上浮的直接参考。
- `ui/app-window.slint:10-283`
  - 当前已经拥有 `tooltip-overlay` 与 `sidebar-tooltip-overlay` 两个根层 overlay，适合作为 `AssetsCreateMenu` 的最终宿主。
- `ui/components/assets-create-menu.slint:3-71`
  - 当前菜单项仅有文字，没有图标。
- `tests/assets_sidebar_toolbar_ui_contract_smoke.sh:4-30`
  - 当前 grep 契约仍锁定中文标题和旧结构。
- `tests/assets_sidebar_toolbar_smoke.rs:1-68`
  - 当前只覆盖 open/close 和基础 toolbar 状态，没有验证根窗口层级的 anchor 输出。

## Implementation Notes

- 本轮预计只修改 Slint UI 和测试文件；`src/app/bootstrap.rs` 应只作为行为参考，原则上不需要改动。
- `AssetsCreateMenu` 继续使用 `PopupWindow`，但实例化位置从 `AssetsSidebar` 移到 `AppWindow`。
- 建议使用以下命名，避免实现时命名漂移：
  - `search-icon`
  - `tree-expand-icon`
  - `tree-collapse-icon`
  - `tree-view-icon`
  - `list-view-icon`
  - `create-add-icon`
  - `create-chevron-icon`
  - `create-menu-anchor-x`
  - `create-menu-anchor-y`
  - `create-menu-anchor-width`
  - `create-menu-anchor-height`

### Task 1: Fix Header Copy And Toolbar Icon Bindings

**Files:**
- Modify: `tests/assets_sidebar_toolbar_ui_contract_smoke.sh:11-30`
- Modify: `ui/shell/assets-sidebar.slint:40-100`
- Reference: `assets/icons/fluent/search-20-regular.svg`
- Reference: `assets/icons/fluent/arrow-expand-all-20-regular.svg`
- Reference: `assets/icons/fluent/arrow-collapse-all-20-regular.svg`
- Reference: `assets/icons/fluent/branch-20-regular.svg`
- Reference: `assets/icons/fluent/list-20-regular.svg`

**Step 1: Write the failing shell contract assertions**

在 `tests/assets_sidebar_toolbar_ui_contract_smoke.sh` 中，先把标题和三个 toolbar 按钮的最终契约写死。新增或替换为以下断言：

```bash
grep -F 'Text { text: "Assets";' "$ASSETS" >/dev/null
grep -F 'search-icon: @image-url("../../assets/icons/fluent/search-20-regular.svg")' "$ASSETS" >/dev/null
grep -F 'tree-expand-icon: @image-url("../../assets/icons/fluent/arrow-expand-all-20-regular.svg")' "$ASSETS" >/dev/null
grep -F 'tree-collapse-icon: @image-url("../../assets/icons/fluent/arrow-collapse-all-20-regular.svg")' "$ASSETS" >/dev/null
grep -F 'tree-view-icon: @image-url("../../assets/icons/fluent/branch-20-regular.svg")' "$ASSETS" >/dev/null
grep -F 'list-view-icon: @image-url("../../assets/icons/fluent/list-20-regular.svg")' "$ASSETS" >/dev/null
grep -F 'icon-source: root.search-icon;' "$ASSETS" >/dev/null
grep -F 'icon-source: root.asset-tree-fully-expanded ? root.tree-collapse-icon : root.tree-expand-icon;' "$ASSETS" >/dev/null
grep -F 'icon-source: root.asset-view-mode == "flat" ? root.list-view-icon : root.tree-view-icon;' "$ASSETS" >/dev/null
```

**Step 2: Run the shell contract to verify it fails**

Run:

```bash
bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh
```

Expected: FAIL，至少报出缺少 `Text { text: "Assets";` 或缺少新的 `icon-source` 绑定。

**Step 3: Write the minimal Slint implementation**

在 `ui/shell/assets-sidebar.slint` 中完成以下最小实现：

- 将 header 左侧标题从 `资产列表` 改为 `Assets`
- 在组件顶部声明 5 个本地图标属性：
  - `search-icon`
  - `tree-expand-icon`
  - `tree-collapse-icon`
  - `tree-view-icon`
  - `list-view-icon`
- 分别给：
  - `search-button`
  - `tree-expansion-button`
  - `view-mode-button`
  绑定最终图标表达式

建议目标代码形态：

```slint
private property <image> search-icon: @image-url("../../assets/icons/fluent/search-20-regular.svg");
private property <image> tree-expand-icon: @image-url("../../assets/icons/fluent/arrow-expand-all-20-regular.svg");
private property <image> tree-collapse-icon: @image-url("../../assets/icons/fluent/arrow-collapse-all-20-regular.svg");
private property <image> tree-view-icon: @image-url("../../assets/icons/fluent/branch-20-regular.svg");
private property <image> list-view-icon: @image-url("../../assets/icons/fluent/list-20-regular.svg");

Text { text: "Assets"; color: ThemeTokens.text-primary; vertical-alignment: center; }

search-button := SidebarToolbarIconButton {
    icon-source: root.search-icon;
}

tree-expansion-button := SidebarToolbarIconButton {
    icon-source: root.asset-tree-fully-expanded ? root.tree-collapse-icon : root.tree-expand-icon;
}

view-mode-button := SidebarToolbarIconButton {
    icon-source: root.asset-view-mode == "flat" ? root.list-view-icon : root.tree-view-icon;
}
```

**Step 4: Re-run the shell contract**

Run:

```bash
bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh
```

Expected: PASS，前提是本任务新增的断言全部满足；如果脚本还在检查旧中文标题或未完成的后续任务断言，先把脚本同步到本任务粒度再运行。

**Step 5: Commit**

```bash
git add tests/assets_sidebar_toolbar_ui_contract_smoke.sh ui/shell/assets-sidebar.slint
git commit -m "fix: wire assets toolbar icons and copy"
```

### Task 2: Promote Create Menu Host To `AppWindow`

**Files:**
- Modify: `tests/assets_sidebar_toolbar_ui_contract_smoke.sh:11-30`
- Modify: `ui/shell/assets-sidebar.slint:5-20,80-100,179-194`
- Modify: `ui/shell/sidebar.slint:15-43,146-186`
- Modify: `ui/app-window.slint:1-8,21-28,179-227,265-283`
- Reference: `src/app/bootstrap.rs:156-162,446-472`

**Step 1: Extend the shell contract with root-overlay assertions**

向 `tests/assets_sidebar_toolbar_ui_contract_smoke.sh` 追加以下断言：

```bash
grep -F 'out property <length> create-menu-anchor-x' "$ASSETS" >/dev/null
grep -F 'out property <length> create-menu-anchor-y' "$ASSETS" >/dev/null
grep -F 'out property <length> create-menu-anchor-width' "$ASSETS" >/dev/null
grep -F 'out property <length> create-menu-anchor-height' "$ASSETS" >/dev/null
grep -F 'out property <length> create-menu-anchor-x' "$SIDEBAR" >/dev/null
grep -F 'out property <length> create-menu-anchor-y' "$SIDEBAR" >/dev/null
grep -F 'out property <length> create-menu-anchor-width' "$SIDEBAR" >/dev/null
grep -F 'out property <length> create-menu-anchor-height' "$SIDEBAR" >/dev/null
grep -F 'assets-create-menu-overlay := AssetsCreateMenu {' "$APP_WINDOW" >/dev/null
grep -F 'x: sidebar.create-menu-anchor-x;' "$APP_WINDOW" >/dev/null
grep -F 'y: sidebar.create-menu-anchor-y + sidebar.create-menu-anchor-height + 6px;' "$APP_WINDOW" >/dev/null
! grep -F 'create-menu := AssetsCreateMenu {' "$ASSETS" >/dev/null
```

**Step 2: Run the shell contract to verify it fails**

Run:

```bash
bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh
```

Expected: FAIL，当前代码仍然把 `AssetsCreateMenu` 嵌套在 `AssetsSidebar` 中，且没有 anchor 输出属性。

**Step 3: Write the minimal root-overlay implementation**

按以下顺序实现：

1. `ui/shell/assets-sidebar.slint`
   - 删除本地 `create-menu := AssetsCreateMenu`
   - 为 `create-button` 暴露 4 个 `out property`：
     - `create-menu-anchor-x`
     - `create-menu-anchor-y`
     - `create-menu-anchor-width`
     - `create-menu-anchor-height`
   - 值直接绑定到 `create-button.absolute-position` 与 `create-button.width/height`

2. `ui/shell/sidebar.slint`
   - 透传上述 4 个 `out property`
   - 值绑定到 `assets-sidebar` 的同名输出

3. `ui/app-window.slint`
   - 新增 `import { AssetsCreateMenu } from "components/assets-create-menu.slint";`
   - 在根层 overlay 区域实例化唯一的 `assets-create-menu-overlay := AssetsCreateMenu`
   - 位置使用 `sidebar.create-menu-anchor-*`
   - 开闭逻辑仍然绑定 `root.asset-create-menu-open`
   - 关闭和动作回调继续走：
     - `root.close-assets-create-menu-requested()`
     - `root.assets-create-action-selected(action-id)`

4. `src/app/bootstrap.rs`
   - 不要重构现有 open/close/action 回调链
   - 只有在 Slint 绑定编译报错时才做最小修复

**Step 4: Re-run the shell contract**

Run:

```bash
bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh
```

Expected: PASS，至少本任务新增的 overlay/anchor 断言全部通过。

**Step 5: Commit**

```bash
git add tests/assets_sidebar_toolbar_ui_contract_smoke.sh ui/shell/assets-sidebar.slint ui/shell/sidebar.slint ui/app-window.slint
git commit -m "refactor: host assets create menu at root window"
```

### Task 3: Implement The Composite `Create` Button And Menu Row Icons

**Files:**
- Modify: `tests/assets_sidebar_toolbar_ui_contract_smoke.sh:11-30`
- Modify: `ui/shell/assets-sidebar.slint:80-100`
- Modify: `ui/components/assets-create-menu.slint:3-71`
- Reference: `assets/icons/fluent/add-20-regular.svg`
- Reference: `assets/icons/fluent/chevron-down-20-regular.svg`
- Reference: `assets/icons/fluent/folder-20-regular.svg`
- Reference: `assets/icons/fluent/window-console-20-regular.svg`

**Step 1: Add the failing shell assertions for the new button/menu structure**

继续扩展 `tests/assets_sidebar_toolbar_ui_contract_smoke.sh`，新增：

```bash
grep -F 'create-add-icon: @image-url("../../assets/icons/fluent/add-20-regular.svg")' "$ASSETS" >/dev/null
grep -F 'create-chevron-icon: @image-url("../../assets/icons/fluent/chevron-down-20-regular.svg")' "$ASSETS" >/dev/null
grep -F 'create-icon := Image {' "$ASSETS" >/dev/null
grep -F 'create-label := Text {' "$ASSETS" >/dev/null
grep -F 'text: "Create";' "$ASSETS" >/dev/null
grep -F 'create-chevron := Image {' "$ASSETS" >/dev/null
grep -F 'in property <image> icon-source;' "$MENU" >/dev/null
grep -F 'new-folder-icon: @image-url("../../assets/icons/fluent/folder-20-regular.svg")' "$MENU" >/dev/null
grep -F 'new-ssh-connection-icon: @image-url("../../assets/icons/fluent/window-console-20-regular.svg")' "$MENU" >/dev/null
grep -F 'icon-source: root.new-folder-icon;' "$MENU" >/dev/null
grep -F 'icon-source: root.new-ssh-connection-icon;' "$MENU" >/dev/null
```

**Step 2: Run the shell contract to verify it fails**

Run:

```bash
bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh
```

Expected: FAIL，当前 `Create` 仍是纯文字按钮，`AssetsCreateMenu` 仍是纯文字 row。

**Step 3: Write the minimal Slint implementation**

在 `ui/shell/assets-sidebar.slint` 中：

- 声明：
  - `create-add-icon`
  - `create-chevron-icon`
- 将 `create-button` 改为单一点击目标的复合按钮
- 内部至少包含：
  - `create-icon := Image`
  - `create-label := Text`
  - `create-chevron := Image`

目标形态：

```slint
private property <image> create-add-icon: @image-url("../../assets/icons/fluent/add-20-regular.svg");
private property <image> create-chevron-icon: @image-url("../../assets/icons/fluent/chevron-down-20-regular.svg");

create-button := Rectangle {
    create-icon := Image { source: root.create-add-icon; }
    create-label := Text { text: "Create"; }
    create-chevron := Image { source: root.create-chevron-icon; }
    touch := TouchArea {
        clicked => { root.toggle-assets-create-menu-requested(); }
    }
}
```

在 `ui/components/assets-create-menu.slint` 中：

- 给 `MenuActionItem` 增加 `in property <image> icon-source;`
- 为 `New Folder` / `New SSH Connection` 增加本地图标属性
- 每一行渲染为 `Image + Text`

目标形态：

```slint
component MenuActionItem inherits Rectangle {
    in property <string> label;
    in property <image> icon-source;

    icon := Image {
        source <=> root.icon-source;
    }

    Text {
        text: root.label;
    }
}
```

**Step 4: Re-run the shell contract**

Run:

```bash
bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh
```

Expected: PASS，新增的复合按钮和菜单项图标断言全部满足。

**Step 5: Commit**

```bash
git add tests/assets_sidebar_toolbar_ui_contract_smoke.sh ui/shell/assets-sidebar.slint ui/components/assets-create-menu.slint
git commit -m "fix: restyle assets create controls"
```

### Task 4: Expose Root Layout Anchor Outputs And Add Focused Rust Smoke Coverage

**Files:**
- Modify: `tests/assets_sidebar_toolbar_smoke.rs:1-68`
- Modify: `ui/app-window.slint:29-49`
- Reference: `ui/shell/sidebar.slint:27-33`

**Step 1: Write the failing Rust smoke test**

在 `tests/assets_sidebar_toolbar_smoke.rs` 中新增一个根窗口级别的 layout smoke test：

```rust
#[test]
fn assets_create_menu_anchor_is_exposed_at_root_window() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    assert!(app.get_layout_assets_create_menu_anchor_width() > 0.0);
    assert!(app.get_layout_assets_create_menu_anchor_height() > 0.0);
}
```

**Step 2: Run the focused Rust smoke test to verify it fails**

Run:

```bash
cargo test --test assets_sidebar_toolbar_smoke assets_create_menu_anchor_is_exposed_at_root_window -- --nocapture
```

Expected: FAIL，编译报错类似 `no method named get_layout_assets_create_menu_anchor_width`，因为 `AppWindow` 还未导出这组 layout out properties。

**Step 3: Write the minimal `AppWindow` layout outputs**

在 `ui/app-window.slint` 现有 layout 输出区域追加：

```slint
out property <length> layout-assets-create-menu-anchor-x: sidebar.create-menu-anchor-x;
out property <length> layout-assets-create-menu-anchor-y: sidebar.create-menu-anchor-y;
out property <length> layout-assets-create-menu-anchor-width: sidebar.create-menu-anchor-width;
out property <length> layout-assets-create-menu-anchor-height: sidebar.create-menu-anchor-height;
```

只做这组输出，不要再新增多余的 anchor state。

**Step 4: Re-run the focused Rust smoke test**

Run:

```bash
cargo test --test assets_sidebar_toolbar_smoke assets_create_menu_anchor_is_exposed_at_root_window -- --nocapture
```

Expected: PASS。

**Step 5: Commit**

```bash
git add tests/assets_sidebar_toolbar_smoke.rs ui/app-window.slint
git commit -m "test: cover assets create menu anchor layout"
```

### Task 5: Full Verification Gate

**Files:**
- Reference: `tests/assets_sidebar_toolbar_ui_contract_smoke.sh`
- Reference: `tests/assets_sidebar_toolbar_smoke.rs`
- Reference: `tests/assets_sidebar_toolbar_spec.rs`

**Step 1: Run the final shell contract**

Run:

```bash
bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh
```

Expected: PASS，无输出。

**Step 2: Run the focused Rust test binaries**

Run:

```bash
cargo test --test assets_sidebar_toolbar_smoke -- --nocapture
cargo test --test assets_sidebar_toolbar_spec -- --nocapture
```

Expected:

- `assets_sidebar_toolbar_smoke`: PASS
- `assets_sidebar_toolbar_spec`: PASS

**Step 3: Only if verification fails, return to the owning task**

- shell contract fail: 回到 Task 1 / 2 / 3
- root anchor getter fail: 回到 Task 4
- view model/action id fail: 检查是否意外破坏既有菜单回调语义

**Step 4: Final commit only if Task 5 forced residual fixes**

如果 Task 5 只做验证且工作区已干净，不要额外制造“空提交”。

如果 Task 5 为修复残留问题产生了新改动，再执行：

```bash
git add ui/app-window.slint ui/shell/assets-sidebar.slint ui/components/assets-create-menu.slint tests/assets_sidebar_toolbar_ui_contract_smoke.sh tests/assets_sidebar_toolbar_smoke.rs
git commit -m "fix: finalize assets toolbar bugfix"
```

## Expected End State

- `AssetsSidebar` 顶部标题为 `Assets`
- Search / Tree / View 三个按钮均绑定 Fluent SVG
- `Create` 为 `Add icon + Create + ChevronDown` 的单一点击目标
- `AssetsCreateMenu` 只在 `AppWindow` 根层实例化一次
- 菜单项为 `leading icon + label`
- 现有 `asset_create_menu_open` 行为不变
- 未触碰 renderer、runtime、rounded-corner 路线

## Execution Order

1. Task 1
2. Task 2
3. Task 3
4. Task 4
5. Task 5

# Windows Console 资产列表右键菜单 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 为 `Window Console` 资产列表补齐最小可右击节点壳层、根窗口自绘多列右键菜单、动态能力解析、Windows 风格级联交互，以及未接线动作的轻量反馈与文档追踪。

**Architecture:** 保持当前项目已验证的 `Rust state -> bootstrap sync -> Slint root overlay` 路线不变。Rust 侧新增 `context_menu` 领域模块，负责目标类型、动作树、能力解析、可见列投影、Explorer 风格右键选中和键盘/hover 状态；Slint 侧只负责最小资产节点壳层、根窗口 `ContextMenuOverlay`、多列菜单渲染、根层 dismiss 和 pointer hit-testing。为了降低 Slint 1.15.1 的桥接复杂度，菜单列数据采用“最多三级菜单的固定三列 slots”方式投影，而不是把递归树直接交给 Slint。

**Tech Stack:** Rust 2024, Slint 1.15.1, `winit + femtovg-wgpu`, `TouchArea.pointer-event`, `Timer`, `FocusScope`, `i-slint-backend-testing`, shell smoke scripts, `cargo test`, `cargo check`, `cargo clippy`

---

## Execution Notes

- 设计输入固定为 `docs/plans/2026-03-17-windows-console-assets-context-menu-design.md`。
- 未接线动作清单固定为 `docs/plans/2026-03-17-windows-console-assets-context-menu-unimplemented-actions.md`，实现结束后必须同步更新。
- 每个任务都先走 `@superpowers:test-driven-development`：先写失败测试，再做最小实现，再跑通过。
- 如果 `TouchArea.pointer-event`、submenu hover/corridor、根层 overlay hit-testing 行为与计划不一致，立即切换 `@superpowers:systematic-debugging`，不要猜。
- 第一版只实现设计里已确认的 `12A` 键盘闭环；`12B` 完整桌面菜单键盘体验只能留在 `TODO`，不能顺手扩 scope。
- 保持 ASCII 优先；文档与注释继续使用中文，技术名词、命令、路径和类型名保留英文。

## Current Code Map

- `src/shell/assets.rs`
  - 当前只有 `AssetViewMode`
  - 还没有真实资产节点类型、mock asset rows 或 target kind
- `src/shell/view_model.rs`
  - 当前只覆盖 assets toolbar 状态
  - 还没有 context menu open state、selection context、hover/open path
- `src/app/bootstrap.rs`
  - 当前只绑定 toolbar callbacks
  - 还没有资产节点右击、context menu callbacks、planned-action feedback
- `ui/shell/assets-sidebar.slint`
  - 当前仍是占位文本
  - 还没有 `AssetNodeRow`、mock asset list、右键事件桥接
- `ui/app-window.slint`
  - 当前根层只有 search dismiss、create menu dismiss 和 `AssetsCreateMenu`
  - 还没有 `ContextMenuOverlay` 宿主与反馈 pill
- `tests/assets_sidebar_toolbar_spec.rs`
  - 当前覆盖搜索与 create menu 状态
  - 可以继续保留，不要把右键菜单测试塞进去
- `tests/assets_sidebar_toolbar_smoke.rs`
  - 当前覆盖 toolbar 桥接
  - 右键菜单建议新建独立 smoke / contract 文件，避免混淆范围

## Proposed File Layout

### Rust

- Create: `src/shell/context_menu.rs`
- Modify: `src/shell/assets.rs`
- Modify: `src/shell/mod.rs`
- Modify: `src/shell/view_model.rs`
- Modify: `src/app/bootstrap.rs`

### Slint

- Create: `ui/components/asset-node-row.slint`
- Create: `ui/components/assets-context-menu-row.slint`
- Create: `ui/components/assets-context-menu-column.slint`
- Create: `ui/components/assets-context-menu-overlay.slint`
- Modify: `ui/shell/assets-sidebar.slint`
- Modify: `ui/shell/sidebar.slint`
- Modify: `ui/app-window.slint`

### Tests

- Create: `tests/assets_context_menu_spec.rs`
- Create: `tests/assets_context_menu_smoke.rs`
- Create: `tests/assets_context_menu_ui_contract_smoke.sh`
- Modify: `tests/shell_view_model.rs`
- Optionally Modify: `tests/window_shell.rs` only if新增根窗口导出布局属性需要 contract 覆盖

### Docs

- Modify: `docs/plans/2026-03-17-windows-console-assets-context-menu-unimplemented-actions.md`
- Modify: `verification.md`

## Task 1: 建立 Rust 侧 context menu 领域模型与 resolver

**Files:**
- Create: `src/shell/context_menu.rs`
- Modify: `src/shell/assets.rs`
- Modify: `src/shell/mod.rs`
- Test: `tests/assets_context_menu_spec.rs`

**Step 1: Write the failing resolver tests**

在 `tests/assets_context_menu_spec.rs` 先写纯 Rust 契约测试，至少覆盖：

```rust
#[test]
fn resolver_returns_blank_area_groups_in_expected_order() {}

#[test]
fn resolver_returns_ssh_actions_with_planned_proxy_tools() {}

#[test]
fn resolver_marks_paste_disabled_when_clipboard_is_empty() {}

#[test]
fn visible_columns_projection_opens_new_connection_submenu() {}
```

最小断言范围：

- 空白区主菜单包含 `new-folder`、`new-connection`、`batch-open`
- SSH 菜单包含 `close-connection`、`open-in-new-tab`、`proxy-chrome-via-server`
- 空剪贴板时 `paste-asset` 为 disabled
- 打开路径指向 `new-connection` 时，可见列投影包含二级菜单项 `ssh`、`local-terminal`、`serial`、`telnet`、`ssh-tunnel`

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test --test assets_context_menu_spec -- --nocapture
```

Expected:

- 编译失败，因为 `src/shell/context_menu.rs` 和相关类型尚不存在

**Step 3: Write the minimal domain model**

在 `src/shell/context_menu.rs` 创建最小领域模型：

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextTargetKind {
    BlankArea,
    SshConnection,
    Folder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextMenuActionState {
    Enabled,
    Disabled,
    Planned,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextMenuActionNode {
    pub id: &'static str,
    pub title: &'static str,
    pub state: ContextMenuActionState,
    pub children: Vec<ContextMenuActionNode>,
    pub divider_before: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionContext {
    pub selected_ids: Vec<String>,
    pub clipboard_has_asset_payload: bool,
    pub target_mutable: bool,
    pub target_has_active_connection: bool,
}

pub fn resolve_action_tree(
    target: ContextTargetKind,
    selection: &SelectionContext,
) -> Vec<ContextMenuActionNode> {
    // 先按设计文档硬编码三类场景的 IA，再把 disabled / planned 叠加进去
}

pub fn visible_columns_for_path(
    roots: &[ContextMenuActionNode],
    open_path: &[usize],
) -> [Vec<ContextMenuActionNode>; 3] {
    // 固定最多三列，超出不投影
}
```

同时在 `src/shell/assets.rs` 增加最小 target kind / mock asset 相关枚举或辅助函数；在 `src/shell/mod.rs` 导出 `context_menu` 模块。

**Step 4: Run tests to verify it passes**

Run:

```bash
cargo test --test assets_context_menu_spec -- --nocapture
```

Expected:

- PASS

**Step 5: Commit**

```bash
git add src/shell/context_menu.rs src/shell/assets.rs src/shell/mod.rs tests/assets_context_menu_spec.rs
git commit -m "feat: add assets context menu domain model"
```

## Task 2: 扩展 `ShellViewModel`，把右键菜单状态纳入 Rust 真相源

**Files:**
- Modify: `src/shell/view_model.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `tests/shell_view_model.rs`
- Create: `tests/assets_context_menu_smoke.rs`

**Step 1: Write the failing state tests**

在 `tests/shell_view_model.rs` 添加：

```rust
#[test]
fn shell_view_model_starts_with_context_menu_closed() {}

#[test]
fn opening_context_menu_tracks_target_anchor_and_resets_open_path() {}

#[test]
fn selecting_submenu_path_updates_visible_columns() {}

#[test]
fn closing_context_menu_clears_open_path_but_keeps_selection() {}
```

在 `tests/assets_context_menu_smoke.rs` 添加窗口桥接级别的测试：

```rust
#[test]
fn bootstrap_exposes_context_menu_closed_by_default() {}

#[test]
fn right_click_request_opens_context_menu_and_sets_anchor() {}
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test shell_view_model --test assets_context_menu_smoke -- --nocapture
```

Expected:

- 编译失败，因为 `ShellViewModel` 还没有 context menu 字段和对应方法

**Step 3: Write the minimal state implementation**

在 `src/shell/view_model.rs` 增加字段与方法：

```rust
pub struct ShellViewModel {
    // existing fields...
    pub console_asset_items: Vec<MockConsoleAssetItem>,
    pub selected_asset_ids: Vec<String>,
    pub context_menu_open: bool,
    pub context_menu_target_kind: Option<ContextTargetKind>,
    pub context_menu_anchor_x: f32,
    pub context_menu_anchor_y: f32,
    pub context_menu_open_path: Vec<usize>,
    pub context_menu_feedback_text: String,
}

impl ShellViewModel {
    pub fn open_context_menu_for_target(
        &mut self,
        target_kind: ContextTargetKind,
        target_id: Option<String>,
        anchor_x: f32,
        anchor_y: f32,
    ) { /* Explorer 风格选择 + open path reset */ }

    pub fn close_context_menu(&mut self) { /* close only */ }

    pub fn set_context_menu_open_path(&mut self, path: Vec<usize>) { /* replace path */ }
}
```

在 `src/app/bootstrap.rs` 先补最小 sync：

- `context_menu_open`
- `context_menu_anchor_x`
- `context_menu_anchor_y`
- `context_menu_feedback_text`

并为后续 UI callback 预留：

- `asset-context-menu-requested(...)`
- `close-assets-context-menu-requested()`

**Step 4: Re-run the focused tests**

Run:

```bash
cargo test --test shell_view_model --test assets_context_menu_smoke -- --nocapture
```

Expected:

- PASS

**Step 5: Commit**

```bash
git add src/shell/view_model.rs src/app/bootstrap.rs tests/shell_view_model.rs tests/assets_context_menu_smoke.rs
git commit -m "feat: add context menu state to shell view model"
```

## Task 3: 在 `AssetsSidebar` 中补最小可右击节点壳层

**Files:**
- Create: `ui/components/asset-node-row.slint`
- Modify: `ui/shell/assets-sidebar.slint`
- Modify: `ui/shell/sidebar.slint`
- Modify: `ui/app-window.slint`
- Modify: `src/app/bootstrap.rs`
- Test: `tests/assets_context_menu_ui_contract_smoke.sh`
- Test: `tests/assets_context_menu_smoke.rs`

**Step 1: Write the failing UI contract**

在 `tests/assets_context_menu_ui_contract_smoke.sh` 新增断言：

```bash
grep -F 'export component AssetNodeRow inherits Rectangle' "$ROOT_DIR/ui/components/asset-node-row.slint" >/dev/null
grep -F 'pointer-event(event) => {' "$ROOT_DIR/ui/components/asset-node-row.slint" >/dev/null
grep -F 'event.button == PointerEventButton.right' "$ROOT_DIR/ui/components/asset-node-row.slint" >/dev/null
grep -F 'callback asset-context-menu-requested(string, string, length, length);' "$ROOT_DIR/ui/shell/assets-sidebar.slint" >/dev/null
grep -F 'in property <[ConsoleAssetItem]> console-asset-items' "$ROOT_DIR/ui/shell/assets-sidebar.slint" >/dev/null
grep -F 'callback asset-context-menu-requested(string, string, length, length);' "$ROOT_DIR/ui/app-window.slint" >/dev/null
```

在 `tests/assets_context_menu_smoke.rs` 新增：

```rust
#[test]
fn bootstrap_exposes_mock_console_assets() {}
```

至少断言：

- 默认存在 3 个 mock items
- 包含一个 `ssh` 和一个 `folder`

**Step 2: Run tests to verify they fail**

Run:

```bash
bash tests/assets_context_menu_ui_contract_smoke.sh
cargo test --test assets_context_menu_smoke bootstrap_exposes_mock_console_assets -- --nocapture
```

Expected:

- contract FAIL，因为组件和 callback 尚不存在
- smoke FAIL，因为窗口还没有 assets item list

**Step 3: Write the minimal Slint bridge**

1. 创建 `ui/components/asset-node-row.slint`

```slint
export component AssetNodeRow inherits Rectangle {
    in property <string> item-id;
    in property <string> item-kind;
    in property <string> label;
    in property <bool> selected;
    callback clicked(string);
    callback context-menu-requested(string, string, length, length);

    touch := TouchArea {
        pointer-event(event) => {
            if (event.button == PointerEventButton.right) {
                root.context-menu-requested(root.item-id, root.item-kind, self.mouse-x + self.absolute-position.x, self.mouse-y + self.absolute-position.y);
                accept
            } else {
                reject
            }
        }

        clicked => {
            root.clicked(root.item-id);
        }
    }
}
```

2. `ui/shell/assets-sidebar.slint`

- 定义 `export struct ConsoleAssetItem`
- 增加 `in property <[ConsoleAssetItem]> console-asset-items`
- 在 `console` 面板中用 `for item in root.console-asset-items : AssetNodeRow { ... }`
- 转发 `asset-context-menu-requested(...)`

3. `ui/shell/sidebar.slint` 和 `ui/app-window.slint`

- 透传 `console-asset-items`
- 透传 `asset-context-menu-requested(...)`

4. `src/app/bootstrap.rs`

- 在初始状态中注入 3 个 mock items：
  - `ssh-prod-01`
  - `folder-favorites`
  - `ssh-jump-01`

**Step 4: Re-run the focused tests**

Run:

```bash
bash tests/assets_context_menu_ui_contract_smoke.sh
cargo test --test assets_context_menu_smoke bootstrap_exposes_mock_console_assets -- --nocapture
```

Expected:

- PASS

**Step 5: Commit**

```bash
git add ui/components/asset-node-row.slint ui/shell/assets-sidebar.slint ui/shell/sidebar.slint ui/app-window.slint src/app/bootstrap.rs tests/assets_context_menu_ui_contract_smoke.sh tests/assets_context_menu_smoke.rs
git commit -m "feat: add mock console asset rows"
```

## Task 4: 构建根窗口 `ContextMenuOverlay` 与一级菜单渲染

**Files:**
- Create: `ui/components/assets-context-menu-row.slint`
- Create: `ui/components/assets-context-menu-column.slint`
- Create: `ui/components/assets-context-menu-overlay.slint`
- Modify: `ui/app-window.slint`
- Modify: `src/app/bootstrap.rs`
- Test: `tests/assets_context_menu_ui_contract_smoke.sh`
- Test: `tests/assets_context_menu_smoke.rs`

**Step 1: Write the failing contract and smoke tests**

在 `tests/assets_context_menu_ui_contract_smoke.sh` 增加：

```bash
grep -F 'export component AssetsContextMenuRow inherits Rectangle' "$ROOT_DIR/ui/components/assets-context-menu-row.slint" >/dev/null
grep -F 'export component AssetsContextMenuColumn inherits Rectangle' "$ROOT_DIR/ui/components/assets-context-menu-column.slint" >/dev/null
grep -F 'export component AssetsContextMenuOverlay inherits Rectangle' "$ROOT_DIR/ui/components/assets-context-menu-overlay.slint" >/dev/null
grep -F 'Text { text: "操作";' "$ROOT_DIR/ui/components/assets-context-menu-column.slint" >/dev/null
grep -F 'assets-context-menu-overlay := AssetsContextMenuOverlay {' "$ROOT_DIR/ui/app-window.slint" >/dev/null
grep -F 'enabled: root.assets-context-menu-open;' "$ROOT_DIR/ui/app-window.slint" >/dev/null
```

在 `tests/assets_context_menu_smoke.rs` 添加：

```rust
#[test]
fn right_click_request_populates_primary_menu_title() {}
```

至少断言：

- 右击某个 SSH mock item 后 `context_menu_open == true`
- 一级菜单标题为 `操作`

**Step 2: Run tests to verify they fail**

Run:

```bash
bash tests/assets_context_menu_ui_contract_smoke.sh
cargo test --test assets_context_menu_smoke right_click_request_populates_primary_menu_title -- --nocapture
```

Expected:

- FAIL

**Step 3: Write the minimal overlay host**

1. `ui/components/assets-context-menu-row.slint`

- 复用 `AssetsToolbarMenuRow` 的视觉度量，但扩展出：
  - disabled 外观
  - extra slot（submenu arrow）

2. `ui/components/assets-context-menu-column.slint`

- 顶部固定 `操作`
- 下方全局分割线
- 中间 `for item in root.items`

3. `ui/components/assets-context-menu-overlay.slint`

- 固定最多三列：
  - `primary-column`
  - `secondary-column`
  - `tertiary-column`
- 通过 `visible` 控制列存在性

4. `ui/app-window.slint`

- 添加：
  - `assets-context-menu-open`
  - `assets-context-menu-anchor-x`
  - `assets-context-menu-anchor-y`
  - `assets-context-menu-primary-title`
  - `assets-context-menu-primary-items`
  - `close-assets-context-menu-requested()`

- 根层增加 dismiss layer：

```slint
assets-context-menu-dismiss-layer := TouchArea {
    enabled: root.assets-context-menu-open;
    clicked => { root.close-assets-context-menu-requested(); }
}
```

5. `src/app/bootstrap.rs`

- 把一级菜单标题和 items 从 Rust state 同步到窗口属性

**Step 4: Re-run the focused tests**

Run:

```bash
bash tests/assets_context_menu_ui_contract_smoke.sh
cargo test --test assets_context_menu_smoke right_click_request_populates_primary_menu_title -- --nocapture
```

Expected:

- PASS

**Step 5: Commit**

```bash
git add ui/components/assets-context-menu-row.slint ui/components/assets-context-menu-column.slint ui/components/assets-context-menu-overlay.slint ui/app-window.slint src/app/bootstrap.rs tests/assets_context_menu_ui_contract_smoke.sh tests/assets_context_menu_smoke.rs
git commit -m "feat: add root overlay for assets context menu"
```

## Task 5: 接入场景菜单、二级菜单投影与动作状态

**Files:**
- Modify: `src/shell/context_menu.rs`
- Modify: `src/shell/view_model.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `ui/app-window.slint`
- Modify: `ui/components/assets-context-menu-overlay.slint`
- Test: `tests/assets_context_menu_spec.rs`
- Test: `tests/assets_context_menu_smoke.rs`

**Step 1: Write the failing submenu and state tests**

在 `tests/assets_context_menu_spec.rs` 增加：

```rust
#[test]
fn submenu_projection_exposes_new_connection_children_in_second_column() {}

#[test]
fn ssh_scene_marks_proxy_chrome_as_planned_but_clickable() {}

#[test]
fn blank_area_scene_disables_paste_when_clipboard_is_empty() {}
```

在 `tests/assets_context_menu_smoke.rs` 增加：

```rust
#[test]
fn hovering_or_selecting_new_connection_populates_secondary_column() {}
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test assets_context_menu_spec --test assets_context_menu_smoke -- --nocapture
```

Expected:

- FAIL，因为当前只有一级列

**Step 3: Write the minimal scene wiring**

1. `src/shell/context_menu.rs`

- 完整补齐三类场景菜单树
- 将 `new-connection` 做成父节点
- 将 `planned` 和 `disabled` 状态规则补齐

2. `src/shell/view_model.rs`

- 保留 `context_menu_open_path`
- 增加：

```rust
pub fn hover_context_menu_path(&mut self, path: Vec<usize>) {}
pub fn invoke_context_menu_action(&mut self, action_id: &str) -> ContextMenuInvokeResult {}
```

3. `ui/app-window.slint` / `ui/components/assets-context-menu-overlay.slint`

- 新增 secondary / tertiary 标题和 items 属性
- Row hover / click 回调统一转回 window callbacks

**Step 4: Re-run the focused tests**

Run:

```bash
cargo test --test assets_context_menu_spec --test assets_context_menu_smoke -- --nocapture
```

Expected:

- PASS

**Step 5: Commit**

```bash
git add src/shell/context_menu.rs src/shell/view_model.rs src/app/bootstrap.rs ui/app-window.slint ui/components/assets-context-menu-overlay.slint tests/assets_context_menu_spec.rs tests/assets_context_menu_smoke.rs
git commit -m "feat: wire scene-specific context menu columns"
```

## Task 6: 实现定位翻转、hover 延迟与 corridor 容错

**Files:**
- Modify: `src/shell/context_menu.rs`
- Modify: `src/shell/view_model.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `ui/components/assets-context-menu-overlay.slint`
- Modify: `ui/app-window.slint`
- Test: `tests/assets_context_menu_spec.rs`
- Test: `tests/assets_context_menu_ui_contract_smoke.sh`
- Test: `tests/assets_context_menu_smoke.rs`

**Step 1: Write the failing geometry and corridor tests**

在 `tests/assets_context_menu_spec.rs` 增加纯函数测试：

```rust
#[test]
fn root_menu_flips_left_when_anchor_is_near_right_edge() {}

#[test]
fn submenu_flips_left_when_secondary_column_would_overflow() {}

#[test]
fn corridor_logic_keeps_submenu_open_while_pointer_moves_toward_child_column() {}
```

在 `tests/assets_context_menu_ui_contract_smoke.sh` 增加：

```bash
grep -F 'hover-open-delay := Timer {' "$ROOT_DIR/ui/components/assets-context-menu-overlay.slint" >/dev/null
grep -F 'corridor-close-delay := Timer {' "$ROOT_DIR/ui/components/assets-context-menu-overlay.slint" >/dev/null
grep -F 'callback row-hovered(int, int);' "$ROOT_DIR/ui/components/assets-context-menu-overlay.slint" >/dev/null
grep -F 'callback pointer-moved(length, length);' "$ROOT_DIR/ui/components/assets-context-menu-overlay.slint" >/dev/null
```

在 `tests/assets_context_menu_smoke.rs` 增加：

```rust
#[test]
fn right_click_near_edge_still_keeps_overlay_within_window_bounds() {}
```

**Step 2: Run tests to verify they fail**

Run:

```bash
bash tests/assets_context_menu_ui_contract_smoke.sh
cargo test --test assets_context_menu_spec --test assets_context_menu_smoke -- --nocapture
```

Expected:

- FAIL

**Step 3: Write the minimal geometry implementation**

1. 在 `src/shell/context_menu.rs` 添加可测试的几何纯函数：

```rust
pub struct MenuPlacementInput {
    pub host_width: f32,
    pub host_height: f32,
    pub anchor_x: f32,
    pub anchor_y: f32,
    pub root_width: f32,
    pub root_height: f32,
    pub child_width: f32,
}

pub fn resolve_root_menu_origin(input: MenuPlacementInput) -> (f32, f32, bool) {}
pub fn should_keep_corridor_open(pointer: (f32, f32), parent_rect: Rect, child_rect: Rect) -> bool {}
```

2. `ui/components/assets-context-menu-overlay.slint`

- 用 `Timer` 实现 `10B`
- 用 pointer move callback 把坐标回传 Rust，Rust 计算 corridor 是否保持打开
- overlay 内部根据 direction flag 决定二级/三级列放在左侧还是右侧

3. `src/app/bootstrap.rs`

- 在 `shell_layout_invalidated` 或 window size 变化时同步 host size
- 通过 callback 把 pointer move / hover / timer 结果回写 state

**Step 4: Re-run the focused tests**

Run:

```bash
bash tests/assets_context_menu_ui_contract_smoke.sh
cargo test --test assets_context_menu_spec --test assets_context_menu_smoke -- --nocapture
```

Expected:

- PASS

**Step 5: Commit**

```bash
git add src/shell/context_menu.rs src/shell/view_model.rs src/app/bootstrap.rs ui/components/assets-context-menu-overlay.slint ui/app-window.slint tests/assets_context_menu_spec.rs tests/assets_context_menu_ui_contract_smoke.sh tests/assets_context_menu_smoke.rs
git commit -m "feat: add context menu placement and corridor behavior"
```

## Task 7: 实现基础键盘闭环、planned-action 反馈与文档同步

**Files:**
- Modify: `src/shell/view_model.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `ui/app-window.slint`
- Modify: `ui/components/assets-context-menu-overlay.slint`
- Modify: `docs/plans/2026-03-17-windows-console-assets-context-menu-unimplemented-actions.md`
- Modify: `verification.md`
- Test: `tests/assets_context_menu_spec.rs`
- Test: `tests/assets_context_menu_smoke.rs`
- Test: `tests/assets_context_menu_ui_contract_smoke.sh`

**Step 1: Write the failing keyboard and feedback tests**

在 `tests/assets_context_menu_spec.rs` 增加：

```rust
#[test]
fn esc_closes_context_menu() {}

#[test]
fn right_key_opens_submenu_and_left_key_returns_to_parent() {}

#[test]
fn invoking_planned_action_sets_feedback_text_without_closing_documentation_gap() {}
```

在 `tests/assets_context_menu_smoke.rs` 增加：

```rust
#[test]
fn invoking_planned_action_shows_status_pill_feedback() {}
```

在 `tests/assets_context_menu_ui_contract_smoke.sh` 增加：

```bash
grep -F 'key-pressed(event) => {' "$ROOT_DIR/ui/components/assets-context-menu-overlay.slint" >/dev/null
grep -F 'event.text == Key.Escape' "$ROOT_DIR/ui/components/assets-context-menu-overlay.slint" >/dev/null
grep -F 'StatusPill {' "$ROOT_DIR/ui/app-window.slint" >/dev/null
grep -F 'root.context-menu-feedback-text' "$ROOT_DIR/ui/app-window.slint" >/dev/null
```

**Step 2: Run tests to verify they fail**

Run:

```bash
bash tests/assets_context_menu_ui_contract_smoke.sh
cargo test --test assets_context_menu_spec --test assets_context_menu_smoke -- --nocapture
```

Expected:

- FAIL

**Step 3: Write the minimal keyboard and feedback implementation**

1. `ui/components/assets-context-menu-overlay.slint`

- 包裹 `FocusScope`
- 处理：
  - `Esc`
  - `Left`
  - `Right`
  - `Enter`

2. `src/shell/view_model.rs`

- 新增：

```rust
pub fn navigate_context_menu_left(&mut self) {}
pub fn navigate_context_menu_right(&mut self) {}
pub fn invoke_current_context_menu_item(&mut self) {}
pub fn set_context_menu_feedback(&mut self, text: impl Into<String>) {}
```

3. `src/app/bootstrap.rs`

- 绑定键盘 callback
- planned action 不做假执行，只写入反馈文本和 tracing

4. `ui/app-window.slint`

- 复用 `ui/components/status-pill.slint`
- 根层挂一个临时 `StatusPill`
- 仅在 `context-menu-feedback-text != ""` 时显示

5. `docs/plans/2026-03-17-windows-console-assets-context-menu-unimplemented-actions.md`

- 按实际已接线动作更新：
  - 已接通的动作移出清单或改状态
  - 仍未接线但可点击的动作保留

6. `verification.md`

- 新增 `Windows Console Assets Context Menu Verification` 小节

**Step 4: Re-run the focused tests**

Run:

```bash
bash tests/assets_context_menu_ui_contract_smoke.sh
cargo test --test assets_context_menu_spec --test assets_context_menu_smoke -- --nocapture
```

Expected:

- PASS

**Step 5: Commit**

```bash
git add src/shell/view_model.rs src/app/bootstrap.rs ui/app-window.slint ui/components/assets-context-menu-overlay.slint docs/plans/2026-03-17-windows-console-assets-context-menu-unimplemented-actions.md verification.md tests/assets_context_menu_spec.rs tests/assets_context_menu_smoke.rs tests/assets_context_menu_ui_contract_smoke.sh
git commit -m "feat: add keyboard and planned-action feedback to assets context menu"
```

## Task 8: 全量验证与回归

**Files:**
- Verify only: `src/shell/*.rs`
- Verify only: `src/app/bootstrap.rs`
- Verify only: `ui/app-window.slint`
- Verify only: `ui/shell/assets-sidebar.slint`
- Verify only: `ui/components/assets-context-menu-*.slint`
- Verify only: `docs/plans/2026-03-17-windows-console-assets-context-menu-unimplemented-actions.md`
- Verify only: `verification.md`

**Step 1: Run the new targeted test suite**

Run:

```bash
cargo test --test assets_context_menu_spec --test assets_context_menu_smoke --test shell_view_model -- --nocapture
bash tests/assets_context_menu_ui_contract_smoke.sh
```

Expected:

- PASS

**Step 2: Run existing regression tests that the feature touches**

Run:

```bash
cargo test --test assets_sidebar_toolbar_spec --test assets_sidebar_toolbar_smoke --test window_shell -q
bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh
bash tests/sidebar_ui_contract_smoke.sh
```

Expected:

- PASS，确保新的 root overlay 没有破坏现有 toolbar 与 shell contract

**Step 3: Run compile / lint verification**

Run:

```bash
cargo check --workspace
cargo clippy --workspace -- -D warnings
```

Expected:

- PASS

**Step 4: Sanity-review docs and TODO boundaries**

人工自查清单：

- `12B` 完整键盘体验没有被偷偷实现
- 未接线动作文档和实际反馈保持一致
- `planned` 与 `disabled` 语义没有混淆
- 没有把业务规则塞回 Slint 本地状态

**Step 5: Commit**

```bash
git add verification.md docs/plans/2026-03-17-windows-console-assets-context-menu-unimplemented-actions.md
git commit -m "test: verify windows console assets context menu"
```

## Final Deliverables Checklist

- `Window Console` 面板存在最小可右击节点壳层
- 三类场景右键菜单组合正确
- 根窗口自绘 overlay 支持最多三级菜单
- submenu hover 延迟、点击立即展开和 corridor 容错生效
- `Esc` / `Enter` / `Left` / `Right` 基础闭环生效
- disabled 与 planned 状态分离
- planned action 触发轻量反馈
- 未接线动作文档同步更新
- 现有 assets toolbar / sidebar / shell regression 全部通过

## Explicit Non-Goals For This Plan

- 不实现 `12B` 完整桌面菜单键盘体验
- 不接入真实 SSH / SFTP 运行时
- 不实现多选资产树，只为后续多选预留状态模型
- 不引入新的 toast 系统，优先复用现有 `StatusPill`
- 不改动 renderer、window frame 或 titlebar 体系

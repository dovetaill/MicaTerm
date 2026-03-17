# Assets Sidebar Toolbar Bugfix2 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 基于已确认的 `A1 + B1 + C2 + D2` 方案，完成 `AssetsSidebar` 顶部工具区 bugfix2：让 Search 贴住工具区下沿、修正输入 caret/文字区度量、将 `Create` 迁移为根层 overlay 并统一 menu row，对 outside-click、`Esc` 和互斥行为做完整收敛。

**Architecture:** 保持现有 `AppWindow -> Sidebar -> AssetsSidebar` 壳层结构不变。`AssetsSidebar` 继续作为 toolbar 几何来源，但 Search 的 anchor 不再取自 `search-button`，而是取自顶部工具区内容矩形；`Create` 放弃 `PopupWindow`，与 Search 一样改为 `AppWindow` 根层 overlay。Rust `ShellViewModel` 仍是 `asset_search_expanded`、`assets_search_query`、`asset_create_menu_open` 的唯一业务状态源，UI 层负责 anchor、焦点与键盘事件。

**Tech Stack:** Rust 2024, Slint 1.15.1, `TextInput`, `FocusScope`, root overlay `TouchArea`, `i-slint-backend-testing`, shell smoke scripts, `cargo test`, `cargo check`

---

## Execution Notes

- 设计输入固定为 [2026-03-16-assets-sidebar-toolbar-bugfix2-design.md](/home/wwwroot/mica-term/docs/plans/2026-03-16-assets-sidebar-toolbar-bugfix2-design.md)，实现时不得回退到旧的 `search-button anchor + PopupWindow Create` 混合方案。
- 每个任务都先走 `@superpowers:test-driven-development`：先补失败测试或失败 smoke，再做最小实现，再跑通过。
- 如果 `Slint` 的根层 overlay 命中、焦点或 `Esc` 处理与计划不一致，立即切到 `@superpowers:systematic-debugging`，不要猜。
- 当前仓库已经完成 `288px` 侧栏宽度调整，这一轮 implementation plan 不重复规划宽度变更。
- 本轮不接入真实 terminal / SSH / SFTP runtime，不实现真实搜索过滤、创建向导、标题栏菜单统一化或动画系统扩展。
- 默认在独立 worktree 执行；如果继续在当前工作区执行，改动范围必须严格限制在本文档列出的文件。

## Current Code Map

- `ui/shell/assets-sidebar.slint`
  - 当前 Search anchor 仍直接取自 `search-button.absolute-position`
  - 顶部工具区没有单独命名的 content rect，无法作为整条 Search 的几何基准
  - `Create` 仍只输出 button anchor，不负责 overlay 宿主
- `ui/components/assets-search-popover.slint`
  - 当前仍是 `Rectangle + TextInput`
  - `border-radius` 仍为 `8px`
  - 没有 `close-requested()`，也没有 `Esc` 键处理
- `ui/components/assets-create-menu.slint`
  - 当前仍 `inherits PopupWindow`
  - 使用 `PopupClosePolicy.no-auto-close`
  - 行布局虽然是 `Image + Text`，但没有抽象成可复用的统一 menu row
- `ui/app-window.slint`
  - 当前 Search 是根层 overlay，但几何仍取自 `sidebar.search-anchor-*`
  - 当前 `Create` 仍通过 `PopupWindow.show()/close()` 管理宿主
  - outside-click 由根层 `overlay-dismiss-layer` 处理
- `src/shell/view_model.rs`
  - 当前 `toggle_asset_search()` 只会把 Search 置为 `true`
  - 当前没有“强制关闭 Search”的方法
  - 当前 `toggle_asset_create_menu()` 不会自动关闭 Search
- `src/app/bootstrap.rs`
  - 当前 Search 与 Create 的状态变更都只是简单透传，尚未收敛互斥语义
- `tests/assets_sidebar_toolbar_ui_contract_smoke.sh`
  - 当前仍断言 `AssetsCreateMenu inherits PopupWindow`
  - 当前没有 `Esc` 和 menu row 的契约断言
- `tests/assets_sidebar_toolbar_smoke.rs` / `tests/assets_sidebar_toolbar_spec.rs`
  - 当前只覆盖了基础 toggle / close round-trip
  - 尚未覆盖 Search 与 Create 的互斥行为，也未覆盖 Search 的 force-close 语义

## Task 1: Re-anchor Search To The Toolbar Content Rect

**Files:**
- Modify: `ui/shell/assets-sidebar.slint`
- Modify: `ui/shell/sidebar.slint`
- Modify: `ui/app-window.slint`
- Modify: `tests/assets_sidebar_toolbar_ui_contract_smoke.sh`
- Modify: `tests/assets_sidebar_toolbar_smoke.rs`

**Step 1: Write the failing UI contract and smoke test**

先让契约明确 Search anchor 取自工具区内容矩形，而不是搜索按钮本身。

在 `tests/assets_sidebar_toolbar_ui_contract_smoke.sh` 中新增或替换以下断言：

```bash
grep -F 'toolbar-content := HorizontalLayout {' "$ASSETS" >/dev/null
grep -F 'out property <length> search-anchor-x: toolbar-content.absolute-position.x;' "$ASSETS" >/dev/null
grep -F 'out property <length> search-anchor-y: toolbar-content.absolute-position.y;' "$ASSETS" >/dev/null
grep -F 'out property <length> search-anchor-width: toolbar-content.width;' "$ASSETS" >/dev/null
grep -F 'out property <length> search-anchor-height: toolbar-content.height;' "$ASSETS" >/dev/null
grep -F 'callback focus-assets-search-requested();' "$ASSETS" >/dev/null
grep -F 'callback focus-assets-search-requested();' "$SIDEBAR" >/dev/null
grep -F 'focus-assets-search-requested => {' "$APP_WINDOW" >/dev/null
grep -F 'assets-search-overlay.focus-input();' "$APP_WINDOW" >/dev/null
```

在 `tests/assets_sidebar_toolbar_smoke.rs` 新增一个宽度阈值测试，确保 anchor 不再是 `28px` 图标按钮宽度：

```rust
#[test]
fn search_anchor_tracks_toolbar_content_rect() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    assert!(app.get_layout_assets_search_anchor_width() > 160.0);
    assert!(app.get_layout_assets_search_anchor_height() >= 28.0);
}
```

**Step 2: Run tests to verify they fail**

Run:

```bash
bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh
cargo test --test assets_sidebar_toolbar_smoke search_anchor_tracks_toolbar_content_rect -- --nocapture
```

Expected:

- shell contract FAIL，因为当前 `search-anchor-*` 仍指向 `search-button.absolute-position`
- Rust smoke FAIL，因为当前 anchor width 仍接近 `28px`

**Step 3: Write the minimal anchor implementation**

1. `ui/shell/assets-sidebar.slint`

- 给 header 内部的 `HorizontalLayout` 命名为 `toolbar-content`
- 将 Search anchor 属性从按钮切换到工具区内容矩形
- 增加重复点击 Search 时的 refocus callback

建议实现：

```slint
out property <length> search-anchor-x: toolbar-content.absolute-position.x;
out property <length> search-anchor-y: toolbar-content.absolute-position.y;
out property <length> search-anchor-width: toolbar-content.width;
out property <length> search-anchor-height: toolbar-content.height;
callback focus-assets-search-requested();

toolbar-content := HorizontalLayout {
    // 保留现有 padding / spacing
}

search-button := SidebarToolbarIconButton {
    icon-source: root.search-icon;
    active: root.asset-search-expanded;
    clicked => {
        if root.asset-search-expanded {
            root.focus-assets-search-requested();
        } else {
            root.toggle-assets-search-requested();
        }
    }
}
```

2. `ui/shell/sidebar.slint`

- 透传新的 `focus-assets-search-requested()` callback

```slint
callback focus-assets-search-requested();

assets-sidebar := AssetsSidebar {
    focus-assets-search-requested => {
        root.focus-assets-search-requested();
    }
}
```

3. `ui/app-window.slint`

- 在 `sidebar := Sidebar { ... }` 绑定中接住 `focus-assets-search-requested`
- 只调用 `assets-search-overlay.focus-input()`，不要走 Rust 状态更新

```slint
focus-assets-search-requested => {
    assets-search-overlay.focus-input();
}
```

**Step 4: Re-run the focused tests**

Run:

```bash
bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh
cargo test --test assets_sidebar_toolbar_smoke search_anchor_tracks_toolbar_content_rect -- --nocapture
```

Expected: PASS。

**Step 5: Commit**

```bash
git add ui/shell/assets-sidebar.slint ui/shell/sidebar.slint ui/app-window.slint tests/assets_sidebar_toolbar_ui_contract_smoke.sh tests/assets_sidebar_toolbar_smoke.rs
git commit -m "fix: anchor assets search to toolbar content"
```

## Task 2: Normalize Search Input Metrics And Add `Esc` Close

**Files:**
- Modify: `ui/components/assets-search-popover.slint`
- Modify: `ui/app-window.slint`
- Modify: `src/shell/view_model.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `tests/assets_sidebar_toolbar_ui_contract_smoke.sh`
- Modify: `tests/assets_sidebar_toolbar_smoke.rs`
- Modify: `tests/assets_sidebar_toolbar_spec.rs`

**Step 1: Write the failing tests**

先为 Search 的 force-close 语义和 `Esc` 契约补测试。

在 `tests/assets_sidebar_toolbar_spec.rs` 新增：

```rust
#[test]
fn force_closing_search_hides_it_even_with_query() {
    let mut view_model = ShellViewModel::default();

    view_model.toggle_asset_search();
    view_model.set_asset_search_query("prod".into());
    view_model.close_asset_search();

    assert!(!view_model.asset_search_expanded);
    assert_eq!(view_model.asset_search_query, "prod");
}
```

在 `tests/assets_sidebar_toolbar_smoke.rs` 新增：

```rust
#[test]
fn close_assets_search_requested_hides_non_empty_search() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_toggle_assets_search_requested();
    app.invoke_assets_search_query_changed("prod".into());
    app.invoke_close_assets_search_requested();

    assert!(!app.get_asset_search_expanded());
    assert_eq!(app.get_assets_search_query().as_str(), "prod");
}
```

在 `tests/assets_sidebar_toolbar_ui_contract_smoke.sh` 增加：

```bash
grep -F 'callback close-requested();' "$SEARCH" >/dev/null
grep -F 'border-radius: 0px;' "$SEARCH" >/dev/null
grep -F 'font-size: 13px;' "$SEARCH" >/dev/null
grep -F 'key-pressed(event) => {' "$SEARCH" >/dev/null
grep -F 'event.text == Key.Escape' "$SEARCH" >/dev/null
grep -F 'callback close-assets-search-requested();' "$APP_WINDOW" >/dev/null
```

**Step 2: Run tests to verify they fail**

Run:

```bash
bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh
cargo test --test assets_sidebar_toolbar_spec force_closing_search_hides_it_even_with_query -- --nocapture
cargo test --test assets_sidebar_toolbar_smoke close_assets_search_requested_hides_non_empty_search -- --nocapture
```

Expected:

- shell contract FAIL，因为 Search 还没有 `close-requested` 和 `Esc` 处理
- unit/spec FAIL，因为 `ShellViewModel` 还没有 `close_asset_search()`
- smoke FAIL，因为 `AppWindow` 还没有 `close-assets-search-requested()`

**Step 3: Write the minimal implementation**

1. `src/shell/view_model.rs`

新增强制关闭 Search 的方法：

```rust
pub fn close_asset_search(&mut self) {
    self.asset_search_expanded = false;
}
```

2. `src/app/bootstrap.rs`

为 `window.on_close_assets_search_requested(...)` 增加绑定：

```rust
window.on_close_assets_search_requested(move || {
    let window = handle.unwrap();
    let mut state = state.borrow_mut();
    state.close_asset_search();
    sync_assets_toolbar_state(&window, &state);
});
```

3. `ui/app-window.slint`

- 增加 callback：

```slint
callback close-assets-search-requested();
```

- 将 Search overlay 的 `close-requested` 回调接回 root：

```slint
assets-search-overlay := AssetsSearchPopover {
    visible: root.asset-search-expanded;
    x: sidebar.search-anchor-x;
    y: sidebar.search-anchor-y + sidebar.search-anchor-height + 6px;
    width: sidebar.search-anchor-width;
    query: root.assets-search-query;

    close-requested => {
        root.close-assets-search-requested();
    }
}
```

4. `ui/components/assets-search-popover.slint`

- 增加 `close-requested()` callback
- 移除圆角，收敛为方角输入壳
- 明确单行输入度量
- 在 `TextInput.key-pressed` 中处理 `Esc`

建议结构：

```slint
import { ThemeTokens } from "../theme/tokens.slint";

export component AssetsSearchPopover inherits Rectangle {
    in property <string> query: "";
    callback query-changed(string);
    callback close-requested();

    height: 34px;
    border-radius: 0px;
    border-width: 1px;
    border-color: ThemeTokens.shell-stroke;
    background: ThemeTokens.panel-tint;
    z: 100;

    public function focus-input() {
        search-input.focus();
    }

    search-input := TextInput {
        x: 10px;
        y: 7px;
        width: parent.width - 20px;
        height: 20px;
        font-size: 13px;
        text: root.query;

        edited => {
            root.query-changed(self.text);
        }

        key-pressed(event) => {
            if (event.text == Key.Escape) {
                root.close-requested();
                accept
            } else {
                reject
            }
        }
    }
}
```

**Step 4: Re-run the focused tests**

Run:

```bash
bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh
cargo test --test assets_sidebar_toolbar_spec force_closing_search_hides_it_even_with_query -- --nocapture
cargo test --test assets_sidebar_toolbar_smoke close_assets_search_requested_hides_non_empty_search -- --nocapture
```

Expected: PASS。

**Step 5: Commit**

```bash
git add ui/components/assets-search-popover.slint ui/app-window.slint src/shell/view_model.rs src/app/bootstrap.rs tests/assets_sidebar_toolbar_ui_contract_smoke.sh tests/assets_sidebar_toolbar_smoke.rs tests/assets_sidebar_toolbar_spec.rs
git commit -m "fix: tighten assets search overlay metrics"
```

## Task 3: Replace `AssetsCreateMenu` PopupWindow With A Root Overlay And Shared Menu Row

**Files:**
- Create: `ui/components/assets-toolbar-menu-row.slint`
- Modify: `ui/components/assets-create-menu.slint`
- Modify: `ui/app-window.slint`
- Modify: `tests/assets_sidebar_toolbar_ui_contract_smoke.sh`

**Step 1: Write the failing UI contract**

在 `tests/assets_sidebar_toolbar_ui_contract_smoke.sh` 中替换旧的 PopupWindow 断言，并新增统一 menu row 契约：

```bash
ROW="$ROOT_DIR/ui/components/assets-toolbar-menu-row.slint"

grep -F 'export component AssetsToolbarMenuRow inherits Rectangle' "$ROW" >/dev/null
grep -F 'icon-slot := Rectangle' "$ROW" >/dev/null
grep -F 'text-slot := Rectangle' "$ROW" >/dev/null
grep -F 'export component AssetsCreateMenu inherits Rectangle' "$MENU" >/dev/null
! grep -F 'export component AssetsCreateMenu inherits PopupWindow' "$MENU" >/dev/null
! grep -F 'close-policy:' "$MENU" >/dev/null
grep -F 'public function focus-menu()' "$MENU" >/dev/null
grep -F 'visible: root.asset-create-menu-open;' "$APP_WINDOW" >/dev/null
! grep -F 'assets-create-menu-overlay.show();' "$APP_WINDOW" >/dev/null
! grep -F 'assets-create-menu-overlay.close();' "$APP_WINDOW" >/dev/null
```

**Step 2: Run the contract to verify it fails**

Run:

```bash
bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh
```

Expected: FAIL，因为当前 `AssetsCreateMenu` 仍然是 `PopupWindow`，也没有单独的 `AssetsToolbarMenuRow`。

**Step 3: Write the minimal overlay migration**

1. 创建 `ui/components/assets-toolbar-menu-row.slint`

建议结构：

```slint
import { ThemeTokens } from "../theme/tokens.slint";

export component AssetsToolbarMenuRow inherits Rectangle {
    in property <string> label;
    in property <image> icon-source;
    callback invoked;

    height: 36px;
    border-radius: 0px;
    background: touch.pressed
        ? ThemeTokens.command-tint
        : touch.has-hover
            ? ThemeTokens.shell-surface
            : transparent;

    HorizontalLayout {
        padding-left: 12px;
        padding-right: 12px;
        spacing: 10px;

        icon-slot := Rectangle {
            width: 16px;
            height: 16px;
            background: transparent;

            Image {
                width: 16px;
                height: 16px;
                source <=> root.icon-source;
                image-fit: contain;
                colorize: ThemeTokens.text-primary;
            }
        }

        text-slot := Rectangle {
            vertical-stretch: 1;
            background: transparent;

            Text {
                text: root.label;
                color: ThemeTokens.text-primary;
                vertical-alignment: center;
            }
        }

        Rectangle {
            horizontal-stretch: 1;
            background: transparent;
        }
    }

    touch := TouchArea {
        clicked => {
            root.invoked();
        }
    }
}
```

2. 修改 `ui/components/assets-create-menu.slint`

- 改为 `inherits Rectangle`
- 使用 `AssetsToolbarMenuRow`
- 增加 `public function focus-menu()`
- 使用 `FocusScope` 处理 `Esc`

建议结构：

```slint
import { ThemeTokens } from "../theme/tokens.slint";
import { AssetsToolbarMenuRow } from "assets-toolbar-menu-row.slint";

export component AssetsCreateMenu inherits Rectangle {
    callback new-folder-selected;
    callback new-ssh-connection-selected;
    callback close-requested;

    width: 216px;
    height: 88px;
    border-radius: 0px;
    border-width: 1px;
    border-color: ThemeTokens.shell-stroke;
    background: ThemeTokens.panel-tint;
    z: 110;

    public function focus-menu() {
        menu-focus.focus();
    }

    menu-focus := FocusScope {
        width: parent.width;
        height: parent.height;

        key-pressed(event) => {
            if (event.text == Key.Escape) {
                root.close-requested();
                accept
            } else {
                reject
            }
        }
    }

    VerticalLayout {
        padding: 8px;
        spacing: 4px;

        AssetsToolbarMenuRow {
            label: "New Folder";
            icon-source: root.new-folder-icon;
            invoked => {
                root.new-folder-selected();
                root.close-requested();
            }
        }

        AssetsToolbarMenuRow {
            label: "New SSH Connection";
            icon-source: root.new-ssh-connection-icon;
            invoked => {
                root.new-ssh-connection-selected();
                root.close-requested();
            }
        }
    }
}
```

3. 修改 `ui/app-window.slint`

- 删除 `changed asset-create-menu-open` 中的 `.show()` / `.close()`
- 改为可见性驱动
- 在打开时聚焦菜单

建议实现：

```slint
changed asset-create-menu-open => {
    if root.asset-create-menu-open {
        assets-create-menu-overlay.focus-menu();
    }
}

assets-create-menu-overlay := AssetsCreateMenu {
    visible: root.asset-create-menu-open;
    x: sidebar.create-menu-anchor-x + sidebar.create-menu-anchor-width - self.width;
    y: sidebar.create-menu-anchor-y + sidebar.create-menu-anchor-height + 6px;

    close-requested => {
        root.close-assets-create-menu-requested();
    }
}
```

**Step 4: Re-run the contract**

Run:

```bash
bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh
```

Expected: PASS。

**Step 5: Commit**

```bash
git add ui/components/assets-toolbar-menu-row.slint ui/components/assets-create-menu.slint ui/app-window.slint tests/assets_sidebar_toolbar_ui_contract_smoke.sh
git commit -m "refactor: move assets create menu to root overlay"
```

## Task 4: Enforce Mutual Exclusion And Finalize Toolbar Dismiss Semantics

**Files:**
- Modify: `src/shell/view_model.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `tests/assets_sidebar_toolbar_spec.rs`
- Modify: `tests/assets_sidebar_toolbar_smoke.rs`
- Modify: `tests/assets_sidebar_toolbar_ui_contract_smoke.sh`

**Step 1: Write the failing unit and smoke tests**

在 `tests/assets_sidebar_toolbar_spec.rs` 新增：

```rust
#[test]
fn activating_search_closes_create_menu() {
    let mut view_model = ShellViewModel::default();

    view_model.toggle_asset_create_menu();
    assert!(view_model.asset_create_menu_open);

    view_model.activate_asset_search();

    assert!(view_model.asset_search_expanded);
    assert!(!view_model.asset_create_menu_open);
}

#[test]
fn opening_create_menu_closes_search_even_when_query_exists() {
    let mut view_model = ShellViewModel::default();

    view_model.activate_asset_search();
    view_model.set_asset_search_query("prod".into());
    view_model.toggle_asset_create_menu();

    assert!(view_model.asset_create_menu_open);
    assert!(!view_model.asset_search_expanded);
    assert_eq!(view_model.asset_search_query, "prod");
}
```

在 `tests/assets_sidebar_toolbar_smoke.rs` 新增：

```rust
#[test]
fn search_and_create_are_mutually_exclusive_in_window_contract() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_toggle_assets_search_requested();
    assert!(app.get_asset_search_expanded());
    assert!(!app.get_asset_create_menu_open());

    app.invoke_assets_search_query_changed("prod".into());
    app.invoke_toggle_assets_create_menu_requested();
    assert!(app.get_asset_create_menu_open());
    assert!(!app.get_asset_search_expanded());

    app.invoke_toggle_assets_search_requested();
    assert!(app.get_asset_search_expanded());
    assert!(!app.get_asset_create_menu_open());
}
```

在 `tests/assets_sidebar_toolbar_ui_contract_smoke.sh` 增加新的 callback 契约：

```bash
grep -F 'callback close-assets-search-requested();' "$APP_WINDOW" >/dev/null
grep -F 'callback focus-assets-search-requested();' "$ASSETS" >/dev/null
grep -F 'callback focus-assets-search-requested();' "$SIDEBAR" >/dev/null
```

**Step 2: Run tests to verify they fail**

Run:

```bash
bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh
cargo test --test assets_sidebar_toolbar_spec -- --nocapture
cargo test --test assets_sidebar_toolbar_smoke search_and_create_are_mutually_exclusive_in_window_contract -- --nocapture
```

Expected:

- spec FAIL，因为 `ShellViewModel` 还没有 `activate_asset_search()`
- smoke FAIL，因为当前 Create 打开后不会自动关闭 Search

**Step 3: Write the minimal state and bootstrap implementation**

1. `src/shell/view_model.rs`

新增并替换 Search/Create 相关状态方法：

```rust
pub fn activate_asset_search(&mut self) {
    self.asset_search_expanded = true;
    self.asset_create_menu_open = false;
}

pub fn close_asset_search(&mut self) {
    self.asset_search_expanded = false;
}

pub fn toggle_asset_create_menu(&mut self) {
    if self.asset_create_menu_open {
        self.asset_create_menu_open = false;
    } else {
        self.asset_create_menu_open = true;
        self.asset_search_expanded = false;
    }
}
```

保留 `collapse_asset_search_if_empty()`，继续服务 outside-click 的“空值才关闭”规则。

2. `src/app/bootstrap.rs`

- `on_toggle_assets_search_requested` 改为调用 `activate_asset_search()`
- `on_close_assets_search_requested` 调用 `close_asset_search()`
- `on_toggle_assets_create_menu_requested` 保持走 `toggle_asset_create_menu()`

建议实现：

```rust
window.on_toggle_assets_search_requested(move || {
    let window = handle.unwrap();
    let mut state = state.borrow_mut();
    state.activate_asset_search();
    sync_assets_toolbar_state(&window, &state);
});
```

**Step 4: Run the full toolbar-focused verification**

Run:

```bash
bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh
cargo test --test assets_sidebar_toolbar_spec -- --nocapture
cargo test --test assets_sidebar_toolbar_smoke -- --nocapture
cargo check
```

Expected:

- `assets_sidebar_toolbar_ui_contract_smoke.sh`: PASS
- `assets_sidebar_toolbar_spec`: PASS
- `assets_sidebar_toolbar_smoke`: PASS
- `cargo check`: PASS

**Step 5: Commit**

```bash
git add src/shell/view_model.rs src/app/bootstrap.rs tests/assets_sidebar_toolbar_spec.rs tests/assets_sidebar_toolbar_smoke.rs tests/assets_sidebar_toolbar_ui_contract_smoke.sh
git commit -m "fix: unify assets toolbar overlay state"
```

## Final Verification Checklist

- Search anchor width 明显大于 icon button 宽度，且随工具区内容区而非按钮位置变化。
- Search 整体贴在工具区正下方，窗口尺寸变化后不再向右漂移。
- Search 保持方角风格，不引入圆角回退。
- Search 输入 caret、文字区和上下留白协调，重复点击 Search 按钮会重新聚焦。
- `Esc` 能无条件关闭 Search。
- `Create` 不再依赖 `PopupWindow`，而是与 Search 一样使用根层 overlay。
- `Create` 菜单项统一使用共享 menu row 规则，图标与文字对齐稳定。
- `Esc` 能无条件关闭 `Create`。
- outside-click 对 `Create` 无条件关闭，对 `Search` 仅在 query 为空时关闭。
- `Search` 与 `Create` 全程互斥，绝不同时展开。
- 现有 tooltip、titlebar menu 和 sidebar 主体布局没有明显回归。

## Handoff

- 参考设计文档：[2026-03-16-assets-sidebar-toolbar-bugfix2-design.md](/home/wwwroot/mica-term/docs/plans/2026-03-16-assets-sidebar-toolbar-bugfix2-design.md)
- 本计划刻意不包含标题栏菜单统一化；如后续要统一 `TitlebarMenu` 与 `Assets` 工具区菜单，请另开设计文档。

# Assets Sidebar Toolbar Bugfix3 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 基于已确认的 `P1B + P2A + P3A + P4A` 方案，修复 `AssetsSidebar` 顶部 Search 的两个遗留问题：空搜索框点击外部时要稳定收起，暗色模式下输入文字必须具备正确对比度。

**Architecture:** 保持现有 `AppWindow -> Sidebar -> AssetsSidebar -> AssetsSearchPopover` inline search row 结构，不回退到 root search overlay。交互上新增显式 click-away host：在 `AssetsSidebar` 的背景宿主和 `AppWindow` 的 workspace 区域显式转发 `collapse-assets-search-requested()`，但仅在 `asset_search_expanded && assets_search_query == ""` 时启用；关闭语义仍由 Rust `ShellViewModel::collapse_asset_search_if_empty()` 与 `close_asset_search()` 决定。视觉上不新增局部硬编码颜色，优先复用 `ThemeTokens` 为 `TextInput` 显式绑定前景色与选区颜色。

**Tech Stack:** Rust 2024, Slint 1.15.1, `TextInput`, `TouchArea`, `i-slint-backend-testing`, shell contract smoke tests, `cargo test`, `cargo check`

---

## Execution Notes

- 我正在使用 `writing-plans` skill 来创建 implementation plan。
- 设计输入固定为 [2026-03-16-assets-sidebar-toolbar-bugfix3-design.md](/home/wwwroot/mica-term/docs/plans/2026-03-16-assets-sidebar-toolbar-bugfix3-design.md)；实施时不得回退到 root search overlay，也不得把 click-away 改成一律 `close_asset_search()`。
- 本计划按 `@superpowers:test-driven-development` 执行：先补失败 contract，再做最小实现，再跑通过。
- 如果 Slint `TouchArea` 的命中顺序与预期不一致，立即切到 `@superpowers:systematic-debugging`，不要猜。
- 本轮不改 terminal runtime、renderer、SSH/SFTP，不改 `Create` 菜单结构，不重命名组件文件。
- 现有状态语义已经正确，重点是 UI 层把它稳定触发出来。

## Current Code Map

- [ui/components/assets-search-popover.slint](/home/wwwroot/mica-term/ui/components/assets-search-popover.slint)
  - 现在已经有 `collapse-requested()` 与 `close-requested()`
  - 现在只依赖 `changed has-focus` 作为 collapse 兜底
  - 现在没有显式 `TextInput` 前景色或选区颜色
- [ui/shell/assets-sidebar.slint](/home/wwwroot/mica-term/ui/shell/assets-sidebar.slint)
  - Search 已经是 `search-row-host` 下的 inline row
  - Search 以外还没有显式 click-away host
- [ui/shell/sidebar.slint](/home/wwwroot/mica-term/ui/shell/sidebar.slint)
  - 只是 callback/geometry 透传层，本轮不应引入新状态
- [ui/app-window.slint](/home/wwwroot/mica-term/ui/app-window.slint)
  - `overlay-dismiss-layer` 只服务 `Create` 菜单
  - 目前没有 Search workspace click-away host
- [ui/theme/tokens.slint](/home/wwwroot/mica-term/ui/theme/tokens.slint)
  - 已有 `text-primary`、`accent`、`panel-tint`，优先复用
- [tests/assets_sidebar_toolbar_ui_contract_smoke.sh](/home/wwwroot/mica-term/tests/assets_sidebar_toolbar_ui_contract_smoke.sh)
  - 目前覆盖了 inline row 与 Search 基本结构
  - 还没有锁定 click-away host 和输入文字颜色
- [tests/assets_sidebar_toolbar_spec.rs](/home/wwwroot/mica-term/tests/assets_sidebar_toolbar_spec.rs)
  - 已锁定 `collapse_asset_search_if_empty()` 与 `close_asset_search()` 语义
- [tests/assets_sidebar_toolbar_smoke.rs](/home/wwwroot/mica-term/tests/assets_sidebar_toolbar_smoke.rs)
  - 已覆盖 Search row 高度与基本开关
  - 适合作为 UI contract 之外的窗口桥接护栏

## Task 1: Tighten The Contract Around Click-Away And Input Theme

**Files:**
- Modify: `tests/assets_sidebar_toolbar_ui_contract_smoke.sh`
- Test: `tests/assets_sidebar_toolbar_spec.rs`
- Test: `tests/assets_sidebar_toolbar_smoke.rs`

**Step 1: Write the failing contract**

在 [tests/assets_sidebar_toolbar_ui_contract_smoke.sh](/home/wwwroot/mica-term/tests/assets_sidebar_toolbar_ui_contract_smoke.sh) 追加以下断言，先把本轮确认的结构写成失败契约：

```bash
grep -F 'header-search-dismiss-touch := TouchArea {' "$ASSETS" >/dev/null
grep -F 'panel-search-dismiss-touch := TouchArea {' "$ASSETS" >/dev/null
grep -F 'workspace-search-dismiss-layer := TouchArea {' "$APP_WINDOW" >/dev/null
grep -F 'enabled: root.asset-search-expanded && root.assets-search-query == "";' "$APP_WINDOW" >/dev/null
grep -F 'enabled: root.asset-search-expanded && root.assets-search-query == "";' "$ASSETS" >/dev/null
grep -F 'root.collapse-assets-search-requested();' "$APP_WINDOW" >/dev/null
grep -F 'color: ThemeTokens.text-primary;' "$SEARCH" >/dev/null
grep -F 'selection-background-color: ThemeTokens.accent;' "$SEARCH" >/dev/null
grep -F 'selection-foreground-color: ThemeTokens.text-primary;' "$SEARCH" >/dev/null
! grep -F '#101418' "$SEARCH" >/dev/null
! grep -F '#f5f7fb' "$SEARCH" >/dev/null
```

不要改动已有的状态语义测试，只把下面这些测试作为回归护栏继续跑：

```bash
cargo test --test assets_sidebar_toolbar_spec collapsing_empty_search_hides_search_row -- --nocapture
cargo test --test assets_sidebar_toolbar_spec non_empty_search_stays_open_when_focus_leaves -- --nocapture
cargo test --test assets_sidebar_toolbar_spec force_closing_search_hides_it_even_with_query -- --nocapture
cargo test --test assets_sidebar_toolbar_smoke search_row_occupies_height_only_when_expanded -- --nocapture
```

**Step 2: Run test to verify it fails**

Run:

```bash
bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh
```

Expected:

- FAIL，因为当前没有显式 click-away host
- FAIL，因为当前 `TextInput` 没有显式前景色与选区颜色

**Step 3: Do not implement yet**

这一 task 只负责把失败 contract 写出来，不改任何业务代码。

**Step 4: Commit the failing contract**

```bash
git add tests/assets_sidebar_toolbar_ui_contract_smoke.sh
git commit -m "test: lock assets search click-away and theme contract"
```

## Task 2: Add Explicit Click-Away Hosts Without Changing State Semantics

**Files:**
- Modify: `ui/shell/assets-sidebar.slint`
- Modify: `ui/app-window.slint`
- Test: `tests/assets_sidebar_toolbar_ui_contract_smoke.sh`
- Test: `tests/assets_sidebar_toolbar_spec.rs`
- Test: `tests/assets_sidebar_toolbar_smoke.rs`

**Step 1: Write the minimal implementation in `AssetsSidebar`**

在 [ui/shell/assets-sidebar.slint](/home/wwwroot/mica-term/ui/shell/assets-sidebar.slint) 为 Search 之外的 sidebar 背景区域增加显式 dismiss host。

1. 在 `header := Rectangle` 内，先声明一个背景 `TouchArea`，再保留现有 `toolbar-content`，利用层级让按钮仍然在上层接管点击：

```slint
header-search-dismiss-touch := TouchArea {
    width: parent.width;
    height: parent.height;
    enabled: root.asset-search-expanded && root.assets-search-query == "";

    clicked => {
        root.collapse-assets-search-requested();
    }
}
```

2. 把当前三个 `if root.active-panel == ... : VerticalLayout` 改成 `Rectangle + TouchArea + VerticalLayout` 结构。示例以 `console` 面板为准：

```slint
if root.active-panel == "console" : Rectangle {
    background: transparent;

    panel-search-dismiss-touch := TouchArea {
        width: parent.width;
        height: parent.height;
        enabled: root.asset-search-expanded && root.assets-search-query == "";

        clicked => {
            root.collapse-assets-search-requested();
        }
    }

    VerticalLayout {
        padding: 16px;
        spacing: 8px;

        Text {
            text: root.asset-view-mode == "tree"
                ? (root.asset-tree-fully-expanded ? "Console Tree — Expanded" : "Console Tree — Collapsed")
                : "Console Flat List";
            color: ThemeTokens.text-primary;
        }

        Text {
            text: root.assets-search-query == ""
                ? "Hosts, recent sessions, favorites"
                : "Filter: " + root.assets-search-query;
            color: ThemeTokens.text-primary;
        }
    }
}
```

对 `snippets` 和 `keychain` 面板保持同一模式，不额外创造新回调。

**Step 2: Add the workspace-side click-away host in `AppWindow`**

在 [ui/app-window.slint](/home/wwwroot/mica-term/ui/app-window.slint) 新增一个只覆盖 sidebar 右侧区域的 dismiss host，不要复用 `overlay-dismiss-layer`，避免把 Search 和 `Create` 菜单逻辑重新耦合。

加入：

```slint
workspace-search-dismiss-layer := TouchArea {
    x: sidebar.width;
    y: titlebar.height;
    width: root.width - sidebar.width;
    height: root.height - titlebar.height;
    enabled: root.asset-search-expanded && root.assets-search-query == "";

    clicked => {
        root.collapse-assets-search-requested();
    }
}
```

要求：

- 继续保留现有 `overlay-dismiss-layer` 专门处理 `asset-create-menu-open`
- 不把 `workspace-search-dismiss-layer` 扩到 sidebar 区域
- 不新增新的 root state

**Step 3: Run tests to verify it passes**

Run:

```bash
bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh
cargo test --test assets_sidebar_toolbar_spec collapsing_empty_search_hides_search_row -- --nocapture
cargo test --test assets_sidebar_toolbar_spec non_empty_search_stays_open_when_focus_leaves -- --nocapture
cargo test --test assets_sidebar_toolbar_spec force_closing_search_hides_it_even_with_query -- --nocapture
cargo test --test assets_sidebar_toolbar_smoke search_row_occupies_height_only_when_expanded -- --nocapture
```

Expected:

- shell contract PASS
- 现有状态语义测试全部 PASS
- 不需要新增 `ShellViewModel` 字段

**Step 4: Commit**

```bash
git add ui/shell/assets-sidebar.slint ui/app-window.slint
git add tests/assets_sidebar_toolbar_ui_contract_smoke.sh
git commit -m "fix: route assets search click-away through explicit hosts"
```

## Task 3: Bind Search Input Colors To Theme Tokens

**Files:**
- Modify: `ui/components/assets-search-popover.slint`
- Optionally Modify: `ui/theme/tokens.slint`
- Test: `tests/assets_sidebar_toolbar_ui_contract_smoke.sh`

**Step 1: Write the minimal themed input implementation**

在 [ui/components/assets-search-popover.slint](/home/wwwroot/mica-term/ui/components/assets-search-popover.slint) 的 `search-input := TextInput` 内新增显式主题绑定：

```slint
search-input := TextInput {
    x: 10px;
    y: 5px;
    width: parent.width - 20px;
    height: 22px;
    font-size: 13px;
    text: root.query;
    color: ThemeTokens.text-primary;
    selection-background-color: ThemeTokens.accent;
    selection-foreground-color: ThemeTokens.text-primary;

    changed has-focus => {
        if !self.has-focus {
            root.collapse-requested();
        }
    }

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
```

规则：

- 第一轮优先直接复用 `ThemeTokens.text-primary` 和 `ThemeTokens.accent`
- 不要在组件内写 dark/light 十六进制值
- 只有当 visual review 明确证明选区前景不够好时，才最小化扩充 [ui/theme/tokens.slint](/home/wwwroot/mica-term/ui/theme/tokens.slint)

**Step 2: Run verification**

Run:

```bash
bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh
cargo check -q
```

Expected:

- shell contract PASS
- `cargo check` PASS

**Step 3: Commit**

```bash
git add ui/components/assets-search-popover.slint
git add ui/theme/tokens.slint
git add tests/assets_sidebar_toolbar_ui_contract_smoke.sh
git commit -m "fix: theme assets search input for dark mode"
```

如果 `ui/theme/tokens.slint` 最终没有改动，就不要加入 `git add`。

## Task 4: Full Verification And Close-Out

**Files:**
- Reference: `tests/assets_sidebar_toolbar_ui_contract_smoke.sh`
- Reference: `tests/assets_sidebar_toolbar_spec.rs`
- Reference: `tests/assets_sidebar_toolbar_smoke.rs`
- Reference: `src/shell/view_model.rs`

**Step 1: Run the full verification set**

Run:

```bash
bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh
cargo test --test assets_sidebar_toolbar_spec --test assets_sidebar_toolbar_smoke --test shell_view_model -q
cargo check -q
```

Expected:

- `assets_sidebar_toolbar_ui_contract_smoke.sh`: PASS
- `assets_sidebar_toolbar_spec`: PASS
- `assets_sidebar_toolbar_smoke`: PASS
- `shell_view_model`: PASS
- `cargo check`: PASS

**Step 2: Review the diff for scope discipline**

Run:

```bash
git diff -- ui/components/assets-search-popover.slint ui/shell/assets-sidebar.slint ui/app-window.slint ui/theme/tokens.slint tests/assets_sidebar_toolbar_ui_contract_smoke.sh tests/assets_sidebar_toolbar_spec.rs tests/assets_sidebar_toolbar_smoke.rs
```

Expected:

- 只出现 Search click-away host 和 dark-mode text theme 相关变更
- 不出现 renderer、terminal runtime、Create menu 结构变更

**Step 3: Commit the verification pass**

```bash
git add ui/components/assets-search-popover.slint ui/shell/assets-sidebar.slint ui/app-window.slint
git add tests/assets_sidebar_toolbar_ui_contract_smoke.sh tests/assets_sidebar_toolbar_spec.rs tests/assets_sidebar_toolbar_smoke.rs
git commit -m "test: verify assets search dismiss and theme fixes"
```

如果前面 task 已经包含所有代码与测试变更，这一步可以只在需要额外修订测试或验证留痕时提交；不要为了凑提交而制造空提交。

## Risks And Rollback

- 风险 1：`workspace-search-dismiss-layer` 命中顺序不符合预期，导致 workspace 点击没有触发 collapse。
  - 处理：立即使用 `@superpowers:systematic-debugging` 检查 Slint hit-test 顺序，不要扩散到更多状态变更。
- 风险 2：新增 sidebar 背景 `TouchArea` 后误拦截 Search 本体或 toolbar button 点击。
  - 处理：优先通过声明顺序和宿主分层修正，不要改动业务 callback。
- 风险 3：只修复 `color` 但遗漏选区可见性。
  - 处理：保留 `selection-background-color` / `selection-foreground-color` 的显式 contract。
- 回滚策略：
  - 先回滚新增 click-away host，不回滚 inline row 结构
  - 保留 `TextInput` 颜色绑定，即使 click-away 方案需要重做

Plan complete and saved to `docs/plans/2026-03-16-assets-sidebar-toolbar-bugfix3-implementation-plan.md`. Two execution options:

1. Subagent-Driven (this session) - I dispatch fresh subagent per task, review between tasks, fast iteration
2. Parallel Session (separate) - Open new session with executing-plans, batch execution with checkpoints

Which approach?


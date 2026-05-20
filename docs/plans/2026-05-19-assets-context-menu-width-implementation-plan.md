# Assets Context Menu Width Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 修复左侧共享 assets context menu 的文案截断问题，引入可测试的分档自适应菜单宽度契约，同时保持现有 overlay、placement、corridor 与单行菜单视觉不变。

**Architecture:** 保持当前 `Rust context-menu domain -> bootstrap bridge -> Slint root overlay` 路线不变。Rust 负责决定本次菜单会话的 column width，并把该 width 传给 placement、rect 计算和 Slint overlay；Slint 只消费运行时 width 来渲染 column，不做额外业务判断。

**Tech Stack:** Rust 2024, Slint 1.15.1, `VecModel`, existing assets context-menu overlay, `cargo test`, shell smoke scripts.

---

## Execution Notes

- Design source: `docs/plans/2026-05-19-assets-context-menu-width-design.md`
- I'm using the writing-plans skill to create the implementation plan.
- REQUIRED before coding: `@superpowers:test-driven-development`
- 如果实现中出现 placement 抖动、hover corridor 退化或菜单开合异常，立即切到 `@superpowers:systematic-debugging`
- 必须在新的 `.worktrees/...` 会话里执行，不要在当前窗口直接改产品代码
- 不要顺手扩大到 workspace tab context menu；那个菜单已经是独立组件

### Task 1: 锁定宽度契约测试与 UI contract smoke

**Files:**
- Modify: `tests/assets_context_menu_spec.rs`
- Modify: `tests/assets_context_menu_smoke.rs`
- Modify: `tests/assets_context_menu_ui_contract_smoke.sh`

**Step 1: Write the failing test**

在 `tests/assets_context_menu_spec.rs` 增加宽度分档测试，至少覆盖 blank area 和 SSH 长菜单场景：

```rust
#[test]
fn blank_area_context_menu_uses_compact_width_tier() {
    let roots = resolve_action_tree(ContextTargetKind::BlankArea, &blank_selection());
    assert_eq!(context_menu_column_width_for_items(&roots), 256.0);
}

#[test]
fn ssh_context_menu_uses_expanded_width_tier_for_long_planned_actions() {
    let selection = SelectionContext {
        selected_ids: vec!["ssh-prod-01".into()],
        clipboard_has_asset_payload: false,
        target_mutable: true,
        selected_file_count: 0,
        selected_directory_count: 0,
    };
    let roots = resolve_action_tree(ContextTargetKind::SshConnection, &selection);
    assert_eq!(context_menu_column_width_for_items(&roots), 368.0);
}
```

在 `tests/assets_context_menu_smoke.rs` 增加 bridge 级断言，验证 overlay 宽度会跟 target 类型变化：

```rust
#[test]
fn ssh_context_menu_projects_expanded_overlay_width() {
    let app = AppWindow::new().unwrap();
    bind_with_fake_sessions(&app);
    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_context_menu_requested(ssh_id.into(), "ssh".into(), 96.0, 160.0);

    assert_eq!(app.get_layout_assets_context_menu_width(), 368.0);
}
```

在 `tests/assets_context_menu_ui_contract_smoke.sh` 增加 contract 检查：

```bash
grep -F 'in-out property <length> assets-context-menu-column-width:' "$APP_WINDOW" >/dev/null
grep -F 'in property <length> column-width:' "$MENU_OVERLAY" >/dev/null
grep -F 'in property <length> column-width:' "$MENU_COLUMN" >/dev/null
grep -F 'width: root.column-width;' "$MENU_COLUMN" >/dev/null
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test assets_context_menu_spec --test assets_context_menu_smoke -- --nocapture`
Expected: FAIL，因为宽度 helper 和 AppWindow width projection 还不存在

Run: `bash tests/assets_context_menu_ui_contract_smoke.sh`
Expected: FAIL，因为 Slint contract 还没有 `column-width`

**Step 3: Write minimal implementation target notes**

- 宽度分三档：`256 / 312 / 368`
- Smoke test 只先锁最关键的 SSH 长菜单场景
- 现有 `overflow: elide` 不删除，但正常路径不应再触发

**Step 4: Re-run the assertions mentally before coding**

确保测试名直接表达“共享菜单宽度契约”，不要写成泛化样式优化。

**Step 5: Commit**

```bash
git add tests/assets_context_menu_spec.rs \
        tests/assets_context_menu_smoke.rs \
        tests/assets_context_menu_ui_contract_smoke.sh
git commit -m "test: lock assets context menu width contract"
```

### Task 2: 在 Rust domain 中引入分档宽度与运行时 placement

**Files:**
- Modify: `src/shell/context_menu.rs`
- Modify: `src/app/bootstrap/assets_keychain.rs`
- Modify: `tests/assets_context_menu_spec.rs`

**Step 1: Write the failing test**

补充 placement / rect 测试，确认不再写死 `224.0`：

```rust
#[test]
fn context_menu_column_rects_use_runtime_width() {
    let width = 368.0;
    let parent = Rect { x: 100.0, y: 100.0, width, height: 120.0 };
    let child = Rect { x: 100.0 + width + 8.0, y: 100.0, width, height: 120.0 };

    assert!(should_keep_corridor_open((100.0 + width + 4.0, 140.0), parent, child));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test assets_context_menu_spec -- --nocapture`
Expected: FAIL，因为当前 rect / offset / placement 仍然依赖固定宽度

**Step 3: Write minimal implementation**

在 `src/shell/context_menu.rs` 增加：

```rust
pub const CONTEXT_MENU_COLUMN_WIDTH_COMPACT: f32 = 256.0;
pub const CONTEXT_MENU_COLUMN_WIDTH_STANDARD: f32 = 312.0;
pub const CONTEXT_MENU_COLUMN_WIDTH_EXPANDED: f32 = 368.0;

pub fn context_menu_column_width_for_items(items: &[ContextMenuActionNode]) -> f32 {
    let longest = items.iter().map(|item| item.label.chars().count()).max().unwrap_or(0);
    match longest {
        0..=18 => CONTEXT_MENU_COLUMN_WIDTH_COMPACT,
        19..=26 => CONTEXT_MENU_COLUMN_WIDTH_STANDARD,
        _ => CONTEXT_MENU_COLUMN_WIDTH_EXPANDED,
    }
}
```

把 `context_menu_column_offset(...)` 改成接收 `column_width: f32` 参数，而不是继续依赖固定 `224.0`。

在 `src/app/bootstrap/assets_keychain.rs` 新增本次菜单会话宽度 helper，建议形态：

```rust
fn context_menu_column_width_for(state: &ShellViewModel) -> f32 {
    context_menu_columns_for(state)
        .into_iter()
        .filter(|column| !column.is_empty())
        .map(|column| context_menu_column_width_for_items(column.as_slice()))
        .fold(CONTEXT_MENU_COLUMN_WIDTH_COMPACT, f32::max)
}
```

然后把以下逻辑全部改成消费该运行时 width：

- `context_menu_child_width_for(...)`
- `context_menu_column_rects_for(...)`
- `update_context_menu_placement(...)`

**Step 4: Run test to verify it passes**

Run: `cargo test --test assets_context_menu_spec -- --nocapture`
Expected: PASS，宽度 tier 与 placement 计算通过

**Step 5: Commit**

```bash
git add src/shell/context_menu.rs \
        src/app/bootstrap/assets_keychain.rs \
        tests/assets_context_menu_spec.rs
git commit -m "feat: add adaptive assets context menu width contract"
```

### Task 3: 把运行时 width 贯穿到 AppWindow、Overlay、Column

**Files:**
- Modify: `ui/app-window.slint`
- Modify: `ui/components/assets-context-menu-overlay.slint`
- Modify: `ui/components/assets-context-menu-column.slint`
- Modify: `src/app/bootstrap/assets_keychain.rs`
- Modify: `tests/assets_context_menu_smoke.rs`
- Modify: `tests/assets_context_menu_ui_contract_smoke.sh`

**Step 1: Write the failing test**

在 `tests/assets_context_menu_smoke.rs` 增加 blank area 与 SSH target 对比，确认 runtime width 真正投影到 Slint：

```rust
#[test]
fn different_targets_project_different_context_menu_overlay_widths() {
    let app = AppWindow::new().unwrap();
    bind_with_fake_sessions(&app);

    app.invoke_asset_context_menu_requested("".into(), "blank".into(), 96.0, 160.0);
    assert_eq!(app.get_layout_assets_context_menu_width(), 256.0);

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_context_menu_requested(ssh_id.into(), "ssh".into(), 96.0, 160.0);
    assert_eq!(app.get_layout_assets_context_menu_width(), 368.0);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test assets_context_menu_smoke -- --nocapture`
Expected: FAIL，因为 AppWindow 还没有 `assets-context-menu-column-width` contract

Run: `bash tests/assets_context_menu_ui_contract_smoke.sh`
Expected: FAIL，因为 overlay / column 还未接线

**Step 3: Write minimal implementation**

在 `ui/app-window.slint` 增加：

```slint
in-out property <length> assets-context-menu-column-width: 256px;
```

并把它传给 overlay：

```slint
assets-context-menu-overlay := AssetsContextMenuOverlay {
    column-width: root.assets-context-menu-column-width;
}
```

在 `ui/components/assets-context-menu-overlay.slint` 增加：

```slint
in property <length> column-width: 256px;
```

并向三列传递：

```slint
primary-column := AssetsContextMenuColumn { column-width: root.column-width; }
secondary-column := AssetsContextMenuColumn { column-width: root.column-width; }
tertiary-column := AssetsContextMenuColumn { column-width: root.column-width; }
```

在 `ui/components/assets-context-menu-column.slint` 增加：

```slint
in property <length> column-width: 256px;
width: root.column-width;
```

删除原固定 `224px`。

在 `src/app/bootstrap/assets_keychain.rs` 的 `sync_assets_context_menu_state(...)` 里同步设置该 property。

**Step 4: Run test to verify it passes**

Run: `cargo test --test assets_context_menu_smoke -- --nocapture`
Expected: PASS，`layout-assets-context-menu-width` 会按 target 改变

Run: `bash tests/assets_context_menu_ui_contract_smoke.sh`
Expected: PASS，contract 链路完整

**Step 5: Commit**

```bash
git add ui/app-window.slint \
        ui/components/assets-context-menu-overlay.slint \
        ui/components/assets-context-menu-column.slint \
        src/app/bootstrap/assets_keychain.rs \
        tests/assets_context_menu_smoke.rs \
        tests/assets_context_menu_ui_contract_smoke.sh
git commit -m "fix: project adaptive width into assets context menu overlay"
```

### Task 4: 收尾验证并在 worktree 中执行正式实现

**Files:**
- Modify: any files above as needed

**Step 1: Run formatting**

Run: `cargo fmt --all --manifest-path Cargo.toml`
Expected: PASS

**Step 2: Run compile verification**

Run: `cargo test -q --manifest-path Cargo.toml --no-run`
Expected: PASS

**Step 3: Run focused tests**

Run: `cargo test -q --manifest-path Cargo.toml --test assets_context_menu_spec --test assets_context_menu_smoke -- --nocapture`
Expected: PASS

Run: `bash tests/assets_context_menu_ui_contract_smoke.sh`
Expected: PASS

**Step 4: Manual verification checklist**

- 右键 blank area，`New SSH Connection` 完整显示
- 右键 SSH 资产，`Proxy Chrome via Server` 与 `Upload SSH Public Key (ssh-copy-id)` 完整显示
- 右键 SFTP 空白区域，`Upload Files...` 与 `Upload Folder...` 完整显示
- 菜单靠近窗口右边缘时仍保持 clamp / flip 正常
- hover、submenu corridor、dismiss layer 无退化
- workspace tab context menu 未受影响

**Step 5: Commit**

```bash
git add src/shell/context_menu.rs \
        src/app/bootstrap/assets_keychain.rs \
        ui/app-window.slint \
        ui/components/assets-context-menu-overlay.slint \
        ui/components/assets-context-menu-column.slint \
        tests/assets_context_menu_spec.rs \
        tests/assets_context_menu_smoke.rs \
        tests/assets_context_menu_ui_contract_smoke.sh
git commit -m "fix: untruncate shared assets context menu labels"
```

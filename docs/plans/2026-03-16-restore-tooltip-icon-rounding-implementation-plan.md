# Restore Tooltip And Icon Button Rounding Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 在保持窗口壳与大多数组件方角基线不变的前提下，恢复 tooltip 和纯图标按钮的圆角轮廓。

**Architecture:** 只恢复 `TitlebarTooltip`、`TitlebarIconButton` 与 `SidebarToolbarIconButton` 三个组件的 `border-radius`，并同步收窄 smoke contract，允许这三个明确例外，其余 `ui/` 文件继续禁止非零半径。

**Tech Stack:** Rust, Cargo, Slint 1.15.1, shell smoke scripts, `cargo check`, `cargo clippy`

---

## Task 1: 恢复 tooltip 与纯图标按钮圆角

**Files:**
- Modify: `tests/square_component_contract_smoke.sh`
- Modify: `tests/top_status_bar_ui_contract_smoke.sh`
- Modify: `ui/components/titlebar-tooltip.slint`
- Modify: `ui/components/titlebar-icon-button.slint`
- Modify: `ui/components/sidebar-toolbar-icon-button.slint`

**Step 1: Write the failing test**

把 `tests/top_status_bar_ui_contract_smoke.sh` 扩展为要求：

```bash
grep -F 'border-radius: 8px;' "$ROOT_DIR/ui/components/titlebar-tooltip.slint" >/dev/null
grep -F 'border-radius: 8px;' "$ROOT_DIR/ui/components/titlebar-icon-button.slint" >/dev/null
grep -F 'border-radius: 6px;' "$ROOT_DIR/ui/components/sidebar-toolbar-icon-button.slint" >/dev/null
```

把 `tests/square_component_contract_smoke.sh` 改成“允许 3 个例外文件，其它文件禁止非零圆角”，例如：

```bash
ALL_RADIUS_LINES="$(rg -n 'border-radius:' "$ROOT_DIR/ui")"
echo "$ALL_RADIUS_LINES" | rg -v \
  'ui/components/titlebar-tooltip\\.slint:.*border-radius:\\s*8px;|ui/components/titlebar-icon-button\\.slint:.*border-radius:\\s*8px;|ui/components/sidebar-toolbar-icon-button\\.slint:.*border-radius:\\s*6px;|border-radius:\\s*0px;'
```

**Step 2: Run test to verify it fails**

Run: `bash tests/top_status_bar_ui_contract_smoke.sh`
Expected: FAIL，因为当前三个目标组件仍然是 `0px`

Run: `bash tests/square_component_contract_smoke.sh`
Expected: FAIL，因为 contract 尚未允许新的例外

**Step 3: Write minimal implementation**

在以下文件恢复圆角：

- `ui/components/titlebar-tooltip.slint` -> `border-radius: 8px;`
- `ui/components/titlebar-icon-button.slint` -> `border-radius: 8px;`
- `ui/components/sidebar-toolbar-icon-button.slint` -> `border-radius: 6px;`

不要改动：

- `ui/components/sidebar-nav-button.slint`
- `ui/components/window-control-button.slint`
- `ui/app-window.slint`

**Step 4: Run test to verify it passes**

Run: `bash tests/top_status_bar_ui_contract_smoke.sh`
Expected: PASS

Run: `bash tests/square_component_contract_smoke.sh`
Expected: PASS

Run: `cargo check --workspace`
Expected: PASS

Run: `cargo clippy --workspace -- -D warnings`
Expected: PASS

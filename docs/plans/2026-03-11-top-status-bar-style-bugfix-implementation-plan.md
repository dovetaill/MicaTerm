# Top Status Bar Style Bugfix Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 在不改变现有 `Rust + Slint` frameless shell 基座的前提下，完成顶部状态栏样式 bugfix，实现清晰的信息架构、唯一的左侧 `M` 主菜单、正确的菜单锚点、Fluent SVG 图标化 caption controls 与统一 tooltip。

**Architecture:** 继续沿用现有分层：`AppWindow` 承载顶层属性与回调，`Titlebar` 负责顶栏结构和交互，`TitlebarMenu` / `TitlebarIconButton` / `WindowControlButton` 等 Slint 组件承载具体视觉，`ShellViewModel` 负责可测试状态，`WindowController` 保持窗口控制逻辑。实现策略以最小侵入方式修正现有 `top status bar` 基线，不重写窗口系统、不回退原生标题栏。  
图标采用本地 vendored Fluent SVG 资产，默认 `Regular`，少量激活态使用 `Filled`；tooltip 采用统一 popup 组件而不是分散在每个按钮里。

**Tech Stack:** Rust, Cargo, Slint 1.15.1, `PopupWindow`, `winit` window bridge, shell smoke scripts, `cargo fmt`, `cargo check`, `cargo clippy`, `cargo test`

---

## Execution Notes

- 设计输入文档是 `docs/plans/2026-03-11-top-status-bar-style-bugfix-design.md`，执行时不得偏离已确认的 `1B 2A 3A 4A 5A 6A`。
- 使用 @superpowers:test-driven-development 执行每个任务，先写失败测试或失败 smoke，再写最小实现。
- 对 `tooltip`、`PopupWindow` 锚点这类难以在 headless 环境中做完整行为断言的部分，使用“源码契约 smoke script + GUI 手工 smoke checklist”的双轨验证。
- 维持现有 `TitlebarMenu` 组件文件，不为“主菜单”另起一套平行组件，避免不必要重构。
- 如果执行阶段发现 Fluent SVG 命名与官方仓库略有出入，以 `microsoft/fluentui-system-icons` 最新 icon list 为准，但本地 vendored 文件名保持本仓库一致。

## Task 1: 统一主菜单状态与回调语义

**Files:**
- Modify: `src/shell/view_model.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `ui/app-window.slint`
- Modify: `ui/shell/titlebar.slint`
- Modify: `tests/shell_view_model.rs`
- Modify: `tests/top_status_bar_smoke.rs`

**Step 1: Write the failing test**

先把 `settings menu` 语义替换成 `global menu` 契约。修改 `tests/shell_view_model.rs`，新增或重写断言：

```rust
#[test]
fn shell_view_model_tracks_global_menu_state() {
    let mut view_model = ShellViewModel::default();

    assert!(!view_model.show_global_menu);

    view_model.toggle_global_menu();
    assert!(view_model.show_global_menu);

    view_model.close_global_menu();
    assert!(!view_model.show_global_menu);
}
```

修改 `tests/top_status_bar_smoke.rs`，把生成的 getter / callback 断言改成：

```rust
app.set_show_global_menu(true);
assert!(!app.get_show_global_menu());

app.invoke_toggle_global_menu_requested();
assert!(app.get_show_global_menu());

app.invoke_close_global_menu_requested();
assert!(!app.get_show_global_menu());
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --test shell_view_model --test top_status_bar_smoke -q`  
Expected: FAIL with missing items such as `show_global_menu`, `toggle_global_menu`, `get_show_global_menu`, or `invoke_toggle_global_menu_requested`.

**Step 3: Write minimal implementation**

更新状态命名，不改变交互职责：

```rust
pub struct ShellViewModel {
    pub show_welcome: bool,
    pub show_right_panel: bool,
    pub show_global_menu: bool,
    pub is_window_maximized: bool,
    pub is_window_active: bool,
}

impl ShellViewModel {
    pub fn toggle_global_menu(&mut self) {
        self.show_global_menu = !self.show_global_menu;
    }

    pub fn close_global_menu(&mut self) {
        self.show_global_menu = false;
    }
}
```

同步修改：

- `AppWindow` 顶层属性：`show-global-menu`
- 顶层回调：`toggle-global-menu-requested` / `close-global-menu-requested`
- `bind_top_status_bar()` 内的绑定和 `sync_top_status_bar_state()`
- `Titlebar` 内所有对旧 `settings menu` 状态的引用

**Step 4: Run tests to verify they pass**

Run: `cargo test --test shell_view_model --test top_status_bar_smoke -q`  
Expected: PASS, 并且 `global menu` 成为唯一主菜单语义入口。

**Step 5: Commit**

```bash
git add src/shell/view_model.rs src/app/bootstrap.rs ui/app-window.slint ui/shell/titlebar.slint tests/shell_view_model.rs tests/top_status_bar_smoke.rs
git commit -m "refactor: rename top bar settings state to global menu"
```

## Task 2: 重排顶栏信息架构并移除右侧重复入口

**Files:**
- Modify: `src/shell/metrics.rs`
- Modify: `ui/shell/titlebar.slint`
- Modify: `tests/titlebar_layout_spec.rs`
- Modify: `tests/top_status_bar_smoke.rs`

**Step 1: Write the failing layout test**

修改 `tests/titlebar_layout_spec.rs`，把当前“仅验证旧预算”的测试升级为新的结构预算契约：

```rust
use mica_term::shell::metrics::ShellMetrics;

#[test]
fn top_status_bar_layout_preserves_brand_utility_and_drag_budget() {
    assert_eq!(ShellMetrics::TITLEBAR_HEIGHT, 48);
    assert_eq!(ShellMetrics::TITLEBAR_BRAND_WIDTH, 220);
    assert!(ShellMetrics::TITLEBAR_MIN_DRAG_WIDTH >= 96);
    assert!(ShellMetrics::TITLEBAR_UTILITY_WIDTH >= 84);
    assert_eq!(ShellMetrics::TITLEBAR_WINDOW_CONTROL_WIDTH, 138);
}
```

同时在 `tests/top_status_bar_smoke.rs` 旁新增一个源码契约 smoke 脚本：

- Create: `tests/top_status_bar_ui_contract_smoke.sh`

脚本最小契约：

```bash
#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TITLEBAR="$ROOT_DIR/ui/shell/titlebar.slint"

grep -F 'menu-button := TitlebarIconButton' "$TITLEBAR" >/dev/null
grep -F 'panel-toggle-button := TitlebarIconButton' "$TITLEBAR" >/dev/null
! grep -F 'label: "S"' "$TITLEBAR" >/dev/null
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --test titlebar_layout_spec -q`  
Expected: FAIL with missing constants such as `TITLEBAR_BRAND_WIDTH` or `TITLEBAR_UTILITY_WIDTH`.

Run: `bash tests/top_status_bar_ui_contract_smoke.sh`  
Expected: FAIL because current `titlebar.slint` 仍然包含右侧 `S` 或没有 `menu-button` / `panel-toggle-button` 命名锚点。

**Step 3: Write minimal implementation**

在 `src/shell/metrics.rs` 中补充新预算常量：

```rust
pub struct ShellMetrics;

impl ShellMetrics {
    pub const TITLEBAR_HEIGHT: u32 = 48;
    pub const TITLEBAR_BRAND_WIDTH: u32 = 220;
    pub const TITLEBAR_UTILITY_WIDTH: u32 = 84;
    pub const TITLEBAR_WINDOW_CONTROL_WIDTH: u32 = 138;
    pub const TITLEBAR_MIN_DRAG_WIDTH: u32 = 96;
}
```

重构 `ui/shell/titlebar.slint` 的大分组：

- 左侧：`brand chip + app name + menu-button`
- 中间：`workspace/context + primary drag area`
- 右侧：`status pill + panel-toggle-button`
- 最右：`window-controls`

明确要求：

- 删除右侧重复 `S` 按钮
- 保留 `SSH` 状态 pill，但不要再与多个字母按钮挤在 `120px` 窄区里
- 保留拖拽安全区

**Step 4: Run tests to verify they pass**

Run: `cargo test --test titlebar_layout_spec -q`  
Expected: PASS

Run: `bash tests/top_status_bar_ui_contract_smoke.sh`  
Expected: PASS

**Step 5: Commit**

```bash
git add src/shell/metrics.rs ui/shell/titlebar.slint tests/titlebar_layout_spec.rs tests/top_status_bar_ui_contract_smoke.sh
git commit -m "refactor: reorganize top status bar information architecture"
```

## Task 3: vendoring Fluent SVG 资产并替换文本 glyph

**Files:**
- Create: `assets/icons/fluent/menu-20-regular.svg`
- Create: `assets/icons/fluent/menu-20-filled.svg`
- Create: `assets/icons/fluent/panel-right-20-regular.svg`
- Create: `assets/icons/fluent/panel-right-20-filled.svg`
- Create: `assets/icons/fluent/subtract-20-regular.svg`
- Create: `assets/icons/fluent/maximize-20-regular.svg`
- Create: `assets/icons/fluent/restore-20-regular.svg`
- Create: `assets/icons/fluent/dismiss-20-regular.svg`
- Modify: `ui/components/titlebar-icon-button.slint`
- Modify: `ui/components/window-control-button.slint`
- Modify: `ui/shell/titlebar.slint`
- Create: `tests/fluent_titlebar_assets_smoke.sh`

**Step 1: Write the failing asset smoke**

创建 `tests/fluent_titlebar_assets_smoke.sh`：

```bash
#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

for file in \
  assets/icons/fluent/menu-20-regular.svg \
  assets/icons/fluent/menu-20-filled.svg \
  assets/icons/fluent/panel-right-20-regular.svg \
  assets/icons/fluent/panel-right-20-filled.svg \
  assets/icons/fluent/subtract-20-regular.svg \
  assets/icons/fluent/maximize-20-regular.svg \
  assets/icons/fluent/restore-20-regular.svg \
  assets/icons/fluent/dismiss-20-regular.svg
do
  [[ -f "$ROOT_DIR/$file" ]] || {
    echo "missing $file" >&2
    exit 1
  }
done
```

**Step 2: Run smoke to verify it fails**

Run: `bash tests/fluent_titlebar_assets_smoke.sh`  
Expected: FAIL with `missing assets/icons/fluent/...`.

**Step 3: Write minimal implementation**

从 `microsoft/fluentui-system-icons` 选择对应图标并 vendoring 到本地，保持本仓库文件名稳定。

扩展 `TitlebarIconButton`：

```slint
export component TitlebarIconButton inherits Rectangle {
    in property <image> icon-source;
    in property <image> active-icon-source;
    in property <bool> active: false;
    in property <string> tooltip-text;
    callback clicked;
}
```

扩展 `WindowControlButton`：

```slint
export component WindowControlButton inherits Rectangle {
    in property <image> icon-source;
    in property <string> tooltip-text;
    in property <bool> danger: false;
    callback clicked;
}
```

在 `ui/shell/titlebar.slint` 中完成替换：

- 左侧 `M` 不再显示字母，改成 `menu` Fluent icon
- `panel-toggle-button` 使用 `panel-right` 图标
- 窗口控制改为 `subtract / maximize / restore / dismiss`
- 默认使用 `Regular`
- 仅 `panel-toggle-button` 的 active 态切到 `Filled`

**Step 4: Run smokes to verify they pass**

Run: `bash tests/fluent_titlebar_assets_smoke.sh`  
Expected: PASS

Run: `bash tests/top_status_bar_ui_contract_smoke.sh`  
Expected: PASS，且源码中不再依赖 `label: "M"` / `label: "R+"` / `label: "-"` / `label: "X"` 作为主视觉表达。

**Step 5: Commit**

```bash
git add assets/icons/fluent ui/components/titlebar-icon-button.slint ui/components/window-control-button.slint ui/shell/titlebar.slint tests/fluent_titlebar_assets_smoke.sh tests/top_status_bar_ui_contract_smoke.sh
git commit -m "feat: add fluent svg titlebar icons"
```

## Task 4: 修正 `PopupWindow` 锚点到左侧 `M` 按钮

**Files:**
- Modify: `ui/components/titlebar-menu.slint`
- Modify: `ui/shell/titlebar.slint`
- Modify: `tests/top_status_bar_ui_contract_smoke.sh`

**Step 1: Write the failing popup contract smoke**

扩展 `tests/top_status_bar_ui_contract_smoke.sh`，加入：

```bash
grep -F 'menu-button.absolute-position.x' "$TITLEBAR" >/dev/null
grep -F 'menu-button.absolute-position.y' "$TITLEBAR" >/dev/null
! grep -F 'actions-zone.absolute-position.x' "$TITLEBAR" >/dev/null
```

**Step 2: Run smoke to verify it fails**

Run: `bash tests/top_status_bar_ui_contract_smoke.sh`  
Expected: FAIL，因为当前 popup 仍锚定在 `actions-zone.absolute-position`.

**Step 3: Write minimal implementation**

在 `ui/shell/titlebar.slint` 中建立明确的左侧锚点命名：

```slint
menu-button := TitlebarIconButton { ... }

global-menu := TitlebarMenu {
    x: menu-button.absolute-position.x;
    y: menu-button.absolute-position.y + menu-button.height + 6px;
}
```

同时整理 `TitlebarMenu` 语义：

- 保留 `PopupWindow`
- 保留 `close-policy`
- 保证菜单项点击后会关闭
- 外部点击关闭行为不回退

**Step 4: Run smoke to verify it passes**

Run: `bash tests/top_status_bar_ui_contract_smoke.sh`  
Expected: PASS

**Step 5: Commit**

```bash
git add ui/components/titlebar-menu.slint ui/shell/titlebar.slint tests/top_status_bar_ui_contract_smoke.sh
git commit -m "fix: anchor global menu under titlebar menu button"
```

## Task 5: 为所有顶栏按钮增加统一 tooltip

**Files:**
- Create: `ui/components/titlebar-tooltip.slint`
- Modify: `ui/components/titlebar-icon-button.slint`
- Modify: `ui/components/window-control-button.slint`
- Modify: `ui/shell/titlebar.slint`
- Modify: `tests/top_status_bar_ui_contract_smoke.sh`

**Step 1: Write the failing tooltip contract smoke**

继续扩展 `tests/top_status_bar_ui_contract_smoke.sh`：

```bash
grep -F 'tooltip-text: "Open menu"' "$TITLEBAR" >/dev/null
grep -F 'tooltip-text: "Toggle right panel"' "$TITLEBAR" >/dev/null
grep -F 'tooltip-text: "Minimize window"' "$TITLEBAR" >/dev/null
grep -F 'tooltip-text: "Close window"' "$TITLEBAR" >/dev/null
```

**Step 2: Run smoke to verify it fails**

Run: `bash tests/top_status_bar_ui_contract_smoke.sh`  
Expected: FAIL because tooltip text properties and tooltip component do not exist yet.

**Step 3: Write minimal implementation**

创建统一 tooltip 组件：

```slint
export component TitlebarTooltip inherits PopupWindow {
    in property <string> text;
}
```

为按钮组件增加 hover 回调：

```slint
callback tooltip-open-requested(string, length, length);
callback tooltip-close-requested();
```

在 `Titlebar` 根组件中只维护一个 tooltip popup：

- `menu-button` hover 时显示 `"Open menu"`
- `panel-toggle-button` hover 时显示 `"Toggle right panel"`
- 窗口按钮 hover 时分别显示 `"Minimize window"` / `"Maximize window"` / `"Restore window"` / `"Close window"`

行为要求：

- hover 延迟约 `250-300ms`
- pointer leave 时关闭
- 点击按钮时关闭
- 不允许 tooltip 遮挡主菜单主要点击区域

**Step 4: Run smoke to verify it passes**

Run: `bash tests/top_status_bar_ui_contract_smoke.sh`  
Expected: PASS

**Step 5: Commit**

```bash
git add ui/components/titlebar-tooltip.slint ui/components/titlebar-icon-button.slint ui/components/window-control-button.slint ui/shell/titlebar.slint tests/top_status_bar_ui_contract_smoke.sh
git commit -m "feat: add titlebar tooltip popup"
```

## Task 6: 全量验证并补充人工 smoke 清单

**Files:**
- Modify: `verification.md` (if present)
- Modify: `docs/plans/2026-03-11-top-status-bar-style-bugfix-implementation-plan.md` (only if verification notes need amendment during execution)

**Step 1: Run static verification**

Run: `cargo fmt --all --check`  
Expected: PASS

Run: `cargo check --workspace`  
Expected: PASS

Run: `cargo clippy --all-targets --all-features -- -D warnings`  
Expected: PASS

Run: `cargo test -q`  
Expected: PASS

Run: `bash tests/fluent_titlebar_assets_smoke.sh`  
Expected: PASS

Run: `bash tests/top_status_bar_ui_contract_smoke.sh`  
Expected: PASS

**Step 2: Run GUI smoke on desktop-capable session**

Run: `cargo run`  
Expected: 在可用 GUI 会话中正常启动窗口。

手工检查：

- 顶栏不再出现 `SSR+` 视觉粘连
- 左侧 `M` 为唯一主菜单入口
- 主菜单从 `M` 下方展开
- 右侧只保留独立 utility icon，不再出现重复设置入口
- `Minimize / Maximize / Restore / Close` 图标语义清晰
- 默认 `Regular`，激活态仅少量 `Filled`
- 所有按钮 hover 时都出现 tooltip

如果当前环境没有 `WAYLAND_DISPLAY` / `DISPLAY`：

- 在验证记录中明确标注 GUI smoke 未执行原因
- 不得声称桌面交互已“确认通过”

**Step 3: Commit verification artifacts**

```bash
git add verification.md
git commit -m "docs: capture top status bar style bugfix verification"
```

## Rollback Notes

- 如果 `global menu` 重命名导致改动面超出预期，可先保留 `TitlebarMenu` 文件名，只修正 property/callback 命名，不做文件级 rename。
- 如果 Fluent SVG 接入后出现渲染不一致，先统一回退到全部 `Regular`，不要回退到文本 glyph。
- 如果 tooltip 在 headless 测试里难以稳定断言，保留源码契约 smoke 脚本，避免伪造运行时自动化。
- 不允许为了解决顶栏样式问题而回退 `no-frame: true` 或恢复系统标题栏。

## Final Verification Checklist

- [ ] `global menu` 语义取代旧 `settings menu`
- [ ] 顶栏右侧重复 `S` 按钮已删除
- [ ] 菜单锚点来自左侧 `menu-button`
- [ ] caption controls 全部图标化
- [ ] 图标默认 `Regular`，active 态少量 `Filled`
- [ ] 所有顶栏按钮都有 tooltip 文案
- [ ] `cargo fmt --all --check` 通过
- [ ] `cargo check --workspace` 通过
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` 通过
- [ ] `cargo test -q` 通过
- [ ] 资产与 UI 契约 smoke scripts 通过
- [ ] GUI smoke 结果已记录或明确说明无法执行原因

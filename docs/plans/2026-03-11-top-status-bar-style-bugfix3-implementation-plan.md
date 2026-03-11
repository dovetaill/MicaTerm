# Top Status Bar Style Bugfix3 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 在不改变现有 `Rust + Slint` frameless shell 主结构的前提下，修复顶部状态栏 tooltip 在真实窗口中不稳定或不可见的问题，将其改为窗口内 overlay 呈现，并补齐可落盘的 `logs/` 调试证据。

**Architecture:** 保留现有 `AppWindow -> Titlebar -> TitlebarIconButton / WindowControlButton` 的分层，只替换 tooltip 的呈现方式与调度细节。`Titlebar` 继续拥有唯一共享 tooltip 状态机，按钮只负责发出 tooltip intent；Rust 侧新增一个轻量 `TooltipDebugLog` 模块，通过 `AppWindow` callback 接收 tooltip 生命周期事件并写入运行目录 `logs/titlebar-tooltip.log`。

**Tech Stack:** Rust, Cargo, Slint 1.15.1, `Timer`, Slint callback bridge, `std::fs`, shell smoke scripts, `cargo fmt`, `cargo check`, `cargo test`

---

## Execution Notes

- 设计输入固定为 `docs/plans/2026-03-11-top-status-bar-style-bugfix3-design.md`，实现过程中不得回退到 `PopupWindow` 方案，除非明确执行文档中的回滚策略。
- 执行时使用 `@superpowers:test-driven-development`：先写失败测试或失败 smoke，再写最小实现，再运行通过。
- 完成前使用 `@superpowers:verification-before-completion`，禁止先宣称完成再补验证。
- 真实执行建议先在独立 worktree 中进行，避免污染当前主工作区。
- 本轮不扩散到 SSH / SFTP / terminal / sidebar / tabbar 等模块。
- 文件日志为调试能力，不应阻断程序启动；任何日志初始化失败都应回退为 `eprintln!` 并继续运行。

## Task 1: 新增轻量 Tooltip 文件日志模块

**Files:**
- Modify: `src/app/mod.rs`
- Create: `src/app/tooltip_debug_log.rs`
- Create: `tests/tooltip_debug_log.rs`

**Step 1: Write the failing test**

创建 `tests/tooltip_debug_log.rs`：

```rust
use std::fs;

use mica_term::app::tooltip_debug_log::{TooltipDebugEvent, TooltipDebugLog};

#[test]
fn tooltip_debug_log_creates_log_file_and_appends_event_lines() {
    let temp_dir = std::env::temp_dir()
        .join("mica-term")
        .join("tests")
        .join("tooltip-debug-log");
    let _ = fs::remove_dir_all(&temp_dir);

    let logger = TooltipDebugLog::in_directory(temp_dir.join("logs")).unwrap();
    logger
        .append(TooltipDebugEvent {
            phase: "show-tooltip",
            source_id: "nav-button",
            text: "Open menu",
            anchor_x: 24.0,
            anchor_y: 44.0,
        })
        .unwrap();

    let log_path = temp_dir.join("logs").join("titlebar-tooltip.log");
    let content = fs::read_to_string(log_path).unwrap();

    assert!(content.contains("show-tooltip"));
    assert!(content.contains("nav-button"));
    assert!(content.contains("Open menu"));
    assert!(content.contains("anchor_x=24"));

    let _ = fs::remove_dir_all(temp_dir);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test tooltip_debug_log -q`  
Expected: FAIL with missing module/items such as `tooltip_debug_log`, `TooltipDebugLog`, or `TooltipDebugEvent`.

**Step 3: Write minimal implementation**

在 `src/app/mod.rs` 导出新模块：

```rust
pub mod bootstrap;
pub mod tooltip_debug_log;
pub mod ui_preferences;
pub mod windowing;
```

创建 `src/app/tooltip_debug_log.rs`：

```rust
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::Result;

#[derive(Debug, Clone, PartialEq)]
pub struct TooltipDebugEvent<'a> {
    pub phase: &'a str,
    pub source_id: &'a str,
    pub text: &'a str,
    pub anchor_x: f32,
    pub anchor_y: f32,
}

#[derive(Debug, Clone)]
pub struct TooltipDebugLog {
    directory: PathBuf,
}

impl TooltipDebugLog {
    pub fn in_directory(directory: PathBuf) -> Result<Self> {
        fs::create_dir_all(&directory)?;
        Ok(Self { directory })
    }

    pub fn for_current_dir() -> Result<Self> {
        let base = std::env::current_dir()?;
        Self::in_directory(base.join("logs"))
    }

    pub fn log_path(&self) -> PathBuf {
        self.directory.join("titlebar-tooltip.log")
    }

    pub fn append(&self, event: TooltipDebugEvent<'_>) -> Result<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.log_path())?;

        writeln!(
            file,
            "phase={} source_id={} text={:?} anchor_x={} anchor_y={}",
            event.phase,
            event.source_id,
            event.text,
            event.anchor_x,
            event.anchor_y
        )?;

        Ok(())
    }
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test --test tooltip_debug_log -q`  
Expected: PASS, 且日志文件实际生成在测试临时目录下。

**Step 5: Commit**

```bash
git add src/app/mod.rs src/app/tooltip_debug_log.rs tests/tooltip_debug_log.rs
git commit -m "feat: add tooltip debug file logger"
```

## Task 2: 固定 overlay tooltip 契约与布局预算

**Files:**
- Modify: `src/shell/metrics.rs`
- Modify: `tests/titlebar_layout_spec.rs`
- Modify: `tests/top_status_bar_ui_contract_smoke.sh`

**Step 1: Write the failing tests**

修改 `tests/titlebar_layout_spec.rs`，增加 tooltip 预算断言：

```rust
#[test]
fn top_status_bar_tooltip_budget_matches_bugfix3_overlay_design() {
    assert_eq!(ShellMetrics::TITLEBAR_TOOLTIP_DELAY_MS, 280);
    assert_eq!(ShellMetrics::TITLEBAR_TOOLTIP_OFFSET_Y, 8);
    assert!(ShellMetrics::TITLEBAR_TOOLTIP_MIN_WIDTH >= 96);
}
```

修改 `tests/top_status_bar_ui_contract_smoke.sh`，把 tooltip 契约从 popup 改为 overlay：

```bash
grep -F 'tooltip-overlay := TitlebarTooltip' "$TITLEBAR" >/dev/null
grep -F 'tooltip-visible: root.tooltip-visible' "$TITLEBAR" >/dev/null
grep -F 'tooltip-source-id-value' "$TITLEBAR" >/dev/null
grep -F 'TITLEBAR_TOOLTIP_DELAY_MS' "$ROOT_DIR/src/shell/metrics.rs" >/dev/null
! grep -F 'inherits PopupWindow' "$TOOLTIP" >/dev/null
! grep -F 'tooltip-popup := TitlebarTooltip' "$TITLEBAR" >/dev/null
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --test titlebar_layout_spec -q`  
Expected: FAIL because tooltip budget constants do not exist.

Run: `bash tests/top_status_bar_ui_contract_smoke.sh`  
Expected: FAIL because tooltip still uses `PopupWindow` and old symbol names.

**Step 3: Write minimal implementation**

在 `src/shell/metrics.rs` 增加：

```rust
impl ShellMetrics {
    pub const TITLEBAR_TOOLTIP_DELAY_MS: u32 = 280;
    pub const TITLEBAR_TOOLTIP_OFFSET_Y: u32 = 8;
    pub const TITLEBAR_TOOLTIP_MIN_WIDTH: u32 = 96;
}
```

只更新测试和预算常量，暂不修改 Slint 具体实现，确保 Task 3 有明确的失败目标。

**Step 4: Run tests to verify current state**

Run: `cargo test --test titlebar_layout_spec -q`  
Expected: PASS.

Run: `bash tests/top_status_bar_ui_contract_smoke.sh`  
Expected: 继续 FAIL，表明 overlay UI 仍待实现。

**Step 5: Commit**

```bash
git add src/shell/metrics.rs tests/titlebar_layout_spec.rs tests/top_status_bar_ui_contract_smoke.sh
git commit -m "test: lock overlay tooltip contract"
```

## Task 3: 将 tooltip 从 PopupWindow 改为窗口内 overlay，并补齐共享状态机

**Files:**
- Modify: `ui/components/titlebar-icon-button.slint`
- Modify: `ui/components/window-control-button.slint`
- Modify: `ui/components/titlebar-tooltip.slint`
- Modify: `ui/shell/titlebar.slint`
- Modify: `ui/app-window.slint`

**Step 1: Write the failing smoke expectations**

基于 Task 2 已修改的 `tests/top_status_bar_ui_contract_smoke.sh`，补充更细的 overlay/state 断言：

```bash
grep -F 'in property <bool> tooltip-visible: false;' "$TITLEBAR" >/dev/null
grep -F 'in property <string> tooltip-source-id;' "$ROOT_DIR/ui/components/titlebar-icon-button.slint" >/dev/null
grep -F 'in property <string> tooltip-source-id;' "$ROOT_DIR/ui/components/window-control-button.slint" >/dev/null
grep -F 'callback tooltip-open-requested(string, string, length, length);' "$ROOT_DIR/ui/components/titlebar-icon-button.slint" >/dev/null
grep -F 'callback tooltip-close-requested(string);' "$ROOT_DIR/ui/components/window-control-button.slint" >/dev/null
grep -F 'tooltip-overlay := TitlebarTooltip {' "$TITLEBAR" >/dev/null
grep -F 'visible: root.tooltip-visible;' "$TOOLTIP" >/dev/null
```

**Step 2: Run smoke to verify it fails**

Run: `bash tests/top_status_bar_ui_contract_smoke.sh`  
Expected: FAIL because current Slint implementation still uses `PopupWindow`, old callback signatures, and no shared overlay visibility state.

**Step 3: Write minimal implementation**

在 `ui/components/titlebar-icon-button.slint` 增加 `tooltip-source-id` 并让回调带上 source：

```slint
in property <string> tooltip-source-id;
callback tooltip-open-requested(string, string, length, length);
callback tooltip-close-requested(string);

changed has-hover => {
    if self.has-hover && root.tooltip-text != "" {
        root.tooltip-open-requested(
            root.tooltip-source-id,
            root.tooltip-text,
            self.absolute-position.x + self.width / 2,
            self.absolute-position.y + self.height + 8px,
        );
    } else {
        root.tooltip-close-requested(root.tooltip-source-id);
    }
}
```

`ui/components/window-control-button.slint` 同步采用相同签名。

将 `ui/components/titlebar-tooltip.slint` 从 `PopupWindow` 改为普通 overlay 组件：

```slint
export component TitlebarTooltip inherits Rectangle {
    in property <string> text;
    in property <length> anchor-x: 0px;
    in property <length> anchor-y: 0px;
    in property <bool> visible: false;
    in property <length> host-width: 0px;

    width: max(96px, tooltip-label.preferred-width + 20px);
    height: max(28px, tooltip-label.preferred-height + 10px);
    x: max(0px, min(root.anchor-x - root.width / 2, root.host-width - root.width));
    y: root.anchor-y;
    visible: root.visible;
    z: 100;
}
```

在 `ui/shell/titlebar.slint` 中引入共享 tooltip 状态机：

```slint
private property <bool> tooltip-visible: false;
private property <string> tooltip-source-id-value: "";

function schedule-tooltip(source-id: string, text: string, anchor-x: length, anchor-y: length) {
    root.tooltip-source-id-value = source-id;
    root.tooltip-text-value = text;
    root.tooltip-anchor-x-value = anchor-x - self.absolute-position.x;
    root.tooltip-anchor-y-value = anchor-y - self.absolute-position.y;
    root.tooltip-visible = false;
    tooltip-delay.restart();
}

function close-tooltip(source-id: string) {
    if root.tooltip-source-id-value == source-id || source-id == "" {
        root.tooltip-visible = false;
        root.tooltip-text-value = "";
        root.tooltip-source-id-value = "";
        tooltip-delay.stop();
    }
}

tooltip-delay := Timer {
    interval: 280ms;
    triggered => {
        if root.tooltip-text-value != "" {
            root.tooltip-visible = true;
        }
    }
}
```

并把 `tooltip-popup` 替换为窗口内 overlay：

```slint
tooltip-overlay := TitlebarTooltip {
    text: root.tooltip-text-value;
    anchor-x: root.tooltip-anchor-x-value;
    anchor-y: root.tooltip-anchor-y-value;
    visible: root.tooltip-visible;
    host-width: root.width;
}
```

同时在 `ui/app-window.slint` 保持 tooltip overlay 位于当前窗口树内，不新建 popup window。

**Step 4: Run tests to verify they pass**

Run: `bash tests/top_status_bar_ui_contract_smoke.sh`  
Expected: PASS.

Run: `cargo test --test titlebar_layout_spec -q`  
Expected: PASS.

**Step 5: Commit**

```bash
git add ui/components/titlebar-icon-button.slint ui/components/window-control-button.slint ui/components/titlebar-tooltip.slint ui/shell/titlebar.slint ui/app-window.slint tests/top_status_bar_ui_contract_smoke.sh src/shell/metrics.rs tests/titlebar_layout_spec.rs
git commit -m "feat: move titlebar tooltip to overlay state machine"
```

## Task 4: 将 tooltip 生命周期桥接到 Rust 并落盘到 `logs/`

**Files:**
- Modify: `ui/app-window.slint`
- Modify: `ui/shell/titlebar.slint`
- Modify: `src/app/bootstrap.rs`
- Modify: `tests/top_status_bar_smoke.rs`

**Step 1: Write the failing test**

在 `tests/top_status_bar_smoke.rs` 增加日志桥接测试：

```rust
use std::fs;

use mica_term::app::bootstrap::bind_top_status_bar_with_store_and_log_dir;
use mica_term::app::ui_preferences::UiPreferencesStore;

#[test]
fn bootstrap_routes_tooltip_debug_events_to_log_file() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let temp_root = std::env::temp_dir()
        .join("mica-term")
        .join("tests")
        .join("tooltip-debug-bridge");
    let _ = fs::remove_dir_all(&temp_root);

    bind_top_status_bar_with_store_and_log_dir(
        &app,
        Some(UiPreferencesStore::new(temp_root.join("ui-preferences.json"))),
        Some(temp_root.clone()),
    );

    app.invoke_tooltip_debug_event_requested(
        "nav-button".into(),
        "show-tooltip".into(),
        "Open menu".into(),
        24.0,
        44.0,
    );

    let content = fs::read_to_string(temp_root.join("logs").join("titlebar-tooltip.log")).unwrap();
    assert!(content.contains("show-tooltip"));
    assert!(content.contains("nav-button"));

    let _ = fs::remove_dir_all(temp_root);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test top_status_bar_smoke -q`  
Expected: FAIL because `bind_top_status_bar_with_store_and_log_dir` and tooltip debug callback do not exist yet.

**Step 3: Write minimal implementation**

在 `ui/shell/titlebar.slint` 和 `ui/app-window.slint` 增加新的桥接 callback：

```slint
callback tooltip-debug-event-requested(string, string, string, length, length);
```

在 `Titlebar.schedule-tooltip(...)`、`close-tooltip(...)`、`tooltip-delay.triggered` 中发出事件，例如：

```slint
root.tooltip-debug-event-requested(source-id, "schedule-tooltip", text, local-x, local-y);
root.tooltip-debug-event-requested(source-id, "close-tooltip", root.tooltip-text-value, root.tooltip-anchor-x-value, root.tooltip-anchor-y-value);
root.tooltip-debug-event-requested(root.tooltip-source-id-value, "show-tooltip", root.tooltip-text-value, root.tooltip-anchor-x-value, root.tooltip-anchor-y-value);
```

在 `src/app/bootstrap.rs` 增加可注入日志目录的绑定函数：

```rust
pub fn bind_top_status_bar_with_store_and_log_dir(
    window: &AppWindow,
    store: Option<UiPreferencesStore>,
    log_root: Option<PathBuf>,
) {
    let logger = match log_root {
        Some(root) => TooltipDebugLog::in_directory(root.join("logs")).ok(),
        None => TooltipDebugLog::for_current_dir().ok(),
    }
    .map(Rc::new);

    // existing binding...

    let logger_ref = logger.clone();
    window.on_tooltip_debug_event_requested(move |source_id, phase, text, anchor_x, anchor_y| {
        if let Some(logger) = &logger_ref {
            let _ = logger.append(TooltipDebugEvent {
                phase: phase.as_str(),
                source_id: source_id.as_str(),
                text: text.as_str(),
                anchor_x,
                anchor_y,
            });
        }
    });
}
```

保留原有 `bind_top_status_bar_with_store(...)` / `bind_top_status_bar(...)`，让它们委托到新函数，避免破坏当前调用者。

**Step 4: Run tests to verify they pass**

Run: `cargo test --test top_status_bar_smoke -q`  
Expected: PASS.

Run: `cargo test --test tooltip_debug_log -q`  
Expected: PASS.

**Step 5: Commit**

```bash
git add ui/app-window.slint ui/shell/titlebar.slint src/app/bootstrap.rs tests/top_status_bar_smoke.rs src/app/tooltip_debug_log.rs src/app/mod.rs tests/tooltip_debug_log.rs
git commit -m "feat: bridge titlebar tooltip events to file logs"
```

## Task 5: 完整验证并更新交付文档

**Files:**
- Modify: `verification.md`

**Step 1: Write the failing verification checklist**

先把 `verification.md` 的 source documents 和 checklist 改为 `bugfix3` 目标：

```markdown
- Design: `docs/plans/2026-03-11-top-status-bar-style-bugfix3-design.md`
- Implementation Plan: `docs/plans/2026-03-11-top-status-bar-style-bugfix3-implementation-plan.md`
- [ ] Tooltip is rendered by in-window overlay
- [ ] `logs/titlebar-tooltip.log` is created on demand
- [ ] Hover enter / schedule / show / close events are observable in log order
```

**Step 2: Run the full verification suite**

Run: `cargo fmt --all`  
Expected: PASS.

Run: `cargo check --workspace`  
Expected: PASS.

Run: `cargo test -q`  
Expected: PASS.

Run: `bash tests/fluent_titlebar_assets_smoke.sh`  
Expected: PASS.

Run: `bash tests/icon_svg_assets_smoke.sh`  
Expected: PASS.

Run: `bash tests/top_status_bar_ui_contract_smoke.sh`  
Expected: PASS.

Run: `cargo clippy --workspace -- -D warnings`  
Expected: PASS.

**Step 3: Perform manual GUI smoke on Windows 11**

Run: `cargo run`

手工验证：

- hover `Navigation`
- hover `theme`
- hover `panel-toggle`
- hover `pin`
- hover `minimize / maximize / close`
- 快速横向扫过多个按钮
- 打开 `global-menu` 时 tooltip 是否立即关闭
- 查看程序当前目录 `logs/titlebar-tooltip.log`

日志期望：

- 至少可见 `schedule-tooltip`
- 至少可见 `show-tooltip`
- 至少可见 `close-tooltip`
- source id 和 tooltip text 可对应到实际按钮

**Step 4: Update verification.md with actual results**

把所有真实执行结果、环境限制和 Windows 11 手工结论写回 `verification.md`，不要保留未更新的 `bugfix2` 文案。

**Step 5: Commit**

```bash
git add verification.md
git commit -m "docs: capture top status bar style bugfix3 verification"
```

## Final Review Gate

在宣称完成前，必须同时满足：

- [ ] tooltip 不再使用 `PopupWindow`
- [ ] 共享 tooltip 仍然只有一个实例
- [ ] 按钮 hover 走统一 intent，不在每个按钮里直接渲染 tooltip
- [ ] 当前工作目录能自动生成 `logs/`
- [ ] `titlebar-tooltip.log` 记录可用于排障
- [ ] `cargo test -q` 全绿
- [ ] `bash tests/top_status_bar_ui_contract_smoke.sh` 全绿
- [ ] Windows 11 手工 hover 行为与 design 文档一致

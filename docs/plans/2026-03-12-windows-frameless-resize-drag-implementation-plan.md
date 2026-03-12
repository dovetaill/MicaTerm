# Windows Frameless Resize Drag Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 为当前 `Slint + frameless + self-drawn titlebar` 窗口壳层补齐最小尺寸真源、显式 resize 交互层、Windows 中度 frame adapter，以及最大化切换后的稳定顶栏拖拽链路，解决窗口缩放失真、上下/角落 resize 不可靠、最大化后顶栏偶发拖拽失效这三类问题。

**Architecture:** 以 `ShellMetrics` 作为唯一窗口尺寸预算真源，先将 `min-width / min-height` 固化到 `AppWindow` 与 runtime contract，再引入一层显式 `resize interaction layer` 将边与角的交互直接映射到 `winit::Window::drag_resize_window()`。Windows 侧继续保留中度 `frame adapter`，负责 placement 与 maximize hit-test 语义桥接，但不升级为全量 Win32 non-client 接管；顶栏拖拽则从“移动后触发”改为“按下即启动”，使调用时机与 `winit::Window::drag_window()` 的契约一致。

**Tech Stack:** Rust, Cargo, Slint 1.15.1, `unstable-winit-030`, winit 0.30.13, `windows-sys`, `i-slint-backend-testing`, shell smoke scripts, `cargo fmt`, `cargo test`, `cargo check`, `cargo clippy`

---

## Execution Notes

- 设计输入固定为 `docs/plans/2026-03-12-windows-frameless-resize-drag-design.md`，实现阶段不得偏离已确认组合：`1A + 2C + 3B + 4B + 5B`。
- 使用 `@superpowers:test-driven-development`：每个任务都先写失败测试或失败 smoke，再写最小实现，再跑通过。
- 如果 `Slint TouchArea`、`resize-border-width`、`drag_window()` 或 `drag_resize_window()` 的运行时行为与预期不一致，不允许直接 patch，必须切到 `@superpowers:systematic-debugging` 收集证据。
- 本轮只处理窗口壳层、尺寸与交互契约，不顺手接 terminal、SSH、SFTP，也不做视觉稿重构。
- Windows 专用行为必须收敛在 `src/app/windows_frame.rs` 和 `src/app/windowing.rs` 的平台边界内，禁止把 Win32 细节扩散到通用 UI 组件。
- 计划默认在独立 worktree 执行；如果继续在当前工作区执行，改动必须限制在本计划列出的文件中。

## Target Snapshot

完成后应满足以下结果：

- 窗口不能继续缩小到低于 `688 x 640`
- `AppWindow` 与 runtime 对最小尺寸的认知一致
- 上、下、左、右以及四角都存在稳定的 resize 交互入口
- 右下角拖动会同时改变宽度与高度
- maximize / restore / snap / unsnap 后，Windows frame adapter 不会抢占 resize 交互
- 双击顶栏或点击最大化按钮后，顶栏空白区仍可稳定拖动窗口
- 现有 maximize button hit-test、窗口按钮、theme / sidebar / right panel 行为不退化

## Out of Scope

- `wezterm-term` / `termwiz` 接入
- `russh` / SFTP 功能
- Welcome / Sidebar / TabBar 的视觉重做
- 上次窗口位置与尺寸持久化
- 全量 Win32 `WM_NCHITTEST -> HTCAPTION / HTTOP / HTLEFT / ...` 重写

## Task Ordering

1. 先建立最小尺寸真源，防止后续 resize 行为继续建立在错误几何预算上。
2. 再落 Rust 侧 resize direction bridge 与 UI 显式 grips，先把上下/角落 resize 做成可建模输入。
3. 然后收敛 Windows frame adapter 的命中区边界，避免与 grips 互相抢事件。
4. 再修 titlebar drag 的启动时机，让 maximize 切换后的拖拽不再偶发失效。
5. 最后补全 contract tests、smoke scripts 和手工验证矩阵，防止再次回归。

## Task 1: 固化窗口最小尺寸真源

**Files:**
- Modify: `src/shell/metrics.rs`
- Modify: `src/app/windowing.rs`
- Modify: `ui/app-window.slint`
- Modify: `tests/window_shell.rs`
- Create: `tests/window_resize_drag_contract_smoke.sh`

**Step 1: Write the failing tests**

修改 `tests/window_shell.rs`，补充最小尺寸 contract：

```rust
#[test]
fn window_shell_exposes_minimum_window_budget() {
    let spec = window_command_spec();

    assert_eq!(spec.min_window_width, ShellMetrics::WINDOW_MIN_WIDTH);
    assert_eq!(spec.min_window_height, ShellMetrics::WINDOW_MIN_HEIGHT);
}
```

创建 `tests/window_resize_drag_contract_smoke.sh`：

```bash
#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_FILE="$ROOT_DIR/ui/app-window.slint"
WINDOWING_FILE="$ROOT_DIR/src/app/windowing.rs"

grep -F 'min-width:' "$APP_FILE" >/dev/null
grep -F 'min-height:' "$APP_FILE" >/dev/null
grep -F 'min_window_width' "$WINDOWING_FILE" >/dev/null
grep -F 'min_window_height' "$WINDOWING_FILE" >/dev/null
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --test window_shell -q`  
Expected: FAIL，`WindowCommandSpec` 尚未导出 `min_window_width` / `min_window_height`。

Run: `bash tests/window_resize_drag_contract_smoke.sh`  
Expected: FAIL，`.slint` 里还没有 `min-width` / `min-height`。

**Step 3: Write minimal implementation**

修改 `src/app/windowing.rs`：

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowCommandSpec {
    pub uses_winit_drag: bool,
    pub uses_winit_drag_resize: bool,
    pub self_drawn_controls: bool,
    pub supports_double_click_maximize: bool,
    pub supports_always_on_top: bool,
    pub supports_true_window_state_tracking: bool,
    pub supports_native_frame_adapter: bool,
    pub resize_border_width: u32,
    pub min_window_width: u32,
    pub min_window_height: u32,
}

pub fn window_command_spec() -> WindowCommandSpec {
    WindowCommandSpec {
        uses_winit_drag: true,
        uses_winit_drag_resize: true,
        self_drawn_controls: true,
        supports_double_click_maximize: true,
        supports_always_on_top: true,
        supports_true_window_state_tracking: true,
        supports_native_frame_adapter: true,
        resize_border_width: 6,
        min_window_width: ShellMetrics::WINDOW_MIN_WIDTH,
        min_window_height: ShellMetrics::WINDOW_MIN_HEIGHT,
    }
}
```

修改 `ui/app-window.slint`，将最小尺寸声明为窗口层契约：

```slint
min-width: 688px;
min-height: 640px;
```

如果执行阶段希望避免 magic number，可在注释中明确它们来自 `ShellMetrics::WINDOW_MIN_*`，但不要在 `.slint` 里再造第二套业务语义。

**Step 4: Run tests to verify they pass**

Run: `cargo test --test window_shell -q`  
Expected: PASS，最小尺寸预算成为 `window_command_spec()` 的一部分。

Run: `bash tests/window_resize_drag_contract_smoke.sh`  
Expected: PASS，窗口最小尺寸已经在 UI contract 中显式声明。

**Step 5: Commit**

```bash
git add src/shell/metrics.rs src/app/windowing.rs ui/app-window.slint \
  tests/window_shell.rs tests/window_resize_drag_contract_smoke.sh
git commit -m "fix: define frameless window minimum size contract"
```

## Task 2: 建立显式 resize direction bridge 与 edge grips

**Files:**
- Create: `ui/components/window-resize-grips.slint`
- Modify: `ui/app-window.slint`
- Modify: `src/app/windowing.rs`
- Modify: `src/app/bootstrap.rs`
- Create: `tests/window_resize_direction_spec.rs`
- Modify: `tests/window_resize_drag_contract_smoke.sh`

**Step 1: Write the failing tests**

创建 `tests/window_resize_direction_spec.rs`：

```rust
use mica_term::app::windowing::{
    parse_resize_direction, WindowResizeDirection,
};

#[test]
fn parse_resize_direction_accepts_all_edges_and_corners() {
    assert_eq!(parse_resize_direction("north"), Some(WindowResizeDirection::North));
    assert_eq!(parse_resize_direction("south"), Some(WindowResizeDirection::South));
    assert_eq!(parse_resize_direction("east"), Some(WindowResizeDirection::East));
    assert_eq!(parse_resize_direction("west"), Some(WindowResizeDirection::West));
    assert_eq!(parse_resize_direction("north-east"), Some(WindowResizeDirection::NorthEast));
    assert_eq!(parse_resize_direction("north-west"), Some(WindowResizeDirection::NorthWest));
    assert_eq!(parse_resize_direction("south-east"), Some(WindowResizeDirection::SouthEast));
    assert_eq!(parse_resize_direction("south-west"), Some(WindowResizeDirection::SouthWest));
}

#[test]
fn parse_resize_direction_rejects_unknown_values() {
    assert_eq!(parse_resize_direction("center"), None);
    assert_eq!(parse_resize_direction(""), None);
}
```

扩展 `tests/window_resize_drag_contract_smoke.sh`：

```bash
GRIPS_FILE="$ROOT_DIR/ui/components/window-resize-grips.slint"
BOOTSTRAP_FILE="$ROOT_DIR/src/app/bootstrap.rs"

grep -F 'resize-requested(string)' "$GRIPS_FILE" >/dev/null
grep -F 'drag-resize-requested(string)' "$APP_FILE" >/dev/null
grep -F 'drag_resize_window' "$WINDOWING_FILE" >/dev/null
grep -F 'on_drag_resize_requested' "$BOOTSTRAP_FILE" >/dev/null
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --test window_resize_direction_spec -q`  
Expected: FAIL，`WindowResizeDirection` / `parse_resize_direction` 尚不存在。

Run: `bash tests/window_resize_drag_contract_smoke.sh`  
Expected: FAIL，grips 组件和 `drag-resize-requested` 回调尚未接入。

**Step 3: Write minimal implementation**

在 `src/app/windowing.rs` 增加方向枚举、解析函数和 drag-resize 桥接：

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowResizeDirection {
    North,
    South,
    East,
    West,
    NorthEast,
    NorthWest,
    SouthEast,
    SouthWest,
}

pub fn parse_resize_direction(value: &str) -> Option<WindowResizeDirection> {
    match value {
        "north" => Some(WindowResizeDirection::North),
        "south" => Some(WindowResizeDirection::South),
        "east" => Some(WindowResizeDirection::East),
        "west" => Some(WindowResizeDirection::West),
        "north-east" => Some(WindowResizeDirection::NorthEast),
        "north-west" => Some(WindowResizeDirection::NorthWest),
        "south-east" => Some(WindowResizeDirection::SouthEast),
        "south-west" => Some(WindowResizeDirection::SouthWest),
        _ => None,
    }
}

impl<C: ComponentHandle> WindowController<C> {
    pub fn drag_resize(&self, direction: WindowResizeDirection) -> Result<()> {
        use slint::winit_030::{WinitWindowAccessor, winit};

        let mapped = match direction {
            WindowResizeDirection::North => winit::window::ResizeDirection::North,
            WindowResizeDirection::South => winit::window::ResizeDirection::South,
            WindowResizeDirection::East => winit::window::ResizeDirection::East,
            WindowResizeDirection::West => winit::window::ResizeDirection::West,
            WindowResizeDirection::NorthEast => winit::window::ResizeDirection::NorthEast,
            WindowResizeDirection::NorthWest => winit::window::ResizeDirection::NorthWest,
            WindowResizeDirection::SouthEast => winit::window::ResizeDirection::SouthEast,
            WindowResizeDirection::SouthWest => winit::window::ResizeDirection::SouthWest,
        };

        self.with_window(|window| {
            window
                .with_winit_window(|window: &winit::window::Window| {
                    window.drag_resize_window(mapped).map_err(|err| anyhow!(err.to_string()))
                })
                .unwrap_or_else(|| Err(anyhow!("winit window is unavailable")))
        })
        .unwrap_or_else(|| Err(anyhow!("window is unavailable")))
    }
}
```

创建 `ui/components/window-resize-grips.slint`，提供 8 个 invisible grips：

```slint
export component WindowResizeGrips inherits Rectangle {
    in property <length> grip-size: 10px;
    callback resize-requested(string);

    north := TouchArea {
        height: root.grip-size;
        width: parent.width - root.grip-size * 2;
        x: root.grip-size;
        mouse-cursor: MouseCursor.n-resize;
        pointer-event(event) => {
            if event.kind == PointerEventKind.down {
                root.resize-requested("north");
            }
        }
    }

    // south / east / west / north-east / north-west / south-east / south-west 同理
}
```

在 `ui/app-window.slint` 接入：

```slint
callback drag-resize-requested(string);

resize-grips := WindowResizeGrips {
    width: root.width;
    height: root.height;
    resize-requested(direction) => {
        root.drag-resize-requested(direction);
    }
}
```

在 `src/app/bootstrap.rs` 绑定：

```rust
let controller_ref = Rc::clone(&controller);
window.on_drag_resize_requested(move |direction| {
    if let Some(direction) = parse_resize_direction(direction.as_str()) {
        let _ = controller_ref.drag_resize(direction);
    }
});
```

**Step 4: Run tests to verify they pass**

Run: `cargo test --test window_resize_direction_spec -q`  
Expected: PASS，所有边和角方向都能稳定解析。

Run: `bash tests/window_resize_drag_contract_smoke.sh`  
Expected: PASS，显式 grips、UI callback 和 Rust bridge 已全部接通。

**Step 5: Commit**

```bash
git add ui/components/window-resize-grips.slint ui/app-window.slint \
  src/app/windowing.rs src/app/bootstrap.rs \
  tests/window_resize_direction_spec.rs tests/window_resize_drag_contract_smoke.sh
git commit -m "feat: add explicit frameless drag resize grips"
```

## Task 3: 收敛 Windows frame adapter 的命中区边界

**Files:**
- Modify: `src/app/windows_frame.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `tests/windows_frame_spec.rs`
- Modify: `tests/windows_frame_contract_smoke.sh`

**Step 1: Write the failing tests**

扩展 `tests/windows_frame_spec.rs`，增加 frame adapter 不抢占外层 resize 区域的纯函数验证：

```rust
use mica_term::app::windows_frame::point_hits_outer_resize_band;

#[test]
fn frame_adapter_treats_outer_resize_band_as_reserved() {
    assert!(point_hits_outer_resize_band(2, 2, 1200, 800, 10));
    assert!(point_hits_outer_resize_band(1198, 798, 1200, 800, 10));
    assert!(!point_hits_outer_resize_band(80, 24, 1200, 800, 10));
}
```

扩展 `tests/windows_frame_contract_smoke.sh`：

```bash
grep -F 'point_hits_outer_resize_band' "$FRAME_FILE" >/dev/null
grep -F 'reserved resize band' "$FRAME_FILE" >/dev/null
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --test windows_frame_spec -q`  
Expected: FAIL，新的 helper 尚不存在。

Run: `bash tests/windows_frame_contract_smoke.sh`  
Expected: FAIL，frame adapter 里还没有 resize band 保护逻辑。

**Step 3: Write minimal implementation**

在 `src/app/windows_frame.rs` 增加纯函数 helper：

```rust
pub fn point_hits_outer_resize_band(
    x: i32,
    y: i32,
    window_width: i32,
    window_height: i32,
    band: i32,
) -> bool {
    x < band
        || y < band
        || x >= window_width.saturating_sub(band)
        || y >= window_height.saturating_sub(band)
}
```

然后在 `WM_NCHITTEST` 路径中，只在“非外层 resize band 且命中 maximize button 时”才返回 `HTMAXBUTTON`。保留 `DefSubclassProc` 作为主路径，不新增 `HTTOP/HTLEFT/...` 返回值。

为避免 magic number，band 预算应与 grips 组件的 `grip-size` 保持一致，建议经由 `bootstrap` 把当前预算同步到 adapter。

**Step 4: Run tests to verify they pass**

Run: `cargo test --test windows_frame_spec -q`  
Expected: PASS，adapter 对 maximize hit-test 与外层 resize 区域有明确边界。

Run: `bash tests/windows_frame_contract_smoke.sh`  
Expected: PASS，frame adapter 仍保留 subclass / HTMAXBUTTON 路径，同时已声明 resize band 保护。

**Step 5: Commit**

```bash
git add src/app/windows_frame.rs src/app/bootstrap.rs \
  tests/windows_frame_spec.rs tests/windows_frame_contract_smoke.sh
git commit -m "fix: reserve resize band in windows frame adapter"
```

## Task 4: 将顶栏拖拽改为按下即启动，并保留双击最大化

**Files:**
- Modify: `ui/shell/titlebar.slint`
- Modify: `src/app/windowing.rs`
- Modify: `tests/top_status_bar_ui_contract_smoke.sh`
- Modify: `tests/top_status_bar_smoke.rs`

**Step 1: Write the failing tests**

扩展 `tests/top_status_bar_ui_contract_smoke.sh`：

```bash
TITLEBAR="$ROOT_DIR/ui/shell/titlebar.slint"

grep -F 'pointer-event(event)' "$TITLEBAR" >/dev/null
grep -F 'PointerEventKind.down' "$TITLEBAR" >/dev/null
if grep -F 'moved => {' "$TITLEBAR" >/dev/null; then
  exit 1
fi
```

在 `tests/top_status_bar_smoke.rs` 增加行为契约测试，至少守住 maximize toggle 后状态同步不回归：

```rust
#[test]
fn maximize_toggle_keeps_drag_related_window_state_bindings_consistent() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_maximize_toggle_requested();
    assert!(app.get_is_window_maximized());
    assert!(app.get_use_flat_window_chrome());

    app.invoke_drag_double_clicked();
    assert!(!app.get_is_window_maximized());
    assert!(!app.get_use_flat_window_chrome());
}
```

**Step 2: Run tests to verify they fail**

Run: `bash tests/top_status_bar_ui_contract_smoke.sh`  
Expected: FAIL，当前 titlebar 还是通过 `moved` 触发拖拽。

Run: `cargo test --test top_status_bar_smoke -q`  
Expected: 现有测试应继续 PASS；这是为了给后续改动建立回归基线。

**Step 3: Write minimal implementation**

修改 `ui/shell/titlebar.slint` 的 drag-zone：

```slint
drag-touch := TouchArea {
    width: parent.width;
    height: parent.height;
    mouse-cursor: MouseCursor.grab;

    pointer-event(event) => {
        if event.kind == PointerEventKind.down && event.button == PointerEventButton.left {
            root.drag-requested();
        }
    }

    double-clicked => {
        root.drag-double-clicked();
    }
}
```

如果执行阶段发现 `clicked()` 与 `double-clicked()` 顺序干扰拖拽，必须保留 `double-clicked()` 为最大化路径，并通过 `systematic-debugging` 确认 `pointer-event(down)` 是否需要在双击场景下去重；不允许在没有证据的情况下再退回 `moved` 路线。

**Step 4: Run tests to verify they pass**

Run: `bash tests/top_status_bar_ui_contract_smoke.sh`  
Expected: PASS，顶栏拖拽入口已切换到 `pointer-event(down)`。

Run: `cargo test --test top_status_bar_smoke -q`  
Expected: PASS，maximize / restore 与窗口状态绑定不退化。

**Step 5: Commit**

```bash
git add ui/shell/titlebar.slint tests/top_status_bar_ui_contract_smoke.sh \
  tests/top_status_bar_smoke.rs
git commit -m "fix: start titlebar drag on pointer down"
```

## Task 5: 补齐总体验证与回归守卫

**Files:**
- Modify: `tests/window_shell.rs`
- Modify: `tests/window_geometry_spec.rs`
- Modify: `tests/windows_drag_restore_contract_smoke.sh`
- Modify: `tests/window_resize_drag_contract_smoke.sh`
- Create: `verification.md` entry for this feature

**Step 1: Write the failing tests**

在 `tests/window_geometry_spec.rs` 增加几何 contract：

```rust
#[test]
fn frameless_window_still_exports_resize_border_budget() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);
    app.show().unwrap();

    assert_eq!(app.get_layout_resize_border_width() as u32, 6);
}
```

扩展 `tests/windows_drag_restore_contract_smoke.sh`：

```bash
grep -F 'uses_winit_drag_resize: true' "$WINDOWING_FILE" >/dev/null
grep -F 'drag-resize-requested' "$APP_FILE" >/dev/null
```

**Step 2: Run tests to verify they fail**

Run: `bash tests/windows_drag_restore_contract_smoke.sh`  
Expected: FAIL，新的 drag-resize contract 尚未被 smoke 覆盖。

Run: `cargo test --test window_geometry_spec -q`  
Expected: 如果前面尚未实现完整导出，可能 FAIL；否则作为最终基线使用。

**Step 3: Write minimal implementation**

这里不新增新能力，只做验证文件与 smoke 守卫补齐：

- `tests/window_shell.rs` 断言 `uses_winit_drag_resize: true`
- `tests/window_geometry_spec.rs` 断言 `layout_resize_border_width`
- `tests/windows_drag_restore_contract_smoke.sh` 和 `tests/window_resize_drag_contract_smoke.sh` 同步新 contract
- 在 `verification.md` 记录执行命令、预期结果和手工验证矩阵：
  - restored 四边 / 四角 resize
  - 右下角同时改变宽高
  - 缩到最小值后继续拖动不再变小
  - maximize -> titlebar drag restore
  - maximize button -> restore -> titlebar drag
  - snap -> unsnap 后仍可 resize 与 drag

**Step 4: Run the final verification suite**

Run:

```bash
cargo fmt --check
cargo test --test window_shell --test window_resize_direction_spec --test windows_frame_spec --test window_geometry_spec --test top_status_bar_smoke -q
cargo check
cargo clippy --all-targets --all-features -- -D warnings
bash tests/window_resize_drag_contract_smoke.sh
bash tests/windows_frame_contract_smoke.sh
bash tests/windows_drag_restore_contract_smoke.sh
bash tests/top_status_bar_ui_contract_smoke.sh
```

Expected:

- 所有 Rust tests PASS
- `cargo check` PASS
- `cargo clippy` PASS
- 所有 smoke scripts PASS

**Step 5: Commit**

```bash
git add tests/window_shell.rs tests/window_geometry_spec.rs \
  tests/window_resize_drag_contract_smoke.sh tests/windows_drag_restore_contract_smoke.sh \
  verification.md
git commit -m "test: cover frameless resize and titlebar drag contracts"
```

## Manual Verification Matrix

在 Windows 11 真机上至少执行以下手工验证：

1. 默认启动窗口后，从上、下、左、右、右下角分别拖动，确认方向正确。
2. 将窗口缩小到 `688 x 640` 附近，确认继续拖动不会再缩小。
3. 双击顶栏最大化，再从顶栏空白区拖动恢复，确认恢复后立即可继续拖动。
4. 点击最大化按钮最大化，再从顶栏空白区拖动恢复，确认按钮和拖拽都正常。
5. Snap left / snap right 后再拖出，确认边角、圆角/方角和 resize 都不退化。

## Rollback Plan

- 如果 Task 2 的显式 grips 引入新输入冲突，可先回退 `ui/components/window-resize-grips.slint` 与 `drag-resize-requested`，保留 Task 1 和 Task 4。
- 如果 Task 4 的 `pointer-event(down)` 与双击最大化存在竞争，可回退 titlebar 事件改动，但保留最小尺寸与显式 resize bridge。
- 如果 Task 3 的 frame adapter resize band 保护与 maximize hit-test 冲突，可回退 helper 与保护分支，仅保留现有 `HTMAXBUTTON` 路径。

## References

- Slint `Window`: `min-width / min-height / resize-border-width`
  - https://docs.slint.dev/latest/docs/slint/reference/window/window/
- Slint `TouchArea`: `pointer-event`, `double-clicked`, `moved`
  - https://docs.slint.dev/latest/docs/slint/reference/gestures/toucharea/
- winit `Window::drag_window()` / `Window::drag_resize_window()`
  - https://docs.rs/winit/latest/winit/window/struct.Window.html

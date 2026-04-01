# Windows Native Terminal Surface D2D TDD Handoff Spec

日期: 2026-04-01
执行者: Codex
状态: Task 1-7 已按计划完成，当前文档记录已落地实现与后续测试建议

## 1. 实现范围与当前状态

本轮实现已经把 Windows native terminal surface 从“只有 retained frame token 的桥接骨架”推进到以下状态：

- `terminal_layout` 已保留 `bg_rgba`，background run 可以进入 retained display list
- `WgpuTerminalRenderer::prepare()` 已产出：
  - `PreparedBackgroundRun`
  - `PreparedMonochromeGlyphDraw`
  - `PreparedColorGlyphDraw`
  - underline draw payload
  - upload payload / cache reuse contract
- `WindowsNativePresenter` 已把上述 payload 组装为 `PresentableNativeFrame`
- `NativeTerminalSurface` 已 retain `NativeTerminalFrame`，并在 Slint `BeforeRendering` 时驱动平台 backend
- `WindowsNativeSurfaceBackend` 已实现：
  - render-target lifecycle state
  - background / selection / mono glyph / color glyph / underline / cursor / IME draw stage
  - mono / color glyph resource cache bookkeeping

需要明确说明的是：

- 当前 `windows.rs` 落地的是 **retained draw contract + backend lifecycle/state + draw order bookkeeping**
- 现有验证覆盖了 source contract、Rust 编译、clippy 与 Linux-host Windows 打包
- 本轮没有在当前 Debian 13 环境中完成 Windows 运行态的实际目视 UI 验证
- 文档以下内容以“已经真实落地到代码中的事实”为准，不把未实现的 Direct2D API 细节写成既成事实

## 2. 核心 Struct

### terminal layout / renderer prepare 层

- `src/app/terminal_layout/run_segmentation.rs`
  - `TextStyleKey`
    - `fg_rgba`
    - `bg_rgba`
    - `bold`
    - `underline`
  - 作用：按样式边界切分 run，为 retained frame 的 background / glyph draw 提供稳定输入

- `src/app/terminal_renderer/wgpu_renderer.rs`
  - `PreparedBackgroundRun`
    - `row`
    - `start_col`
    - `end_col`
    - `bg_rgba`
  - `PreparedMonochromeGlyphUploadPayload`
    - `width_px`
    - `height_px`
    - `bearing_x_px`
    - `bearing_y_px`
    - `advance_px`
    - `coverage: Vec<u8>`
  - `PreparedColorGlyphUploadPayload`
    - `width_px`
    - `height_px`
    - `rgba: Vec<u8>`
  - `PreparedMonochromeGlyphDraw`
    - `atlas_entry`
    - `upload: Option<PreparedMonochromeGlyphUploadPayload>`
    - `fg_rgba`
    - retained placement fields
  - `PreparedColorGlyphDraw`
    - `cache_entry`
    - `upload: Option<PreparedColorGlyphUploadPayload>`
    - retained placement fields
  - `PreparedUnderlineRun`
  - `PreparedUnderlineOverlay`
  - `PreparedNativeRendererStats`
  - `PreparedNativeFrame`
    - 包含 background runs / mono draws / color draws / underline overlay / renderer stats

### presenter 层

- `src/app/terminal_presenter.rs`
  - `NativeCursorFrameState`
  - `NativeSelectionFrameState`
  - `NativeCursorOverlay`
  - `NativeSelectionRect`
  - `NativeSelectionOverlay`
  - `NativeUnderlineRun`
  - `NativeUnderlineOverlay`
  - `NativeImePreviewOverlay`
  - `NativeRendererFrameStats`
  - `PresentableNativeFrame`
    - 真正的 retained display list 聚合对象
  - `NativeTerminalFrame`
    - `frame_token`
    - `cell_width_px`
    - `cell_height_px`
    - `presentable_frame`
  - `WindowsNativePresenter`
    - `font_system: DirectWriteFontSystem`
    - `shaper: TerminalTextShaper`
    - `renderer: WgpuTerminalRenderer`
    - `loaded_font: LoadedFont`
    - `previous_frame: Option<TerminalModelFrame>`

### Slint bridge / platform backend 层

- `src/app/terminal_renderer/native_surface.rs`
  - `NativeTerminalSurface`
  - 内部 `NativeTerminalSurfaceState`
    - `backend: Box<dyn PlatformNativeSurfaceBackend>`
    - `retained_frame`
    - `rect`
    - `last_drawn_frame_token`

- `src/app/terminal_renderer/platform/backend.rs`
  - `NativeTerminalSurfaceRect`
  - `RetainedNativeTerminalSurfaceFrame`
  - `PlatformNativeSurfaceBackend`

- `src/app/terminal_renderer/platform/windows.rs`
  - `WindowsD2DFactoryState`
  - `WindowsHwndRenderTargetState`
  - `WindowsD2DBrushState`
  - `WindowsMonochromeGlyphBitmapState`
  - `WindowsColorGlyphBitmapState`
  - `WindowsNativeSurfaceState`
    - `d2d_factory`
    - `hwnd_render_target`
    - `render_target_generation`
    - `render_target_dirty`
    - `d2d_brushes`
    - `monochrome_glyph_bitmaps`
    - `color_glyph_bitmaps`
    - draw counters / last-presented bookkeeping
  - `WindowsNativeSurfaceBackend`

## 3. Trait 与接口契约

### `TerminalPresenter`

路径：`src/app/terminal_presenter.rs`

合同：

- `present(&mut self, surface: &TerminalSurfaceState, options: TerminalPresentationOptions) -> Result<PresentedTerminalFrame>`
- `default_cell_size(&self) -> (u32, u32)`

实现要点：

- 输入是 runtime snapshot，不直接操作 UI
- 输出是 `PresentedTerminalFrame::Native(Box<NativeTerminalFrame>)`
- `TerminalPresentationOptions` 当前承载：
  - selection
  - selection overlay color
  - IME preview overlay

### `PlatformNativeSurfaceBackend`

路径：`src/app/terminal_renderer/platform/backend.rs`

合同：

- `attach(&mut self, window: &AppWindow) -> Result<()>`
- `update_surface_rect(&mut self, rect: NativeTerminalSurfaceRect)`
- `update_frame(&mut self, frame: Option<RetainedNativeTerminalSurfaceFrame>)`
- `present(&mut self)`
- `detach(&mut self)`

实现意图：

- `NativeTerminalSurface` 不知道 Windows / Wayland / X11 细节
- retained frame 的生命周期、surface rect、present 调度全部通过此 trait 抽象

### `FontSystem` / `TextShaper`

相关路径：

- `src/app/terminal_font/backend.rs`
- `src/app/terminal_layout/shaper.rs`

对本轮实现最关键的合同：

- `shape_text_runs()`
- `rasterize_glyph()`
- `rasterize_color_glyph()`
- `TextShaper::shape_row()`

这些合同确保：

- shaping 与 rasterization 继续留在 font/layout seam
- backend 不需要重新解析终端文本，也不需要重新做 fallback / ligature 决策

## 4. Slint callbacks / global state / bindings

### Slint properties

相关路径：

- `ui/app-window.slint`
- `ui/shell/workspace-pane.slint`
- `ui/shell/terminal-session-host.slint`

当前关键绑定：

- `workspace-session-native-frame-token`
- `workspace-session-cell-width`
- `workspace-session-cell-height`
- `workspace-session-cursor-row`
- `workspace-session-cursor-col`
- `workspace-session-cursor-visible`
- `workspace-session-cursor-blinking`
- `workspace-session-cursor-shape`
- `workspace-session-cursor-fg`
- `workspace-session-cursor-bg`
- `workspace-session-default-fg`
- `workspace-session-default-bg`

### bootstrap 中的 global state

路径：`src/app/bootstrap.rs`

当前 thread-local 全局：

- `WORKSPACE_TERMINAL_PRESENTER: RefCell<Box<dyn TerminalPresenter>>`
- `WORKSPACE_NATIVE_TERMINAL_SURFACE: RefCell<Option<NativeTerminalSurface>>`

### Slint rendering notifier

路径：`src/app/terminal_renderer/native_surface.rs`

回调链：

- `RenderingState::BeforeRendering`
  - 调用 `draw_retained_frame()`
  - 再调用 `state.backend.present()`
- `RenderingState::RenderingTeardown`
  - 调用 `teardown_native_surface()`
  - 清 frame 并 detach backend

### bootstrap 到 native surface 的桥接

路径：`src/app/bootstrap.rs`

关键函数：

- `sync_workspace_native_terminal_surface_geometry()`
- `present_workspace_native_terminal_frame()`
- `clear_workspace_native_terminal_frame()`
- `sync_workspace_session_state_with_manager()`

其中：

- `present_workspace_native_terminal_frame()` 会先写 `workspace_session_native_frame_token`
- 然后调用 `surface.update_terminal_rect(rect)`
- 再调用 `surface.update_frame_state(frame)`

## 5. Tokio task / channel / actor 相关交互

本轮实现 **没有新增** Tokio actor、channel 或背压策略代码；渲染链仍是“消费现有 session/runtime 输出”的同步 UI 路径。

当前真实交互关系如下：

- Tokio 侧仍由现有 SSH/session/runtime 体系维护远端 I/O、终端状态与 surface snapshot
- UI 侧在 `sync_workspace_session_state_with_manager()` 中读取 `state.active_workspace_terminal_surface()`
- `TerminalPresenter::present()` 在 UI 线程把 snapshot 投影为 `NativeTerminalFrame`
- `NativeTerminalSurface` retain 该 frame，并等待 Slint `BeforeRendering`
- `PlatformNativeSurfaceBackend::present()` 在 notifier 回调里消费 retained frame

因此当前实现的结论是：

- 新的 Windows native surface path 没有引入额外跨线程共享渲染状态
- 没有新增 channel 积压点
- 渲染正确性主要受“UI 线程调用时机”和“retained frame 与 backend cache 一致性”影响

## 6. 状态流转说明

### Surface / presenter 主流程

1. SSH/runtime 更新 `TerminalSurfaceState`
2. `TerminalModelFrame::from_surface()` 生成 renderer-facing model
3. `segment_row()` 以 `fg_rgba + bg_rgba + bold + underline` 切 run
4. `TerminalTextShaper::shape_row()` 生成 shaped rows
5. `WgpuTerminalRenderer::prepare()` 生成 `PreparedNativeFrame`
6. `WindowsNativePresenter::present()` 聚合为 `NativeTerminalFrame`
7. bootstrap 调用 `present_workspace_native_terminal_frame()`
8. `NativeTerminalSurface` 更新 rect 与 retained frame
9. Slint `BeforeRendering` 时调用 backend `present()`
10. Windows backend 按顺序消费：
    - background runs
    - selection overlay
    - monochrome glyphs
    - color glyphs
    - underline overlay
    - cursor overlay
    - IME preview overlay

### Windows backend 生命周期

1. `attach()`
   - 解析 `HWND`
   - `mark_render_target_dirty()`
2. `update_surface_rect()`
   - rect 变化时重新标脏
3. `update_frame()`
   - retained frame 变化时重新标脏
4. `present()`
   - `ensure_hwnd_render_target()`
   - 执行 draw stages
   - 更新 cache 统计与 `last_presented_frame_token`
5. `detach()`
   - 清 retained frame
   - 清 device resources
   - 清 `d2d_factory` / `hwnd` / rect / generation / token

## 7. 关键错误处理策略

### attach / notifier 错误

- `NativeTerminalSurface::attach()` 如果 backend attach 失败，会记录 warning
- `set_rendering_notifier()` 不可用时也只记录 warning
- 当前策略是保留 detached/no-op backend，而不是 panic

### presenter 错误

路径：`src/app/bootstrap.rs`

- `presenter.present(...)` 返回 `Err` 时：
  - 记录 `tracing::error!`
  - 回退 cell width / height 到默认值
  - 调用 `clear_workspace_native_terminal_frame(window)` 清理 UI frame token 和 retained frame

### backend lifecycle 错误面

虽然当前 `windows.rs` 还是 retained-state / draw bookkeeping 实现，但已经明确了错误恢复边界：

- `HWND` 不存在：清理 device resources，不继续构建 target
- rect 宽高无效：清理 device resources，不继续 present
- rect / frame / attach 状态变化：统一通过 `render_target_dirty` 驱动 target recreate

## 8. 潜在边缘情况（Edge Cases）

### 8.1 Tokio channel 阻塞或消息堆积

当前这条渲染链没有新增 channel，但上游 session/runtime 仍可能高频更新 surface snapshot。
建议后续测试：

- 高频输出时 frame token 是否增长过快
- UI 线程是否因频繁 `request_redraw()` 出现可见滞后
- 是否需要在 presenter 层加入 frame coalescing

### 8.2 UI 线程更新时机不正确

风险点：

- `update_frame_state()` 与 `update_terminal_rect()` 顺序不一致时，retained frame rect 可能滞后
- `BeforeRendering` 如果发生在 rect 尚未更新时，会出现 target 尺寸与实际绘制区域不一致

当前缓解：

- `present_workspace_native_terminal_frame()` 先求 rect，再更新 frame
- geometry 同步与 retained frame 更新都通过 `NativeTerminalSurface` 串行完成

### 8.3 数据竞争或共享状态不一致

当前 backend 状态全部在 UI 线程内部使用，没有新增跨线程共享容器。
主要风险不在数据 race，而在“retained frame / backend cache / frame token”逻辑不一致，例如：

- background 改了但 token 没变
- mono/color glyph cache 命中逻辑错误
- rect 改了但 `render_target_dirty` 未置位

当前缓解：

- `hash_shaped_frame()` 已纳入 `bg_rgba`
- attach / rect update / frame update 都会 `mark_render_target_dirty()`

### 8.4 资源释放时序问题

风险点：

- teardown 发生后仍收到迟到的 redraw
- window 关闭后仍持有旧 `HWND`
- `detach()` 不清 cache，导致 stale resource 状态残留

当前缓解：

- `RenderingState::RenderingTeardown` 调用 `teardown_native_surface()`
- `detach()` 会清空 frame、device resources、factory、hwnd、generation、token

### 8.5 异步任务取消或界面关闭后的悬挂回调

本轮没有新增异步渲染任务，但上游 session 仍可能在 UI 关闭后继续有状态变化。
需要关注：

- `WORKSPACE_NATIVE_TERMINAL_SURFACE` 中的 surface 是否在窗口销毁后仍可被错误访问
- `request_redraw()` 时 `window.upgrade()` 失败的分支是否足够健壮

### 8.6 Slint model 更新与实际数据源不同步

风险点：

- `workspace-session-native-frame-token` 已更新，但 retained frame 未更新
- cursor/cell size 的 Slint property 与 actual frame payload 不一致
- selection active 状态与 presenter 生成的 selection overlay 不一致

当前缓解：

- `present_workspace_native_terminal_frame()` 同步写 token、cell size、retained frame
- cursor 以 native frame 为优先来源，否则才回落到 runtime surface cursor

## 9. 后续适合编写的测试建议

### 单元测试

优先补：

1. `run_segmentation` 背景色切分
   - background 改变时是否切 run
2. `WgpuTerminalRenderer::prepare()`
   - mono glyph 首帧 upload / 第二帧 cache reuse
   - color glyph 首帧 upload / 第二帧 cache reuse
   - background-only style 变化是否导致 fingerprint 变化
3. `WindowsNativeSurfaceState`
   - `render_target_dirty` 在 attach / rect / frame 变化时是否正确翻转
   - `clear_device_resources()` 是否清空 mono/color caches 与 draw counters

### 集成测试

建议新增：

1. presenter -> native_surface -> backend handoff
   - retained frame 被正确更新到 backend
2. selection / underline / cursor / IME overlay
   - overlay draw order 与 present path 顺序一致
3. build verification
   - `./build-win-x64-software.sh` 产物路径和 archive 名称稳定

### UI 交互测试

建议后续在 Windows 真机或 CI runner 上增加：

1. 打开软件后终端普通文本是否可见
2. ANSI background / selection / cursor 是否同时可见
3. emoji / color glyph 是否显示
4. resize 窗口后 terminal rect 与 target 生命周期是否正确
5. 关闭窗口时是否存在 teardown 后异常日志或 crash

### 运行态人工验证建议

因为当前环境不是 Windows 图形环境，建议后续最少做一次人工 smoke：

- 启动包：`dist/mica-term-x86_64-pc-windows-gnu-release-software.zip`
- 验证：
  - 普通 shell 文本
  - ANSI 背景色
  - selection
  - underline
  - cursor blinking / shape
  - emoji / color glyph
  - resize 后继续可见

## 10. 交接结论

本轮已经把 Windows native terminal surface 的关键合同补齐到“retained frame -> Slint bridge -> Windows backend lifecycle + draw stages”的层面，具体包括：

- background runs
- mono/color glyph upload payload
- retained native frame aggregation
- platform backend lifecycle state
- background / selection / mono glyph / color glyph / underline / cursor / IME draw order

当前最重要的交接事实是：

- 代码层面的 retained contract、编译验证、clippy 验证、Linux-host Windows 打包验证都已经完成
- 仍然建议在真实 Windows 运行环境中做一次最终视觉 smoke，确认 draw stage bookkeeping 与实际可见像素一致

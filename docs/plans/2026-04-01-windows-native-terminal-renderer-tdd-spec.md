# Windows Native Terminal Renderer TDD Spec

日期: 2026-04-01
范围: `2026-04-01-windows-native-terminal-renderer-implementation-plan.md` 全部 8 个 Task 的实现收口说明。

## 核心 Struct

- `AppRuntimeProfile`
  - 携带 `AppBuildFlavor`、`RendererMode`、`TerminalRenderMode`
  - 通过 `prefers_native_terminal_renderer()` 表达 Windows mainline 的 native-first 安装策略
- `NativeTerminalFrame`
  - retained surface bridge 消费的主 payload，包含 `frame_token`、cell metrics 和 `presentable_frame`
- `PresentableNativeFrame`
  - presenter 输出给 bootstrap/native surface 的稳定帧状态
  - 当前字段覆盖 shaped row / glyph 统计、dirty rows、cursor、selection、renderer stats、overlay payload
- `NativeCursorFrameState`
  - row、col、visible、blinking、shape、fg/bg RGBA
- `NativeSelectionFrameState`
  - 逻辑 selection 的 start/end grid 坐标和 overlay 颜色
- `NativeSelectionOverlay`
  - native path 显式 selection overlay contract，当前包含 `rect_count` 与标准化 grid 区间
- `NativeUnderlineOverlay`
  - underline 叠加 contract，当前包含 `visible` 与 `run_count`
- `NativeImePreviewOverlay`
  - IME 预编辑 overlay contract，当前先提供 default-safe payload，后续再接真实 IME 数据源
- `PreparedNativeFrame`
  - `WgpuTerminalRenderer::prepare()` 的输出，包含 glyph cache 统计、prepared counts、underline overlay、frame token
- `PreparedNativeRendererStats`
  - mono/color glyph cache 与 prepared glyph 数量统计
- `PreparedUnderlineOverlay`
  - renderer prepare 阶段生成的 underline overlay 摘要
- `RetainedNativeTerminalSurfaceFrame`
  - `NativeTerminalSurface` 内部持有的 retained frame + rect 组合
- `ShapedTerminalFrame` / `ShapedRow` / `ShapedGlyphRun`
  - 文本整形后进入 native renderer 的中间模型

## Trait 与接口契约

- `TerminalPresenter`
  - `present(&TerminalSurfaceState, TerminalPresentationOptions) -> Result<PresentedTerminalFrame>`
  - `set_raster_scale()` 继续保留 bitmap path 的 hidpi 支持
- `WindowsNativePresenter`
  - 负责 `TerminalModelFrame -> shape -> prepare -> PresentableNativeFrame` 的同步装配
- `FontSystem`
  - 已扩展 `discover_fallback_faces()`、`shape_text_runs()`、`rasterize_color_glyph()`
- `TextShaper`
  - 通过 `TextShapingRequest` 发起 shaping，保留 fallback face / OpenType feature / color glyph 信息
- `NativeTerminalSurface`
  - `update_terminal_rect()`、`update_frame_state()`、`clear_frame()` 组成 retained surface bridge 的公开接口
- `build_native_terminal_presenter()`
  - native presenter 构造入口；失败时由 bootstrap 回落到 bitmap presenter
## Slint callbacks / global state / bindings

- 当前阶段没有新增 Slint `global` singleton；状态主要通过 `AppWindow` property 和 bootstrap 内 thread-local 持有者流动
- 关键 property / binding
  - `ui/shell/terminal-session-host.slint`: `session-render-mode`, `session-surface-image`, `session-native-frame-token`
  - `ui/shell/workspace-pane.slint`: `workspace-session-render-mode`, `workspace-session-native-frame-token`
  - `ui/app-window.slint`: 同名 `in-out property` 作为最终宿主状态
- bootstrap 内关键持有者
  - `WORKSPACE_TERMINAL_PRESENTER`
  - `WORKSPACE_NATIVE_TERMINAL_SURFACE`
- native surface callback
  - `NativeTerminalSurface::attach_or_detach()` 使用 `window().set_rendering_notifier(...)`
  - `RenderingState::BeforeRendering` 时调用 retained-frame draw hook
  - geometry 或 frame state 变化时调用 `request_redraw()`
- native path 激活后，bootstrap 仍同步更新 `workspace_session_cursor_*`、cell size、default fg/bg 等宿主属性，避免 UI 逻辑与 native frame 状态脱节

## Tokio task / channel / actor 交互关系

- `AppAsyncRuntime` 提供共享 Tokio runtime
- `SessionManager::new_with_launcher(runtime.handle(), launcher)` 负责 session actor 生命周期
- `SshSessionRuntime` 通过 `mpsc::UnboundedSender<SessionRuntimeEvent>` 向 session manager 推送 `Connected`、`SurfaceChanged`、`SurfaceDirty`、`Error` 等事件
- UI 线程上的 `session_projection_timer` 周期性读取最新 surface/state，并在 Slint 主线程内驱动 presenter
- 当前 native renderer 链路本身仍是同步 prepare + retained present，尚未引入独立 GPU worker task 或额外 channel
- 后续若把 shaping / prepare 移到后台线程，必须通过 `slint::invoke_from_event_loop` 切回 UI 线程更新 `NativeTerminalSurface` 或 Slint properties
## 状态流转说明

1. `SshSessionRuntime` 产生 `TerminalSurfaceState` 并发送 `SessionRuntimeEvent`
2. `SessionManager` 聚合 surface / dirty / connection state
3. bootstrap 的 workspace session projection 读取当前活动 surface 与 UI selection 状态，构造 `TerminalPresentationOptions`
4. `build_workspace_terminal_presenter()` 基于 `AppRuntimeProfile::prefers_native_terminal_renderer()` 在 Windows mainline 上优先选择 `WindowsNativePresenter`；native 构造失败时回落 bitmap
5. `WindowsNativePresenter` 将 `TerminalSurfaceState` 转为 `TerminalModelFrame`，调用 shaper 和 `WgpuTerminalRenderer::prepare()`
6. `prepare()` 统计 mono/color glyph cache、underline overlay 和 frame token
7. presenter 装配 `PresentableNativeFrame`，将 cursor / selection / underline / IME overlay contract 全部显式化
8. bootstrap 将 `NativeTerminalFrame` 写入 `NativeTerminalSurface`，同时把必要 cursor/cell metrics 投影回 `AppWindow`
9. `NativeTerminalSurface` 在 `BeforeRendering` 阶段消费 retained frame 并触发 draw hook

## 关键错误处理策略

- native presenter 不可用或构造失败时，bootstrap 记录错误并回落到 `BitmapAtlasPresenter`
- 渲染阶段 `present()` 返回错误时，bootstrap 清空 native frame token 并恢复 bitmap-safe UI 状态
- color glyph 光栅化返回 `None` 时，renderer 回退到 monochrome atlas 路径，不直接中断整帧准备
- selection 输入无效时，`active_workspace_terminal_selection()` 返回 `None`，避免产生越界 overlay 数据
- IME overlay 当前默认 `NativeImePreviewOverlay::default()`，保证没有真实 IME 数据源时不会制造悬挂状态
- `NativeTerminalSurface` 仅在 rect/frame 发生变化时 `request_redraw()`，减少无意义重绘

## Edge Cases

- Tokio channel 阻塞或消息堆积
  - 当前是 `mpsc::UnboundedSender`，主要风险不是阻塞而是堆积；需要继续依赖 session manager 的 backlog 压缩与 UI 投影节流，后续可考虑 bounded channel 或 surface dirty coalescing 强化
- UI 线程更新时机不正确
  - retained frame 更新必须发生在 Slint UI 线程；未来若后台准备 frame，必须显式 `invoke_from_event_loop`
- 数据竞争或共享状态不一致
  - `WORKSPACE_TERMINAL_PRESENTER` / `WORKSPACE_NATIVE_TERMINAL_SURFACE` 目前按 UI 线程访问设计；未来跨线程共享时要避免直接持有可变 renderer 状态
- 资源释放时序问题
  - `RenderingState::RenderingTeardown` 与 `clear_frame()` 必须保持幂等，避免 surface teardown 后残留 frame token 或 GPU 资源引用
- 异步任务取消或界面关闭后的悬挂回调
  - `NativeTerminalSurface` 通过 `Weak<AppWindow>` 请求重绘，窗口失效时应直接放弃 redraw
- Slint model 更新与实际数据源不同步
  - `workspace-session-render-mode`、`workspace-session-native-frame-token`、cursor 相关 property 必须与当前 retained frame 对齐，不能只更新其中一部分
- wrapped line / multi-row selection
  - 当前 `NativeSelectionOverlay.rect_count` 只表达按 row 切分后的矩形数量，后续真实 GPU draw 需要补齐每个 rect 的精确像素边界
- IME 预编辑宽度与光标定位
  - 当前 contract 只有 grid 坐标摘要，后续真实 IME integration 需要补 cluster 宽度、候选窗口 anchor 与 composition text 测量

## 后续测试建议

- 单元测试
  - `selection_overlay_rect_count()` 的跨行、反向选择、零宽选择边界
  - `AppRuntimeProfile::prefers_native_terminal_renderer()` 对 development / mainline / software compat 三种 profile 的判定
  - `PreparedUnderlineOverlay` 在无 underline、有 underline、多 run underline 时的输出
- 集成测试
  - native presenter 构造失败时 bootstrap 是否稳定回落 bitmap
  - `SurfaceChanged` / `SurfaceDirty` 高频事件下 retained frame token 与 AppWindow properties 是否仍一致
  - mixed mono/color glyph frame 下 mono atlas 与 color cache 统计是否保持分离
- UI 交互测试
  - workspace 切换 render mode 后 `session-render-mode` 与 `session-native-frame-token` 绑定是否立即生效
  - cursor blink / shape / selection overlay 在 native path 下是否与 surface 状态一致
  - IME 打开、取消、窗口关闭、会话断开时 overlay 是否清理干净

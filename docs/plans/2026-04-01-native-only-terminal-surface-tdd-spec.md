# Native-Only Terminal Surface TDD Spec

> Superseded on 2026-04-01. This handoff note is retained only for historical traceability.
> It should not be used as evidence that the Windows native terminal surface is finished; use `mustdo.md`, `docs/plans/2026-04-01-windows-native-terminal-surface-recovery-design.md`, `docs/plans/2026-04-01-windows-native-terminal-surface-recovery-implementation-plan.md` until a new recovery TDD doc is produced after real Windows validation.

日期: 2026-04-01
范围: `2026-04-01-native-only-terminal-surface-implementation-plan.md` 全部 12 个 Task 的实现收口说明。

## 核心 Struct

- `AppRuntimeProfile`
  - 统一承载 `AppBuildFlavor`、`RendererMode`、`TerminalRenderMode`。
  - 当前 shipping contract 只保留 `TerminalRenderMode::Native`，`packaged()` 会把 packaged profile 解析成 native-only terminal contract。
- `NativeTerminalFrame`
  - bootstrap 写入 retained native surface 的主 payload。
  - 包含 `frame_token`、cell metrics 和 `PresentableNativeFrame`。
- `PresentableNativeFrame`
  - presenter 输出给平台 surface backend 的稳定 display-list frame。
  - 聚合 glyph draws、cursor、selection、underline、semantic overlays、IME preview 和 renderer stats。
- `NativeCursorFrameState` / `NativeCursorOverlay`
  - 分别表达 terminal grid 语义上的 cursor 状态和 native draw 所需的 cursor overlay 参数。
- `NativeSelectionFrameState` / `NativeSelectionOverlay` / `NativeSelectionRect`
  - selection 保持 grid 语义，不回写终端 buffer。
  - `rect_count` + `rects` 作为平台 backend 的 retained selection overlay contract。
- `NativeUnderlineOverlay` / `NativeUnderlineRun`
  - 表达 underline draw contract，和 glyph draw 解耦。
- `NativeImePreviewOverlay`
  - 为后续 IME preedit integration 预留 retained overlay 入口，当前默认 default-safe。
- `NativeRendererFrameStats`
  - 记录 mono/color glyph cache 和 prepared glyph 统计，保证 native draw 资源状态可观测。
- `PlatformNativeSurfaceBackend`
  - `NativeTerminalSurface` 持有的跨平台 surface backend trait object。
  - 由 `create_platform_native_surface_backend()` 在 Windows / Wayland / X11 / detached fallback 之间选择。
- `NativeTerminalSurfaceRect` / `RetainedNativeTerminalSurfaceFrame`
  - 保证 rect 和 retained frame 在同一个 surface update contract 中传播。
- `WindowsNativeSurfaceState` / `WaylandNativeSurfaceState` / `X11NativeSurfaceState`
  - 平台后端保留 host surface rect、retained frame 和 `last_presented_frame_token`。
  - Windows 额外保留 `hwnd`，Linux 两个 backend 当前保留结构性 scaffold。
- `SemanticOutputOverlay` / `SemanticOverlayRowRange`
  - 普通 shell 输出的附加 display-list overlay，支持 `Json`、`Xml`、`Log` 三类块。
- `SemanticInputOverlay`
  - 普通 shell 输入行的附加 overlay，支持 `Prompt`、`Command`、`Argument`、`Option`、`Operator`。
- `SshSessionRuntime`
  - SSH terminal runtime actor，持有 `command_tx: mpsc::UnboundedSender<RuntimeCommand>`。
  - 负责把 runtime terminal state 转成 `SessionRuntimeEvent` 推给 `SessionManager`。
- `SurfaceDirtyNotifier`
  - runtime 侧 surface dirty 节流器，避免每次输出都直接向 UI 投递无界 dirty 风暴。

## Trait 与接口契约

- `TerminalPresenter`
  - `present(&TerminalSurfaceState, TerminalPresentationOptions) -> Result<PresentedTerminalFrame>` 是唯一的 terminal projection seam。
  - 当前实现只返回 `PresentedTerminalFrame::Native`，bitmap presenter contract 已被删除。
  - `default_cell_size()` 作为 UI 初始化和 renderer 失败回退时的稳定 cell metric 来源。
- `WindowsNativePresenter`
  - 当前 native-only presenter 实现。
  - 负责 `TerminalModelFrame -> shaping -> renderer.prepare() -> PresentableNativeFrame` 的同步装配。
  - 通过 `detect_output_block_overlays()` 和 `detect_input_line_overlays()` 叠加语义层，但不回写 ANSI 真值。
- `PlatformNativeSurfaceBackend`
  - `attach(&AppWindow)` 绑定宿主窗口或原生 surface。
  - `update_surface_rect(NativeTerminalSurfaceRect)` 同步布局矩形。
  - `update_frame(Option<RetainedNativeTerminalSurfaceFrame>)` 更新 retained frame。
  - `present()` 在 `RenderingState::BeforeRendering` 被调用。
  - `detach()` 在 `RenderingState::RenderingTeardown` 做幂等清理。
- `SessionRuntimeControl`
  - 约束 runtime actor 对外暴露的 `send_text_input`、`send_key_input`、`send_mouse_input`、`send_paste`、`resize`、`terminal_surface`、`scroll_viewport_lines`、`update_theme_mode`。
  - UI/SessionManager 只依赖该 trait，不直接持有 SSH 实现细节。
- `SessionRuntimeLauncher`
  - `launch()` 负责为 session 建立 runtime actor，并把 `event_tx` 注入 runtime。
  - `probe()` 负责 session 可用性探测，不污染主 UI 状态。
- `FontSystem` / `TextShaper`
  - `DirectWriteFontSystem` 和 `TerminalTextShaper` 组成 native text layout contract。
  - `FontRequest::default()` 当前固定到 `Fusion JetBrains Maple Mono`。

## Slint callbacks / global state / bindings

- 当前没有新增 Slint `global` singleton；terminal surface 状态通过 `AppWindow` properties、thread-local presenter/surface 和 bootstrap 投影逻辑流动。
- `ui/shell/terminal-session-host.slint`
  - `in property <int> session-native-frame-token: 0;`
  - `callback key-input(string, bool, bool, bool);`
  - `callback surface-resize-requested(int, int);`
  - `callback copy-selection-requested(int, int, int, int);`
  - `callback paste-requested();`
  - `callback scroll-requested(int, int, int, bool, bool, bool);`
  - `callback jump-to-latest-requested();`
  - `callback mouse-input(string, string, int, int, bool, bool, bool);`
- `ui/shell/workspace-pane.slint`
  - `in property <int> workspace-session-native-frame-token: 0;`
  - 负责把 terminal host callbacks 和 native surface layout 属性转发到 workspace 壳层。
- `ui/app-window.slint`
  - `in-out property <int> workspace-session-native-frame-token: 0;`
  - 作为 bootstrap 向 Slint 主窗口回写 retained frame token、cursor、cell metrics、visible lines、viewport 状态的最终宿主。
- bootstrap 内关键持有者
  - `WORKSPACE_TERMINAL_PRESENTER`
  - `WORKSPACE_NATIVE_TERMINAL_SURFACE`
- `NativeTerminalSurface::attach(window)`
  - 通过 `window().set_rendering_notifier(...)` 接入 Slint 生命周期。
  - `BeforeRendering` 触发 `draw_retained_frame()`，`RenderingTeardown` 触发 `teardown_native_surface()`。
- `session-native-frame-token` / `workspace-session-native-frame-token`
  - 不承载像素或图像资源。
  - 只表达 retained native frame 是否更新的 invalidation contract，供 Slint 布局和 native surface host 保持同步。
## Tokio task / channel / actor 交互关系

- `SshSessionRuntime`
  - 通过 `mpsc::UnboundedSender<SessionRuntimeEvent>` 向 `SessionManager` 推送 `Connected`、`SurfaceChanged`、`SurfaceDirty`、`Error`、`CurrentDirectoryChanged` 等事件。
  - runtime 内 `SurfaceDirtyNotifier` 以 40ms 粒度合并输出脏标记，避免每次输出都触发完整 surface snapshot 推送。
- `SessionManager::spawn_session_attempt()`
  - 创建 `let (event_tx, mut event_rx) = mpsc::unbounded_channel();`。
  - 在 Tokio task 中消费 runtime 事件，并在 `SurfaceChanged` / `SurfaceDirty` 路径上做 backlog coalescing。
- backlog 压缩策略
  - `coalesce_surface_backlog()` 会丢弃前序连续 `SurfaceChanged`，只保留最新 surface snapshot。
  - `coalesce_surface_dirty_backlog()` 会吞并连续 `SurfaceDirty`，避免 UI 投影队列堆积重复 dirty 信号。
- UI 投影
  - bootstrap 内 `session_projection_timer` 以 50ms 周期在 Slint 主线程读取 `SessionManager` 当前活动 session 的 surface/state。
  - 投影后同步 `workspace-session-native-frame-token`、cursor、cell metrics、visible lines、viewport、selection 相关属性。
- UI 线程切换
  - 需要切回 Slint 主线程的更新必须通过 `slint::invoke_from_event_loop(...)` 执行。
  - 当前 terminal presenter/native surface 仍在 UI 投影阶段同步执行；后续若把 shaping 或 prepare 移到后台线程，仍必须在 event loop 中回写 UI 和 retained frame。

## 状态流转说明

1. `wezterm-term` 驱动的 `TerminalSession` 在 runtime 内更新 terminal grid、cursor、selection 与 scrollback 状态。
2. `SshSessionRuntime` 把变化转换为 `SessionRuntimeEvent::SurfaceChanged` 或 `SessionRuntimeEvent::SurfaceDirty`。
3. `SessionManager` 在 actor 聚合层更新 registry，并通过 backlog coalescing 压缩高频 surface 事件。
4. bootstrap 的 `session_projection_timer` 从当前活动 session 拉取 `TerminalSurfaceState`，并构造 `TerminalPresentationOptions`。
5. `WindowsNativePresenter::present()` 把 `TerminalSurfaceState` 转为 `TerminalModelFrame`，随后完成 shaping、glyph prepare、selection overlay 装配和 semantic overlay 检测。
6. `detect_output_block_overlays()` 为普通 shell 输出补充 JSON / XML / log block overlay，`detect_input_line_overlays()` 在安全条件成立时补充 prompt/command/argument/option/operator overlay。
7. bootstrap 通过 `present_workspace_native_terminal_frame()` 把 `NativeTerminalFrame` 写入 `NativeTerminalSurface`，同时回写 `workspace_session_native_frame_token`、cursor 和 cell size。
8. `NativeTerminalSurface` 在 `BeforeRendering` 生命周期把 retained frame 交给 `PlatformNativeSurfaceBackend::present()`，在 `RenderingTeardown` 时清空 retained frame 并调用 `detach()`。

## 关键错误处理策略

- native surface backend attach 失败
  - `NativeTerminalSurface::attach()` 会记录 warning，并保留 detached fallback backend，而不是在 bootstrap 阶段直接崩溃。
- presenter/render 失败
  - `sync_workspace_session_state_with_manager()` 在 `present()` 返回错误时会清空 native frame、恢复默认 cell metrics，并记录 `app.terminal` 错误日志。
- backend 不可用或平台不匹配
  - `create_platform_native_surface_backend()` 会回退到 `DetachedPlatformSurfaceBackend`，保持 native-only contract 不崩但不承诺真实绘制。
- selection 越界或列数异常
  - `selection_overlay_rects()` 会对列范围做 `saturating_sub()` / `min()` 截断，避免 overlay rect 越界。
- semantic input 误判
  - `input_highlighting_is_safe()` 明确要求 `alternate_screen_active == false`、`mouse_grabbed == false`、`viewport_at_bottom == true`，否则直接禁用输入高亮。
- surface teardown
  - `clear_retained_frame()` 与 `teardown_native_surface()` 都是幂等清理路径，防止重复 teardown 残留 frame token。
- stale redraw
  - `NativeTerminalSurface` 只持有 `Weak<AppWindow>`，窗口已销毁时 `request_redraw()` 会直接放弃，不保留悬挂回调。

## Edge Cases

- Tokio channel 阻塞或消息堆积
  - 当前 `mpsc::UnboundedSender` 没有天然背压，风险不是阻塞而是 surface backlog 持续堆积。
  - 现有缓解手段是 runtime 侧 `SurfaceDirtyNotifier` 节流，以及 manager 侧 `coalesce_surface_backlog()` / `coalesce_surface_dirty_backlog()` 压缩事件。
- UI 线程更新时机不正确
  - native frame token、cursor、cell metrics、surface rect 必须在同一轮 UI 投影内同步更新；跨线程回写必须经 `slint::invoke_from_event_loop()`。
- 数据竞争或共享状态不一致
  - `WORKSPACE_TERMINAL_PRESENTER` 与 `WORKSPACE_NATIVE_TERMINAL_SURFACE` 当前按 UI 线程使用设计；未来如果后台 prepare，需要避免直接跨线程持有可变 renderer/font state。
- 资源释放时序问题
  - `RenderingTeardown` 之后不能残留 retained frame、`last_drawn_frame_token` 或平台 surface 句柄；`clear_frame()` 和 `detach()` 的顺序必须保持幂等。
- 异步任务取消或界面关闭后的悬挂回调
  - 窗口关闭后 `Weak<AppWindow>` 可能无法 upgrade；所有 redraw 请求都必须容忍这个状态。
- Slint model 更新与实际数据源不同步
  - `workspace-session-visible-lines`、viewport 状态、selection 属性与 `workspace-session-native-frame-token` 必须来自同一次 projection，不能分批更新。
- semantic overlay 与实际 terminal 数据不同步
  - output/input overlays 都是 display-list 叠加层，不允许回写 ANSI cell；否则会污染复制、selection 和 TUI 语义。
- alternate screen / TUI guard 失效
  - 一旦 `alternate_screen_active`、`mouse_grabbed` 或 `!viewport_at_bottom` 条件判断错误，input-line overlay 就可能污染全屏 TUI 或历史滚动内容。
- 平台 backend 选择边界
  - Linux 上当前优先 Wayland，再回退 X11；环境变量同时存在时必须明确遵守这一顺序，避免 surface backend 和宿主窗口系统不一致。
- retained frame 与 rect 更新时序
  - 先更新 rect 再更新 frame，或反之不一致，都可能造成 selection/cursor 像素位置漂移；当前 contract 通过 `RetainedNativeTerminalSurfaceFrame { frame, rect }` 一起传播来缓解。

## 后续测试建议

- 单元测试
  - `detect_output_block_overlays()` 的 JSON / XML / log block 识别边界，尤其是误把单行数组或日志前缀识别成 JSON 的场景。
  - `detect_input_line_overlays()` 对 prompt、operator、option 分类，以及 `alternate_screen_active` / `mouse_grabbed` / `viewport_at_bottom` 三个 guard 条件的覆盖。
  - `selection_overlay_rects()` 在反向选择、跨行选择、零列数和超界列上的截断行为。
  - `create_platform_native_surface_backend()` 在 Linux Wayland/X11 环境变量组合下的 backend 选择策略。
- 集成测试
  - `SurfaceChanged` / `SurfaceDirty` 高频混合输入下，session manager 的 backlog 压缩后是否仍保持最新 surface seqno。
  - `sync_workspace_session_state_with_manager()` 在 renderer 返回错误时是否稳定清空 native frame 并恢复默认 cell metrics。
  - packaged runtime profile 与 Windows build wrapper 是否继续只输出 native-only terminal metadata。
- UI 交互测试
  - workspace 切换、标签切换、窗口 resize 后，`workspace-session-native-frame-token`、cursor 和 native surface rect 是否保持一致。
  - 滚动离开底部、进入 alternate screen、TUI 抢占鼠标后，semantic input overlay 是否立即禁用。
  - 关闭窗口、关闭 session、surface teardown 时是否还会触发悬挂 redraw 或保留旧 frame token。
  - selection、复制、粘贴、jump-to-latest 与 follow-paused 状态在 native-only contract 下是否仍和 terminal grid 语义一致。

## 结论

本轮实现已经把 terminal 渲染主链切到 native-only：runtime/profile、Slint 壳层绑定、presenter/native-surface retained frame contract、Windows/Wayland/X11 backend scaffold、默认字体与 semantic overlay 都已对齐。后续扩展应继续围绕 retained native frame contract 演进，而不是重新引入 bitmap/image 终端通路。

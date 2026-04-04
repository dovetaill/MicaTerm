# Windows Terminal Text Rendering TDD 交接文档

## 范围说明

- 本文档覆盖 `2026-04-04-windows-terminal-text-rendering` 实施计划的 Task 1 至 Task 7 落地结果。
- 本轮目标是把 Windows 11 首发路径收敛到 `winit-skia` + native terminal renderer，并以 `directwrite-d2d` 作为主文本渲染路径。
- 当前自动化验证已覆盖源码级、脚本级与打包级合同；真实 Windows 视觉验收仍需在后续手工补录。

## 核心 struct

- `AppRuntimeProfile`
  - 定义 packaged/mainline/software-compat 的 renderer、terminal render mode、native present path。
  - 决定 `winit-skia`、`native`、`rendering-notifier` 这些首发口径如何进入运行时。
- `NativeTerminalSurface`
  - 负责 retained frame、damage tracker、present driver 与 backend 的编排。
  - 对外暴露 `diagnostics_snapshot()`，作为 Windows native surface 可观测性的统一入口。
- `WindowsNativeSurfaceBackend`
  - Windows 平台 backend，实现 retained present、D2D bitmap draw、DirectWrite text draw。
  - 在 `diagnostics_snapshot()` 中发布当前 text path、AA/alpha mode、font chain、DPI/scale factor、glyph bounds。
- `WindowsNativeSurfaceState`
  - 保存 `host_hwnd`、render target、brush bitmap cache、retained frame、draw counters、最近 frame token。
  - 负责把 retained frame 转成 D2D/DWrite 调用。
- `WindowsDirectWriteTextRendererState`
  - 保存 DirectWrite factory、font collection、resolved font face cache 与当前 active path。
  - 当前主路径为 `directwrite-d2d`，失败时回退到 `bitmap-mask-compat`。
- `PreparedNativeFrame`
  - 平台 backend 消费的中间帧，包含 background runs、monochrome/color glyph draws、underline/cursor/selection overlay 数据。
- `PreparedMonochromeGlyphDraw`
  - 承载 glyph id、font family、face key、visible bounds、dest origin、logical cell ownership。
  - Windows native path 基于这些字段生成 glyph bounds trace 与 DirectWrite glyph run。
- `NativeTerminalSurfaceDiagnostics`
  - native surface 顶层诊断快照。
  - 新增 `windows_text: Option<NativeTerminalSurfaceWindowsTextDiagnostics>` 用于承载 Windows 文本专项诊断。
- `NativeTerminalSurfaceWindowsTextDiagnostics`
  - 聚合 `text_antialias_mode`、`render_target_alpha_mode`、`font_chain`、`baseline_px`、`pixel_alignment`、`dpi_x`、`dpi_y`、`scale_factor_percent`、`glyph_bounds`。
- `NativeTerminalSurfaceGlyphBoundsTrace`
  - 记录 glyph screen rect、visible bounds、atlas slot 与 cell span，用于回归定位 overhang、clip 与 baseline 对齐问题。

## trait 与接口契约

- `TerminalPresenter`
  - `present(...) -> Result<PresentedTerminalFrame>` 是 terminal 到 UI/native surface 的唯一帧出口。
  - `set_raster_scale(...)` 是 DPI 传播入口，要求 bitmap/native presenter 都遵守同一 scale 契约。
- `PlatformNativeSurfaceBackend`
  - 关键方法：`attach`、`update_surface_rect`、`update_frame`、`present`、`diagnostics_snapshot`、`detach`。
  - `diagnostics_snapshot()` 必须是无副作用读取，用于 UI/trace 在任意 present 后抓取状态。
- `NativeSurfacePresentDriver`
  - 负责选择 event-loop 或 rendering-notifier 的 present 调度方式。
  - 不拥有 terminal 数据，只负责把 draw 回调调度回宿主窗口节奏。
- `FontSystem`
  - 负责字体加载、fallback、glyph rasterization。
  - `PreparedMonochromeGlyphDraw.face_key`、`font_family_name`、visible bounds 都依赖这个接口链提供稳定合同。

## Slint callbacks / global state / bindings

- 关键 Slint state
  - `workspace-session-render-mode`
  - `workspace-session-device-scale-factor`
  - `workspace-session-native-frame-token`
  - `layout-workspace-session-native-surface-x/y/width/height`
- `ui/app-window.slint` 与 `ui/shell/workspace-pane.slint`
  - 负责把 workspace native surface 几何和 scale factor 继续传给 `TerminalSessionHost`。
- `ui/shell/terminal-session-host.slint`
  - bitmap 模式下仍显示 Slint image/cursor overlay。
  - native 模式下 cursor overlay 不再由 Slint 重复绘制，避免 ownership 冲突。
- 当前没有新增 release UI diagnostics overlay。
  - Task 6 的诊断能力通过 `bootstrap.rs` trace hook 与 `windows_frame.rs` helper 暴露。
  - 这意味着调试信息不会污染正式 UI 结构，但仍可被自动化脚本与日志消费。

## Tokio task / channel / actor 交互关系

- `SessionManager` / SSH runtime 仍是终端数据生产者，Tokio 侧持续生成 `TerminalSurfaceState`。
- UI 线程在 `bootstrap.rs` 中把活跃 session surface 交给 `TerminalPresenter`，再决定走 `PresentedTerminalFrame::Bitmap` 或 `PresentedTerminalFrame::Native`。
- native present 不新增独立 Tokio channel。
  - 当前 retained present 依赖 UI 线程内的 present driver 与 host redraw hook。
  - Task 6/7 新增的 diagnostics trace 也发生在 UI/native surface 这条同步链上。
- 交接重点
  - 未来若把 diagnostics 推到异步面板或远程日志，必须重新评估 channel 背压、生命周期同步与 UI thread hop。

## 状态流转说明

1. SSH/Tokio runtime 产出终端 surface 状态。
2. `TerminalPresenter` 依据 runtime profile、renderer mode、scale factor 生成 `PresentedTerminalFrame`。
3. `NativeTerminalSurface` 保存 retained frame，结合 damage tracker 决定本次 present 范围。
4. Windows backend 在 present 时按顺序执行 background、selection、DirectWrite text、bitmap fallback glyph、color glyph、underline、cursor、IME overlay。
5. present 结束后刷新 `NativeTerminalSurfaceDiagnostics`。
6. `windows_frame.rs` helper 从 diagnostics 中读取 text path、AA/alpha、font chain、DPI、glyph bounds。
7. `bootstrap.rs` 的 `trace_workspace_native_terminal_diagnostics(...)` 输出当前 native diagnostics 摘要。

## 关键错误处理策略

- native backend attach 失败
  - 记录 warning，不直接 panic。
  - surface 保持 detached fallback，避免 UI 启动被硬阻塞。
- DirectWrite renderer 初始化失败
  - `WindowsDirectWriteTextRendererState.active_path` 回落到 `bitmap-mask-compat`。
  - diagnostics 中仍可看到当前 path，便于定位是否进入 fallback。
- render target 失效或需要重建
  - 通过 `render_target_generation` 与 `clear_device_resources()` 重建。
  - brush/bitmap generation 跟随 render target generation 失效并重建。
- 打包/交叉编译失败
  - `build-desktop.sh` 与 `build-win-x64.sh` 采用 fail-fast，工具缺失或 target 不支持时直接退出。
  - Task 7 补充了 Windows wrapper 对 text path 与 DPI/font matrix 的显式广告，避免脚本口径漂移。

## 潜在边缘情况（Edge Cases）

- Tokio channel 阻塞或消息堆积
  - 当前 terminal 数据更新频繁，如果未来把 diagnostics 通过异步 channel 推送到 UI，需要限制快照频率与队列长度。
- UI 线程更新时机不正确
  - retained present 与 diagnostics trace 都依赖 UI/native surface 时序；若在 redraw 前读取快照，可能拿到上一帧数据。
- 数据竞争或共享状态不一致
  - `NativeTerminalSurface` 通过 `Rc<RefCell<...>>` 管理 UI 线程内共享状态；未来若跨线程读取 diagnostics，必须引入明确同步模型。
- 资源释放时序问题
  - `detach()`、`clear_device_resources()`、`release_bound_dc()` 的顺序错误会导致 stale DC、stale render target 或 font/bitmap cache 残留。
- 异步任务取消或界面关闭后的悬挂回调
  - present driver 或 rendering notifier 触发回调时，窗口可能已经 teardown；必须继续依赖 weak state/`surface_alive` 守卫。
- Slint model 更新与实际数据源不同步
  - bitmap/native render mode 切换时，Slint cursor overlay 与 native overlay ownership 必须保持互斥，避免双绘。
- glyph visible bounds 与 logical cell span 不一致
  - overhang glyph 允许超出 cell span，但 hit-test、selection、cursor 仍必须以逻辑 cell 归属为准。
- DPI/scale factor 漂移
  - diagnostics 的 `scale_factor_percent` 与 Slint 的 `workspace-session-device-scale-factor` 必须能互相对照，否则会出现“编译通过但视觉漂移”的隐性回归。
- Windows text path 广告与实际 fallback 不一致
  - `build-win-x64.sh`、runtime profile、diagnostics trace 三者口径必须同步，否则会导致打包脚本宣称 native path，但运行时静默退回 bitmap mask。

## 手工验证备注

- 当前自动化已验证
  - `build-win-x64.sh` 打包成功
  - Windows wrapper/source smoke 对 `directwrite-d2d`、`rendering-notifier`、DPI/font matrix 广告口径保持一致
  - diagnostics hook 能暴露 DPI/scale factor 与 glyph bounds 信息
- 当前仍需在真实 Windows 11 环境补录的人工结果
  - dark background English text
  - Chinese text
  - mixed CJK/Latin lines
  - Nerd Font / powerline glyphs
  - emoji
  - DPI：`100%`、`125%`、`150%`
  - font px：`12px`、`13px`、`14px`、`15px`

## 后续适合编写的测试建议

- 单元测试
  - `WindowsNativeSurfaceState::windows_text_diagnostics()` 的字段构造测试，覆盖空 frame、单 glyph、多 glyph、fallback path。
  - `windows_frame.rs` helper 对 `Option` 字段的读取测试，覆盖无 diagnostics 与 partial diagnostics。
- 集成测试
  - 增加 source-level 断言，验证 `build-win-x64.sh` 的 `MICA_TERM_EXPECTED_TEXT_RENDERER_PATH`、fallback path、DPI/font matrix 不被改丢。
  - 为 packaged runtime profile 增加测试，验证 mainline 包装口径与 wrapper export 保持一致。
- UI 交互测试
  - native 模式下 cursor/selection overlay 不被 Slint 重复绘制。
  - bitmap/native 切换时 `workspace-session-native-frame-token` 与 render mode 同步变化。
- 回归测试
  - glyph overhang、CJK fallback、emoji/color glyph 混排时的 diagnostics trace 保持稳定。
  - `GetDpiForWindow` 可用时 diagnostics 与 Slint scale factor 的对应关系保持一致。

# Windows Theme Minimal Repro And Skia Removal TDD Spec

日期: 2026-03-12
来源: `docs/plans/2026-03-12-windows-theme-min-repro-and-skia-removal-design.md`
实现计划: `docs/plans/2026-03-12-windows-theme-min-repro-and-skia-removal-implementation-plan.md`

## 目标

为下一阶段 `test-driven-development` 提供稳定输入，覆盖本轮主线 Skia 实验路径移除、最小 Windows repro binary、新的负向 smoke 契约，以及后续 Windows 真机复现时需要重点观察的边界。

## 核心 Rust 结构与接口

### `src/app/runtime_profile.rs`

- `AppBuildFlavor`
  - `Formal`
- `RendererMode`
  - `Software`
- `AppRuntimeProfile`
  - 字段:
    - `build_flavor: AppBuildFlavor`
    - `renderer_mode: RendererMode`
  - 核心方法:
    - `formal() -> AppRuntimeProfile`
    - `is_experimental(self) -> bool`
    - `requires_backend_lock(self) -> bool`
    - `forced_backend(self) -> Option<&'static str>`
    - `uses_theme_redraw_recovery(self) -> bool`

当前契约已经收敛到单一路径：

- `is_experimental()` 恒为 `false`
- `requires_backend_lock()` 恒为 `false`
- `forced_backend()` 恒为 `None`
- `uses_theme_redraw_recovery()` 恒为 `true`

### `src/main.rs`

- 启动链固定为 formal 路径:
  - `AppRuntimeProfile::formal()`
  - `try_init_global_logging()`
  - `emit_runtime_profile_metadata(profile)`
  - `bootstrap::run_with_profile(profile)`

已移除：

- `select_runtime_profile()`
- `apply_renderer_lock(profile)`
- `unsafe { std::env::set_var("SLINT_BACKEND", ...) }`

### `src/app/bootstrap.rs`

- `runtime_window_title(profile: AppRuntimeProfile) -> String`
  - 当前固定返回 `Mica Term`
- `startup_failure_message(profile: AppRuntimeProfile, err: &str) -> Option<String>`
  - 当前固定返回 `None`

这代表主线不再保留 experimental renderer 标题或失败提示契约。

### `build.rs`

- 继续为正式应用执行:
  - `slint_build::compile("ui/app-window.slint")`
- 新增最小 repro 生成链:
  - `slint_build::compile_with_output_path(...)`
  - 输入: `ui/windows-theme-repro.slint`
  - 输出: `OUT_DIR/windows_theme_repro.rs`

这是本轮最关键的构建接口之一，因为它让最小 repro 可以独立生成，不影响正式应用现有 `slint::include_modules!()` 机制。

### `src/bin/windows_theme_repro.rs`

- 直接 `include!(concat!(env!("OUT_DIR"), "/windows_theme_repro.rs"));`
- 不依赖：
  - `bootstrap::run_with_profile`
  - `window_effects`
  - `AppWindow::new`
- 启动链:
  - `WindowsThemeRepro::new()?`
  - `on_toggle_theme_requested(...)`
  - `window.run()`

## Slint 组件与回调

### `ui/windows-theme-repro.slint`

- `export component WindowsThemeRepro inherits Window`
- 关键 property:
  - `in-out property <bool> dark-mode`
- 关键 callback:
  - `toggle_theme_requested()`
- 关键 UI 面:
  - 顶部状态文本
  - 一个 `Toggle Theme` 按钮
  - 一整块纯色 body
    - dark: `#000000`
    - light: `#ffffff`
- 当前窗口参数:
  - `preferred-width: 1280px`
  - `preferred-height: 1800px`

后续若要继续压缩 repro 变量，优先保持这几个元素不变，只调整窗口 chrome、尺寸或交互方式。

## 已覆盖测试与 smoke

- `tests/runtime_profile.rs`
  - 运行时 profile 已收敛到单一路径
  - 源文件中不应再出现 `SkiaExperimental` / `SkiaSoftware` / `skia_experimental`
- `tests/bootstrap_profile_smoke.rs`
  - bootstrap 只应接受 formal/software 路径
- `tests/logging_runtime.rs`
  - runtime metadata 只记录 `Formal` / `Software`
- `tests/top_status_bar_smoke.rs`
  - 标题契约只保留 `Mica Term`
- `tests/panic_logging.rs`
  - formal 路径不再暴露 experimental 启动失败提示
- `tests/window_theme_contract_smoke.sh`
  - 主入口不得回流 `winit-skia-software` / `SLINT_BACKEND` / `windows-skia-experimental`
- `tests/build_win_x64_skia_script_smoke.sh`
  - 旧 Skia wrapper 不得重新出现
- `tests/windows_theme_repro_smoke.rs`
  - 最小 repro 源文件必须存在
  - 最小 repro 不得依赖正式 bootstrap

## 边缘情况与后续 TDD 关注点

- `build.rs` 多输出链
  - 后续若再增加新的独立 `.slint` binary，必须确认不会覆盖主应用的 `SLINT_INCLUDE_GENERATED`
  - 优先继续使用 `compile_with_output_path(...)` 的独立输出方式
- 最小 repro 与正式应用行为差异
  - 如果 Windows 上最小 repro 不复现问题，而正式应用仍复现，说明变量更可能在：
    - 透明背景
    - 无框窗口
    - window effects / Mica
    - 更复杂的组件树或刷新链
- 标准窗口路径
  - 当前 repro 使用标准窗口，故意不引入 `no-frame` / `background: transparent`
  - 后续若要做第二层对照实验，应单独新增分支或 binary，而不是污染当前最小样本
- 日志与崩溃记录
  - 本轮没有为 repro 增加 logging runtime 或 panic hook
  - 如果后续需要为 repro 记录图形复现日志，优先新增极薄记录层，不要直接搬正式应用 bootstrap
- Tokio / channel / data race
  - 本轮没有新增 Tokio runtime、channel、actor mailbox 或跨线程共享状态
  - 当前 repro 的主题切换完全发生在 Slint UI 线程内
  - 后续如果为了录制复现场景引入异步通道，必须优先覆盖：
    - channel 堵塞
    - UI 线程回调顺序
    - 事件循环竞争
    - `invoke_from_event_loop` 调度时机

## 建议的下一轮 TDD 顺序

1. 在 Windows 真机先执行 `cargo run --bin windows_theme_repro`，确认最小样本是否仍复现“超屏区域不整体刷新”。
2. 如果最小样本也复现，下一轮优先补可观察性，而不是先改 workaround。
3. 如果最小样本不复现，下一轮按变量分层做对照实验：
   - 标准窗口 vs 无框窗口
   - 不透明背景 vs 透明背景
   - 无 window effects vs Mica/window-vibrancy
4. 保持正式应用主线继续只走 workaround 路径，不再恢复 Skia experimental 分支。

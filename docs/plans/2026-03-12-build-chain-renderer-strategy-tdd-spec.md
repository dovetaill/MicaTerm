# Build Chain And Renderer Strategy TDD Spec

日期: 2026-03-12
来源: `docs/plans/2026-03-12-build-chain-renderer-strategy-design.md`
实现计划: `docs/plans/2026-03-12-build-chain-renderer-strategy-implementation-plan.md`

## 目标

为下一阶段 `test-driven-development` 提供稳定输入，覆盖本轮 build chain 与 renderer strategy 改动的核心 Rust 接口、脚本契约、日志可观测性与边缘情况。

## 核心 Rust 结构与接口

### `src/app/runtime_profile.rs`

- `AppBuildFlavor`
  - `Formal`
  - `SkiaExperimental`
- `RendererMode`
  - `Software`
  - `SkiaSoftware`
- `AppRuntimeProfile`
  - 字段:
    - `build_flavor: AppBuildFlavor`
    - `renderer_mode: RendererMode`
  - 核心方法:
    - `formal() -> AppRuntimeProfile`
    - `skia_experimental() -> AppRuntimeProfile`
    - `is_experimental(self) -> bool`
    - `requires_backend_lock(self) -> bool`
    - `forced_backend(self) -> Option<&'static str>`

### `src/main.rs`

- `select_runtime_profile() -> AppRuntimeProfile`
  - 按 `windows-skia-experimental` feature 选择 formal / experimental profile
- `apply_renderer_lock(profile: AppRuntimeProfile)`
  - experimental 包内部强制 `SLINT_BACKEND=winit-skia-software`
  - formal 包不锁定 backend
- 启动主链:
  - `try_init_global_logging()`
  - `apply_renderer_lock(profile)`
  - `emit_runtime_profile_metadata(profile)`
  - `bootstrap::run_with_profile(profile)`
  - `startup_failure_message(profile, err)` 仅在 experimental 包失败时输出明确提示

### `src/app/logging/runtime.rs`

- `emit_runtime_profile_metadata(profile: AppRuntimeProfile)`
  - 日志 target: `app.renderer`
  - 关键字段:
    - `build_flavor`
    - `renderer_mode`
    - `forced_backend`

### `src/app/bootstrap.rs`

- `app_title() -> &'static str`
- `runtime_window_title(profile: AppRuntimeProfile) -> String`
  - formal: `Mica Term`
  - experimental: `Mica Term [Skia Experimental]`
- `startup_failure_message(profile: AppRuntimeProfile, err: &str) -> Option<String>`
  - experimental: `Mica Term Skia Experimental failed to initialize winit-skia-software: ...`
  - formal: `None`
- `run_with_profile(profile: AppRuntimeProfile) -> anyhow::Result<()>`
  - 当前仍复用现有 `AppWindow::new()` 与 `bind_top_status_bar()` 主链
  - 当前未把派生标题真正写回 Slint window，因为现有 `AppWindow` 生成接口没有可直接调用的 Rust-side `set_title()` setter

## Build / Script 契约

### Cargo features

- `default = ["slint-renderer-software"]`
- `slint-renderer-software = ["slint/renderer-software"]`
- `slint-renderer-skia = ["slint/renderer-skia"]`
- `windows-skia-experimental = ["slint-renderer-skia"]`

### 脚本入口

- `build-desktop.sh`
  - 通用 per-target 打包入口
- `build-win-x64.sh`
  - formal `x86_64-pc-windows-gnu` wrapper
- `build-win-x64-skia.sh`
  - experimental `x86_64-pc-windows-msvc` pure-Skia wrapper
  - 默认值:
    - `TARGET=x86_64-pc-windows-msvc`
    - `CARGO_NO_DEFAULT_FEATURES=1`
    - `CARGO_FEATURES=windows-skia-experimental`
- `build-release.sh`
  - Debian formal release aggregator
  - formal targets:
    - `x86_64-unknown-linux-gnu`
    - `x86_64-pc-windows-gnu`
  - mode:
    - `fail-fast`
    - `best-effort`

## 已覆盖测试与 smoke

- `tests/runtime_profile.rs`
  - formal / experimental profile 契约
- `tests/bootstrap_profile_smoke.rs`
  - backend lock 决策与 `forced_backend()`
- `tests/logging_runtime.rs`
  - runtime profile metadata 日志输出
- `tests/panic_logging.rs`
  - experimental 启动失败提示文本契约
- `tests/top_status_bar_smoke.rs`
  - `runtime_window_title(profile)` 文本契约
- `tests/build_win_x64_skia_script_smoke.sh`
  - pure-Skia wrapper 默认值
- `tests/build_release_script_smoke.sh`
  - Debian formal release aggregator help/target 契约
- `tests/window_theme_contract_smoke.sh`
  - `main.rs` 中可见的 `SLINT_BACKEND` / `winit-skia-software` 锁定路径

## Slint Callbacks 与现有 UI 约束

- 本轮没有新增 Slint callback，也没有改动 `.slint` 结构。
- 下列现有 callback / event path 仍是回归关注点:
  - `on_toggle_theme_mode_requested`
  - `on_winit_window_event`
  - `on_shell_layout_invalidated`
- `ui/app-window.slint` 仍保留静态 `title: "Mica Term"`。
- 如果下一轮要把 `runtime_window_title(profile)` 真正反映到窗口标题，需要先确认 Slint root window 是否暴露可写 title 属性，或改成显式的 Slint property + Rust setter。

## 边缘情况与后续 TDD 关注点

- `unsafe { std::env::set_var("SLINT_BACKEND", ...) }`
  - 依赖当前仍处于单线程 startup 阶段
  - 后续若把 Tokio runtime、线程池或其他后台初始化前移，必须补测试确保环境变量写入时机仍然安全
- external `SLINT_BACKEND` 冲突
  - experimental 包应覆盖并记录日志
  - formal 包不应强制改写
- pure-Skia wrapper 平台边界
  - 仅 `Windows MSVC` 是当前受支持实验链
  - Linux -> Windows GNU cross-build 不应被误宣称支持 Skia experimental
- 标题可见性
  - 当前只有 helper 契约，没有真实窗口标题更新
  - 后续若要做 UI 可见标识，需要新增自动化测试覆盖 `.slint` property 或 window adapter 路径
- GUI 实机验证缺口
  - Windows GNU formal 包是否稳定维持 `winit-software`
  - Windows MSVC Skia Experimental 包是否稳定启动 `winit-skia-software`
  - experimental 失败提示与日志在真实机环境中的可观察性
- Tokio / channel / actor 风险
  - 本轮没有新增 Tokio channel、actor mailbox 或跨线程共享状态
  - 后续若把 runtime profile 或 build profile 透传到异步层，应优先覆盖 channel 堵塞、消息顺序和 data race 风险

## 建议的下一轮 TDD 顺序

1. 为 `main.rs` 的 profile 选择与 backend lock 抽出更细的可测试 helper，减少启动路径上的隐式副作用。
2. 如果需要真实窗口标题标识，先做失败测试，再决定是扩 Slint property 还是走 window adapter。
3. 在 Windows 专用环境补 GUI smoke，验证 formal / experimental 两条链在真实机上的 renderer 和日志表现。
4. 若后续引入 Tokio startup work，先补环境变量设置时机与日志初始化顺序的回归测试。

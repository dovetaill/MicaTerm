# Windows Native Terminal Surface Recovery TDD Spec

日期: 2026-04-02
执行者: Codex
范围: `2026-04-01-windows-native-terminal-surface-recovery-implementation-plan.md` 与 2026-04-02 software scene-composition follow-up task 的真实收口说明。

> 本文档取代 `docs/plans/2026-04-01-native-only-terminal-surface-tdd-spec.md` 作为下一阶段 TDD / Windows 真机补验输入。
> 它只记录当前 worktree 已验证的事实，不把 source-level contract、Linux-host 构建结果或 scene-composition 切换包装成“Windows 真机已最终完成”。

## 当前结论

- recovery 主链之外，2026-04-02 又补了 3 个 follow-up task：重新定义 software 合成契约、切换到 scene-owned terminal image presenter、收敛 metrics / hit-test 真源。
- Windows software 兼容包的可见 terminal 主输出现在不再依赖 `NativeTerminalSurface` 的 whole-window after-draw post-pass；它已切为 `SceneImageTerminalRenderer -> workspace_session_surface_image -> Slint scene`。
- `WindowsMainline` 仍保留 `PostRenderNativeSurface` 路线，作为 mainline / native-only 路径继续存在；`WindowsSoftwareCompat` 则显式走 `SceneImage`。
- `./build-win-x64-software.sh` 已于 2026-04-02 在 Linux host 重新打包成功，产物为 `dist/mica-term-x86_64-pc-windows-gnu-release-software.zip`。
- `./build-win-x64.sh` 仍不能在当前 Linux host 完成；脚本会直接要求 Windows MSVC shell / Git Bash 环境。
- Windows 真机视觉补验仍未在本轮完成，因此“Windows 真机最终修复完成”依旧不能下结论。

## 最终验证记录

| 命令 / 动作 | 结果 | 说明 |
| --- | --- | --- |
| `cargo test --test native_terminal_surface_contract_spec --test terminal_renderer_dwrite_spec -q` | PASS | recovery 主链 Task 7 收尾时通过 |
| `cargo test --test runtime_profile --test native_terminal_surface_contract_spec --test terminal_scene_image_spec -q` | PASS | 2026-04-02 software scene-composition follow-up tests 通过 |
| `cargo check --workspace` | PASS | 2026-04-02 follow-up 收尾再次复跑通过 |
| `cargo clippy --workspace -- -D warnings` | PASS | 2026-04-02 follow-up 收尾再次复跑通过 |
| `./build-win-x64-software.sh` | PASS | 2026-04-02 follow-up 收尾再次生成 GNU software 包 |
| `./build-win-x64.sh` | FAIL / 环境限制 | 当前不是 Windows MSVC shell，脚本按设计拒绝继续 |
| Windows real-machine verification | NOT RUN | 当前执行环境不具备 Windows GUI / 真机条件 |

## 已交付的核心 struct / trait / 模块

### Runtime / packaging

- `AppRuntimeProfile`
  - 统一承载 `TerminalRenderMode`、`NativePresentPath`、`TerminalCompositionMode`、`AppBuildFlavor`。
  - 当前区分 `windows-software-compat` / `SceneImage` 与 `windows-mainline` / `PostRenderNativeSurface`。
- `NativePresentPath`
  - 明确 packaged native present 路径，而不是把 present 触发链路写死在 `NativeTerminalSurface`。
- `TerminalCompositionMode`
  - 显式区分 `SceneImage` 与 `PostRenderNativeSurface`。
  - software 包用它强制把可见 terminal 输出留在 Slint scene 内。

### Presenter / retained frame

- `TerminalPresenter`
  - terminal projection 的唯一入口，现在既可产出 `PresentedTerminalFrame::Native`，也可产出 scene-owned `PresentedTerminalFrame::Bitmap`。
- `WindowsNativePresenter`
  - 负责 `TerminalSurfaceState -> TerminalModelFrame -> shaping -> renderer.prepare() -> PresentableNativeFrame`。
- `WindowsSceneImagePresenter`
  - software 包的可见 terminal presenter。
  - 复用 native glyph pipeline，但把最终输出离屏合成为 Slint `Image`。
- `SceneImageTerminalRenderer`
  - software scene path 的离屏 RGBA renderer。
  - 消费 `PresentableNativeFrame`，渲染 background / selection / mono glyph / color glyph / underline / cursor / IME preview。
  - 保证 glyph 绘制被裁剪到声明的 cell span 内，避免跨格堆叠继续污染 hit-test。
- `NativeTerminalFrame`
  - retained native 或 scene-image compositor 消费的统一 frame payload。
- `PresentableNativeFrame`
  - 聚合 `background_runs`、mono/color glyph draws、selection/cursor/underline/IME overlays、renderer stats。

### Surface scheduling / lifecycle

- `NativeTerminalSurface`
  - retained native surface bridge，现仅服务真正需要 scene 外 native present 的路径。
- `NativeSurfacePresentDriver`
  - present 调度 seam。
- `EventLoopPresentDriver`
  - 仍保留给 retained native surface 调度；不再承担 Windows software 可见 terminal 主输出。
- `RenderingNotifierPresentDriver`
  - Windows mainline 预期路径，仍待 Windows 主机补验。
- `NativeFrameDamageTracker`
  - 处理 full vs overlay-only damage，负责 resize / retained frame 变更的脏区判断。
- `NativeTerminalSurfaceDiagnostics`
  - 输出 `hwnd`、`render_target_generation`、`last_prepared_frame_token`、`last_presented_frame_token` 与 draw counters。
- `PlatformNativeSurfaceBackend`
  - 跨平台 retained native surface backend trait。
- `WindowsNativeSurfaceBackend`
  - Windows D2D backend，负责 render target、brush/bitmap cache、overlay draw stages、detach / device-loss guard。

### Font / shaping / renderer

- `DirectWriteFontSystem`
  - 名称保留为 staged Windows backend，但当前实现仍以 bundled primary font + `swash` / `rustybuzz` 为核心。
- `WindowsFontLocator`
  - system-backed font discover / locate helper。
- `WindowsFontFallbackResolver`
  - mixed text fallback family discover helper。
- `LoadedFont`
  - presenter / renderer / shaping 共用的 font object contract。
- `TerminalTextShaper`
  - 将 fallback `resolved_face`、`source_byte_range`、cluster 信息继续往 renderer 传递。
- `WgpuTerminalRenderer`
  - 准备 retained `PreparedBackgroundRun`、`PreparedMonochromeGlyphDraw`、`PreparedColorGlyphDraw` 与 renderer stats。

### Runtime actor / channel

- `SshSessionRuntime`
  - SSH terminal runtime actor。
- `RuntimeCommand`
  - runtime 控制命令通道。
- `SessionRuntimeEvent`
  - UI / manager 消费的 runtime 事件。
- `SurfaceDirtyNotifier`
  - runtime 侧 dirty 节流。
- `coalesce_surface_backlog()` / `coalesce_surface_dirty_backlog()`
  - `SessionManager` 侧 backlog 压缩逻辑。

## Slint callbacks / UI 绑定 / 线程切换点

- `ui/shell/terminal-session-host.slint`
  - `key-input`
  - `surface-resize-requested`
  - `copy-selection-requested`
  - `paste-requested`
  - `scroll-requested`
  - `jump-to-latest-requested`
  - `mouse-input`
- `ui/shell/workspace-pane.slint` 与 `ui/app-window.slint`
  - 继续转发 `workspace-session-native-frame-token` 与 terminal host callbacks。
  - software scene path 下通过 `workspace-session-surface-image` 承载可见 terminal 图像。
- bootstrap thread-local 持有者
  - `WORKSPACE_TERMINAL_PRESENTER`
  - `WORKSPACE_NATIVE_TERMINAL_SURFACE`
- 关键调度点
  - `present_workspace_native_terminal_frame()`
  - `workspace_native_terminal_rect()`
  - `window.set_workspace_session_surface_image(...)`
  - `window.set_workspace_session_cell_width(...)`
  - `window.set_workspace_session_cell_height(...)`
  - `window.set_workspace_session_native_frame_token(0)`（scene image 路径必须清零 native frame token）
  - `slint::invoke_from_event_loop(...)`

## 本轮明确移植 / 借鉴的方向

### 主要参考 WezTerm 的部分

- Windows font locate / fallback chain 的组织方式。
- OpenType feature / ligature / fallback shaping 的思路。
- color glyph / emoji raster path 的方向。

### 主要参考 Alacritty / Windows native backend 的部分

- damage tracking 的设计方向。
- overlay draw stages 与 frame lifecycle 的组织顺序。
- cell-span / row-band 裁剪要先于交互命中真源收敛。

### 仍然属于本项目自定义的部分

- `NativeSurfacePresentDriver` / `EventLoopPresentDriver` / `RenderingNotifierPresentDriver`
- `NativeTerminalSurfaceDiagnostics`
- bootstrap 与 Slint properties 的 retained native frame integration
- bootstrap 与 Slint properties 的 scene-image terminal integration
- `WindowsNativeSurfaceBackend` 的当前 D2D resource/cache layout
- `SceneImageTerminalRenderer` 的 offscreen glyph cache / clip 策略
- `SessionManager` 与 UI projection timer 的现有协作方式

## 剩余风险与真实未完成项

1. `./build-win-x64.sh` 仍未在 Windows MSVC shell 上通过，因此 mainline packaged path 尚未在本轮封板。
2. `RenderingNotifierPresentDriver` 只完成了 source-level / Linux-host 侧 wiring，尚未做 Windows 真机 first-paint 验证。
3. `WindowsSoftwareCompat` 现在虽然已切到 scene-owned image，但 scene image renderer 仍是 app 内部离屏合成器，不等同于 Windows Terminal / WezTerm 那种平台级 atlas renderer。
4. `DirectWriteFontSystem` 虽然已有 system locator/fallback，但实现仍是 staged backend，不应被描述成“完整 DirectWrite shaping/raster stack”。
5. 真机尚未补验：
   - first-paint text visibility
   - software scene path 下的 tabs / status bar / sidebar / tooltip 是否彻底恢复正确 z-order
   - selection / underline / cursor / IME preview 是否更新
   - emoji / symbol / Nerd Font / CJK mixed text
   - resize / scroll / close / reconnect 是否稳定
6. runtime 仍使用 `mpsc::UnboundedSender`；虽然已有 dirty/backlog coalescing，但极端输出风暴下仍需要关注内存与 UI 延迟。
7. native drawing 仍要求发生在 UI / window thread；后续若把 prepare 再下沉到后台线程，必须继续通过 `slint::invoke_from_event_loop(...)` 回写。

## 下一阶段 TDD 输入

1. 先在 Windows 主机复测最新 `dist/mica-term-x86_64-pc-windows-gnu-release-software.zip`。
2. 启动 packaged app，逐项验证：
   - 首帧文字是否可见
   - tabs / status bar / sidebar / tooltip 是否还会被 terminal 覆盖
   - selection / underline / cursor / IME preview 是否更新
   - emoji / fallback / ligature / CJK 是否正常
   - resize / close / reconnect 是否稳定
3. 若 software scene path 视觉恢复后，再在 Windows MSVC shell / Git Bash 中运行 `./build-win-x64.sh` 验 mainline 路径。
4. 一旦真机发现新问题，先补最小失败测试，再做最小实现，不要跳回“先改代码再补文档”。
5. 真机补验期间优先记录：
   - `NativeTerminalSurfaceDiagnostics`
   - `TerminalCompositionMode`
   - present path（`event-loop` vs `rendering-notifier`）
   - `last_prepared_frame_token` / `last_presented_frame_token`
   - draw counters 与任何 `D2DERR_RECREATE_TARGET` 现象

## 交接结论

当前仓库已经完成 recovery 主链与 software scene-composition follow-up 的代码落地，Linux-host 侧 contracts / compile / clippy / GNU software packaging 都已通过；
但“Windows 真机已完成”这个结论仍然不成立。下一阶段必须把焦点放在 Windows 最新 software 包的真实视觉补验、随后 mainline MSVC build，
并继续用最小 RED / GREEN 的方式收口剩余问题。

# Windows Native Terminal Surface D2D Design

日期: 2026-04-01
执行者: Codex
状态: 已确认方向，待按实现计划逐 Task 落地

## 背景

`native-only terminal surface` 的主线已经把 Windows 包装脚本切到 `terminal-native-renderer`，
也已经让 `bootstrap -> terminal_presenter -> native_surface -> platform backend` 这条链路贯通。
但当前 Windows 运行时仍出现“终端区域空白、只剩闪烁光标”的现象，说明 native-only 迁移还停留在桥接骨架，
并没有完成真正的文本绘制闭环。

当前代码现状已经给出了直接证据：

- `src/app/terminal_renderer/platform/windows.rs` 中的 `WindowsNativeSurfaceBackend::present()` 只更新 `last_presented_frame_token`
- `src/app/terminal_renderer/native_surface.rs` 的 `draw_retained_frame()` 只会在 `BeforeRendering` 时调用 `state.backend.present()`
- `src/app/terminal_presenter.rs` 与 `src/app/terminal_renderer/wgpu_renderer.rs` 已经能准备 `monochrome_glyph_draws`、`color_glyph_draws`、`underline_overlay`、`selection_overlay`、`cursor_overlay`
- 但这些 retained frame payload 目前没有被 Windows backend 消费并画到宿主 `HWND`

换句话说，当前问题不是 `wezterm-term`、SSH runtime、Slint shell、打包脚本，
而是 Windows native backend 仍然是“空壳 present”。

## 目标

本轮设计聚焦 Windows 首发路径，目标是：

- 在同一个 `AppWindow` / Slint 宿主窗口内，真正绘制终端文本、背景、selection、underline、cursor、IME preview
- 保持 `native-only` 目标，不回退到 `session-surface-image` 或 bitmap terminal path
- 保留现有 `terminal_presenter` / `native_surface` / `platform` 抽象，避免把 Windows 特例写死到 UI 层
- 为后续 Wayland / X11 backend 保留共享 display-list / retained-frame 合同
- 让 `./build-win-x64-software.sh` 产物在 Debian 13 Linux-host 交叉打包后，运行时能看到真实终端文本而非空白区域

## 非目标

- 本轮不重写 Linux Wayland / X11 backend
- 本轮不更换 Slint 宿主框架，也不引入 WebView / xterm.js
- 本轮不创建独立 child window、外部 native terminal window、或额外 swapchain 子窗口
- 本轮不把 renderer 重新做成完整 GPU atlas pipeline；重点是先把 retained native frame 正确画到 Windows 宿主表面
- 本轮不新增新的“兼容 bitmap fallback”产品路径

## 根因拆解

### 1. Windows backend 没有真实绘制实现

当前 `WindowsNativeSurfaceBackend` 的状态只有：

- `hwnd`
- `rect`
- `retained_frame`
- `last_presented_frame_token`

它没有持有任何 Direct2D / DirectWrite 绘制资源，也没有把 `retained_frame` 转成可见像素。
因此即使上游 presenter 已经准备好了 frame token 和 overlay 元数据，`present()` 仍然不会画出文本。

### 2. prepared draw 仍偏向“缓存统计”，没有稳定下沉到平台绘制层

`PreparedMonochromeGlyphDraw` / `PreparedColorGlyphDraw` 当前只携带：

- grid 位置信息
- glyph id
- atlas / color cache 元数据
- 偏移量和前景色

这些信息足够做“renderer 内部缓存统计”，但不够让 Windows backend 独立重建绘制资源：

- 单色 glyph 缺少稳定的 alpha mask payload 交付方式
- 彩色 glyph 缺少稳定的 RGBA payload 交付方式
- backend 无法知道当前 frame 需要上传哪些新位图，哪些缓存可复用

### 3. display-list 还缺少背景填充合同

`src/app/terminal_layout/run_segmentation.rs` 里的 `TextStyleKey` 目前只有：

- `fg_rgba`
- `bold`
- `underline`

但终端正确绘制不只需要前景文字，还需要 cell / run 的背景色。
如果不把 `bg_rgba` 纳入 segmentation 和 retained frame：

- ANSI 背景色块无法在 native backend 正确复现
- selection / cursor 叠加顺序容易出错
- backend 只能整块 clear 默认背景，无法表达真实终端背景语义

因此“修复空白文本”不能只补一个 `draw_text()`，还必须把 background run 合同补齐。

## 方案对比

### 方案 A：回退到 bitmap terminal path

优点：

- 可以最快止血
- 现有产品路径已经存在

缺点：

- 明确违背本轮 `native-only` 目标
- 会重新引入模糊、字重漂移和高 DPI 观感问题
- 只是绕开问题，不是修复 Windows native backend

结论：拒绝。

### 方案 B：在 backend 中每帧重新做 DirectWrite layout

做法是让 `WindowsNativeSurfaceBackend` 直接拿 row text 重新做 `TextLayout` / shaping，再逐帧绘制。

优点：

- backend 自给自足
- 可以直接利用 DirectWrite 的文本 API

缺点：

- 会复制现有 `terminal_layout` / `terminal_presenter` 的职责
- grid、selection、semantic overlay、cursor 命中语义会在两处实现，容易漂移
- 与现有 retained frame 架构不一致，后续跨平台抽象更难收敛

结论：拒绝。

### 方案 C：`Direct2D HwndRenderTarget + retained frame raster payload`（推荐）

做法：

- 保留现有 `terminal_presenter -> native_surface -> platform backend` 主线
- 扩展 `PreparedNativeFrame` / `PresentableNativeFrame`，使其携带平台可消费的 glyph/background payload
- Windows backend 持有 `ID2D1Factory + ID2D1HwndRenderTarget + bitmap caches`
- 在 `present()` 中真正完成 background、glyph、overlay 绘制

优点：

- 符合 native-only 目标
- 不重复 shaping / terminal 语义逻辑
- backend 只负责资源生命周期和绘制，职责清晰
- display-list 合同可继续复用到未来 Linux backend

缺点：

- 需要补齐 retained frame 的 payload 合同
- 需要处理 Direct2D target recreate、clip rect、bitmap cache 生命周期

结论：选择此方案。

## 最终架构

## 分层保持不变的部分

以下部分继续保持共享：

- `wezterm-term` 产出 `TerminalSurfaceState`
- `terminal_model.rs` 负责把 runtime surface 投影成 renderer-facing frame
- `terminal_layout` 负责 row segmentation 与 shaping
- `terminal_presenter.rs` 负责把 model + shaped rows 组合成 retained native frame
- `native_surface.rs` 负责和 Slint `RenderingNotifier` 对接

本轮新增和收紧的重点，是 `renderer prepare contract` 与 `Windows platform backend`。

### retained frame 需要表达的内容

为了让 Windows backend 不再依赖“猜测式重建”，retained frame 需要显式携带：

1. frame-level 元数据
   - `frame_token`
   - `cell_width_px` / `cell_height_px`
   - `surface rect`
   - renderer stats（保留诊断用途）

2. background runs
   - `row`
   - `start_col`
   - `end_col`
   - `bg_rgba`

3. monochrome glyph draws
   - glyph cache key
   - grid / pixel origin
   - glyph size / bearing / advance
   - `fg_rgba`
   - 首次出现时附带 alpha mask payload；缓存命中时只保留 key + placement

4. color glyph draws
   - color glyph cache key
   - grid / pixel origin
   - glyph size
   - 首次出现时附带 RGBA payload；缓存命中时只保留 key + placement

5. overlays
   - selection rects
   - underline runs
   - cursor overlay
   - IME preview overlay

### Windows backend 持有的资源

`WindowsNativeSurfaceBackend` 需要从“纯状态对象”升级为真正的绘制后端，至少持有：

- `HWND`
- `ID2D1Factory`
- `ID2D1HwndRenderTarget`
- 终端区域 clip / rect 状态
- 单色 glyph bitmap cache（A8 mask 或等价 alpha bitmap）
- 彩色 glyph bitmap cache（premultiplied BGRA）
- 画刷缓存（背景、前景、selection、cursor 等）
- `last_presented_frame_token`
- retained frame 的最近一次已上传资源索引

### 绘制顺序

Windows backend 的 `present()` 采用固定顺序：

1. `BeginDraw`
2. 按 terminal rect 建立 clip
3. clear 默认背景
4. 绘制 `background runs`
5. 绘制 selection 背景
6. 绘制 monochrome glyphs
7. 绘制 color glyphs
8. 绘制 underline
9. 绘制 cursor
10. 绘制 IME preview
11. `EndDraw`

关键点：

- selection 是文字下层覆盖，不覆盖文字本身
- cursor 在最上层，但不能破坏 IME preview 的可见性
- 若 frame 中不存在新上传 payload，不重复创建 bitmap 资源

## 数据流

新的 Windows native surface 数据流如下：

1. `TerminalSurfaceState` 进入 `TerminalModelFrame`
2. `run_segmentation` 以 `fg_rgba + bg_rgba + bold + underline` 划分 runs
3. `shaper` 基于现有 font backend 输出 glyph runs
4. `WgpuTerminalRenderer::prepare()` 继续承担“准备 display-list”的职责，但其输出不再只是缓存统计，必须包含 background runs 和 glyph upload payload
5. `WindowsNativePresenter` 把 prepared payload 组装成 `NativeTerminalFrame`
6. `NativeTerminalSurface` retain 该 frame，并在 `BeforeRendering` 时调用 backend `present()`
7. `WindowsNativeSurfaceBackend` 将 frame 中的新增 payload 上传到 Direct2D bitmap cache，并把 frame draw list 画到宿主 `HWND`

这样 Windows backend 只消费“已经 shape 好、已经决定颜色和网格语义”的 retained frame，
不重复进行 terminal 语义推导。

## 接口契约调整

### `run_segmentation`

`TextStyleKey` 需要补上 `bg_rgba`，否则 background draw 无法拥有稳定输入。

### `wgpu_renderer`

虽然文件名仍叫 `wgpu_renderer.rs`，但当前职责更接近“native frame prepare stage”。
本轮不强制改文件名，但需要新增：

- background run payload
- monochrome glyph raster upload payload
- color glyph raster upload payload
- 可被 backend 复用的 glyph cache key

### `terminal_presenter`

`PresentableNativeFrame` 需要从“诊断元数据 + overlay”为主，升级为真正的 retained display list：

- 增加 background runs
- 增加 glyph upload / draw payload
- 保留 cursor、selection、underline、IME overlays
- 保留 renderer stats 作为日志和测试辅助

### `platform/windows.rs`

需要从当前 scaffold 变成真正的 Windows D2D backend：

- `attach()` 解析 `HWND`
- `update_surface_rect()` 驱动 target resize / clip 更新
- `update_frame()` retain 最新 frame
- `present()` 完成 resource upload + draw
- `detach()` 释放 target、bitmap、brush 和 HWND 绑定状态

## 资源生命周期与错误处理

### attach / recreate / detach

- `attach()` 只解析并保存 `HWND`，D2D factory / target 可延迟到首次 `present()` 再初始化
- 当 rect 变化或 `EndDraw()` 返回需要 recreate target 的错误时，backend 重新创建 `ID2D1HwndRenderTarget`
- `detach()` 必须清空 retained frame、bitmap caches、brush caches 与 target 句柄，避免窗口销毁后留下悬挂资源

### 失败策略

本轮明确不回退到 bitmap terminal path。
失败时采用以下策略：

- attach 失败：记录 warning，保持 detached backend，不伪造成功
- target recreate 失败：保留 retained frame，等待下次 redraw 重试
- 单帧 upload / draw 失败：记录详细日志，不破坏上游 session 状态
- window teardown 后收到迟到的 redraw：直接忽略，不访问已释放的 `HWND`

## 验证策略

实现完成后至少需要覆盖以下验证：

1. source-level contract tests
   - `tests/native_terminal_surface_contract_spec.rs`
   - `tests/terminal_renderer_dwrite_spec.rs`
   - 新增或更新 D2D backend 合同测试

2. renderer / presenter 合同验证
   - background run 是否进入 retained frame
   - mono / color glyph payload 是否有明确上传合同
   - present 是否不再是 no-op

3. workspace 级编译质量
   - `cargo check --workspace`
   - `cargo clippy --workspace -- -D warnings`

4. 打包验证
   - `./build-win-x64-software.sh`
   - 产物运行后可见真实终端文本、selection、cursor，而非空白区域

## 关键风险与边缘情况

- `D2DERR_RECREATE_TARGET` 导致 target 重建时，cache 与 retained frame 的同步顺序必须正确
- rect 为 0 或 terminal host 未布局完成时，backend 不能错误创建无效 target
- 同一 frame token 下如果仅 rect 改变，仍需要触发重绘，而不是被 token 判定为“无变化”
- UI teardown 后如果 `BeforeRendering` 再次触发，不能访问已释放的 `HWND` 或 D2D 资源
- 如果 glyph upload payload 只在首帧出现，backend 必须正确缓存；如果 payload 每帧都拷贝，内存成本会迅速放大
- background run 与 selection / cursor 叠加顺序不正确时，会出现文字被整块覆盖或高亮错层

## 结论

本轮推荐的正确修复路径是：

- 保留现有 native-only shared pipeline
- 把 retained frame 补成真正可绘制的 display list
- 在 Windows backend 内引入 `Direct2D HwndRenderTarget + glyph bitmap caches`
- 显式补齐 background、monochrome glyph、color glyph、selection、underline、cursor、IME 的绘制顺序
- 不再接受 `present()` 为空实现的“半接线状态”

这条路径工作量明显大于“小补丁”，但它是唯一符合 `native-only` 目标、
并能真正解决 Windows 包运行时空白终端问题的方案。

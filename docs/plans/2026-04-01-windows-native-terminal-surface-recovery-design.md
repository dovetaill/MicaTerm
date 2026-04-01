# Windows Native Terminal Surface Recovery Design

日期: 2026-04-01
执行者: Codex
状态: 新 recovery 设计，取代“2026-04-01 已完成实现”口径

## 背景

`mica-term` 现在已经有一套 Windows native terminal surface 的代码骨架：
- `NativeTerminalSurface`
- `PlatformNativeSurfaceBackend`
- `WindowsNativeSurfaceBackend`
- `DirectWriteFontSystem`

表面上看，D2D backend、字体接口、overlay 绘制函数都已经存在；
但 Windows 真机构建后的实际结果仍然是“终端区域空白，只有光标闪烁”。

这说明当前问题不是“还缺一个小绘制补丁”，而是系统性问题：
- 渲染触发链不可靠
- Windows 字体栈仍是过渡实现
- emoji / fallback / ligature / damage tracking 仍停留在合同层或半成品阶段

因此本设计文档不再延续旧文档的“完成态叙事”，而是明确把当前工作定义为 recovery / repair：
先恢复真实可见的 Windows native terminal text，再继续把字体、fallback、emoji、damage 等能力补齐到可发布水平。

## 当前真实状态

### 已有能力

1. `src/app/terminal_renderer/platform/windows.rs` 已具备：
   - `ID2D1Factory`
   - `ID2D1HwndRenderTarget`
   - monochrome bitmap cache
   - color bitmap cache
   - background / selection / underline / cursor / IME overlay draw 函数
2. `NativeTerminalFrame` 已经能携带相对丰富的 prepared frame payload。
3. `tests/terminal_renderer_dwrite_spec.rs` 与 `tests/terminal_color_emoji_spec.rs` 已经覆盖了一批合同字段和结构。

### 未完成状态

1. `NativeTerminalSurface` 依赖 `set_rendering_notifier()` 触发 `backend.present()`。
2. 当前 shipping 的 `winit-software` 路径不一定支持这个 hook，导致 backend 代码可能根本没有机会执行。
3. `DirectWriteFontSystem` 仍然只基于 bunded font + `rustybuzz` + `swash`，不是真实的 Windows system font + fallback pipeline。
4. 彩色 emoji 目前仍是假 `RGBA` 方块 fallback，不是真正的 color glyph raster。
5. OpenType feature / ligature 合同已经存在，但没有真正进入 shaping。
6. damage tracking、partial redraw、resize/device-loss/shutdown hardening 仍不足以对齐成熟终端。

## 问题陈述

我们需要解决的不是单点 bug，而是一条完整渲染链的真实性问题：

1. `NativeTerminalFrame` 是否已经准备好了绘制数据？
2. `WindowsNativeSurfaceBackend::present()` 是否真的在 Windows 进程里被调用？
3. D2D target 是否真的在目标 `HWND` 上完成了 `BeginDraw/EndDraw`？
4. 绘制的字体、fallback、emoji、overlay 是否是真实现而不是测试替身？
5. 当窗口 resize、session 销毁、IME 更新、Tokio 任务取消时，surface 生命周期是否仍然稳定？

只要其中任一环节不成立，就会再次出现“代码很多、测试不少、真机却空白”的情况。

## 目标

### 主目标

在 Windows 路径上真正恢复并完成 native terminal surface：
- 文本真实可见
- background / selection / underline / cursor / IME overlay 可见且时序正确
- fallback font / emoji / ligature / OpenType feature 真正工作
- resize / close / device-loss / session dispose 稳定

### 次目标

- 为后续 Linux/macOS 路径保留可共享的契约：prepared frame、font shaping、damage contract、overlay contract
- 让 “构建成功” 与 “Windows 真机渲染成功” 之间有明确验证链，不再依赖主观判断

## 非目标

- 这次设计不追求一次性把整个渲染系统重写成另一套 GUI 框架
- 不追求现在就把 Linux / macOS backend 全部同步完工
- 不把“临时能看见字”视为最终完成；字体/fallback/emoji/damage 仍必须补齐

## 方案对比

### 方案 A：继续沿用当前 `set_rendering_notifier()` 路线，只在 `windows.rs` 上补更多 D2D 代码

优点：
- 改动看起来最小
- 能复用已有 Windows backend 结构

缺点：
- 没有解决“`present()` 根本没被稳定触发”的核心风险
- 很容易继续出现“backend 代码越来越多，真机仍是空白”
- 会把排障时间浪费在错误层面

结论：不推荐继续把它当主路径。

### 方案 B：引入独立的 present driver / render trigger seam，把 native present 从单点 notifier 依赖里解耦

优点：
- 直接解决当前最大 blocker
- 能把“frame prepared”和“frame actually presented”拆开诊断
- 可支持多种驱动方式：render notifier、事件循环主动 present、自定义 `Platform`/`WindowAdapter`
- 可以让 Windows backend 更专注于 D2D 绘制与资源生命周期

缺点：
- 需要重构 `NativeTerminalSurface` 的组织方式
- 需要新增 runtime diagnostics 和更严格的生命周期管理

结论：推荐作为主方案。

### 方案 C：彻底脱离当前 Slint host 渲染钩子，自己管理 child `HWND` / `WM_PAINT` / 独立 native child surface

优点：
- 对 native terminal surface 控制力最强
- 最接近“终端区域完全自行绘制”的纯原生路线

缺点：
- 与当前 Slint shell 的集成成本最高
- 焦点、布局、输入法、DPI、裁剪、命中测试都要自己重新接一层
- 很容易把当前问题从“可修复”升级为“架构级重开”

结论：保留为兜底方案，仅在方案 B 被证明确实走不通时再启用。

## 推荐方案

采用 **方案 B：present driver + Windows native backend + 真正的 Windows font stack**。

推荐理由：
- 它先解决“有没有真的画”的问题，再解决“画得对不对”的问题。
- 它允许把当前 `WindowsNativeSurfaceBackend` 保留下来继续演进，而不是整体推翻。
- 它可以把 Windows 真机验证变成可观测系统，而不是只能靠肉眼猜。

## 推荐架构

### 1. `NativeTerminalSurface` 拆成三个职责

#### `NativeTerminalSurface`

负责：
- 持有 `retained_frame`
- 管理 rect / dirty 标记
- 协调 present driver 与 platform backend
- 记录 diagnostics snapshot

它不再只依赖 `set_rendering_notifier()` 一个入口来完成一切。

#### `NativeSurfacePresentDriver`

负责：
- 决定什么时候触发 `backend.present()`
- 把 present 调度回 UI 线程
- 记录“调度成功 / 实际执行 / 执行失败”三类状态

建议支持的实现：
- `RenderingNotifierPresentDriver`：给 Skia 或明确支持 notifier 的 renderer 用
- `EventLoopPresentDriver`：给 Windows 软件路径或自定义平台路径用
- 预留 `CustomPlatformPresentDriver`：以后如果必须接自定义 `Platform`/`WindowAdapter`

#### `PlatformNativeSurfaceBackend`

负责：
- `attach/update_surface_rect/update_frame/present/detach`
- D2D target、brush、bitmap、device-loss、overlay draw
- 严格保持“只关心怎么画，不关心什么时候叫它画”

### 2. Windows 字体栈拆成三层

#### `WindowsFontLocator`

负责：
- system font enumerate / locate
- fallback font resolution
- family/style/weight/stretch 到真实 face 的映射

#### `WindowsTextShaper`

负责：
- HarfBuzz shaping
- OpenType feature tag 注入
- cluster / ligature / cell width 计算
- fallback recursion

#### `WindowsGlyphRasterizer`

负责：
- monochrome glyph raster
- color glyph / emoji raster
- mono 与 color cache 分离

这样可以避免继续让 `windows_dwrite.rs` 一边假装 locator，一边假装 shaper，一边假装 rasterizer。

### 3. 引入独立 `NativeFrameDamageTracker`

职责：
- 行级/矩形级 damage 聚合
- 全量 vs partial redraw 决策
- resize 后整帧失效
- selection / cursor / IME / scroll 触发的额外 damage

思路上可借鉴 Alacritty，但保持本项目自己的 prepared frame 和 D2D backend 接口。

### 4. diagnostics 变成正式接口，而不是调试时临时打印

至少需要保留：
- last attached `HWND`
- render target generation
- last prepared frame token
- last presented frame token
- last present timestamp / result
- draw counts: background / mono glyph / color glyph / selection / underline / cursor / IME
- last device-loss / recreate-target error

只有这样，Windows 真机空白时才能快速定位是：
- 没 attach
- 没 present
- present 失败
- frame payload 为空
- 字体/emoji/fallback 数据为空

## 数据流

推荐的数据流如下：

1. `wezterm-term` / session runtime 更新 terminal state
2. presenter 生成 `NativeTerminalFrame`
3. `NativeTerminalSurface` 接收 frame，标记 dirty
4. `NativeSurfacePresentDriver` 在 UI 线程调度一次 present
5. `WindowsNativeSurfaceBackend`：
   - 确保 render target
   - background
   - selection
   - mono glyph
   - color glyph
   - underline
   - cursor
   - IME preview
   - `EndDraw`
6. `NativeTerminalSurfaceDiagnostics` 记录此次 present 的结果

关键原则：
- frame prepare 与 frame present 分开记录
- 所有 native drawing 都必须在 UI 线程/窗口线程发生
- 异步 Tokio 任务只能产出状态，不直接碰 D2D 资源

## 错误处理与生命周期约束

### 1. channel / task 约束

- Tokio 侧只发送 terminal state / frame request，不直接调用 backend
- UI 线程统一负责 `present()`
- 如果 frame 更新过快，允许 coalesce，只保留最新 frame token

### 2. resize / device-loss

- rect 变化后必须标记 render target dirty
- `D2DERR_RECREATE_TARGET` 触发时必须清理 brush / bitmap device resource，并允许下一帧重建

### 3. window close / session dispose

- `detach()` 后禁止再提交 surface draw
- 所有挂起的 UI 回调必须在执行前检查 surface 是否仍 alive

### 4. IME / selection / cursor

- overlay 不应该依赖“文本 draw 是否成功”来更新 dirty
- 必须能独立触发重绘，并进入 damage 计算

## GitHub 参考策略

### 可直接吸收的方法

1. WezTerm
   - `wezterm-font/src/locator/gdi.rs`
   - `wezterm-font/src/shaper/harfbuzz.rs`
   - `wezterm-font/src/rasterizer/colr.rs`

2. Alacritty
   - `alacritty/src/display/damage.rs`
   - `alacritty/src/display/mod.rs`

### 吸收原则

- 抄算法和职责划分，不抄整套宿主渲染栈
- 本项目要保留 `Slint shell + NativeTerminalFrame + PlatformNativeSurfaceBackend` 这些已有契约
- 如果外部实现依赖 OpenGL/glutin/crossfont/cairo，不要整包带进来，只迁移必要逻辑

## 验证策略

### 本地静态验证

- `cargo check --workspace`
- `cargo clippy --workspace -- -D warnings`
- 对应单测：
  - `tests/native_terminal_surface_contract_spec.rs`
  - `tests/terminal_renderer_dwrite_spec.rs`
  - `tests/terminal_color_emoji_spec.rs`
  - `tests/terminal_layout_harfbuzz_spec.rs`

### 构建验证

- `./build-win-x64-software.sh`
- 必要时补 `./build-win-x64.sh`

### Windows 真机验证

必须至少验证：
- 首屏文本可见
- selection / underline / cursor / IME preview 可见
- emoji / symbol / CJK / Nerd Font 混排可见
- resize / close / re-open 不崩溃
- 连续输入时不会再次退化为空白区

## 风险

1. 如果 Slint 当前软件路径确实无法提供稳定 present seam，可能需要阶段性引入自定义 `Platform` / `WindowAdapter`。
2. 真实 Windows fallback + emoji 路径会显著增加字体子系统复杂度。
3. damage tracking 一旦设计不当，容易出现“字符没刷新”或“整帧全刷导致卡顿”两个极端。
4. 若没有严格的 diagnostics，后续仍可能出现“代码改了很多但不知道哪一环没生效”的情况。

## 阶段性完成定义

### Phase 1 完成

- Windows 真机能稳定看到 background + monochrome glyph
- 能证明 `present()` 和 `EndDraw()` 正常发生

### Phase 2 完成

- fallback font / ligature / OpenType feature 工作
- color glyph / emoji 工作

### Phase 3 完成

- damage tracking / resize / device-loss / close 可靠
- packaging 与 Windows 真机回归通过
- 新 TDD 文档真实记录剩余风险

## 结论

当前最重要的不是继续给 `windows.rs` 加更多“应该能画”的代码，
而是先把 Windows native draw 的触发链和诊断链变成可信系统。

推荐路线是：
1. 先建立 present driver 与 runtime diagnostics，恢复“真正可见文本”
2. 再把字体、fallback、emoji、OpenType feature 做成真实 Windows 路径
3. 最后补 damage/lifecycle hardening，并以 Windows 真机结果而不是 source-level test 作为完成标准

本设计文档从现在起作为 recovery 主文档，旧的“已完成实现”叙事不再作为事实依据。

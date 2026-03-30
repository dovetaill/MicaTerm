# Mica Term Terminal Rendering Stack Design

日期: 2026-03-30  
执行者: Codex  
状态: 已确认方案，待进入实现规划

## 背景

当前仓库的终端显示链路仍然是“终端状态快照 -> 自绘 atlas 位图 -> Slint `Image` 显示”：

- [src/app/ssh/runtime.rs](/home/wwwroot/mica-term/src/app/ssh/runtime.rs) 负责维护 `TerminalSurfaceState`
- [src/app/terminal_atlas.rs](/home/wwwroot/mica-term/src/app/terminal_atlas.rs) 使用 `ab_glyph` 将可见终端内容栅格化为整张 RGBA 图片
- [src/app/bootstrap.rs](/home/wwwroot/mica-term/src/app/bootstrap.rs) 将 atlas 输出同步到窗口属性
- [ui/shell/terminal-session-host.slint](/home/wwwroot/mica-term/ui/shell/terminal-session-host.slint) 用 `session-surface-image` 作为终端承载

这条链路已经足以完成功能，但文本观感存在结构性问题：

- 字形 spacing 依赖 `chars() + kern + advance` 的简化模型
- shaping、fallback、emoji、宽字符、ligature 不由成熟文本栈统一处理
- 最终终端内容以整屏 bitmap 进入 Slint，容易在 DPI、fractional scaling、filtering 下发虚
- alpha coverage、混合和 gamma 处理不具备 Ghostty / WezTerm / Windows Terminal 级别的文本栈控制能力

用户已经明确表示：

- 不再接受继续调字体、hinting 或 atlas 参数作为主方向
- 不接受 `xterm.js`
- 更偏好 `Ghostty / WezTerm` 式的跨平台文本栈
- 允许将 `Windows-first DirectWrite` 作为对照方案讨论
- 可以保留 `libghostty` 作为止损备选，而不是主路线

## 目标

- 保留当前自研终端核心、session 管理、Slint 外层 UI 与交互结构
- 将文本渲染栈升级为“成熟 shaping + 平台字体后端 + per-glyph GPU renderer”
- 主方案采用 Ghostty / WezTerm 式跨平台架构：
  - `Windows = DirectWrite`
  - `macOS = CoreText`
  - `Linux = Fontconfig + FreeType`
  - 统一 `HarfBuzz` shaping
- 让 Windows 文本质量尽快接近 Windows Terminal / Termius 观感
- 为后续平台扩展和 renderer 继续演进保留清晰的模块边界

## 非目标

- 不在本轮设计中重写 terminal state、SSH runtime 或 session manager
- 不在本轮设计中替换 Slint 的整套外层 UI
- 不在本轮设计中立刻实现完整 GPU renderer
- 不承诺一次性达到 Ghostty 全部字体功能或全部视觉细节
- 不把 `libghostty` 作为首选接入路径

## 外部调研结论

### 业界成熟文本栈不是单库，而是组合

终端产品里“清晰、稳、实”的文本显示，通常由以下四层组合而成：

- 字体发现 / fallback：`DirectWrite`、`CoreText`、`Fontconfig`
- shaping：`HarfBuzz` 或 `CoreText`
- glyph rasterization：`DirectWrite`、`CoreText`、`FreeType`
- 最终合成：`Direct3D`、`Direct2D`、`Metal`、`OpenGL`、`WebGPU`

因此，“换个字体文件”或“调 atlas 参数”只能改善局部；若目标是 Ghostty / WezTerm / Windows Terminal 级文本质感，必须升级显示栈。

### Ghostty 的实现方式

基于 Ghostty 官方 README、源码与文档，可确认：

- [Ghostty README](https://github.com/ghostty-org/ghostty) 将 `libghostty` 定义为可嵌入的 `C/Zig` library
- [src/font/backend.zig](https://github.com/ghostty-org/ghostty/blob/main/src/font/backend.zig) 抽象了 `freetype`、`fontconfig_freetype`、`coretext`、`coretext_harfbuzz` 等多种后端
- [src/font/shape.zig](https://github.com/ghostty-org/ghostty/blob/main/src/font/shape.zig) 按 backend 分发 shaping 到 `HarfBuzz` / `CoreText`
- [src/font/shaper/harfbuzz.zig](https://github.com/ghostty-org/ghostty/blob/main/src/font/shaper/harfbuzz.zig) 直接调用 `harfbuzz.shape(...)`
- [src/build/SharedDeps.zig](https://github.com/ghostty-org/ghostty/blob/main/src/build/SharedDeps.zig) 可见 `FreeType`、`HarfBuzz`、`Fontconfig` 是一等依赖
- [Ghostty config reference](https://ghostty.org/docs/config/reference) 还将 `freetype-load-flags` 和 alpha blending color space 显式暴露

结论：

- Ghostty 不是“单一神奇 renderer”
- Ghostty 是“平台字体后端 + 成熟 shaping + 自己的 GPU renderer”组合
- Ghostty 最终也会使用 atlas / GPU 绘制，但它的上游文本栈远比当前仓库完整

### WezTerm / Alacritty / Windows Terminal 的共同点

- [WezTerm font rasterizer](https://wezterm.org/config/lua/config/font_rasterizer.html) 明确 rasterizer 使用 `FreeType`
- [WezTerm font shaping](https://wezterm.org/config/font-shaping.html) 明确使用 `HarfBuzz`
- [WezTerm front_end](https://wezterm.org/config/lua/config/front_end.html) 说明前端可为 `OpenGL` / `WebGpu`
- [crossfont README](https://github.com/alacritty/crossfont) 明确：
  - `Linux/BSD = Freetype`
  - `Windows = DirectWrite`
  - `macOS = Core Text`
- [Windows Terminal AtlasEngine README](https://github.com/microsoft/terminal/blob/main/src/renderer/atlas/README.md) 说明：
  - 文本先转换为 `DWRITE_GLYPH_RUN`
  - `BackendD2D` 负责 Direct2D 文本 renderer
  - `BackendD3D` 负责带 glyph cache 的高性能自定义 renderer

结论：

- 顶级终端普遍都不是“整屏 bitmap 终端”
- 真正稳定的文本观感来自成熟字体后端和 per-glyph renderer

## 现状诊断

当前终端链路的核心限制如下：

### 1. 整屏图片是主要观感瓶颈

[ui/shell/terminal-session-host.slint](/home/wwwroot/mica-term/ui/shell/terminal-session-host.slint) 当前使用：

- `in property <image> session-surface-image`
- `Image { source: root.session-surface-image; image-fit: fill; }`

只要文本最终以整张图片进入 UI，就很难避免：

- 整屏 bitmap 二次采样
- scaling / DPI 条件下整体发虚
- glyph 无法按设备像素精细对齐

### 2. glyph layout 仍然是简化模型

[src/app/terminal_atlas.rs](/home/wwwroot/mica-term/src/app/terminal_atlas.rs) 当前使用 `ab_glyph`：

- 逐字符取 `glyph_id`
- 基于 `h_advance` / `kern` 计算位置
- 将 `outline_glyph` coverage 写进 alpha sprite

这不足以替代：

- `HarfBuzz` shaping
- 平台 fallback 策略
- 颜色 emoji / 合字 / 复杂脚本 / cluster positioning

### 3. UI 与 renderer 职责边界不清晰

当前链路中：

- 终端文本由 atlas 生成整图
- cursor、selection、scrollbar、布局命中仍由 Slint 负责

这会阻碍未来升级，因为 renderer 无法统一掌握 terminal canvas 内的像素关系。

## 方案对比

### 方案 A：保留自研终端核心，重做文本渲染栈

做法：

- 保留 `TerminalSurfaceState`、session 管理、Slint 主 UI
- 替换 `terminal_atlas` 路线
- 新建 `layout + font + raster + renderer` 分层

优点：

- 与当前仓库最连续
- 风险可分阶段隔离
- 能逐步演进到 Ghostty / WezTerm 式结构
- 仍由仓库完全掌控 terminal canvas 行为

缺点：

- 集成期较长
- renderer 与平台后端都要自己搭

### 方案 B：Windows-first DirectWrite，然后回抽跨平台抽象

做法：

- 先做 `DirectWrite + D3D/D2D`
- 先把 Windows 文本质量拉到位
- 再补 Linux / macOS 后端

优点：

- Windows 上最先见效
- 最快接近 Windows Terminal 观感

缺点：

- 前期平台不对称
- 如果抽象做得不好，后续会出现 Windows 偏置架构

### 方案 C：嵌入 `libghostty` 等成熟核心

做法：

- 保留当前窗口 / tab / sidebar 等外层 UI
- terminal pane 改为嵌入成熟 terminal core/render surface

优点：

- 可显著降低自研 terminal rendering 风险
- 止损能力强

缺点：

- 与现有 terminal data flow 的边界重组更硬
- 对现有自研 terminal control 面有侵入

### 最终选择

采用：

- **主方案：方案 A**
- **设计目标吸收：方案 B 的 Windows 质量目标**
- **止损备选：方案 C**

即：

- 架构层面按跨平台文本栈设计
- 实现顺序上允许 Windows-first
- 若 renderer 集成风险失控，则保留 `libghostty` 接管 terminal pane 的可能性

## 推荐架构

推荐将终端显示拆成以下层次：

### 1. Terminal Model

职责：

- 接收 `TerminalSurfaceState`
- 归一化为适合渲染的 frame model
- 提供 dirty rows、selection、cursor、palette、viewport 信息

建议文件：

- `src/app/terminal_model.rs`

说明：

- 该层不做 shaping，不做 rasterization，不直接依赖平台 API
- 继续复用当前 runtime / session manager 的表述能力

### 2. Text Layout Engine

职责：

- 将 terminal rows 拆分成 `GlyphRun`
- 做 cluster segmentation、fallback font、emoji / wide char / ligature 处理
- 输出 `ShapedRow` / `ShapedFrame`

建议目录：

- `src/app/terminal_layout/mod.rs`
- `src/app/terminal_layout/run_segmentation.rs`
- `src/app/terminal_layout/shaper.rs`

核心依赖：

- `HarfBuzz`

说明：

- 该层负责将“terminal cells”转换为“可绘制 glyph runs”
- 这一步是改善 spacing 和 cluster 稳定性的关键

### 3. Font System / Raster Backend

职责：

- 字体发现
- fallback 解析
- glyph rasterization
- 暴露统一的 glyph bitmap / color glyph 接口

建议目录：

- `src/app/terminal_font/mod.rs`
- `src/app/terminal_font/backend.rs`
- `src/app/terminal_font/windows_dwrite.rs`
- `src/app/terminal_font/macos_coretext.rs`
- `src/app/terminal_font/linux_freetype_fontconfig.rs`

平台映射：

- `Windows = DirectWrite`
- `macOS = CoreText`
- `Linux = Fontconfig + FreeType`

说明：

- 该层只负责字体后端，不碰 session，也不碰 UI
- 这是替代 `ab_glyph` 的核心层

### 4. Terminal GPU Renderer

职责：

- 维护 glyph atlas
- 管理 instance buffer / draw list
- 绘制背景块、glyph、selection、cursor、underline、IME overlay
- 处理 blending / color space / device pixel alignment

建议目录：

- `src/app/terminal_renderer/mod.rs`
- `src/app/terminal_renderer/atlas.rs`
- `src/app/terminal_renderer/frame.rs`
- `src/app/terminal_renderer/native_surface.rs`

说明：

- 输入应该是 `ShapedFrame`
- 输出不再是“整张终端图片”，而是“提交到 render surface 的渲染命令”

## UI 承载方案

### 短期过渡方案

保留 Slint 外层 UI 和终端区交互壳，但把终端内容区从 `Image` 过渡到 native render host。

也就是说：

- `terminal-session-host.slint` 继续负责尺寸、命中、上下文菜单、滚动条
- 终端文本区不再以 `session-surface-image` 为主，而是承载 native render surface

### 长期目标

让 renderer 统一绘制 terminal canvas 内的内容：

- 文本
- selection
- cursor
- underline / decorations
- color emoji

Slint 只保留：

- 外层 chrome
- shell 布局
- 菜单 / 弹层 / titlebar / sidebar / tabs

这样可以避免 terminal canvas 内由两个系统分别绘制、造成像素错位。

## 迁移策略

### 阶段 0：引入 Presenter 抽象，冻结旧 atlas 接口

目标：

- 不改行为
- 先把“surface -> UI 输出”的接口隔离出来

建议新增：

- `src/app/terminal_presenter.rs`

建议处理：

- 将 [src/app/bootstrap.rs](/home/wwwroot/mica-term/src/app/bootstrap.rs) 里直接设置 `workspace_session_surface_image` 的逻辑迁移到 presenter
- 旧 atlas renderer 作为 `BitmapPresenter` 保留

价值：

- 为新旧 renderer 并存打基础

### 阶段 1：替换 layout / font / raster，允许仍输出 bitmap

目标：

- 先验证 spacing、fallback、emoji、glyph metrics
- 先把“松散感”解决掉

说明：

- 这一阶段仍可临时输出 bitmap，以降低改造耦合度
- 但 glyph bitmap 必须来自新的字体后端，而不是 `ab_glyph`

价值：

- 最先改善字符间距和字形一致性

### 阶段 2：将终端区从 Slint `Image` 切换到 native render surface

目标：

- 解决整体发虚、缩放下发糊的问题
- 让 glyph 进入设备像素级合成

说明：

- Windows 先行
- 终端区域开始以 native render target 承载内容

价值：

- 这是改善“虚、糊、浮”的关键阶段

### 阶段 3：把 cursor / selection / decorations 收入 renderer

目标：

- 让 terminal canvas 内所有绘制统一归 renderer

价值：

- 解决“文字和 cursor 像两个系统画出来”的割裂感

### 阶段 4：补齐平台后端

推荐实现顺序：

1. `Windows = DirectWrite + D3D11/D2D`
2. `macOS = CoreText + Metal`
3. `Linux = Fontconfig + FreeType + OpenGL/WGPU`

说明：

- 这实现了“主走 A，但吸收 B 的 Windows-first 质量目标”

## 预期收益与见效点

### 阶段 1 可见收益

- 字符间距更稳定
- fallback/emoji 更一致
- 复杂字符与 cluster positioning 更合理

主要改善：

- “松散”

### 阶段 2 可见收益

- 缩放场景下文本不再整体发虚
- glyph 清晰度接近成熟终端产品
- blending 更稳定

主要改善：

- “虚”
- “糊”
- “浮”

### 阶段 3 可见收益

- cursor / selection / text 融合度提升
- 整体观感从“清晰的文字”升级为“成熟的终端产品”

## 风险

### 1. Slint 与 native render surface 集成复杂

风险：

- 当前终端区域深度依赖 Slint property 模型
- 将其改为 native surface host 需要仔细处理尺寸、重绘、输入映射

对策：

- 阶段 0 保留 presenter 抽象
- 阶段 2 先做单平台验证

### 2. 平台字体后端抽象可能过早设计

风险：

- DirectWrite/CoreText/FreeType 的能力模型不完全一致

对策：

- 先统一“最低可用交集”
- 将高级能力保持为 backend-specific extension

### 3. Renderer 升级周期长

风险：

- 若一次性全量替换，跨度过大

对策：

- 新旧 renderer 并存
- 以阶段化迁移替代一次性切换

## 止损路线：`libghostty`

若在以下任一情况出现后，阶段 2 仍无法稳定落地：

- Slint 原生承载 custom render surface 的复杂度持续过高
- 多平台 renderer 集成成本超出可接受范围
- 文本质量仍然难以达到产品预期

则建议启动备选：

- 保留当前窗口、tab、sidebar、session 生命周期
- terminal pane 改为嵌入 `libghostty` surface
- 由成熟 terminal core 接管 terminal canvas 的渲染与文本栈

这不是首选路线，但应作为明确的工程止损方案保留。

## 最终建议

建议采用以下工程策略：

- 架构上：走跨平台 `Ghostty / WezTerm` 式文本栈
- 实施上：优先兑现 Windows 文本质量，先做 `DirectWrite`
- 管理上：保留 `libghostty` 作为止损备选

一句话总结：

> 不再继续调 atlas 参数，而是把当前“整屏 bitmap 终端”升级为“成熟文本后端 + per-glyph renderer”的终端。

## 参考资料

- [Ghostty README](https://github.com/ghostty-org/ghostty)
- [Ghostty font backend](https://github.com/ghostty-org/ghostty/blob/main/src/font/backend.zig)
- [Ghostty shaping switch](https://github.com/ghostty-org/ghostty/blob/main/src/font/shape.zig)
- [Ghostty HarfBuzz shaper](https://github.com/ghostty-org/ghostty/blob/main/src/font/shaper/harfbuzz.zig)
- [Ghostty config reference](https://ghostty.org/docs/config/reference)
- [Windows Terminal AtlasEngine README](https://github.com/microsoft/terminal/blob/main/src/renderer/atlas/README.md)
- [WezTerm font_rasterizer](https://wezterm.org/config/lua/config/font_rasterizer.html)
- [WezTerm font shaping](https://wezterm.org/config/font-shaping.html)
- [WezTerm front_end](https://wezterm.org/config/lua/config/front_end.html)
- [Alacritty](https://github.com/alacritty/alacritty)
- [crossfont](https://github.com/alacritty/crossfont)
- [Pango rendering pipeline](https://docs.gtk.org/Pango/pango_rendering.html)
- [HarfBuzz README](https://github.com/harfbuzz/harfbuzz)

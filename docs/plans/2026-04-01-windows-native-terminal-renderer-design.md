# Windows Native Terminal Renderer Design

日期: 2026-04-01
执行者: Codex
状态: 已确认方向，待进入实现规划

## 背景

当前仓库在 Windows 发货时，终端显示质量的核心问题不在 `SSH`、远端主机或 `wezterm-term`，而在本地渲染路径：

- Windows 兼容包仍由 [`build-win-x64-software.sh`](/home/wwwroot/mica-term/build-win-x64-software.sh#L33) 固定到 `bitmap` 终端渲染模式
- 终端图像仍主要由 [`src/app/terminal_atlas.rs`](/home/wwwroot/mica-term/src/app/terminal_atlas.rs) 在 CPU 侧生成整块位图，再交给 Slint 显示
- 当前 native 路径虽然已经通过 [`src/app/terminal_presenter.rs`](/home/wwwroot/mica-term/src/app/terminal_presenter.rs#L100) 和 [`src/app/terminal_renderer/wgpu_renderer.rs`](/home/wwwroot/mica-term/src/app/terminal_renderer/wgpu_renderer.rs#L43) 建了骨架，但还没有完成真正的 GPU draw/present 闭环

这导致 Windows 版本持续出现：

- 单字符粗细和亮度漂移，尤其是 `i/l/1`
- 单词内部字距不稳定
- glyph 右侧裁边和基线观感漂移
- emoji、fallback font 和 Nerd Font 符号混排不稳定

用户已经明确约束：

- 必须保留纯原生 Rust 体验
- 不能使用 WebView 或 xterm.js
- 不能使用独立子窗口或外部 child window
- 终端必须继续嵌在 `Slint surface` 内
- Windows 为首发优先级
- 第一阶段即需要覆盖 `ligatures`、彩色 emoji、复杂 OpenType 特性

## 调研结论

### 1. 当前 `bitmap` 主线不适合继续做 Windows 长期方案

当前位图路径的主要特点是：

- fixed-grid cell metrics
- 自定义 fractional x offset
- 自定义 alpha remap
- 再由宿主 UI 合成最终位图

这类链路可以做兼容 fallback，但不适合追求 Windows 首发的高质量文本渲染。继续在这条线上微调，只会反复在同一个错误模型里修边角。

### 2. `Alacritty` 值得借的是架构，不是直接移植 renderer

`Alacritty` 的优势在于：

- `crossfont` 在 Windows 后端使用 `DirectWrite`
- renderer 结构成熟，有清晰的 glyph cache、render batch、damage tracking

但它并不是现成的可嵌入 renderer crate：

- renderer 没有被拆成独立可嵌入库
- display 子系统直接耦合 `window management + font rasterization + GPU drawing`
- 上游也明确没有把 renderer 独立化为优先方案

更重要的是，用户要求的三项能力并不能通过“直接移植 Alacritty”天然获得：
- `ligatures`：上游长期 open
- `OpenType feature selection`：上游长期 open
- `Windows colored emoji support`：上游长期 open

因此本项目不能把“直接移植 Alacritty renderer”作为目标。

### 3. 应采用 Windows-first 的原生文本栈

在用户约束下，最合适的方向是：

- 保留当前 terminal session/runtime/UI
- 借鉴成熟 GPU terminal renderer 的数据流和缓存设计
- 将 shaping/fallback/color glyph 路径改成 Windows-first 的原生文本栈
- 最终在 `Slint surface` 内完成 present

这条路既保留纯原生体验，也比继续修当前位图 atlas 更有机会接近 Windows Terminal 级别的文本观感。

## 方案对比

### 方案 A：继续修当前 `bitmap atlas` 路线

优点：

- 改动面最小
- 短期仍可保持 Linux 打包兼容

缺点：

- 架构模型错误，继续调参无法根治粗细漂移、字距漂移和裁边问题
- 第一阶段难以稳定承载 `ligatures`、彩色 emoji 和 OpenType features
- 会继续产生“为了适配位图合成而牺牲文本质量”的副作用

结论：拒绝。

### 方案 B：直接 fork / 嵌入 `Alacritty` renderer

优点：

- 可以借到成熟 terminal renderer 的一部分实现
- 架构方向比当前位图路径正确

缺点：

- renderer 与 `glutin/winit/window` 强耦合，不适合直接塞进现有 `Slint surface`
- 即便强行移植，也不能直接满足第一阶段的 `ligatures + OpenType + Windows 彩色 emoji`
- 实际改造量接近“拆解并重构半个 Alacritty display stack”

结论：拒绝。

### 方案 C：保留 `Slint surface`，重建 Windows-native renderer 主线

优点：

- 满足纯原生、同 surface 嵌入、Windows 首发的全部硬约束
- 可以显式设计 `ligatures`、emoji、fallback、OpenType 的 renderer 合同
- 保留当前 terminal core 和 UI 结构，不需要重做产品外壳
- 能将 `bitmap` 线保留为 fallback，而不是长期主线

缺点：

- 实现量较大
- 需要验证 Slint 的 graphics API/渲染通知接缝是否足够支撑自定义 GPU draw pass

结论：选择。

## 最终决策

采用新的 Windows v1 原生渲染主线：

- 终端继续嵌入当前 `Slint surface`
- 新建 Windows 文本引擎层，负责 `DirectWrite shaping + fallback + color glyph detection`
- 新建真正可 present 的 native terminal renderer，负责 atlas、batch、cursor、selection、damage 和 GPU draw
- 当前 `bitmap` 路线仅作为 fallback/兼容路径保留，不再作为 Windows 主发货路径

## 目标

Windows v1 原生渲染主线需要在第一阶段同时满足：

- 单宽终端文本稳定渲染
- `ligatures`
- 可配置 OpenType feature 开关
- Windows 彩色 emoji
- 稳定的 fallback font 混排
- 稳定 cursor、selection、underline、IME 叠加
- 继续工作在现有 `Slint surface` 内

## 非目标

- 本轮不把 Linux/macOS 与 Windows 一起做完
- 本轮不引入 Web 技术前端
- 本轮不打开独立原生子窗口
- 本轮不整块移植 `Alacritty` 或 `Tabby`
- 本轮不追求一次性覆盖所有彩色字体格式和所有脚本语言优化

## 架构设计

### 1. Terminal Core 维持不变

以下层继续保留：

- `wezterm-term` 终端状态机
- SSH/runtime/session 管理
- pane、workspace、tab、selection 外层业务逻辑
- Slint 组件树和现有产品 UI

这保证本轮重构集中在“文本渲染链”，而不是重新发明完整终端产品。

### 2. 新增 `text_engine_win`

新增 Windows 专用文本引擎层，建议收敛到 `src/app/terminal_font/` 和 `src/app/terminal_layout/` 现有 seam 内，职责为：

- 发现和加载 primary / fallback faces
- 解析用户字体配置与 OpenType feature 配置
- 进行 run segmentation 与 shaping
- 输出 glyph run、glyph advances、glyph offsets、cluster metadata
- 标记普通单色 glyph 与 color glyph
- 统一 cell metrics、baseline、line height 和 fallback 对齐规则

这一层的输出必须是结构化的 run/glyph 数据，而不是位图整帧。

### 3. 新增 `terminal_renderer_native` 真正绘制闭环

native renderer 需要完成：

- monochrome glyph atlas
- color glyph cache
- row/run batching
- 终端文本 draw pass
- selection / cursor / underline / IME overlays
- damage tracking
- frame token 与 GPU present

当前 [`WgpuTerminalRenderer`](/home/wwwroot/mica-term/src/app/terminal_renderer/wgpu_renderer.rs#L27) 只做到 prepare glyph cache，不足以承担 Windows 主发货路径，后续应扩展为真正的渲染器。

### 4. `Slint surface` bridge

当前 [`NativeTerminalSurface`](/home/wwwroot/mica-term/src/app/terminal_renderer/native_surface.rs#L19) 只负责：

- 保存 rect
- 保存 frame token
- 触发 `request_redraw`

后续需要将其扩展成真正的 surface bridge：

- 与 Slint graphics API 对接
- 接收 native renderer 的 frame state
- 在同一个 `Slint surface` 生命周期内提交 draw pass

这一步是整个方案能否成立的关键技术点。

## 文本能力设计

### Ligatures

- 不能在 atlas 合成阶段“拼接替换”
- 必须在 shaping 层产生 run/glyph 序列
- renderer 的 hit-test 和 selection 仍然必须基于 terminal grid，而不是任由视觉连字破坏单宽语义

### OpenType Features

- 第一阶段就支持 feature list 配置
- 以 `run` 为单位应用，而不是在全局位图阶段做不可控替换
- 先支持高价值特性，如 `liga`、`calt`、`ss01` 等

### 彩色 Emoji

- 不能继续和普通 alpha glyph 使用同一 coverage remap
- 需要单独的 color glyph 流程与缓存
- 需要明确 fallback 和不可用时的退化行为

### Fallback Metrics

- fallback face 不能破坏 cell grid
- 需要显式 baseline、advance、bearing 对齐合同
- 对 Sarasa、JetBrains Mono、Maple Mono、Nerd Font 符号和 emoji fallback 的混排必须有稳定规则

## 与现有代码的对应关系

### 保留

- [`src/app/ssh/runtime.rs`](/home/wwwroot/mica-term/src/app/ssh/runtime.rs)
- [`src/app/terminal_model.rs`](/home/wwwroot/mica-term/src/app/terminal_model.rs)
- [`ui/shell/terminal-session-host.slint`](/home/wwwroot/mica-term/ui/shell/terminal-session-host.slint)
- [`src/app/bootstrap.rs`](/home/wwwroot/mica-term/src/app/bootstrap.rs) 中的 workspace/session 投影主流程

### 重构

- [`src/app/terminal_font/windows_dwrite.rs`](/home/wwwroot/mica-term/src/app/terminal_font/windows_dwrite.rs)
- [`src/app/terminal_layout/shaper.rs`](/home/wwwroot/mica-term/src/app/terminal_layout/shaper.rs)
- [`src/app/terminal_presenter.rs`](/home/wwwroot/mica-term/src/app/terminal_presenter.rs)
- [`src/app/terminal_renderer/wgpu_renderer.rs`](/home/wwwroot/mica-term/src/app/terminal_renderer/wgpu_renderer.rs)
- [`src/app/terminal_renderer/native_surface.rs`](/home/wwwroot/mica-term/src/app/terminal_renderer/native_surface.rs)

### 降级为 fallback

- [`src/app/terminal_atlas.rs`](/home/wwwroot/mica-term/src/app/terminal_atlas.rs)
- [`build-win-x64-software.sh`](/home/wwwroot/mica-term/build-win-x64-software.sh)

## 主要风险

### 1. Slint graphics API 接缝风险

如果 Slint 当前公开的 rendering notifier / graphics API 不足以注入稳定的 native draw pass，那么需要先补 bridge 方案。这是最大的技术风险。

### 2. Windows color glyph 路径复杂度

彩色 emoji 在 Windows 上的字形输出格式、fallback 和 GPU 上传路径复杂度高于普通 alpha glyph，需要专门设计缓存和格式转换。

### 3. Grid 语义与复杂文本能力冲突

`ligatures`、fallback、emoji、double-width/CJK 和 terminal selection/hit-test 天然存在张力。必须先定义 renderer 合同，否则会把“视觉正确”和“交互正确”再次做崩。

## 验证标准

### 视觉质量

- `i/l/1` 不再忽明忽暗、忽粗忽细
- 常见英文单词内部 spacing 稳定
- glyph 不再出现系统性右侧裁边
- Sarasa 为主字体时观感接近成熟 Windows 终端

### 字体能力

- `ligatures` 可控且不破坏 grid 交互
- 彩色 emoji 在 Windows 上可见且稳定
- OpenType features 可配置并能实际影响输出
- fallback font 混排不再导致基线和宽度明显飘移

### 交互正确性

- SSH 到远端运行 `vim/tmux/htop/lazygit` 时 cursor、selection、scroll 和鼠标坐标稳定
- wide char、emoji、ligature 不破坏 terminal hit-test
- IME 预编辑区域和光标叠加稳定

## 后续

下一步进入实现规划，按以下顺序拆任务：

- 先验证 Slint-native surface bridge
- 再落文本引擎与 renderer 合同
- 最后替换 Windows 主 presenter 与发货路径

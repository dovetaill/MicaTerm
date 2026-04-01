# WezTerm Font Adoption Design

日期: 2026-03-31
执行者: Codex
状态: 已确认方向，进入第一阶段实现

## 背景

当前仓库的终端文字问题，不是 `SSH`、远端 Linux 或 `wezterm-term` 本身导致的，而是本地渲染路径还停留在过渡方案：

- 终端核心使用 [`wezterm-term`](/home/wwwroot/mica-term/src/app/ssh/runtime.rs#L22)
- 终端显示仍由 [`src/app/terminal_atlas.rs`](/home/wwwroot/mica-term/src/app/terminal_atlas.rs) 生成整张位图
- UI 再用 [`TerminalSessionHost`](/home/wwwroot/mica-term/ui/shell/terminal-session-host.slint#L820) 的 `Image` 显示位图
- Windows native 渲染线目前只有分层骨架，没有真正完成 present 闭环

这意味着：

- 终端状态机已经足够成熟
- 文本显示栈仍然不成熟
- 继续在当前 atlas 图片路径上微调，只能缓解，不能从根上追平 WezTerm / Windows Terminal

用户已明确接受新的方向：

- 不引入 `libghostty`
- 不整块接入 `wezterm-gui`
- 优先把 WezTerm 成熟的字体发现、shaping、rasterization 能力带进当前仓库
- Windows 版本优先

## 调研结论

### 1. 许可证没有障碍

WezTerm 仓库使用 `MIT` 许可证，可以复制、修改和合并代码，只需保留许可声明。

来源：

- <https://github.com/wezterm/wezterm/blob/main/LICENSE.md>

### 2. `wezterm-gui` 不能直接当作 drop-in renderer

`wezterm-gui` 的渲染代码并不是独立的 renderer crate，而是直接耦合：

- `window`
- `config`
- `mux::pane`
- `TermWindow`
- selection / tab bar / pane layout
- OpenGL / WebGPU 前端状态

因此不能简单地“把 render 目录拷过来”。

来源：

- <https://github.com/wezterm/wezterm/blob/main/wezterm-gui/src/termwindow/render/mod.rs>
- <https://github.com/wezterm/wezterm/blob/main/wezterm-gui/src/renderstate.rs>
- <https://github.com/wezterm/wezterm/blob/main/wezterm-gui/src/termwindow/webgpu.rs>
- <https://github.com/wezterm/wezterm/blob/main/wezterm-gui/Cargo.toml>

### 3. `wezterm-font` 是更合适的第一阶段切入点

`wezterm-font` 自身就是一个可独立复用的字体层，内部已经包含：

- font database / locator
- HarfBuzz shaping
- rasterizer 抽象
- fallback 逻辑
- metrics / glyph info / glyph raster output

这正好对应当前仓库最薄弱的地方。

来源：

- <https://github.com/wezterm/wezterm/blob/main/wezterm-font/src/lib.rs>
- <https://github.com/wezterm/wezterm/blob/main/wezterm-font/src/shaper/mod.rs>
- <https://github.com/wezterm/wezterm/blob/main/wezterm-font/src/rasterizer/mod.rs>
- <https://github.com/wezterm/wezterm/blob/main/wezterm-font/Cargo.toml>

## 目标

第一阶段只解决“把成熟文字栈接进来”：

- 继续保留当前 `wezterm-term` 会话、输入、滚动、选区和外层 Slint UI
- 不尝试整块移植 `wezterm-gui`
- 将当前伪 `DirectWrite` / 自研字体路径替换为 WezTerm 字体后端适配层
- 给后续两条路都留接口：
  - bitmap 渲染路径改用 WezTerm 字体栈输出 glyph
  - native renderer 后续直接消费同一套 shaped / rasterized glyph 数据

## 非目标

- 本轮不移植 `wezterm-gui`
- 本轮不替换整个窗口系统
- 本轮不完成完整 GPU atlas renderer 迁移
- 本轮不处理 Linux/macOS 全量一致性
- 本轮不同时引入 `libghostty`

## 方案对比

### 方案 A：整块移植 `wezterm-gui` renderer

优点：

- 理论上最接近 WezTerm 现成观感

缺点：

- 与 `window/config/mux/TermWindow` 高耦合
- 实际改动面接近重新嵌入半个 WezTerm GUI
- 与当前 Slint 宿主关系冲突明显

结论：拒绝。

### 方案 B：先引入 `wezterm-font`，替换当前字体与 shaping 栈

优点：

- 切口小
- 直接打到当前最痛的文字质量问题
- 能同时服务 bitmap 路径和后续 native 路径
- 风险显著低于整块移植 `wezterm-gui`

缺点：

- 第一阶段还不会立刻得到 WezTerm 完整 GUI renderer 的全部能力
- 仍需要本仓库自己处理 pane 合成与呈现

结论：选择。

## 最终决策

采用分阶段方案：

### 第一阶段

先在当前仓库内建立本地 `WeztermFontSystem` 适配层，并锁定上游对齐文件。

目标：

- 不再把当前 `windows_dwrite.rs` 伪装成成熟 Windows 字体后端的长期终点
- 在本仓库内建立本地适配边界：
  - WezTerm 字体配置包装
  - glyph shaping 包装
  - glyph raster 输出包装
- 固化当前真实阻塞：
  - `wezterm-font` 与现有 `harfbuzz_rs` 不能直接并存，因为都会链接原生 `harfbuzz`
- 先在代码结构上替换掉“继续围绕 `ab_glyph` 做长期主线”的假设

### 第二阶段

让 bitmap atlas 路径改用 WezTerm 字体层输出 glyph。

目标：

- 先改善 Windows 首发版本的文字观感
- 不等待 native renderer 完整闭环

### 第三阶段

再决定是否继续借鉴 WezTerm 的 atlas / quad / WebGPU 设计，或完成自己的 native pane renderer。

## 架构边界

第一阶段新增一层 `WeztermFontSystem`，职责仅限于字体发现、shaping、rasterization：

- 输入：
  - 终端文本
  - 样式信息
  - DPI / 字号
- 输出：
  - metrics
  - shaped glyph runs
  - rasterized glyph bitmaps

这一层不直接拥有：

- pane 布局
- 鼠标交互
- 选区 overlay
- Slint 组件树
- GPU present

这样可以保证：

- 当前 UI 结构不被一次性推翻
- 后续无论继续 bitmap 还是 native，都复用同一套文字栈

## 实施原则

- 优先替换字体后端，不直接移植 GUI renderer
- 优先保留现有 terminal pane 宿主
- 优先建立最小可编译、可测试的适配层
- Windows 优先，但接口不能写死为 Windows-only

## 预期结果

第一阶段完成后，仓库将具备两个关键变化：

- 代码层面不再继续围绕“自研伪 DirectWrite + bitmap 图片”为长期主路径
- 后续可以真正开始做“借 WezTerm 文字栈改善现有渲染质量”，而不是继续在现有 atlas 参数上反复试错

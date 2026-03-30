# Maple Atlas Terminal Renderer Design

日期: 2026-03-30  
执行者: Codex  
状态: 已确认，进入实现

## 背景

当前终端路径的内存和渲染压力，不是单一字体文件大小造成的，而是三类问题叠加：

- 终端 UI 在 [ui/shell/terminal-session-host.slint](/home/wwwroot/mica-term/ui/shell/terminal-session-host.slint) 中按 `session-cells` 为每个 cell 创建 `Rectangle + Text`
- 终端运行时在 [src/app/ssh/runtime.rs](/home/wwwroot/mica-term/src/app/ssh/runtime.rs) 的 `TerminalSurfaceState` 中同时保留 `visible_rows`、`visible_lines`、`cells` 多份投影
- UI 同步层在 [src/app/bootstrap.rs](/home/wwwroot/mica-term/src/app/bootstrap.rs) 将 `cells` 再次拷贝进 Slint model

这意味着：

- 换成 `Maple` 可以解决字体观感，但无法单独解决异常内存占用
- 继续沿用逐 cell Slint 文本节点，即使字体换对了，也仍然会放大分配、布局、缓存和重绘成本

用户已经明确本轮边界：

- 字体统一切到 `Maple`
- 保留 Nerd Font 图标和中文混排
- 删除 `Sarasa` / `Iosevka`
- 不再使用延迟注册
- 终端渲染必须改成更接近 `xterm` / `Alacritty` / `Ghostty` 的 atlas renderer

## 目标

- 将终端主字体切换为 bundled `MapleMonoNormalNL-NF-CN`
- 从仓库和运行时路径中移除 `Sarasa` / `Iosevka` 终端方案
- 用 atlas-backed renderer 替换当前逐 cell Slint 文本节点
- 将终端内容在 UI 侧收敛为单张图像面板，而不是海量 `Text` 节点
- 保留现有终端交互能力：
  - 输入
  - 选区
  - 复制/粘贴
  - 鼠标事件
  - 滚动条
  - cursor overlay
  - context menu
- 把 atlas renderer 的数据流设计成可继续演进到 GPU/renderer notifier 路线，而不是做一次性 CPU-only 死路

## 非目标

- 本轮不引入 `xterm.js`
- 本轮不把应用整体迁移到别的 GUI 框架
- 本轮不重写 SSH/PTY/terminal state machine
- 本轮不追求一次做完 GPU backend 集成；先把 atlas renderer 核心和 UI 热路径替换完成
- 本轮不承诺一次性达到 `Alacritty` / `Ghostty` 同级性能上限；目标是先把当前错误的渲染模型替换掉

## 外部参考结论

### 1. `Tabby / xterm`

`Tabby` 的终端前端实际使用 `xterm`，并启用 `CanvasAddon` 或 `WebglAddon`，不是逐 cell 原生文本控件。

参考：

- <https://github.com/Eugeny/tabby/blob/master/tabby-terminal/src/frontends/xtermFrontend.ts>

这说明 `Tabby` 的低 UI 节点成本来自“专用终端 renderer”，不是来自某个“更省内存的字体”。

### 2. `Alacritty`

`Alacritty` 使用 `GlyphCache`、atlas、按 cell batch draw 的 OpenGL 文本渲染路径。

参考：

- <https://github.com/alacritty/alacritty/blob/master/alacritty/src/renderer/text/mod.rs>
- <https://github.com/alacritty/alacritty/blob/master/alacritty/src/renderer/text/glyph_cache.rs>

### 3. `Ghostty`

`Ghostty` 的 renderer 维护网格内容、atlas texture、row-wise 内容结构和 GPU buffer，不依赖 UI toolkit 为每个 cell 提供文本节点。

参考：

- <https://github.com/ghostty-org/ghostty/blob/main/src/renderer/cell.zig>
- <https://github.com/ghostty-org/ghostty/blob/main/src/renderer/generic.zig>

### 4. 对本仓库的直接启示

三者共同点不是“字体不同”，而是：

- 终端内容是 grid state
- 渲染层是 atlas/cache/batch
- UI 容器只负责承载 surface、输入和少量 overlay

因此本轮正确方向是：

- 把终端内容绘制从 Slint 文本树中剥离
- 让 Rust 持有字体、metrics、atlas 和 raster surface
- Slint 只吃一个终端图像结果和少量元数据

## 当前仓库约束

### 1. 当前渲染栈是 `software` / `skia-software`

[Cargo.toml](/home/wwwroot/mica-term/Cargo.toml) 当前 feature 结构和 runtime profile 已经围绕 `slint-renderer-software` 与 `slint-renderer-skia` 建模。

本轮如果直接把终端绑到 `RenderingNotifier` / OpenGL overlay，会让 atlas renderer 受 Slint backend 支持面和当前渲染模式约束，风险偏高。

### 2. 当前终端交互逻辑强绑定 `TerminalSessionHost`

[ui/shell/terminal-session-host.slint](/home/wwwroot/mica-term/ui/shell/terminal-session-host.slint) 已经承载：

- hit testing
- 选区拖拽
- scrollbar drag / jump
- paste / copy shortcut
- mouse reporting
- cursor overlay

这些交互逻辑可以继续保留，但文本绘制本体必须被移除。

### 3. 当前 `TerminalSurfaceState` 同时承担渲染和交互数据来源

[src/app/ssh/runtime.rs](/home/wwwroot/mica-term/src/app/ssh/runtime.rs) 中的 `TerminalSurfaceState` 目前包含：

- `visible_rows`
- `visible_lines`
- `cells`
- `cursor`
- viewport metadata

这套结构可以继续作为 atlas renderer 的输入，但 UI 不应该再直接持有 `cells` 的镜像。

## 方案对比

### 方案 A：只换字体，保留逐 cell Slint 文本节点

优点：

- 改动最小

缺点：

- 不解决根因
- UI 节点规模、布局成本、模型拷贝依旧存在
- 大概率继续出现“字体一重一点，整体内存立刻放大”的情况

结论：拒绝。

### 方案 B：直接做 GPU overlay / custom render notifier

优点：

- 理论性能上限最高

缺点：

- 当前仓库 renderer 组合并不适合立刻把终端绑定到 backend-specific notifier
- 会把 atlas renderer 实现与 Slint backend 细节提前耦合
- 调试面太大，第一轮很难稳定落地

结论：本轮不选。

### 方案 C：先做独立 atlas core，在 Rust 端输出单张终端 surface 图像，Slint 只负责承载图像和 overlay

做法：

- Rust 端引入终端专用字体栅格模块
- 终端 host 改为显示单张 atlas-rendered image
- UI 保留选区 / scrollbar / cursor 等 overlay
- 运行时不再向 Slint 传 `session-cells`
- 后续如果需要切到 GPU backend，可复用 atlas core 和行脏区策略

优点：

- 能直接砍掉逐 cell 文本节点
- 不依赖 backend-specific custom rendering hook
- 兼容当前 `software` / `skia-software`
- 改动路径清晰，可测试

缺点：

- 第一版 atlas surface 仍以 CPU raster 为主，性能上限不如直接 GPU path
- 需要自己维护 glyph atlas、dirty row 和 surface cache

结论：选择。

## 最终决策

选择 `方案 C`。

本轮实现一个“独立 atlas core + 单图像终端 surface”的新终端 renderer：

- 主字体：`MapleMonoNormalNL-NF-CN`
- 旧字体：彻底移除 `Sarasa` / `Iosevka`
- 终端绘制：Rust atlas renderer
- UI 承载：单张 `Image`
- overlay：继续由 Slint 处理选区、cursor、scrollbar、context menu

这条路线在当前仓库约束下，是最接近 `xterm/alacritty/ghostty` 架构、同时能实际落地的最优解。

## 字体策略

### 选择

锁定 `MapleMonoNormalNL-NF-CN` 的 `Regular` face。

理由：

- `NF`：保留 Nerd Font 图标
- `CN`：保留中文混排
- `NormalNL`：避免 ligature 给终端网格渲染引入额外不可控因素

参考：

- <https://github.com/subframe7536/maple-font/blob/variable/README_CN.md>
- <https://github.com/subframe7536/maple-font/releases/tag/v7.9>

### 使用方式

- 不再通过 `.slint` import 注册终端字体
- 字体字节由 Rust atlas renderer 直接持有
- cell metrics、glyph raster、atlas cache 全部以这个字体为唯一数据源

## 新架构设计

### 1. 新增终端 atlas core

新增一个终端渲染模块，例如：

- [src/app/terminal_atlas.rs](/home/wwwroot/mica-term/src/app/terminal_atlas.rs)

职责：

- 加载 `MapleMonoNormalNL-NF-CN` 字体字节
- 计算 terminal cell metrics
- 维护 glyph/grapheme atlas
- 将 `TerminalSurfaceState` 转换成 RGBA surface
- 追踪 dirty rows，避免整屏重栅格化

核心数据结构建议：

- `TerminalAtlasRenderer`
- `TerminalAtlasMetrics`
- `AtlasKey`
- `AtlasEntry`
- `TerminalSurfaceFrame`
- `DirtyRowSet`

### 2. atlas key 不按“字符”，而按“终端 grapheme + style bucket”

当前 runtime 的 `TerminalCellState.text` 已经是 cell 级文本单元，适合做第一版 atlas key。

建议 key 至少包含：

- `text`
- `width`（1 / 2）
- `fg style bucket`
- `bg style bucket` 不进入 glyph atlas，但参与 surface compositing

第一版不把颜色烘进 glyph atlas，只缓存 alpha mask / monochrome sprite，绘制时再着色。

这样可以：

- 减少同一文本在不同颜色下的重复缓存
- 兼容 Nerd Font 图标与中日韩字符

### 3. surface 输出为单张 RGBA 图像

新增 UI contract：

- `workspace-session-surface-image`
- `workspace-session-cell-width`
- `workspace-session-cell-height`

Rust 侧输出：

- `slint::Image`
- cell width / height
- rows / cols / viewport / cursor data

Slint host 只负责：

- 显示这张 image
- 用 metrics 做 hit testing
- 画 cursor overlay
- 画 selection overlay
- 画 scrollbar

### 4. 取消 UI 层的 `session-cells` model

当前这条链路必须移除：

- `TerminalSurfaceState.cells`
  -> `bootstrap.rs` clone
  -> `AppWindow.workspace-session-cells`
  -> `TerminalSessionHost` repeater

注意：

- runtime 内部仍可保留 `cells`，因为 atlas renderer 和 selection text 仍需要它
- 但 UI 不再持有 `cells` 副本

### 5. 收敛 UI 不必要投影

本轮同时收敛以下 UI 热路径：

- `workspace-session-visible-lines`
- `workspace-session-cells`

如果某些测试仍需要文本快照，测试应改为直接从 runtime surface state 或 atlas frame contract 读取，而不是要求 UI 维护一份文本镜像。

### 6. 选区和 cursor 继续用 overlay

不把 cursor blink 和 selection drag 烘进 atlas surface。

理由：

- 这样 atlas surface 只在终端内容变更时更新
- 鼠标拖拽选区不会触发整张 surface 重栅格化
- cursor blink 只影响少量 Slint overlay 节点

## 数据流

新的数据流：

1. SSH/runtime 生成 `TerminalSurfaceState`
2. Rust atlas renderer 接收 `TerminalSurfaceState`
3. atlas renderer 只更新 dirty rows，产出 `TerminalSurfaceFrame`
4. `bootstrap.rs` 将 image 与 metrics 同步到 `AppWindow`
5. `TerminalSessionHost` 用 image 显示终端文本，用 overlay 处理交互态

旧数据流中最重的一段：

`cells -> Vec<TerminalCellItem> -> Slint repeater`

本轮删除。

## 测试策略

### 1. 字体契约

- 终端字体改为 `MapleMonoNormalNL-NF-CN`
- 仓库中不再引用 `SarasaTermSCNerd-Regular.ttf`
- 仓库中不再引用 `IosevkaTerm-Regular.ttf`

### 2. UI 契约

- `TerminalSessionHost` 不再存在 `for cell in root.session-cells`
- 终端正文显示路径变为单张 surface image
- hit testing / selection / scrollbar callback 契约仍保留

### 3. atlas renderer 单元测试

- 相同 grapheme 复用 atlas entry
- dirty row 更新不会重建未变化行
- 宽字符 / Nerd Font icon / 中文字符 metrics 正确
- theme 切换后 surface 颜色正确

### 4. bootstrap / 集成测试

- surface 更新时 UI image 刷新
- 清空 surface 时 image 同步清空
- 终端输入/滚动后仍触发 surface seqno 变化
- copy selection 仍按 runtime selection text 正确返回

## 风险

### 1. 第一版 atlas 仍是 CPU raster

这是已接受风险，但它依然比逐 cell Slint 文本树正确得多。

### 2. glyph shaping 完整性

`MapleMonoNormalNL-NF-CN` 覆盖 Nerd Font 与 CJK，第一版按 cell-level text atlas 足以覆盖主要终端场景；复杂脚本 shaping 不是本轮重点。

### 3. 测试面较大

因为旧实现把很多 UI contract 建在 `session-cells` 上，切换后需要系统性调整 source-contract 和 bootstrap smoke tests。

## 实施摘要

本轮实施分五块：

1. 引入 `MapleMonoNormalNL-NF-CN` 并清理旧字体契约
2. 落地 atlas renderer core 与 metrics contract
3. 将 `bootstrap` / `AppWindow` / `TerminalSessionHost` 改为单图像 surface
4. 删除 `session-cells` UI 热路径和旧字体延迟注册逻辑
5. 更新测试与文档，验证 atlas renderer 成为新的终端主路径

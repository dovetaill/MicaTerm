# Terminal Interaction Polish Follow-up Design

日期: 2026-03-27
方案名: `terminal-interaction-polish-followup`
状态: 已确认，可进入 implementation planning

## 背景

`terminal-interaction-polish` 第一轮实现已经补上了基础 scrollback metadata、scrollbar callback 和部分本地快捷键，但实际体验仍然没有达到可接受水平。根据当前回归与用户截图，问题不是“主观喜好”，而是存在明确的行为和视觉缺陷：

- `Ctrl+Shift+<letter>` 除少数白名单外仍会落入远端输入路径，导致 shell 历史、方向键语义被误触发；
- 单独按 `Ctrl`、`Shift` 不应该触发任何行为，也不应该经过远端输入编码层；
- 鼠标滚轮当前每次只滚一行，和 VS Code / Windows Terminal 这类现代终端的滚动密度不在一个数量级；
- 亮色模式下终端空白区域仍使用 UI token 背景，而不是终端 palette 背景，导致“只有文本后面白、同一行其余区域仍是默认底色”的割裂感；
- 当前终端字体栈和 cell metrics 仍是原型级硬编码，字符显得虚、浮、字间距偏大，不接近 IDE 代码编辑器或成熟桌面终端观感。

本轮 follow-up 的目标是补掉这些“已经明确暴露出来的错位”，而不是继续加新功能。

## 目标

- `Ctrl+Shift` 修饰键命名空间完全从远端透传路径移除，避免再触发 shell 历史或终端控制序列；
- 单独按 `Ctrl`、`Shift` 时不触发本地动作，也不向远端发送任何输入；
- 将鼠标滚轮改为更接近 VS Code / Windows Terminal 的多行、累积式 scrollback 逻辑；
- 将终端画布空白区域背景切换为 runtime palette 投影，而不是固定 UI token；
- 将终端默认字体与 metrics 重做为统一、紧致、全平台尽量一致的代码编辑器风格；
- 继续保持 `runtime -> SessionManager -> bootstrap -> Slint` 这条投影链条，不在本轮重写 renderer。

## 非目标

- 不引入完整 terminal search UI、command palette、tab management 快捷键动作页；
- 不在本轮实现真正的 terminal font preferences 面板；
- 不做自定义 GPU terminal renderer；
- 不为每个平台分别做独立视觉系统，本轮只做统一终端默认值。

## 方案比较

### 方案 A：继续小修补

只把 `Ctrl+Shift` 误透传和滚轮一行一滚修掉，字体和亮色背景先不碰。

优点：

- 改动面最小；
- 风险最低；
- 最快止住输入层面的明显 bug。

缺点：

- 视觉问题仍然明显；
- 用户截图里最刺眼的亮色背景和字体问题不会解决；
- 终端整体质感仍然停留在原型阶段。

### 方案 B：输入与视觉一起修正

在当前架构内同时修正输入路由、scrollback wheel 行为、runtime canvas background 投影，以及终端字体/metrics。

优点：

- 能一次性修掉本轮已经暴露的核心问题；
- 仍然限制在现有 runtime / bootstrap / Slint host 体系内；
- 可以通过测试与 smoke contract 稳定约束行为。

缺点：

- 改动会同时触及 runtime、UI contract、bootstrap；
- 需要补新的回归测试来避免再次“功能看似存在、实际仍错”的情况。

### 方案 C：直接做完整 VS Code 终端仿制层

进一步引入 terminal search、find widget、multi-command whitelist、用户级滚动灵敏度配置和字体设置。

优点：

- 体验会更接近成熟 IDE 终端。

缺点：

- 范围明显扩大；
- 会把 follow-up 从修正回归变成新功能开发；
- 与当前“先把错误的行为和丑的默认值修正”不匹配。

## 最终决策

采用方案 B：输入与视觉一起修正。

## 架构决策

### 1. `Ctrl+Shift` 命名空间从远端输入路径剥离

`TerminalSessionHost` 的键盘处理逻辑将改为三层：

- 纯修饰键：`Ctrl`、`Shift`、`Alt` 单独按下时直接忽略；
- 本地终端白名单：明确命中的快捷键执行本地动作；
- 远端透传：仅允许不属于本地保留命名空间的组合继续进入 runtime 编码。

其中：

- 所有 `Ctrl+Shift+<letter>` 默认不再进入远端编码层；
- 已实现动作的组合执行本地逻辑；
- 未实现但属于常见桌面终端保留组合的按键直接吞掉，不发给远端。

本轮保留的本地命名空间：

- 已执行本地动作：
  - `Ctrl+Shift+C`
  - `Ctrl+Shift+V`
  - `Ctrl+Insert`
  - `Shift+Insert`
  - `Shift+PageUp`
  - `Shift+PageDown`
  - `Ctrl+Home`
  - `Ctrl+End`
- 已保留但本轮仅吞掉、不透传：
  - `Ctrl+Shift+F`
  - `Ctrl+Shift+P`
  - `Ctrl+Shift+T`
  - `Ctrl+Shift+W`
  - `Ctrl+Shift+R`
  - `Ctrl+Shift+K`
  - `Ctrl+Shift+N`
  - `Ctrl+Shift+O`
  - `Ctrl+Shift+L`
  - `Ctrl+Shift+B`
  - `Ctrl+Shift+A`

### 2. 滚轮采用累积式、多行 scrollback 语义

当前 `delta-y > 0 ? 1 : -1` 的实现过于粗糙。本轮将把 wheel 逻辑改为：

- 保存未消费的 wheel delta；
- 依据固定灵敏度将 wheel delta 转换为多行滚动步进，而不是一行一滚；
- 保留剩余 delta，避免触控板或高分辨率鼠标事件丢失；
- `mouse_grabbed == true` 时继续优先转发远端 mouse input，不影响 TUI 程序。

这不是逐字复制 VS Code 内部实现，但语义目标一致：

- 普通鼠标滚轮每次明显滚动多行；
- 高频、小幅度 delta 可以被累积；
- 不会出现“手感像日志 tail 而不是现代编辑器”的问题。

### 3. 终端画布背景改为 runtime palette 投影

当前空白画布使用 `ThemeTokens.terminal-canvas-surface`，而已有文字的 cell 背景使用 runtime palette resolved background，导致亮色模式下背景不连续。

本轮将：

- 在 `TerminalSurfaceState` 中投影默认终端前景/背景色；
- 让 `blank-surface`、`surface-frame` 和未被 cell 覆盖的画布区域使用 runtime palette background；
- 继续让 cell 自身背景遵守 ANSI / reverse / selection 逻辑。

这样终端空白区和文本行背景属于同一套 palette，不再出现截图里的“白色只在字符区域出现”的断裂。

### 4. 统一终端字体方案

本轮选择“固定统一方案”，而不是继续依赖操作系统各自的 monospace fallback。

设计决定：

- 终端 primary font 改为 bundled `Iosevka Term`；
- 对缺失字符仅允许少量系统 fallback；
- 同时重调默认 metrics，使终端更靠近 IDE 代码编辑器观感：
  - 更紧的 `font-size`
  - 更紧的 `cell-width`
  - 更稳定的 `cell-height`
  - 更克制的 cursor 厚度与 padding

选择 `Iosevka Term` 的原因：

- 比当前 `Cascadia/Consolas` fallback 组合更紧致；
- 观感更接近代码编辑器而不是系统控制台；
- 开源、可分发、跨平台一致性强于依赖系统默认字体；
- 终端字符宽度控制稳定，适合 SSH / TUI 场景。

### 5. 测试策略

这轮必须先补“行为失败测试”，再改实现。重点覆盖：

- `Ctrl+Shift+<letter>` 未命中本地动作时不会再进入远端输入；
- 纯修饰键不会触发输入；
- wheel 事件会按累积逻辑滚动多行；
- light mode / dark mode 下默认 canvas background 与 runtime palette 一致；
- Slint host contract 继续暴露字体与 wheel 相关属性，不在 follow-up 中被回退。

## 风险与缓解

### 风险 1：吞掉过多 `Ctrl+Shift` 组合导致用户预期不一致

缓解：

- 仅吞掉现代终端普遍保留的组合；
- 通过测试明确记录哪些组合是“本地动作”与“保留但无动作”。

### 风险 2：wheel 累积算法在不同输入设备上手感不一致

缓解：

- 先以常规鼠标为目标实现固定步进；
- 保留剩余 delta，避免高分辨率事件被完全离散化；
- 把 sensitivity 常量集中定义，方便下一轮再调。

### 风险 3：bundled font 引入资源体积和 fallback 问题

缓解：

- 仅引入 terminal 默认所需字重；
- 继续允许缺字 fallback，但不再把系统 fallback 当作 primary stack；
- 测试中锁定 `font-family` 与 metrics 配置，避免被后续改回。

## 验收标准

- `Ctrl+Shift+<letter>` 不再触发 shell 历史或方向键语义；
- 单独按 `Ctrl`、`Shift` 无行为、无远端输入；
- wheel 每次滚动明显大于 1 行，且支持平滑累积；
- light mode 终端整块背景统一，不再只在字符后显示白底；
- 终端字体和间距明显比当前更紧致、更接近 IDE 代码编辑器；
- 相关测试、`cargo check --workspace`、`cargo clippy --workspace -- -D warnings` 全部通过。

# Terminal Theme System Design

日期: 2026-03-28
方案名: `terminal-theme-system`
状态: 已确认，可进入 implementation planning

## 背景

当前终端已经完成了基础 SSH 会话、scrollback、palette 投影、copy/paste 和部分交互修复，但整体观感仍然停留在“原型终端”阶段。主要问题不是单一 bug，而是终端视觉系统本身过于初级：

- palette 仍然内嵌在 `src/app/ssh/runtime.rs` 的 `build_terminal_color_palette(...)` 里，只有一组手写 dark/light 颜色；
- dark/light 主题虽然存在，但没有形成完整的 terminal theme contract；
- 远端 PTY 仍然以 `xterm-256color` 声明，没有补 `COLORTERM=truecolor`，导致很多现代 CLI/TUI 程序不会主动启用更细腻的 truecolor 主题；
- 终端的背景、cursor、selection、scrollbar tone 仍然缺少统一设计语义；
- 视觉风格既不像 Ghostty 这种克制的现代终端，也不像 VS Code 这类开发者熟悉的工具终端。

用户目标非常明确：

- dark / light 两种模式都要可用，且都要偏开发者喜欢的风格；
- 不只是 ANSI 颜色“能看”，而是终端整体要像成熟产品；
- 可以借鉴 Ghostty、Alacritty、Rio、console-rs 等项目的优点，必要时可移植部分功能；
- 第一阶段先做主题系统，不先做大规模渲染重构。

## 目标

- 建立独立的 terminal theme preset system，不再把主题硬编码散落在 runtime 实现中；
- 定义完整的终端主题契约，覆盖 default fg/bg、cursor、selection、ANSI 16 色、scrollbar tone 等核心视觉元素；
- 提供一对正式内建主题：
  - `Mica Code Dark`
  - `Mica Code Light`
- 让暗色和亮色都具有开发者工具气质，但不简单照搬现成产品；
- 在 SSH 会话建立时补 `COLORTERM=truecolor`，同时保持 `TERM=xterm-256color` 的兼容基线；
- 明确哪些能力借鉴现有项目，哪些能力暂不移植。

## 非目标

- 本轮不做 terminal renderer 重写，不引入 GPU glyph atlas 或自定义文本批渲染器；
- 不在本轮引入用户可配置主题 UI、主题市场或主题导入功能；
- 不在本轮实现 shell integration、prompt marks、jump-to-prompt 或 command decorations；
- 不在本轮改变现有 `ThemeMode` 的全局语义，只让终端按 dark/light 模式切换不同 preset；
- 不在本轮把整体桌面 UI 字体系统一起完成，终端主题阶段只聚焦 terminal visual contract。

## 外部项目借鉴结论

### Ghostty

Ghostty 最值得借鉴的是“主题结构”和“终端能力声明”，不是直接照搬渲染实现。

直接可借的点：

- dark/light 分离主题，而不是对同一套颜色做机械翻转；
- terminal theme 是完整契约，不只是 ANSI 16 色；
- 明确区分兼容性基线和高级能力声明。

本项目对应设计：

- 我们也建立独立 preset system；
- `ThemeMode::Dark` 和 `ThemeMode::Light` 分别映射到不同主题 preset；
- 保持 `TERM=xterm-256color`，额外补 `COLORTERM=truecolor`。

本轮不借的点：

- Ghostty 自己的 renderer；
- Ghostty 的平台窗口层；
- Ghostty 的 shell integration 功能集。

参考：

- <https://ghostty.org/docs/features/theme>
- <https://ghostty.org/docs/help/terminfo>
- <https://github.com/ghostty-org/ghostty>

### Alacritty

Alacritty 最值得借鉴的是“默认值克制”和“主题资产生态”。

直接可借的点：

- 配色不花哨，但对比和层次明确；
- 主题仓库展示了大量成熟 terminal palette 的组织方式；
- 适合作为终端主题预设的参考样本库。

本项目对应设计：

- `Mica Code Dark/Light` 会吸收 VS Code 开发者熟悉感，但整体饱和度向更克制的终端风格收敛；
- 后续若支持更多 preset，可参考 Alacritty theme 的组织方法。

本轮不借的点：

- OpenGL/GPU renderer；
- window/config 整体架构。

参考：

- <https://github.com/alacritty/alacritty>
- <https://github.com/alacritty/alacritty-theme>

### Rio

Rio 值得借鉴的是“主题预设包装方式”和“更现代的终端视觉表达”。

直接可借的点：

- 主题可以作为独立资产集合存在；
- light/dark 均可作为一等主题，而不是只重视 dark mode。

本项目对应设计：

- 主题 preset 独立成模块；
- 后续可继续扩展 preset，不需要把主题逻辑继续塞回 runtime。

参考：

- <https://github.com/raphamorim/rio>
- <https://github.com/raphamorim/rio-terminal-themes>

### console-rs/console

`console-rs/console` 不是 terminal emulator，本轮只作为“工具性参考”，不作为视觉或渲染参考。

直接可借的点：

- ANSI / 宽度 / 样式处理的接口思路；
- 某些文本测量或 ANSI 清洗辅助能力未来可参考。

本轮不借的点：

- 渲染层；
- 终端视觉系统；
- SSH / PTY 架构。

参考：

- <https://github.com/console-rs/console>

## 方案比较

### 方案 A：只替换 ANSI 16 色

仅在现有 `build_terminal_color_palette(...)` 里改一套更好看的颜色，不引入新的 theme module，也不补 truecolor 声明。

优点：

- 范围最小；
- 很快能看到一些颜色改善。

缺点：

- 仍然没有 terminal theme contract；
- cursor、selection、scrollbar、search tone 等仍然分散；
- 后续想继续扩展 preset 会很别扭；
- 对“为什么还像原型”这个问题帮助有限。

### 方案 B：建立完整 terminal theme system，并映射到 dark/light

抽离独立终端主题结构，内建 `Mica Code Dark` / `Mica Code Light`，runtime 根据 `ThemeMode` 选择 preset，并在 SSH 侧补 truecolor 能力声明。

优点：

- 结构清晰，收益明显；
- 先把最影响观感的部分系统化；
- 兼容当前 `ThemeMode` 与 runtime 链路；
- 为后续主题扩展和更深层终端能力打基础。

缺点：

- 比单改 16 色多一个模块抽离；
- 需要补充测试和文档。

### 方案 C：主题系统 + shell integration + renderer 优化一起做

优点：

- 一步到位。

缺点：

- 范围明显失控；
- 会把“观感升级”混成“终端架构重做”；
- 不适合作为当前阶段。

## 最终决策

采用方案 B：先建立完整 terminal theme system，并显式补 truecolor 能力声明。

## 视觉方向

### 总体原则

- 终端观感以 VS Code 开发者熟悉感为主，但整体色彩收敛到更接近 Ghostty 的克制、中性色导向；
- dark 和 light 分开设计，不做简单翻转；
- 默认背景避免纯黑和死白，保留一点冷色调；
- ANSI 颜色要保留开发者熟悉的语义，但整体饱和度比传统终端主题更低；
- cursor、selection、scrollbar tone 与 palette 属于同一套视觉系统。

### `Mica Code Dark`

目标气质：

- 让熟悉 VS Code Dark+ 的用户觉得自然；
- 但去掉“高饱和 RGB”感，转向更稳、更高级的对比。

建议基色：

- background: `#11161d`
- foreground: `#d7dee9`
- cursor bg: `#7fb7ff`
- cursor fg: `#11161d`
- selection bg: `rgba(97, 175, 239, 0.26)`

ANSI 16 色建议：

- black: `#2a313c`
- red: `#e06c75`
- green: `#98c379`
- yellow: `#d7ba7d`
- blue: `#61afef`
- magenta: `#c792ea`
- cyan: `#56b6c2`
- white: `#b8c2d1`
- bright black: `#5d6877`
- bright red: `#f08b92`
- bright green: `#b2d98c`
- bright yellow: `#e6cb8b`
- bright blue: `#7cc3ff`
- bright magenta: `#d8a6ff`
- bright cyan: `#74cad6`
- bright white: `#eef2f7`

### `Mica Code Light`

目标气质：

- 不像旧式浅色控制台；
- 更接近 GitHub Light / VS Code Light+ 这种冷白开发工具风格。

建议基色：

- background: `#f7f9fc`
- foreground: `#1f2328`
- cursor bg: `#2563eb`
- cursor fg: `#ffffff`
- selection bg: `rgba(37, 99, 235, 0.16)`

ANSI 16 色建议：

- black: `#1f2328`
- red: `#c74e39`
- green: `#2f855a`
- yellow: `#a16207`
- blue: `#2563eb`
- magenta: `#7c3aed`
- cyan: `#0f766e`
- white: `#d8dee8`
- bright black: `#6b7280`
- bright red: `#dd6b55`
- bright green: `#3c9c6a`
- bright yellow: `#b7791f`
- bright blue: `#3b82f6`
- bright magenta: `#8b5cf6`
- bright cyan: `#0f8b83`
- bright white: `#ffffff`

## 架构设计

### 1. 新建 terminal theme module

新增一个独立模块，例如：

- `src/app/terminal_theme.rs`

职责：

- 定义 `TerminalThemePreset` 或等价结构；
- 提供 `Mica Code Dark` / `Mica Code Light`；
- 提供从 preset 到 `wezterm_term::color::ColorPalette` 的转换函数。

这样 runtime 不再直接维护 palette 常量表，只负责根据当前 `ThemeMode` 选择 preset。

### 2. 保持现有 dark/light 驱动模型

当前系统已经有 `ThemeMode`，终端主题阶段不再增加第三套主题状态。

映射关系：

- `ThemeMode::Dark` -> `Mica Code Dark`
- `ThemeMode::Light` -> `Mica Code Light`

这样不需要新增设置 UI，也不会破坏现在的主题切换路径。

### 3. SSH 终端能力声明

现有 `request_pty(...)` 继续使用 `xterm-256color` 作为兼容基线。

新增行为：

- 在 `request_shell(...)` 之前通过 SSH channel `set_env(...)` 补：
  - `COLORTERM=truecolor`

原因：

- 远端大量现代程序会优先看 `COLORTERM=truecolor`；
- 保持 `TERM=xterm-256color` 可降低兼容风险；
- 这样既不冒进，又能把 truecolor 能力尽量透露给远端环境。

### 4. 终端视觉统一项

本轮主题系统统一这些视觉元素：

- 默认前景 / 背景
- cursor fg/bg
- selection bg
- ANSI 16 色
- terminal scrollbar tone
- split / chrome tone

如果当前代码里尚未全部使用这些字段，也应先在结构里预留，避免后续继续散写 magic values。

## 数据流

1. `ThemeMode` 改变。
2. runtime 通过 terminal theme module 获取对应 preset。
3. preset 转成 `ColorPalette`。
4. `TerminalSurfaceState` 继续投影 default fg/bg、cells、cursor。
5. Slint host 根据投影后的颜色绘制 blank canvas、cell text、cursor、selection。

SSH 连接建立时：

1. 打开 session channel。
2. 请求 PTY：`TERM=xterm-256color`。
3. 通过 `set_env(...)` 请求设置 `COLORTERM=truecolor`。
4. 请求 shell。

## 测试策略

必须先补测试，再改实现。重点覆盖：

- `ThemeMode::Dark` / `ThemeMode::Light` 映射到不同 preset；
- dark/light 默认背景、前景、cursor、selection 投影正确；
- ANSI 0/7/8/15 这些在亮色模式最容易出错的颜色被正确映射；
- runtime 不再直接依赖硬编码 palette 表；
- SSH 会话在请求 shell 前尝试设置 `COLORTERM=truecolor`；
- 主题 preset 模块包含 `Mica Code Dark` / `Mica Code Light` 两套正式主题。

## 风险与缓解

### 风险 1：远端忽略或拒绝 `COLORTERM=truecolor`

缓解：

- `set_env(...)` 失败时只记录错误，不让整个会话失败；
- `TERM=xterm-256color` 继续保留作为兼容基线。

### 风险 2：light theme 再次出现 ANSI 黑白语义错误

缓解：

- 针对 ANSI `0/7/8/15` 建专门回归测试；
- 不再在 runtime 中手改索引而缺少结构化命名。

### 风险 3：主题模块抽离后字段过多，使用不一致

缓解：

- 统一通过 `TerminalThemePreset -> ColorPalette` 转换；
- runtime 和 UI 不再各自保存分散颜色常量。

## 验收标准

- dark 和 light 模式都具备更成熟的开发者终端观感；
- terminal theme 不再散落硬编码，而是来自独立 preset system；
- 远端 shell 默认获得 `COLORTERM=truecolor` 声明；
- 现有 palette 投影测试、终端交互测试、`cargo check --workspace`、`cargo clippy --workspace -- -D warnings` 保持通过；
- 第一阶段结束后，终端观感虽然尚未完成渲染层升级，但不再是一眼“原型配色”。

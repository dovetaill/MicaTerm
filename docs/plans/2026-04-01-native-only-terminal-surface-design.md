# Native-Only Terminal Surface Design

> Superseded on 2026-04-01. This document is kept only as historical context.
> The earlier "completed" framing is no longer authoritative; use `mustdo.md`, `docs/plans/2026-04-01-windows-native-terminal-surface-recovery-design.md`, `docs/plans/2026-04-01-windows-native-terminal-surface-recovery-implementation-plan.md` as the current source of truth.

日期: 2026-04-01
执行者: Codex
状态: Superseded on 2026-04-01; 保留作历史记录，不再作为当前实现事实依据

## 背景

当前终端显示问题的根源不是 SSH 会话层，而是 terminal 最终仍长期依赖 `bitmap image` 进入 UI。
这会带来：
- glyph 被位图化后再参与 UI 合成与缩放，导致发虚
- `i/l/1` 等细竖线字重不稳定
- 字距、基线、裁边在高 DPI 下漂移
- fallback font、emoji、Nerd Font 混排不稳定

当前仓库虽然已经存在 `WindowsNativePresenter`、`WgpuTerminalRenderer`、`NativeTerminalSurface` 这些骨架，但 Windows native draw 还没有真正完成，Linux 也没有对应的 native terminal surface backend。

用户新的目标已经明确：
- Windows、Linux X11、Linux Wayland 都必须支持 native-only terminal
- `bitmap` 不再作为最终产品路径保留
- 默认终端字体切换为 `Fusion-JetBrainsMapleMono`
- 可以在普通 shell 文本流上做“超出普通终端”的语义增强高亮
- 一旦进入 `alternate screen` 或 TUI app，增强高亮必须自动禁用

## 目标

构建一条新的 terminal 渲染主线：
- 终端区直接画到平台 native surface，不再经由 `session-surface-image`
- 保留当前 Slint 外壳、workspace、tab、sidebar、modal 和会话管理
- 保留 `wezterm-term` 作为终端状态机
- 使用统一 text layout/shaping/display list，再按平台后端绘制
- Windows 与 Linux 的两个宿主打包脚本都能输出 native-only 包
- 默认内置并使用 `Fusion-JetBrainsMapleMono`

## 完成态回写

当前实现已经落到完成态：
- runtime、bootstrap、Slint shell contract 已全部切到 native-only，`session-surface-image` 与 bitmap render mode 合同已移除
- `PlatformNativeSurfaceBackend` 已提供 Windows / Wayland / X11 / detached fallback 选择逻辑
- semantic overlay 已覆盖 output block(`Json` / `Xml` / `Log`) 与 input-line(`Prompt` / `Command` / `Argument` / `Option` / `Operator`) 两类增强，并在 `alternate screen`、TUI mouse-grab、非底部视口下自动禁用输入高亮
- 默认终端字体已切换为 `Fusion-JetBrainsMapleMono`，打包脚本同步携带 `OFL.txt`
- 最终交接契约见 `docs/plans/2026-04-01-native-only-terminal-surface-tdd-spec.md`

## 非目标

- 不引入 WebView 或 xterm.js
- 不把整个应用改写成另一套 GUI 框架
- 不保留长期 bitmap fallback 路径
- 不在第一阶段把智能高亮扩展到 TUI/alternate-screen
- 不在第一阶段覆盖 macOS、iOS、Android

## 核心决策

### 1. 采用 shared core + platform native surface backend

终端渲染拆成共享核心和平台后端：
- `terminal-core`
- `text-layout-core`
- `semantic-overlay`
- `platform-surface-backend`
- `slint-shell-bridge`

这样做的原因是：
- Windows、Wayland、X11 的窗口与 surface 生命周期差异很大
- 但终端状态、selection、cursor、glyph run、overlay 计算应保持共享
- 删除 bitmap 后，必须仍然有一条统一的 display list 语义层

### 2. `build-win-x64-software.sh` 也改成 native-only

这个脚本不再保留“software compatibility / bitmap fallback”语义。
但它的 native-only 含义不是“整个 UI 都必须走 Skia”，而是：
- 终端区域不再使用 bitmap terminal renderer
- 即便宿主 UI 继续走 `slint-renderer-software`，terminal 区域也必须通过平台 native surface 直接绘制

### 3. 默认字体切换为 `Fusion-JetBrainsMapleMono`

默认终端字体改成 `Fusion-JetBrainsMapleMono`，旧 bundled terminal font 删除。
首选包型：`JetBrainsMapleMono-NF-XX-XX-XX`。
原因：
- `NF` 对终端图标和开发场景有价值
- 不先使用 `HT`，避免高分屏额外模糊
- 不先使用 `NR`，避免破坏终端 2:1 宽度对齐

该字体项目仓库声明为 `OFL-1.1`，并提供 `OFL.txt`，可作为内置默认终端字体候选。
参考：
- `https://github.com/SpaceTimee/Fusion-JetBrainsMapleMono`
- `https://github.com/SpaceTimee/Fusion-JetBrainsMapleMono/blob/main/OFL.txt`

## 架构分层

### terminal-core

职责：
- `wezterm-term` 输出的 screen buffer、scrollback、cursor、selection、ANSI 属性
- 与 SSH/runtime/session manager 对接
- 为 display list 提供稳定、平台无关的数据模型

### text-layout-core

职责：
- 默认字体与 fallback font 解析
- shaping、glyph run、cluster、cell 对齐
- emoji、Nerd Font、CJK 宽度规则
- OpenType feature 开关与 ligature 策略

### semantic-overlay

职责：
- 在普通 shell 文本流中做附加高亮，不覆盖 ANSI 真值
- 输出块识别：JSON、XML、log block
- 输入行实时高亮：先用 regex/bash-aware，后续可升级到 `tree-sitter-bash`
- alternate-screen / TUI app 自动禁用

### platform-surface-backend

职责：
- 接收统一 display list
- 直接绘制文本、selection、cursor、underline、IME preview
- 管理各平台 native surface 生命周期

平台拆分：
- Windows: `DirectWrite + Direct2D/DirectComposition`
- Linux Wayland: Wayland native child/subsurface backend
- Linux X11: X11 child/native surface backend

### slint-shell-bridge

职责：
- 只负责 terminal rect、focus、input、DPI、redraw、IME 同步
- 不再承载 terminal bitmap image
- 保持 terminal 与 Slint 外壳布局和交互一致

## 渲染与状态流

统一数据流为：
1. `wezterm-term` 更新 terminal state
2. `terminal-core` 产出 row/cell/span/cursor/selection model
3. `text-layout-core` 生成 glyph run 和 display list
4. `semantic-overlay` 为普通 shell 文本流叠加额外颜色层
5. `platform-surface-backend` 直接绘制到 native surface
6. `slint-shell-bridge` 同步 rect、input、focus、IME 和 invalidation

关键原则：
- ANSI 颜色是底层真相
- 语义增强只能叠加，不允许回写终端状态
- selection 和复制仍基于 terminal grid，不因 ligature 或视觉连字破坏逻辑语义

## 删除 bitmap 的正确时机

目标是全仓库移除 bitmap，但实现顺序必须是：
1. 先建立 surface backend 抽象层
2. 先完成 Windows native backend
3. 再完成 Wayland native backend
4. 再完成 X11 native backend
5. 切 runtime profile 为 native-only
6. 最后统一删除：
- `BitmapAtlasPresenter`
- `TerminalRenderMode::Bitmap`
- `session-surface-image` 与相关 Slint contract
- fallback-to-bitmap 分支
- `software compatibility` 文案和构建语义

这意味着“最终彻底删除 bitmap”是终态目标，而不是第一天直接硬删。

## 主要风险

- Wayland native child/subsurface 生命周期与 parent 绑定最复杂
- X11 与 Wayland 的焦点、鼠标命中、IME 行为差异明显
- 若 display list 抽象不足，三套平台后端会快速分叉
- 默认字体切换会影响 cell metrics、baseline、selection geometry
- 过早删除 bitmap 会让 Linux 某一后端未完成时失去可用 terminal
- 智能高亮如果直接篡改 cell 颜色，会破坏 ANSI/TUI 正确性

## 验收标准

新的 native-only terminal surface 方案在设计层面必须保证：
- Windows、Linux X11、Linux Wayland 都存在明确 backend 路线
- `build-win-x64.sh` 与 `build-win-x64-software.sh` 都不再以 bitmap terminal 为目标
- 默认终端字体与许可证处理方案明确
- semantic overlay 的启用条件和禁用边界明确
- bitmap 删除顺序和切换门槛明确

## 结论

推荐路线是：
- 以 shared core + platform native surface backend 为核心架构
- 先完成 Windows，再完成 Wayland，再完成 X11
- 默认内置 `Fusion-JetBrainsMapleMono`
- 智能高亮作为第二层增强，始终服从 ANSI 真值和 terminal 语义
- 待三套 backend 稳定后，再统一移除 bitmap 全链路

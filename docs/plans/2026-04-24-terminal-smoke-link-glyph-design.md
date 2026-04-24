# Terminal Smoke, Link Interaction, and Glyph Fix Design

**Date:** 2026-04-24
**Branch:** `feat/terminal-visual-highlight-redesign`

## Context

当前分支已经在推进 terminal visual highlight redesign，但用户这轮新增了三类需求，且希望继续在该分支上完成：

1. 为终端补一套可复用的真实 TUI smoke 验证交付。
2. 把终端里的 HTTP/HTTPS 链接交互补齐到稳定一致的体验。
3. 修复 `drwx-----` 这类权限字符串中连续 `-` 看起来连成“大横线”的渲染问题。

与此同时，当前分支 checkpoint 并非全绿基线。提交 `5d0cfed chore: checkpoint terminal visual highlight redesign` 时的 fresh verification 结果如下：

- `cargo check --quiet`：通过
- `cargo test --test terminal_theme_selection_spec --quiet`：失败
- `cargo test --test theme_semantic_token_contract_spec --quiet`：失败
- `cargo test --test theme_terminal_redesign_spec --quiet`：通过
- `bash tests/sidebar_ui_contract_smoke.sh`：失败

这些失败属于当前 visual-highlight redesign 分支已有工作的一部分，不是这次新增设计本身引入的回归。后续实现要在这个基础上继续推进，并避免把 smoke/link/glyph 三个需求和既有主题重构进一步耦合。

## Goals

### 1. Terminal Smoke 验证交付

交付一套既能人工执行、又能重复复用的 smoke 验证资产：

- 一份手工验证清单
- 一个可执行 smoke 脚本
- 少量自动化 contract/integration 测试

重点覆盖：

- Codex、vim/nvim、less、htop/btop 等 full-screen / alternate-screen TUI
- resize 后的贴底行为
- spinner/progress 的 CR/EL 原地刷新
- 终端链接 hover / Ctrl+click 行为
- `drwx-----` 等重复符号渲染观察点

### 2. 链接交互补全

在不重做整条链路的前提下，让终端中的 `http://` / `https://` 链接交互达到一致性：

- hover 命中 URL 时能查询是否为可打开链接
- 按住 Ctrl 时显式进入“可点击打开”状态
- 鼠标指针切换为手型
- `Ctrl+左键` 真正打开链接
- 选区拖拽、普通文本点击、alt-screen/TUI 鼠标场景不误触

### 3. 重复 `-` 的终端字形修复

修复权限字符串、分隔符等典型终端内容里，连续 ASCII `-` 被绘制成视觉上过分连贯的一整条横线的问题，同时不破坏普通英文文本排版和已有 emoji / 宽字符路径。

## Non-Goals

- 不把 URL 识别升级为完整 RFC parser。
- 不引入完整 GUI E2E 自动化平台。
- 不做新一轮大范围 terminal visual redesign。
- 不在没有证据的情况下大改 font backend / shaping backend。

## Design

## A. Smoke 验证资产

### A1. 文档

新增一份 smoke checklist，按场景拆成：

- Codex-like full-screen status line
- vim/nvim / less alternate-screen 进出
- htop/btop 高频刷新
- spinner / progress / partial line rewrite
- link hover + Ctrl+click
- glyph case：`drwx-----`、`----------`、`___`、`===`

每个场景记录：

- 启动命令或替代脚本
- 要观察的 UI 点
- 通过条件
- 常见失败症状

### A2. 脚本

新增一个可执行脚本，统一输出或拉起 smoke 场景。脚本分两层：

1. 若本机存在真实命令（如 `vim`、`less`、`htop`、`codex`），优先打印推荐启动方式。
2. 若命令不存在，回退到 repo 内置的 fixture 输出场景。

脚本不负责直接验证 GUI 结果，而是负责提供稳定输入序列与人工观察步骤，避免每次靠临时命令拼凑。

### A3. 自动化回归

自动化只覆盖最关键 contract：

- smoke fixture 的 scripted terminal sequence
- URL token trimming / gating
- alt-screen 下不误触发 link hover
- glyph visual-fit 的定向回归

不追求把真实 Codex/vim/htop 全部纳入 CI。

## B. 链接交互补全

### B1. 现有链路复用

当前链接链路已落在：

- `src/app/bootstrap.rs`
- `src/app/bootstrap/workspace_terminal.rs`
- `ui/app-window.slint`
- `ui/shell/workspace-pane.slint`
- `ui/shell/terminal-session-host.slint`

本轮不重建，只补齐一致性和边界。

### B2. 交互 contract

统一为以下行为：

- hover 命中 URL：允许显示 hover 提示
- 未按 Ctrl：提示“按住 Ctrl 再点击可打开链接”
- 已按 Ctrl：进入 pointer cursor，可点击状态更明确
- 仅 `Ctrl+左键` 才真正调用打开逻辑
- 非链接文本、选区拖拽、鼠标上报给 TUI 的 alt-screen 场景，必须不误开链接

### B3. URL 识别范围

当前 `openable_url_at_surface()` 采用按 cell 回溯 token 的轻量实现；本轮只做终端常见输出的裁剪补强：

- 去掉结尾句点、右括号、逗号、分号等尾随标点
- 保留 `http://` / `https://` 作为唯一可打开 scheme
- 不扩展 mailto/file/custom scheme

这样既能覆盖命令行/日志中常见 URL，又不会把本轮范围膨胀成通用链接解析器。

## C. 重复 `-` 字形修复

### C1. 根因判断

这类问题更像 native terminal 渲染路径里的 cell-fit / visual-fit 问题，而不是 terminal model 的字符存储错误。当前 glyph 准备层已经区分：

- grid-fitted symbol
- body text

但 `-` 这样的 ASCII 终端高频符号目前没有进入 grid-fitted 路径，导致连续符号在 body-text 对齐下视觉上过分连成一体。

### C2. 最小修法

优先改 `src/app/terminal_renderer/wgpu_renderer.rs` 的 visual-fit 判定：

- 为终端高频重复符号引入更保守的 grid-fit 归类
- 定向覆盖 `-`、`_`、`=`、`~` 等终端分隔/权限符号
- 避免扩大到普通英文标点和自然语言文本

这样修的好处是：

- 作用面小于修改 shaping backend
- 更符合 terminal cell grid 的渲染语义
- 不会轻易影响 emoji / 宽字符 / Nerd Font 图标路径

### C3. 回归方式

优先用 renderer/glyph contract 断言，而不是直接做截图测试：

- 重复符号序列应进入预期 visual-fit 路径
- 或其 prepared glyph origin/advance 应保持 cell-based 落位

## Testing Strategy

### 自动化

- URL token trimming / open gating tests
- alt-screen link hover suppression test
- glyph visual-fit regression test
- smoke fixture scripted sequence tests

### 手工 smoke

- Codex：底部状态行是否贴底，进入/退出 TUI 是否干净
- vim / less：alternate-screen 进出是否留残影
- htop / btop：高频刷新是否稳定
- progress/spinner：长文本缩短后是否清尾
- link：hover、Ctrl、pointer、Ctrl+click 是否一致
- glyph：`drwx-----` 是否仍看成一整条横线

## Risks

- 当前分支基线已有主题相关红灯，新增实现必须避免把诊断噪声混进 smoke/link/glyph 新需求。
- alt-screen 鼠标协议与链接点击存在天然边界，若 gating 不清楚，容易误伤 TUI 交互。
- glyph 修复若做得过重，可能把普通 body text 也强制 grid-fit，带来新排版问题。

## Recommended Execution Order

1. 先补 smoke 脚本与手工 checklist，建立验证载体。
2. 再补链接交互 gating 和 URL token trimming 回归。
3. 最后做 glyph visual-fit 定向修复与回归。
4. 每一步都只加最小自动化覆盖，不顺手扩展成完整 UI 平台。

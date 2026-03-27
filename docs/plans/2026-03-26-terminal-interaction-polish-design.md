# Terminal Interaction Polish Design

日期: 2026-03-26
方案名: `terminal-interaction-polish`
状态: 已确认，可进入 implementation planning

## 背景

当前 SSH terminal 已经具备可用的 `wezterm-term` surface 投影能力，也已经接通了基础输入、selection、paste、mouse forwarding 与本地 scrollback。

但从产品体验看，离 `Termius`、`Tabby`、`Xshell`、`Windows Terminal` 这一层级还有明显差距，主要集中在以下几个方面：

- 本地快捷键契约不完整，`Ctrl+Shift+C/V`、`Ctrl+Insert`、`Shift+Insert` 等常用组合仍不稳定；
- `Ctrl+Shift+<letter>` 误落入远端编码路径，出现类似“按了组合键却像触发历史命令/方向键”的错误行为；
- 本地 scrollback 只有最基础的 wheel 行滚动，没有滚动条，也没有“离开底部后输入即回到底部”的终端常见语义；
- 字体栈、cell metrics、cursor 呈现与 palette 仍偏原型，终端气质不够成熟；
- 输出颜色当前主要依赖默认 ANSI 投影，亮度、对比度和主题协调性仍不够，给人“字发黑、终端不通透”的观感。

本轮设计目标不是重写终端内核，而是在现有 Rust + Slint + Tokio + `wezterm-term` 架构上，把交互细节和视觉契约打磨到成熟终端产品水位。

## 目标

- 对齐 `Windows Terminal / VS Code` 风格的本地快捷键基线；
- 修复 `Ctrl+Shift` 组合键误触发远端历史/方向键的行为；
- 为 terminal surface 增加更顺手的本地滚动、可见滚动条与“输入即回底”语义；
- 改善终端字体栈、cell metrics、cursor 与整体画面质感；
- 提升 ANSI palette 的可读性和主题一致性，使终端输出更接近成熟 IDE/terminal 的高对比观感；
- 保持现有 session/runtime/view 投影架构不被破坏，后续仍能演进到更强的自定义 renderer。

## 非目标

- 不实现真正的“IDE 语义高亮”。终端仍以远端 ANSI/TUI 输出为准；
- 不引入 shell 语法解析、命令 AST 或本地 prompt decoration；
- 不新增多 pane、split、quake mode、session profile 配置面板；
- 不重写 terminal renderer，也不替换 `wezterm-term`；
- 不在本轮实现所有商业终端快捷键，只覆盖高频、明确、符合 Windows 习惯的一组基础规则。

## 方案比较

### 方案 A：Hotfix-only

只修 `Ctrl+Shift` 误编码、补 `Ctrl+Shift+C/V`、提高 wheel 灵敏度，并在输入时强制回到底部。

优点：

- 改动面最小；
- 风险最低；
- 最快见效。

缺点：

- 没有滚动条；
- 视觉改善有限；
- 很快还会暴露出下一轮交互与审美问题。

### 方案 B：Interaction-first

在现有 runtime/session/view 分层上补齐“本地终端交互契约”，同时升级字体与 palette，但不扩大成 renderer 重写。

优点：

- 可以系统性解决这轮反馈的主要问题；
- 对现有架构侵入有限；
- 后续可以继续叠加更多高级终端行为。

缺点：

- 比单纯 hotfix 更需要补测试和状态投影；
- 需要同时改动 runtime、Slint host 与 bootstrap wiring。

### 方案 C：Terminal-shell overhaul

额外引入更完整的键盘映射层、滚动模型、拖拽滚动条、终端配置系统。

优点：

- 最接近商业终端长期形态。

缺点：

- 范围明显扩大；
- 回归风险更高；
- 与当前用户反馈的“细节打磨”目标不匹配。

## 最终决策

采用方案 B：`Interaction-first`。

## 架构决策

### 1. 终端快捷键分层

保留现有“UI 采集事件，runtime/session 负责终端语义”的总体架构，但增加明确的本地快捷键白名单。

本地拦截规则：

- `Ctrl+Shift+C`：复制 selection；
- `Ctrl+Shift+V`：本地 paste；
- `Ctrl+Insert`：复制 selection；
- `Shift+Insert`：本地 paste；
- `Shift+PageUp`：本地向上滚动；
- `Shift+PageDown`：本地向下滚动；
- `Ctrl+Shift+Home`：本地滚到最顶部；
- `Ctrl+Shift+End`：本地滚到底部；
- `Ctrl+Shift+T/W`：本轮只做预留，不在 terminal host 内误发给远端。

远端透传规则：

- 普通字符；
- `Ctrl+C`、`Ctrl+V`、`Ctrl+Z` 等 shell 常用控制组合；
- `Alt+...`；
- 真实 terminal 功能键。

关键约束：

- `Ctrl+Shift+<letter>` 不能再落入远端“伪普通输入”路径；
- 本地快捷键只在 terminal host 明确识别命中时拦截；
- 未命中的组合键继续交给现有终端编码层处理。

### 2. 输入即回底

当 terminal 处于本地 scrollback 状态且 viewport 不在底部时，以下事件必须触发 `snap-to-bottom`：

- 文本输入；
- paste；
- `Enter` / `Backspace` / `Delete`；
- 任何会产生远端写入的键盘事件；
- 远端新输出到达。

原因：

- 这符合专业终端在“用户开始继续交互”后的主流语义；
- 避免用户在旧 viewport 位置输入后，以为终端没有响应。

### 3. 本地滚动模型

保留 `mouse_grabbed` 作为本地 scrollback 与远端 wheel 透传的路由开关：

- `mouse_grabbed = true`：滚轮发给远端 TUI；
- `mouse_grabbed = false`：滚轮作用于本地 scrollback。

同时升级 scrollback 投影状态：

- 当前 viewport offset；
- 最大可滚动范围；
- 是否已经在底部；
- 滚动条 thumb 的高度与位置所需数据。

wheel 体验调整：

- 不再固定每次仅滚 1 行；
- 统一改成较轻、更像桌面终端的多行步长；
- 后续可以再按 OS 做设备差异化，但本轮先固化常量。

### 4. 滚动条

在 `TerminalSessionHost` 右侧增加轻量自绘滚动条：

- 默认低对比、hover 时增强；
- thumb 位置与高度来自 runtime 投影的 scrollback 状态；
- 初版支持点击空白区域分页跳转与拖拽 thumb；
- 不引入通用 `ScrollView`，避免和当前 terminal 自绘表面冲突。

### 5. 视觉与字体

终端字体栈调整为更接近 Windows 11 开发工具生态的 monospace 组合：

- `Cascadia Code`
- `Cascadia Mono`
- `Consolas`
- `JetBrains Mono`

视觉策略：

- 更紧凑的 cell width / height；
- 调整 terminal padding，让内容更贴近现代 terminal；
- 维持清晰的 block/bar/underline cursor；
- 保持 selection 对比度，但不过度刺眼。

### 6. ANSI palette 策略

本轮不伪造 IDE 语义高亮，只增强 ANSI palette 质量。

策略：

- dark/light 两套 palette 都重新调校；
- 保证默认 foreground/background、ANSI 16 色、cursor、selection、scrollbar 色与 app theme 一致；
- 目标风格参考 `VS Code` / `Windows Terminal` 的高对比深浅主题；
- 解决“字整体发黑、层次不够”的问题，但不破坏远端本身输出的颜色语义。

## 模块边界

### `src/app/ssh/runtime.rs`

负责：

- 维护 `TerminalSession` 本地 scrollback 状态；
- 处理输入即回底；
- 提供滚动条投影所需状态；
- 管理 terminal theme palette；
- 保持 terminal surface 作为唯一终端真值来源。

不负责：

- UI 手势识别；
- 本地 clipboard API；
- 视图级 hover/drag 呈现。

### `src/app/ssh/session_manager.rs`

负责：

- 暴露新的 scrollback 控制入口；
- 维持 session surface 更新时序；
- 保证本地滚动与远端输入仍通过现有 runtime control 边界传递。

### `src/app/bootstrap.rs`

负责：

- 接线新的 terminal scroll/shortcut callback；
- 在 active session 上调用新的 manager/runtime 能力；
- 将 scrollback 投影同步进 Slint window state。

### `ui/shell/terminal-session-host.slint`

负责：

- 本地快捷键判定；
- 鼠标滚轮、本地 scroll thumb、点击与拖拽交互；
- 呈现字体、cursor、scrollbar、selection 与 terminal cells。

不负责：

- 自己维护终端文本真值；
- 自己计算 ANSI 颜色。

## 状态流

### 输入流

`TextInput / key-pressed / pointer / scroll` -> `TerminalSessionHost` -> `AppWindow callbacks` -> `bootstrap` -> `SessionManager` -> `SessionRuntimeControl` -> `TerminalSession`

### 输出流

`SSH channel output` -> `TerminalSession.apply_remote_bytes(...)` -> `TerminalSurfaceState` -> `SessionManager` cache -> `bootstrap` projection -> `Slint models/properties`

### 本地滚动流

`wheel / scrollbar drag` -> `TerminalSessionHost` -> `bootstrap` -> `SessionManager.scroll_session_*` -> `TerminalSession` viewport update -> `surface projection refresh`

### 回底流

`text input / key input / paste / remote output` -> `TerminalSession.snap_to_bottom_if_needed()` -> 新 surface 投影 -> `Slint` 更新

## 风险与约束

- `Ctrl+Shift` 处理必须避免吞掉本该发送给 shell 的普通 `Ctrl` 组合；
- 本地滚动和远端 `mouse_grabbed` 模式要严格区分，否则会破坏 `vim` / `less` / `htop`；
- 滚动条拖拽如果完全放在 Slint 层，必须保证和 runtime 投影状态一致，不要出现 thumb 跳动；
- palette 调整必须保留 ANSI 色位含义，不能把终端变成“看起来像编辑器，但语义错乱”的伪终端；
- “像 VS Code”只体现在主题和对比度，不意味着本地解析 shell 输出做代码语义着色。

## 验收标准

- `Ctrl+Shift+C/V`、`Ctrl+Insert`、`Shift+Insert` 可稳定工作；
- `Ctrl+Shift+<letter>` 不再误触发历史命令/方向键类行为；
- 本地滚轮更顺手，且存在可见滚动条；
- 离开底部后，输入/paste/远端输出会回到底部最新；
- dark/light 主题下终端输出都保持高可读性，默认文本不再显得“整体发黑”；
- `mouse_grabbed` 模式下 wheel 仍透传远端，不破坏 TUI；
- 新行为有对应测试覆盖，且不会破坏当前 SSH session/tab/runtime 既有用例。

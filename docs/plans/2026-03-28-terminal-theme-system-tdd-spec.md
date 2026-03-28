# Terminal Theme System TDD Spec

日期: 2026-03-28
状态: implementation complete, ready for test-driven follow-up
工作树: `feature/terminal-theme-system`

## 已落地范围

- 终端配色从 `src/app/ssh/runtime.rs` 内联常量表抽离到 `src/app/terminal_theme.rs`
- 内建两个正式 preset:
  - `Mica Code Dark`
  - `Mica Code Light`
- `ThemeMode::{Dark, Light}` 现在统一映射到 terminal preset，再转换为 `wezterm_term::color::ColorPalette`
- SSH bootstrap 在 `request_pty` 成功后、`request_shell` 前尝试协商 `COLORTERM=truecolor`
- Slint 层的 terminal palette fallback 已与 runtime preset 对齐，避免回退到旧的绿色 cursor / 纯白背景默认值

## 核心 Rust 结构与接口

### `TerminalThemePreset`

文件: `src/app/terminal_theme.rs`

关键字段:

- `name: &'static str`
- `background: u32`
- `foreground: u32`
- `cursor_bg: u32`
- `cursor_fg: u32`
- `selection_bg: (u8, u8, u8, f32)`
- `ansi: [(u8, u8, u8); 16]`
- `scrollbar_thumb: (u8, u8, u8)`
- `split: (u8, u8, u8)`

关键方法:

- `to_color_palette(self) -> ColorPalette`

当前正式入口:

- `mica_code_dark() -> TerminalThemePreset`
- `mica_code_light() -> TerminalThemePreset`
- `preset_for_theme_mode(theme_mode: ThemeMode) -> TerminalThemePreset`
- `palette_for_theme_mode(theme_mode: ThemeMode) -> ColorPalette`

### runtime palette 接口

文件: `src/app/ssh/runtime.rs`

关键点:

- `SessionTerminalConfig::color_palette()` 现在只调用 `palette_for_theme_mode(self.theme_mode())`
- runtime 不再维护 terminal ANSI/default/cursor/selection 常量表

### SSH truecolor 协商接口

文件: `src/app/ssh/runtime.rs`

关键函数:

- `negotiated_terminal_environment() -> [(&'static str, &'static str); 1]`
- `negotiate_terminal_environment(channel, pending_output)`

当前协商值:

- `COLORTERM=truecolor`

当前握手顺序:

1. `channel_open_session()`
2. `request_pty(true, "xterm-256color", ...)`
3. `await_channel_success("pty")`
4. `negotiate_terminal_environment(...)`
5. `request_shell(true)`
6. `await_channel_success("shell")`

### 与上层集成的 trait

文件: `src/app/ssh/session_manager.rs`

仍然是关键集成边界:

- `SessionRuntimeControl::terminal_surface() -> Result<TerminalSurfaceState>`
- `SessionRuntimeControl::update_theme_mode(mode: ThemeMode) -> Result<Option<TerminalSurfaceState>>`
- `SessionRuntimeControl::scroll_viewport_lines(delta: i32) -> Result<TerminalSurfaceState>`
- `SessionRuntimeControl::send_text_input(...)`
- `SessionRuntimeControl::send_key_input(...)`
- `SessionRuntimeControl::send_mouse_input(...)`
- `SessionRuntimeControl::send_paste(...)`
- `SessionRuntimeControl::resize(...)`

## UI / Slint 数据链路

### palette 投影链路

1. `TerminalSurfaceState` 产出:
   - `default_fg_rgba`
   - `default_bg_rgba`
   - `cursor.fg_rgba`
   - `cursor.bg_rgba`
2. `src/app/bootstrap.rs` 把这些值写入 `AppWindow`
3. `ui/app-window.slint` 把 palette 属性转发给 `WorkspacePane`
4. `ui/shell/workspace-pane.slint` 把 palette 属性转发给 `TerminalSessionHost`
5. `ui/shell/terminal-session-host.slint` 用这些属性绘制 blank canvas 与 cursor

### 关键 Slint palette 属性

`ui/app-window.slint`

- `workspace-session-cursor-fg`
- `workspace-session-cursor-bg`
- `workspace-session-default-fg`
- `workspace-session-default-bg`

`ui/shell/workspace-pane.slint`

- `workspace-session-cursor-fg`
- `workspace-session-cursor-bg`
- `workspace-session-default-fg`
- `workspace-session-default-bg`

`ui/shell/terminal-session-host.slint`

- `session-cursor-fg`
- `session-cursor-bg`
- `session-default-fg`
- `session-default-bg`

### 关键 Slint callbacks

`AppWindow`

- `workspace-session-text-input`
- `workspace-session-key-input`
- `workspace-session-resize-requested`
- `workspace-session-copy-selection-requested`
- `workspace-session-paste-requested`
- `workspace-session-scroll-requested`
- `workspace-session-scroll-thumb-drag-requested`
- `workspace-session-scroll-jump-requested`
- `workspace-session-mouse-input`

`WorkspacePane`

- `text-input`
- `key-input`
- `surface-resize-requested`
- `copy-selection-requested`
- `paste-requested`
- `scroll-requested`
- `scroll-thumb-drag-requested`
- `scroll-jump-requested`
- `mouse-input`

`TerminalSessionHost`

- `text-input`
- `key-input`
- `surface-resize-requested`
- `copy-selection-requested`
- `paste-requested`
- `scroll-requested`
- `scroll-thumb-drag-requested`
- `scroll-jump-requested`
- `mouse-input`

下一阶段测试应继续把这些 callbacks 当作稳定 contract，不要绕过 `WorkspacePane` 直接假设 host 与 runtime 直连。

## 当前测试覆盖

### terminal theme / runtime

- `tests/terminal_session_spec.rs`
  - dark/light default fg/bg
  - dark/light cursor fg/bg
  - theme toggle 后 palette projection 刷新

- `tests/ssh_terminal_interaction_spec.rs`
  - light theme ANSI `40m`
  - light theme ANSI `107m`
  - `\x1b[0m` 后默认背景恢复到 `#f7f9fc`

### SSH bootstrap

- `tests/ssh_session_manager_spec.rs`
  - 嵌入式 `russh` server 记录 `pty -> env:COLORTERM -> shell`
  - PTY 基线仍为 `xterm-256color`

### UI / runtime contract

- `tests/bootstrap_smoke.rs`
  - `AppWindow` 接收 default fg/bg 与 cursor fg/bg

- `tests/workspace_tabs_spec.rs`
  - `AppWindow -> WorkspacePane -> TerminalSessionHost` palette 属性转发链
  - 禁止 Slint 层保留旧的 `#52ad70` cursor fallback 和旧的纯白 terminal background fallback

## 关键边缘情况

### 1. SSH env 协商失败不能阻断会话

当前行为:

- `channel.set_env(...)` 发送失败: `tracing::warn!`，继续
- 服务端返回 `ChannelMsg::Failure`: `tracing::warn!`，继续

测试建议:

- 下一阶段可补一个服务端显式拒绝 `env_request` 的回归，确认 shell 仍能成功启动

### 2. handshake 阶段的输出不能丢

`await_channel_success(...)` 会把 `Data` / `ExtendedData` 累积到 `pending_output`

影响:

- PTY/env/shell 之间若远端提前输出 banner，不应丢失
- `Connected` 后需要继续把 `pending_output` 投影进 terminal

### 3. Slint fallback 必须与 preset 同步

原因:

- runtime 尚未投影 surface 前，UI 仍可能短暂显示 fallback palette
- 如果 Slint fallback 落后于 `terminal_theme.rs`，会出现启动瞬间颜色跳变或错误对比

### 4. `ThemeMode` 目前只有两态

当前假设:

- `ThemeMode::Dark -> Mica Code Dark`
- `ThemeMode::Light -> Mica Code Light`

若未来新增 mode:

- 需要同时更新 `terminal_theme.rs`
- 需要更新 Slint fallback
- 需要更新 runtime / UI contract tests

### 5. 不要把 palette 常量重新散回 UI 或 runtime 其他位置

当前正确模式:

- terminal preset 只在 `src/app/terminal_theme.rs`
- runtime 只取 `palette_for_theme_mode(...)`
- UI 只消费投影后的颜色属性

## 下一阶段 TDD 建议

优先补充:

1. env 协商被拒绝但 shell 仍成功启动的回归
2. `TerminalThemePreset::to_color_palette()` 的更细粒度单元测试
3. dark mode ANSI `0/7/8/15` 的固定断言
4. Slint fallback palette 与 preset 一致性的源码契约测试
5. `update_theme_mode()` 在运行中切换后对 `TerminalSurfaceState` 的稳定性测试

## 本轮最终验证

已通过:

- `cargo test --test terminal_session_spec --test ssh_terminal_interaction_spec --test ssh_session_manager_spec --test bootstrap_smoke --test workspace_tabs_spec -- --nocapture`
- `cargo check --workspace`
- `cargo clippy --workspace -- -D warnings`

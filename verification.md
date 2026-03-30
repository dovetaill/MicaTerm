# Verification Reports

## Sarasa Atlas Terminal Renderer Notes

Date: 2026-03-30 CST

### Source Documents

- Design: `docs/plans/2026-03-30-maple-atlas-terminal-renderer-design.md`
- Implementation Plan: `docs/plans/2026-03-30-maple-atlas-terminal-renderer-implementation-plan.md`

### Current Contract

- `./build-win-x64.sh` is the Windows Skia wrapper and packages the `winit-skia-software` route on `x86_64-pc-windows-msvc`.
- `./build-win-x64-software.sh` is the Windows compatibility wrapper and packages the `winit-software` route.
- `./build-release.sh` keeps the Linux x64 leg on the current default path and routes the Windows GNU leg through the software compatibility wrapper because `rust-skia` does not ship `x86_64-pc-windows-gnu` Skia binaries.
- `Cargo.toml` still pins `i-slint-backend-winit` to `vendor/i-slint-backend-winit`, so the vendored Windows partial-visibility patch remains active.
- `ui/fonts/SarasaTermSCNerd-Unhinted.ttf` is now the only bundled terminal font asset.
- `src/app/terminal_atlas.rs` owns Sarasa font loading, cell metrics, sprite caching, dirty-row redraws, and `slint::Image` frame output.
- `src/app/terminal_atlas.rs` now uses `ab_glyph` so glyph parsing stays lazy instead of paying `fontdue`'s larger upfront font expansion cost.
- `ui/shell/terminal-session-host.slint` no longer renders one `Text` node per cell; it now consumes a single rendered image surface plus overlay metadata.

### Verification Status

- The focused renderer/font verification matrix completed on 2026-03-30 and the active tests passed.

## Top Status Bar Style Bugfix3 Verification

Date: 2026-03-11 14:33:42 CST

### Source Documents

- Design: `docs/plans/2026-03-11-top-status-bar-style-bugfix3-design.md`
- Implementation Plan: `docs/plans/2026-03-11-top-status-bar-style-bugfix3-implementation-plan.md`

### Commands Executed

- [x] `cargo fmt --all`
- [x] `cargo check --workspace`
- [x] `cargo test -q`
- [x] `bash tests/fluent_titlebar_assets_smoke.sh`
- [x] `bash tests/icon_svg_assets_smoke.sh`
- [x] `bash tests/top_status_bar_ui_contract_smoke.sh`
- [x] `cargo clippy --workspace -- -D warnings`

### Automated Results

- `cargo fmt --all`: passed
- `cargo check --workspace`: passed
- `cargo test -q`: passed
- `bash tests/fluent_titlebar_assets_smoke.sh`: passed
- `bash tests/icon_svg_assets_smoke.sh`: passed
- `bash tests/top_status_bar_ui_contract_smoke.sh`: passed
- `cargo clippy --workspace -- -D warnings`: passed

### Final Review Gate

- [x] Tooltip is rendered by in-window overlay
- [x] Shared tooltip remains a single instance in `Titlebar`
- [x] Button hover still flows through shared intent callbacks
- [x] `logs/titlebar-tooltip.log` is created on demand by the Rust bridge
- [x] Hover `schedule / show / close` events are emitted for debugging
- [ ] Windows 11 manual hover behavior verified against the design

### GUI Smoke Status

- [ ] `cargo run` on Windows 11
- GUI smoke was not executed in this environment.
- Environment evidence:
  - `DISPLAY=`
  - `WAYLAND_DISPLAY=`
- Manual follow-up is still required for real hover rendering, rapid button sweep, menu-open close behavior, and log inspection on Windows 11.

## Theme Toggle Window Appearance Verification

Date: 2026-03-11 06:46:34 UTC

### Source Documents

- Design: `docs/plans/2026-03-11-theme-toggle-window-appearance-design.md`
- Implementation Plan: `docs/plans/2026-03-11-theme-toggle-window-appearance-implementation-plan.md`

### Commands Executed

- [x] `cargo fmt --all`
- [x] `cargo check --workspace`
- [x] `cargo test --test window_effects --test top_status_bar_smoke --test window_shell -q`
- [x] `bash tests/window_theme_contract_smoke.sh`
- [x] `cargo clippy --workspace -- -D warnings`

### Automated Results

- `cargo fmt --all`: passed
- `cargo check --workspace`: passed
- `cargo test --test window_effects --test top_status_bar_smoke --test window_shell -q`: passed
- `bash tests/window_theme_contract_smoke.sh`: passed
- `cargo clippy --workspace -- -D warnings`: passed

### GUI Smoke Status

- [ ] `cargo run`
- GUI smoke was not executed in this environment.
- Environment evidence:
  - `Linux 6.12.57+deb13-amd64 x86_64`
  - `DISPLAY=`
  - `WAYLAND_DISPLAY=`
- No desktop-capable Windows 11 session was available for manual window interaction.

### Windows 11 Manual Checklist

- [ ] `Dark -> Light -> Dark` 正常切换，窗口整体颜色一致
- [ ] 窗口底部超出屏幕时切换，超出区域不残留旧主题色
- [ ] 窗口左侧超出屏幕时切换，超出区域不残留旧主题色
- [ ] 窗口右侧超出屏幕时切换，超出区域不残留旧主题色
- [ ] 窗口顶部超出屏幕时切换，超出区域不残留旧主题色
- [ ] 最大化后切换主题，窗口外壳与内容区一致
- [ ] 还原后切换主题，窗口外壳与内容区一致
- [ ] 重启后主题持久化与窗口原生外观一致
- [ ] Windows 不支持 backdrop 或系统关闭透明效果时，应用能平稳降级

### Notes

- 本报告仅确认自动化验证矩阵通过。
- Windows 11 手工验证尚未执行，因此当前只能确认代码路径与契约层正确，不能在本环境中宣称“窗口越界切换主题”场景已实机验证。

## System Logging Verification

Date: 2026-03-11 17:00:25 CST

### Source Documents

- Design: `docs/plans/2026-03-11-system-logging-design.md`
- Implementation Plan: `docs/plans/2026-03-11-system-logging-implementation-plan.md`

### Commands Executed

- [x] `cargo fmt`
- [x] `cargo fmt --check`
- [x] `cargo check --workspace`
- [x] `cargo clippy --workspace -- -D warnings`
- [x] `cargo test --test logging_paths --test logging_runtime --test logging_cleanup --test panic_logging -q`
- [x] `cargo test --test bootstrap_smoke --test top_status_bar_smoke -q`
- [x] `bash tests/bootstrap_logging_contract_smoke.sh`
- [x] `cargo test --tests -q`

### Automated Results

- `cargo fmt`: passed
- `cargo fmt --check`: passed
- `cargo check --workspace`: passed
- `cargo clippy --workspace -- -D warnings`: passed
- `cargo test --test logging_paths --test logging_runtime --test logging_cleanup --test panic_logging -q`: passed
- `cargo test --test bootstrap_smoke --test top_status_bar_smoke -q`: passed
- `bash tests/bootstrap_logging_contract_smoke.sh`: passed
- `cargo test --tests -q`: passed

## Windows Console Assets Context Menu Verification

Date: 2026-03-17 18:15:00 CST

### Source Documents

- Design: `docs/plans/2026-03-17-windows-console-assets-context-menu-bugfix-design.md`
- Implementation Plan: `docs/plans/2026-03-17-windows-console-assets-context-menu-bugfix-implementation-plan.md`

## Windows Console Assets Style Optimization Verification

Date: 2026-03-20 CST

### Source Documents

- 设计文档：`docs/plans/2026-03-19-windows-console-assets-style-optimization-design.md`
- 实施计划：`docs/plans/2026-03-19-windows-console-assets-style-optimization-implementation-plan.md`

### Commands Executed

- [x] `cargo test --test assets_sidebar_toolbar_spec --test assets_explorer_smoke --test assets_context_menu_spec --test assets_context_menu_smoke --test assets_modal_smoke --test shell_view_model -- --nocapture`
- [x] `bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh`
- [x] `bash tests/assets_explorer_ui_contract_smoke.sh`
- [x] `bash tests/assets_context_menu_ui_contract_smoke.sh`
- [x] `bash tests/assets_modal_ui_contract_smoke.sh`
- [x] `cargo check --workspace`
- [x] `cargo clippy --workspace -- -D warnings`

## SSH Terminal Input Rendering Verification

Date: 2026-03-26 CST

### Source Documents

- 设计文档：`docs/plans/2026-03-26-ssh-terminal-input-rendering-design.md`
- 实施计划：`docs/plans/2026-03-26-ssh-terminal-input-rendering-implementation-plan.md`
- TDD 交接：`docs/plans/2026-03-26-ssh-terminal-input-rendering-tdd-spec.md`

### Commands Executed

- [x] `cargo test --test terminal_session_spec --test ssh_terminal_interaction_spec --test ssh_session_manager_spec --test terminal_scrollback_spec --test workspace_tabs_spec -- --nocapture`
- [x] `cargo check --workspace`
- [x] `cargo clippy --workspace -- -D warnings`
- [x] `git diff --stat`

### Automated Results

- `cargo test --test terminal_session_spec --test ssh_terminal_interaction_spec --test ssh_session_manager_spec --test terminal_scrollback_spec --test workspace_tabs_spec -- --nocapture`: passed
- `cargo check --workspace`: passed
- `cargo clippy --workspace -- -D warnings`: passed
- `git diff --stat`: reviewed

### Verification Conclusions

- [x] parser 前 exact-match 横幅过滤完成
- [x] live session key encoder / structured key routing 完成
- [x] terminal-native clipboard 与 bracketed paste 完成
- [x] wheel 在 `mouse_grabbed` 与本地 scrollback 间的路由完成
- [x] palette 已跟随 app `ThemeMode` 同步
- [x] blank surface canvas 已补齐
- [x] shared terminal metrics contract 已收敛到统一 helper
- [x] 最终目标测试矩阵通过

### Manual Follow-up

- [ ] Windows 11 实机核对 Fluent 风格下的 terminal spacing、cursor、selection、theme toggle 观感
- [ ] Windows 11 实机核对 wheel / mouse_grabbed 在 `vim`、`less`、`htop` 场景下的真实交互
- [ ] 关闭窗口与 SSH session 残留任务的时序观察

### Automated Results

- `cargo test --test assets_sidebar_toolbar_spec --test assets_explorer_smoke --test assets_context_menu_spec --test assets_context_menu_smoke --test assets_modal_smoke --test shell_view_model -- --nocapture`：通过（`16 + 16 + 3 + 3 + 14 + 31` 个测试全部通过）
- `bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh`：通过
- `bash tests/assets_explorer_ui_contract_smoke.sh`：通过
- `bash tests/assets_context_menu_ui_contract_smoke.sh`：通过
- `bash tests/assets_modal_ui_contract_smoke.sh`：通过
- `cargo check --workspace`：通过
- `cargo clippy --workspace -- -D warnings`：通过

### Final Review Gate

- [x] `Flat` 模式下保留 tree controls 槽位，但交互禁用且 tooltip 明确解释
- [x] explorer pane 宽度与 activity rail 比例已收敛到 `44px + 320px`
- [x] tree / flat row 已统一为单行高密度契约
- [x] `path_hint` 仍可见，但已改为单行尾部提示
- [x] `New Folder` / `New SSH Connection` modal 已全量英文
- [x] modal 打开后的 focus 通过 `invoke_from_event_loop` + `focus-sequence` 稳定调度
- [x] context menu 已区分本地 visual hover 与 Rust `open_path`
- [x] `assets-context-menu-pointer-moved` 已接到 Rust corridor 判定链路
- [x] explorer/menu 已改用专用 hover / selected / open surface token

### GUI / Visual Risks

- [ ] 未在真实 Windows 11 GUI 会话中完成肉眼验收
- 当前环境缺少桌面显示能力：
  - `DISPLAY=`
  - `WAYLAND_DISPLAY=`
- 后续仍建议在真实窗口环境下重点检查：
  - `Flat/Tree` 切换时 toolbar ghost 按钮的视觉对比度
  - 长 `path_hint` 与长 label 并存时的截断观感
  - 多列 submenu 横向移动时的 corridor 容错边界
  - dark / light 两套主题下 hover / open / selected 的主观清晰度
- Planned Actions: `docs/plans/2026-03-17-windows-console-assets-context-menu-unimplemented-actions.md`

### Commands Executed

- [x] `bash tests/assets_context_menu_ui_contract_smoke.sh`
- [x] `cargo test --test assets_context_menu_spec --test assets_context_menu_smoke --test shell_view_model -- --nocapture`

## Terminal Interaction Polish Follow-up Verification

Date: 2026-03-27 15:25:44 CST

### Source Documents

- 设计文档：`docs/plans/2026-03-27-terminal-interaction-polish-followup-design.md`
- 实施计划：`docs/plans/2026-03-27-terminal-interaction-polish-followup-implementation-plan.md`
- TDD 交接：`docs/plans/2026-03-27-terminal-interaction-polish-followup-tdd-spec.md`

### Commands Executed

- [x] `cargo test --test terminal_session_spec --test ssh_terminal_interaction_spec --test terminal_scrollback_spec --test ssh_session_manager_spec --test bootstrap_smoke --test workspace_tabs_spec -- --nocapture`
- [x] `cargo check --workspace`
- [x] `cargo clippy --workspace -- -D warnings`

### Automated Results

- `cargo test --test terminal_session_spec --test ssh_terminal_interaction_spec --test terminal_scrollback_spec --test ssh_session_manager_spec --test bootstrap_smoke --test workspace_tabs_spec -- --nocapture`：通过
- `cargo check --workspace`：通过
- `cargo clippy --workspace -- -D warnings`：通过

### Verification Conclusions

- [x] `TerminalSurfaceState` 已稳定投影 `default_fg_rgba` / `default_bg_rgba`
- [x] blank canvas / surface frame 已跟随 runtime palette background
- [x] `Ctrl+Shift+<letter>` 保留命名空间已从远端输入路径剥离
- [x] 单独按 `Ctrl` / `Shift` / `Alt` 不再进入远端输入
- [x] wheel 已改为累积式、多行本地 scrollback，并保留 `mouse_grabbed` 远端鼠标优先级
- [x] terminal 默认字体已切换到 bundled `SarasaTermSCNerd-Unhinted`
- [x] terminal 文本主路径已切换到 Rust atlas renderer + 单张 `slint::Image`
- [x] terminal atlas 内核已从 `fontdue` 切换到 `ab_glyph`
- [x] UI 侧逐 cell `Rectangle/Text` repeater 已移除
- [x] terminal cell metrics 已由 Rust atlas renderer 直接提供给 overlay 命中测试与 resize 计算
- [x] 本轮 follow-up 目标测试矩阵全部通过

### Manual Follow-up

- [ ] Windows 11 实机核对 `SarasaTermSCNerd-Unhinted` 在常见 SSH/TUI 场景下的字宽、行高、cursor 对齐
- [ ] Windows 11 实机核对 light / dark mode 下 blank canvas 与 ANSI 背景拼接是否完全连续
- [ ] Windows 11 实机核对高分辨率触控板与普通鼠标在 wheel accumulation 下的体感差异
- [ ] 长时间会话下观察 atlas sprite cache、surface buffer 与高频输出场景下的内存曲线
- [x] `cargo check --workspace`
- [x] `cargo clippy --workspace -- -D warnings`

### Automated Results

- `bash tests/assets_context_menu_ui_contract_smoke.sh`: passed
- `cargo test --test assets_context_menu_spec --test assets_context_menu_smoke --test shell_view_model -- --nocapture`: passed
- `cargo check --workspace`: passed
- `cargo clippy --workspace -- -D warnings`: passed

### Verification Conclusions

- [x] Bootstrap no longer seeds demo assets; `Windows Console` starts from a true empty state
- [x] Blank-area right-click is available in both empty-state and list-tail fill regions without a full-surface touch overlay
- [x] Blank-area create IA is reduced to `New Folder` and `New SSH Connection`
- [x] Toolbar and context-menu create actions now insert placeholder assets into Rust state
- [x] Placeholder assets immediately enter rename mode through `renaming_asset_id` / `renaming_asset_text`
- [x] Inline rename now bridges through `AssetsSidebar -> Sidebar -> AppWindow -> bootstrap -> ShellViewModel`
- [x] Rename draft / commit / cancel paths are covered by automated tests
- [x] Planned SSH-only actions still keep a visible feedback path instead of pretending to execute
- [x] Planned-action inventory is tracked in the dedicated markdown file

### Manual Follow-up

- [ ] Verify inline rename focus lands in the row `TextInput` on a real Windows 11 desktop session
- [ ] Verify right-click hit testing still prefers row interactions over blank-area fill in dense lists
- [ ] Verify planned-action `StatusPill` placement and legibility against the Fluent visual target
- [ ] Verify rename commit / cancel behavior feels correct with real pointer focus changes and IME input

## Windows Console Assets Context Menu Bugfix4 Verification

Date: 2026-03-19 14:10:00 CST

### Source Documents

- Design: `docs/plans/2026-03-19-windows-console-assets-context-menu-bugfix4-design.md`
- Implementation Plan: `docs/plans/2026-03-19-windows-console-assets-context-menu-bugfix4-implementation-plan.md`

### Commands Executed

- [x] `cargo test --test shell_view_model --test assets_modal_smoke --test assets_context_menu_smoke --test assets_explorer_projection --test assets_explorer_smoke --test assets_sidebar_toolbar_spec --test assets_sidebar_toolbar_smoke -q`
- [x] `bash tests/assets_modal_ui_contract_smoke.sh`
- [x] `bash tests/assets_context_menu_ui_contract_smoke.sh`
- [x] `bash tests/assets_explorer_ui_contract_smoke.sh`
- [x] `bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh`
- [x] `cargo check --workspace`
- [x] `cargo clippy --workspace -- -D warnings`

### Automated Results

- `cargo test --test shell_view_model --test assets_modal_smoke --test assets_context_menu_smoke --test assets_explorer_projection --test assets_explorer_smoke --test assets_sidebar_toolbar_spec --test assets_sidebar_toolbar_smoke -q`: passed
- `bash tests/assets_modal_ui_contract_smoke.sh`: passed
- `bash tests/assets_context_menu_ui_contract_smoke.sh`: passed
- `bash tests/assets_explorer_ui_contract_smoke.sh`: passed
- `bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh`: passed
- `cargo check --workspace`: passed
- `cargo clippy --workspace -- -D warnings`: passed

### Verification Conclusions

- [x] folder modal opens and saves correctly
- [x] SSH modal opens and saves correctly
- [x] create does not insert placeholder nodes before confirm
- [x] disclosure click expands/collapses reliably
- [x] `Flat` only shows SSH rows
- [x] `Flat` path hint is visible for SSH rows
- [x] context menu / modal / inline rename do not overlap

### Manual Follow-up

- [ ] Run a real desktop session and verify the Explorer row hit targets feel correct
- [ ] Verify large SSH modal does not close on accidental background click
- [ ] Verify `Tree` visuals are closer to VS Code Explorer than the previous prototype
- [ ] Verify `Flat` mode hides folder rows while preserving enough folder context in the hint text

## 2026-03-16 Remove Rounded Corners

Date: 2026-03-16

### Source Documents

- Design: `docs/plans/2026-03-16-remove-rounded-corners-design.md`
- Implementation Plan: `docs/plans/2026-03-16-remove-rounded-corners-implementation-plan.md`

### Contract Baseline

- `tests/window_chrome_contract_smoke.sh`
- `tests/square_component_contract_smoke.sh`
- `tests/top_status_bar_ui_contract_smoke.sh`
- `tests/window_theme_contract_smoke.sh`

### Commands Executed

- [x] `bash tests/window_chrome_contract_smoke.sh`

## 2026-03-18 - Windows Console assets explorer bugfix3

Date: 2026-03-18

### Source Documents

- Design: `docs/plans/2026-03-18-windows-console-assets-explorer-bugfix3-design.md`
- Implementation Plan: `docs/plans/2026-03-18-windows-console-assets-explorer-bugfix3-implementation-plan.md`
- TDD Handoff: `docs/plans/2026-03-18-windows-console-assets-explorer-bugfix3-tdd-spec.md`

### Commands Executed

- [x] `cargo test --test assets_explorer_projection --test shell_view_model --test assets_context_menu_spec --test assets_context_menu_smoke --test assets_sidebar_toolbar_spec --test assets_sidebar_toolbar_smoke --test assets_explorer_smoke -- --nocapture`
- [x] `bash tests/assets_context_menu_ui_contract_smoke.sh`
- [x] `bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh`
- [x] `bash tests/assets_explorer_ui_contract_smoke.sh`
- [x] `bash tests/sidebar_assets_smoke.sh`
- [x] `cargo check --workspace`
- [x] `cargo clippy --workspace -- -D warnings`

### Automated Results

- `cargo test --test assets_explorer_projection --test shell_view_model --test assets_context_menu_spec --test assets_context_menu_smoke --test assets_sidebar_toolbar_spec --test assets_sidebar_toolbar_smoke --test assets_explorer_smoke -- --nocapture`: passed
- `bash tests/assets_context_menu_ui_contract_smoke.sh`: passed
- `bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh`: passed
- `bash tests/assets_explorer_ui_contract_smoke.sh`: passed
- `bash tests/sidebar_assets_smoke.sh`: passed
- `cargo check --workspace`: passed
- `cargo clippy --workspace -- -D warnings`: passed

### Verification Conclusions

- [x] `AssetTree` tree/flat/search projection contracts are covered
- [x] blank-area click commits rename and clears selection/focus
- [x] Console toolbar create still writes root-level nodes
- [x] folder-target context create writes child nodes and auto-expands parent
- [x] Slint window model now projects `depth / has_children / expanded / focused / selected / renaming`
- [x] create popover, Explorer row, and context-menu callback contracts are covered by shell smoke scripts
- [x] `bash tests/square_component_contract_smoke.sh`
- [x] `bash tests/top_status_bar_ui_contract_smoke.sh`
- [x] `bash tests/window_theme_contract_smoke.sh`
- [x] `cargo test --test window_state_spec --test shell_view_model --test window_effects --test window_geometry_spec --test top_status_bar_smoke -q`
- [x] `cargo check --workspace`
- [x] `cargo clippy --workspace -- -D warnings`

### Automated Results

- `bash tests/window_chrome_contract_smoke.sh`: passed
- `bash tests/square_component_contract_smoke.sh`: passed
- `bash tests/top_status_bar_ui_contract_smoke.sh`: passed
- `bash tests/window_theme_contract_smoke.sh`: passed
- `cargo test --test window_state_spec --test shell_view_model --test window_effects --test window_geometry_spec --test top_status_bar_smoke -q`: passed
- `cargo check --workspace`: passed
- `cargo clippy --workspace -- -D warnings`: passed

### Verification Conclusions

- [x] `WindowChromeMode`、`chrome_mode()`、`uses_flat_window_chrome()` 与 `use-flat-window-chrome` 已从生产代码中移除
- [x] `AppWindow` 外层 `shell-frame` / `chrome-host` 已固定为 `0px` 方角，恢复态与最大化态几何一致
- [x] Windows native appearance request 现在固定输出 `CornerPreference::DoNotRound`
- [x] `ui/` 下当前已无非零 `border-radius` 残留
- [x] superseded `top-status-bar-corner-background` 文档链路已从当前分支移除

### Manual Windows Checklist

- [ ] Windows 11 restored / maximized / snapped 状态下实机确认顶层窗口不再被系统补圆角
- [ ] Mica/backdrop 与 `CornerPreference::DoNotRound` 在真实 Windows 11 桌面上无视觉冲突
- [ ] 方角后的 hover / active / focus 反馈在标题栏、侧边栏和菜单中仍然清晰

### Notes

- 本轮自动化验证全部在 Linux CLI 环境完成，未包含 Windows 11 GUI 实机检查。
- The superseded `top-status-bar-corner-background` design, implementation plan, and TDD spec were removed from this branch.

## Build Chain And Renderer Strategy Verification

Date: 2026-03-12 11:59:15 CST

### Source Documents

- Design: `docs/plans/2026-03-12-build-chain-renderer-strategy-design.md`
- Implementation Plan: `docs/plans/2026-03-12-build-chain-renderer-strategy-implementation-plan.md`

### Commands Executed

- [x] `cargo fmt --check`
- [x] `cargo check --workspace`
- [x] `cargo test --workspace -q`
- [x] `cargo clippy --workspace --all-targets -- -D warnings`
- [x] `bash tests/build_desktop_script_smoke.sh`
- [x] `bash tests/build_win_x64_script_smoke.sh`
- [x] `bash tests/build_win_x64_skia_script_smoke.sh`
- [x] `bash tests/build_release_script_smoke.sh`
- [x] `bash tests/window_theme_contract_smoke.sh`

### Automated Results

- `cargo fmt --check`: passed
- `cargo check --workspace`: passed
- `cargo test --workspace -q`: passed
- `cargo clippy --workspace --all-targets -- -D warnings`: passed
- `bash tests/build_desktop_script_smoke.sh`: passed
- `bash tests/build_win_x64_script_smoke.sh`: passed
- `bash tests/build_win_x64_skia_script_smoke.sh`: passed
- `bash tests/build_release_script_smoke.sh`: passed
- `bash tests/window_theme_contract_smoke.sh`: passed

### Build Chain Conclusions

- [x] formal linux build path documented
- [x] formal windows gnu build path documented
- [x] skia experimental path removed from mainline docs and scripts
- [x] experimental backend override removed from the formal runtime entry
- [x] `build-release.sh` exposes `fail-fast` and `best-effort`
- [x] `build_win_x64_skia_script_smoke.sh` now guards the absence of the old skia wrapper
- [x] runtime diagnostics no longer advertise the removed experimental renderer route

### Notes

- `cargo test --workspace -q` 的输出里仍会出现 `panic_hook_writes_crash_file_for_child_process ... FAILED`，这是测试子进程故意触发 panic 的既有 smoke 行为；父测试依赖该非零退出码和 `crash/` 文件完成断言，因此整条命令返回成功。
- 当前环境未执行 Windows 11 图形实机验证，因此本报告只确认自动化契约、脚本入口、日志路径和编译验证通过。

### GUI Smoke Status

- [ ] `cargo run` on Windows 11
- GUI smoke was not executed in this environment.
- Environment evidence:
  - `Linux 6.12.57+deb13-amd64 x86_64 GNU/Linux`
  - `DISPLAY=`
  - `WAYLAND_DISPLAY=`
- Manual follow-up is still required for the Windows formal package and real-machine title / log observation paths.

## Windows Theme Minimal Repro And Skia Removal Verification

Date: 2026-03-12 14:35:00 CST

### Source Documents

- Design: `docs/plans/2026-03-12-windows-theme-min-repro-and-skia-removal-design.md`
- Implementation Plan: `docs/plans/2026-03-12-windows-theme-min-repro-and-skia-removal-implementation-plan.md`

### Commands Executed

- [x] `cargo test -q`
- [x] `cargo check --workspace`
- [x] `cargo clippy --workspace -- -D warnings`
- [x] `bash tests/window_theme_contract_smoke.sh`
- [x] `bash tests/build_win_x64_skia_script_smoke.sh`
- [x] `cargo test --test windows_theme_repro_smoke -q`
- [x] `cargo check --bin windows_theme_repro`

### Automated Results

- `cargo test -q`: passed
- `cargo check --workspace`: passed
- `cargo clippy --workspace -- -D warnings`: passed
- `bash tests/window_theme_contract_smoke.sh`: passed
- `bash tests/build_win_x64_skia_script_smoke.sh`: passed
- `cargo test --test windows_theme_repro_smoke -q`: passed
- `cargo check --bin windows_theme_repro`: passed

### Verification Conclusions

- [x] 主线 Cargo feature 已不再暴露已移除的实验渲染入口
- [x] 主入口不再强制旧实验渲染 backend 环境变量
- [x] 旧 Windows Skia wrapper 脚本已删除
- [x] README 和 verification 不再把 Skia experimental 作为正式构建路径
- [x] 负向 smoke 已覆盖旧 Skia 入口不会回流
- [x] 独立最小 repro binary `windows_theme_repro` 已加入构建
- [x] 最小 repro 不依赖正式 `bootstrap`、`window_effects` 或 `AppWindow::new`

### Notes

- `cargo test -q` 的输出里仍会出现 `panic_hook_writes_crash_file_for_child_process ... FAILED`，这是测试子进程故意触发 panic 的既有 smoke 行为；父测试依赖该非零退出码和 `crash/` 文件完成断言，因此整条命令返回成功。
- 当前环境只能确认主线收敛、最小 repro 编译通过和自动化契约通过，不能替代 Windows 真机上的图形现象验证。

### Windows Manual Repro Checklist

- [ ] 在 Windows 机器执行 `cargo run --bin windows_theme_repro`
- [ ] 启动后确认窗口默认高度足够大，能够拖出屏幕底部
- [ ] 将窗口底部拖出屏幕，再点击 `Toggle Theme`
- [ ] 验证 `Dark -> Light -> Dark` 时，屏外区域是否会整体刷新
- [ ] 将窗口左侧、右侧、顶部分别拖出屏幕，再重复切换
- [ ] 若问题仍复现，记录“哪一侧超屏 + 哪次切换残留旧颜色”
- [ ] 若问题不再复现，记录标准窗口最小 repro 与正式应用行为差异

### GUI Smoke Status

- [ ] `cargo run --bin windows_theme_repro` on Windows 11
- GUI smoke was not executed in this environment.
- Environment evidence:
  - `Linux 6.12.57+deb13-amd64 x86_64 GNU/Linux`
  - `DISPLAY=`
  - `WAYLAND_DISPLAY=`
- Manual follow-up is still required for the offscreen repaint observation on Windows.

## Windows Frameless Resize Drag Verification

Date: 2026-03-12 15:20:44 CST

### Source Documents

- Design: `docs/plans/2026-03-12-windows-frameless-resize-drag-design.md`
- Implementation Plan: `docs/plans/2026-03-12-windows-frameless-resize-drag-implementation-plan.md`

### Commands Executed

- [x] `cargo fmt --check -- src/app/bootstrap.rs src/app/windowing.rs src/app/windows_frame.rs tests/top_status_bar_smoke.rs tests/window_shell.rs tests/windows_frame_spec.rs tests/window_resize_direction_spec.rs`
- [x] `cargo test --test window_shell --test window_resize_direction_spec --test windows_frame_spec --test window_geometry_spec --test top_status_bar_smoke -q`
- [x] `cargo check --workspace`
- [x] `cargo clippy --workspace -- -D warnings`
- [x] `bash tests/window_resize_drag_contract_smoke.sh`
- [x] `bash tests/windows_frame_contract_smoke.sh`
- [x] `bash tests/windows_drag_restore_contract_smoke.sh`
- [x] `bash tests/top_status_bar_ui_contract_smoke.sh`

### Automated Results

- `cargo fmt --check -- src/app/bootstrap.rs src/app/windowing.rs src/app/windows_frame.rs tests/top_status_bar_smoke.rs tests/window_shell.rs tests/windows_frame_spec.rs tests/window_resize_direction_spec.rs`: passed
- `cargo test --test window_shell --test window_resize_direction_spec --test windows_frame_spec --test window_geometry_spec --test top_status_bar_smoke -q`: passed
- `cargo check --workspace`: passed
- `cargo clippy --workspace -- -D warnings`: passed
- `bash tests/window_resize_drag_contract_smoke.sh`: passed
- `bash tests/windows_frame_contract_smoke.sh`: passed
- `bash tests/windows_drag_restore_contract_smoke.sh`: passed
- `bash tests/top_status_bar_ui_contract_smoke.sh`: passed

### Verification Conclusions

- [x] `ShellMetrics` 与 `AppWindow` 的最小尺寸 contract 一致
- [x] `WindowCommandSpec` 已导出 frameless drag / drag-resize capability
- [x] 上下左右与四角 resize direction 均具备显式 bridge
- [x] Windows frame adapter 会避开 outer resize band
- [x] titlebar drag 已切换到 `pointer-event(down)` 触发
- [x] 最终自动化验证矩阵通过

### Notes

- 全仓 `cargo fmt --check` 目前会命中仓库里若干与本 feature 无关的既有格式漂移，因此这里对本次修改涉及的 Rust 文件执行了定向格式检查，避免把无关格式化噪音带入本次合并。
- 当前环境未执行 Windows 11 图形实机验证，因此这里只能确认 Rust/Slint contract、smoke 和编译验证。

### Windows 11 Manual Checklist

- [ ] restored 状态下从上、下、左、右拖动，方向正确
- [ ] restored 状态下从四角拖动，角落拖动可同时改变宽高
- [ ] 缩小到 `688 x 640` 后继续拖动不会再变小
- [ ] 双击顶栏最大化后，再拖动顶栏空白区可恢复并继续拖动
- [ ] 点击 maximize button 最大化后，再拖动顶栏空白区可恢复并继续拖动
- [ ] snap / unsnap 后仍可 resize，且 titlebar drag 与窗口按钮不退化

### GUI Smoke Status

- [ ] `cargo run` on Windows 11
- GUI smoke was not executed in this environment.
- Environment evidence:
  - `Linux 6.12.57+deb13-amd64 x86_64 GNU/Linux`
  - `DISPLAY=`
  - `WAYLAND_DISPLAY=`
- Manual follow-up is still required for real Windows 11 resize / drag behavior.

## Assets Sidebar Toolbar Verification

Date: 2026-03-16 10:38:06 CST

### Source Documents

- Design: `docs/plans/2026-03-16-assets-sidebar-toolbar-design.md`
- Implementation Plan: `docs/plans/2026-03-16-assets-sidebar-toolbar-implementation-plan.md`

### Commands Executed

- [x] `cargo fmt`
- [x] `cargo test --test assets_sidebar_toolbar_spec --test assets_sidebar_toolbar_smoke --test shell_view_model --test sidebar_navigation_spec --test sidebar_navigation_smoke --test top_status_bar_smoke -q`
- [x] `bash tests/sidebar_assets_smoke.sh`
- [x] `bash tests/sidebar_ui_contract_smoke.sh`
- [x] `bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh`
- [x] `bash tests/shell_layout_ui_contract_smoke.sh`
- [x] `cargo check`
- [x] `cargo check --workspace`
- [x] `cargo clippy --all-targets --all-features -- -D warnings`
- [x] `cargo clippy --workspace -- -D warnings`

### Automated Results

- `cargo fmt`: passed
- `cargo test --test assets_sidebar_toolbar_spec --test assets_sidebar_toolbar_smoke --test shell_view_model --test sidebar_navigation_spec --test sidebar_navigation_smoke --test top_status_bar_smoke -q`: passed
- `bash tests/sidebar_assets_smoke.sh`: passed
- `bash tests/sidebar_ui_contract_smoke.sh`: passed
- `bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh`: passed
- `bash tests/shell_layout_ui_contract_smoke.sh`: passed
- `cargo check`: passed
- `cargo check --workspace`: passed
- `cargo clippy --all-targets --all-features -- -D warnings`: passed
- `cargo clippy --workspace -- -D warnings`: passed

### Verification Conclusions

- [x] `ShellViewModel` 暴露了 assets toolbar 的最小状态契约
- [x] toolbar Fluent SVG 资源与 sidebar 资源 smoke 已齐备
- [x] `AppWindow -> Sidebar -> AssetsSidebar` 的 property / callback 链路已贯通
- [x] 搜索行满足“空值失焦收起、非空保持展开”的现有 contract
- [x] `flat` 模式下 tree expansion toggle 保持 no-op
- [x] `Create` `PopupWindow` 存在、可打开、可显式关闭，菜单 action 选择后也会回收状态
- [x] `console` placeholder 已对 `tree / flat` 和搜索 query 产生可见差异
- [x] `cargo clippy --all-targets --all-features` 已通过；软件渲染专用 `titlebar_render_spec` 与当前软件 renderer 策略保持一致

### GUI Smoke Status

- [ ] `cargo run` on Windows 11
- GUI smoke was not executed in this environment.
- Environment evidence:
  - `DISPLAY=`
  - `WAYLAND_DISPLAY=`
- Manual follow-up is still required for toolbar hover/focus feedback, popup anchoring, click-outside close behavior, and placeholder rendering on a real desktop session.

### Notes

- Task 6 期间 `cargo clippy --all-targets --all-features -- -D warnings` 初次失败，根因是 `tests/titlebar_render_spec.rs` 依赖 `slint::platform::software_renderer`，因此后续将该测试显式绑定到软件 renderer 特性，确保验证矩阵与当前主线一致。
- 当前实现仍然只覆盖 shell contract、交互和 placeholder，不包含真实资产树、搜索过滤、创建向导、SSH/SFTP 运行时或 Tokio channel 数据流。

## 2026-03-18 - Windows Console assets context menu bugfix2

### Commands Executed

- [x] `cargo test --test assets_context_menu_spec --test assets_context_menu_smoke --test assets_sidebar_toolbar_spec --test assets_sidebar_toolbar_smoke --test shell_view_model -- --nocapture`
- [x] `bash tests/assets_context_menu_ui_contract_smoke.sh`
- [x] `bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh`
- [x] `bash tests/sidebar_tooltip_ui_contract_smoke.sh`
- [x] `bash tests/sidebar_assets_smoke.sh`
- [x] `cargo check`
- [x] `cargo check --workspace`
- [x] `cargo clippy --workspace -- -D warnings`

### Automated Results

- `cargo test --test assets_context_menu_spec --test assets_context_menu_smoke --test assets_sidebar_toolbar_spec --test assets_sidebar_toolbar_smoke --test shell_view_model -- --nocapture`: passed
- `bash tests/assets_context_menu_ui_contract_smoke.sh`: passed
- `bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh`: passed
- `bash tests/sidebar_tooltip_ui_contract_smoke.sh`: passed
- `bash tests/sidebar_assets_smoke.sh`: passed
- `cargo check`: passed
- `cargo check --workspace`: passed
- `cargo clippy --workspace -- -D warnings`: passed

### Coverage Notes

- Context menu action metadata is now flat, English, iconized, and projected through the compact Slint surface.
- Assets toolbar behavior is panel-aware, with Rust-owned descriptor copy for `console`, `snippets`, and `keychain`.
- Empty-query search dismiss is unified at shell level instead of split across local touch layers.
- Inline rename is explicit-session based and no longer depends on implicit `has-focus` blur commit.
- Default asset names now use same-type smallest-missing numbering for `Folder {n}` and `SSH Connection {n}`.

## 2026-03-20 Windows Console Assets Explorer Behavior

- disclosure state projection
- parent-scope uniqueness
- rename modal workflow
- recursive delete confirm
- focus fallback after removal
- UI contract cleanup for inline rename removal

### 2026-03-23 Re-Verification

- [x] `cargo test --test assets_explorer_projection --test shell_view_model --test assets_context_menu_spec --test assets_context_menu_smoke --test assets_modal_smoke`
- [x] `bash tests/assets_explorer_ui_contract_smoke.sh`
- [x] `bash tests/assets_context_menu_ui_contract_smoke.sh`
- [x] `bash tests/assets_modal_ui_contract_smoke.sh`
- [x] `cargo test --test assets_explorer_smoke --test assets_sidebar_toolbar_smoke`

## Windows Console Assets Persistence Verification

Date: 2026-03-23 11:34:00 CST

### Source Documents

- Design: `docs/plans/2026-03-23-windows-console-assets-persistence-design.md`
- Implementation Plan: `docs/plans/2026-03-23-windows-console-assets-persistence-implementation-plan.md`

### Commands Executed

- [x] `cargo test --test app_paths --test logging_paths --test assets_catalog_domain --test assets_catalog_store --test shell_view_model --test assets_modal_smoke --test bootstrap_smoke`
- [x] `bash tests/assets_modal_ui_contract_smoke.sh`
- [x] `bash tests/assets_persistence_contract_smoke.sh`
- [x] `cargo fmt --check`
- [x] `cargo clippy --tests -- -D warnings`
- [x] `cargo check --workspace`
- [x] `cargo clippy --workspace -- -D warnings`

### Automated Results

- `cargo test --test app_paths --test logging_paths --test assets_catalog_domain --test assets_catalog_store --test shell_view_model --test assets_modal_smoke --test bootstrap_smoke`: passed
- `bash tests/assets_modal_ui_contract_smoke.sh`: passed
- `bash tests/assets_persistence_contract_smoke.sh`: passed
- `cargo fmt --check`: passed
- `cargo clippy --tests -- -D warnings`: passed
- `cargo check --workspace`: passed
- `cargo clippy --workspace -- -D warnings`: passed

### Verification Conclusions

- [x] `Folder` 与 `SSH Connection` create modal 都预填 dash-suffix 默认名
- [x] 手动输入重名时 inline error 与 confirm disabled 正常工作
- [x] 同父级跨类型唯一约束在 create / rename 都生效
- [x] `redb` 中保存了树结构、顺序、节点类型和 SSH 字段
- [x] `expanded / search / selection / context-menu` 未进入 persisted catalog
- [x] portable marker 与标准安装模式都能解析到正确 root
- [x] `assets.redb` 缺失时能初始化空 catalog
- [x] `assets.redb` 损坏时会被隔离而不是静默覆盖
- [x] `readme.md` 已说明 working directory 不影响资产数据位置

## Terminal Interaction Polish Verification

Date: 2026-03-27 09:55:02 CST

### Source Documents

- Design: `docs/plans/2026-03-26-terminal-interaction-polish-design.md`
- Implementation Plan: `docs/plans/2026-03-26-terminal-interaction-polish-implementation-plan.md`

### Commands Executed

- [x] `cargo test --test terminal_session_spec --test ssh_terminal_interaction_spec --test terminal_scrollback_spec --test ssh_session_manager_spec --test bootstrap_smoke --test workspace_tabs_spec -- --nocapture`
- [x] `cargo check --workspace`
- [x] `cargo clippy --workspace -- -D warnings`

### Automated Results

- `cargo test --test terminal_session_spec --test ssh_terminal_interaction_spec --test terminal_scrollback_spec --test ssh_session_manager_spec --test bootstrap_smoke --test workspace_tabs_spec -- --nocapture`: passed
  - `bootstrap_smoke`: 32 passed
  - `ssh_session_manager_spec`: 20 passed
  - `ssh_terminal_interaction_spec`: 5 passed
  - `terminal_scrollback_spec`: 3 passed
  - `terminal_session_spec`: 11 passed
  - `workspace_tabs_spec`: 18 passed
- `cargo check --workspace`: passed
- `cargo clippy --workspace -- -D warnings`: passed

### Verification Conclusions

- [x] runtime surface projection now exposes `viewport_offset_lines / viewport_max_offset_lines / viewport_at_bottom`
- [x] local scrollback snaps back to bottom on terminal text input, key input, paste, and new remote output
- [x] `TerminalSessionHost` exposes the planned local shortcut and scrollbar callback contract
- [x] `SessionManager` can scroll an active session to top, bottom, or a clamped ratio target
- [x] `bootstrap` projects active viewport state into `AppWindow` and refreshes it immediately after local terminal commands

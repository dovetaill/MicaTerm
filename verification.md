# Verification Reports

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

### Logging Conclusions

- [x] 默认模式下 `ERROR` 可写入 `logs/system-error.log`
- [x] 默认模式下 `DEBUG` 事件被过滤
- [x] `env override > portable marker > standard local app data` 路径优先级测试通过
- [x] panic 子进程 smoke 会生成 `crash/panic-*.log`
- [x] cleanup 策略满足 `14 天 + 64MB`
- [x] `bootstrap` 的 `eprintln!` 已迁移为统一 `tracing::error!`
- [x] 顶部状态栏现有绑定与 Fluent SVG 资产未回归

### Notes

- `panic_logging` 的输出中会看到子进程内那次故意触发的 panic 打印为 `FAILED`，这是 smoke 的预期现象；父进程依赖该非零退出码与 `crash/` 文件完成断言，因此整体测试结果为通过。
- 当前环境未执行 GUI 实机验证，因此本报告只确认自动化契约、回归和编译验证通过。

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
- [x] skia experimental path documented
- [x] experimental backend conflict override documented
- [x] explicit skia init failure path documented
- [x] `build-release.sh` exposes `fail-fast` and `best-effort`
- [x] `build-win-x64-skia.sh` remains a pure-Skia wrapper with `CARGO_NO_DEFAULT_FEATURES=1`
- [x] experimental runtime diagnostics expose `Skia Experimental` and `winit-skia-software`

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
- Manual follow-up is still required for Windows `winit-software` 正式包、Windows MSVC `Skia Experimental` 包，以及真实机上的标题/日志观察路径。

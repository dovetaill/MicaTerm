# 内存占用缩减任务文档

日期：2026-06-09  
执行者：Codex  
状态：本轮仅完成文档任务；后续实现必须新开窗口执行

## 1. 本轮文档阶段任务清单（已完成，不含代码修改）

- 已核查本地 `docs/plans/`、`docs/wikis/`、`readme.md`、Windows 打包脚本、启动入口、terminal / font / memory 相关代码与测试。
- 已使用 MCP / 外部搜索工具调研 Microsoft、Skia、Slint、Chromium、WezTerm、Alacritty、Contour 等资料。
- 已启动多位 `gpt-5.4 xh` subagents，覆盖 Windows 内存 profiling、Rust/Slint、terminal renderer、回归守门、瘦身治理、测试发布等角色，并形成多轮辩论摘要。
- 已完成以下三份中文文档：
  - `docs/plans/2026-06-09-memory-footprint-reduction/requirements.md`
  - `docs/plans/2026-06-09-memory-footprint-reduction/designs.md`
  - `docs/plans/2026-06-09-memory-footprint-reduction/tasks.md`
- 本轮没有修改任何业务代码、构建脚本、测试代码或资源文件。

## 2. 正式实现前置要求（需要新开窗口执行）

后续正式实现必须严格遵守：

1. 新开 Codex 窗口。
2. 进入 `.worktrees/feature-memory-footprint-reduction`。
3. 使用 `superpowers:executing-plans` 逐 task 执行。
4. 每个任务开始前使用 `superpowers:test-driven-development`。
5. 遇到异常、指标矛盾或 root cause 不明时，立即使用 `superpowers:systematic-debugging`。
6. 每次声称完成前，使用 `superpowers:verification-before-completion`。

建议起始命令：

```bash
git worktree add .worktrees/feature-memory-footprint-reduction -b feature/memory-footprint-reduction
```

说明：当前仓库 `.worktrees/` 已存在，且已被 `.gitignore` 忽略，适合正式实现使用。

## 3. 后续实现阶段的优先级建议

建议顺序如下：

1. 建立可复现的内存 baseline。
2. 完善或确认 `MICA_TERM_MEMORY_DIAGNOSTICS`。
3. 观测 session close / no-surface idle 后的 cache 与 retained state。
4. 复核 glyph / shape / bitmap cache 的边界与 clear 生命周期。
5. 复核 startup eager path。
6. 复核 font fallback lazy path。
7. dead-code audit 只做候选清单，不马上删除。

以下每个 task 都 **需要新开窗口执行**。

## 4. 任务 1：建立可复现的内存 baseline

- 目的
  - 为 Windows packaged mainline 与 software compatibility 两条路径建立可复现 baseline，先解释 450 MB 的构成，再决定优化顺序。
- 可能涉及文件
  - `readme.md`
  - `docs/plans/2026-06-09-memory-footprint-reduction/verification.md`（建议新增）
  - 若需补 baseline 模板，可新增同目录下的 measurement 记录文档
- 先补的测试
  - `tests/terminal_memory_diagnostics_contract_spec.rs`
  - `tests/runtime_profile.rs`
  - `tests/bootstrap_profile_smoke.rs`
  - 新增 source-contract，锁住 packaged repro 步骤、renderer/path 记录要求与日志口径
- 验证命令
  - `cargo test --test runtime_profile --test bootstrap_profile_smoke --test terminal_memory_diagnostics_contract_spec -q`
  - `bash tests/build_win_x64_script_smoke.sh`
  - `bash tests/build_win_x64_software_script_smoke.sh`
- 内存测量命令 / 清单
  - 构建 package：
    - `./build-win-x64.sh`
    - `./build-win-x64-software.sh`
  - Windows packaged 运行准备：
    ```powershell
    ni .mica-term-portable -ItemType File -Force
    $env:MICA_TERM_LOG = "debug"
    $env:MICA_TERM_MEMORY_DIAGNOSTICS = "1"
    .\mica-term.exe
    ```
  - 记录场景：
    - 冷启动空载
    - 欢迎页空载 30 秒 / 60 秒
    - 首次打开终端
    - 大量输出
    - 重滚动
    - 3 到 5 个 session
    - 关闭全部 session 后立即 / 30 秒 / 60 秒
    - 重启应用
  - 外部工具建议：
    - Process Explorer
    - VMMap
    - PerfMon / `typeperf`
  - 优先记录：
    - `Private Bytes`
    - `Page File Bytes / Commit Size`
    - `Working Set - Private`
    - `Working Set`
    - `Handle Count`
    - `Thread Count`
  - 可选命令示例：
    ```powershell
    typeperf "\Process(mica-term)\Private Bytes" "\Process(mica-term)\Page File Bytes" "\Process(mica-term)\Working Set - Private" "\Process(mica-term)\Working Set" "\Process(mica-term)\Handle Count" "\Process(mica-term)\Thread Count"
    ```
- 回滚标准
  - 若 baseline 文档无法清楚区分 mainline / software compatibility / runtime fallback，则该 task 不算完成。
  - 若 measurement 仍只记录 `working set`、未记录 `private/commit`，则回滚结论并补齐口径。

## 5. 任务 2：完善或确认 `MICA_TERM_MEMORY_DIAGNOSTICS`

- 目的
  - 确认当前 `MICA_TERM_MEMORY_DIAGNOSTICS` 不只是 config / readme 开关，而是能输出足够的 runtime memory 事件；若不足，先补 observability。
- 可能涉及文件
  - `src/app/logging/config.rs`
  - `src/app/logging/runtime.rs`
  - `src/app/memory.rs`
  - `src/app/bootstrap.rs`
  - `src/app/ssh/runtime/pump.rs`
  - `readme.md`
  - `tests/terminal_memory_diagnostics_contract_spec.rs`
  - `tests/logging_runtime.rs`
- 先补的测试
  - 先让以下 contract 失败后再实现：
    - `startup-snapshot`
    - `close-shrink`
    - `idle-shrink`
    - `trim-request`
    - `trim-executed`
    - 默认关闭时保持静默
- 验证命令
  - `cargo test --test terminal_memory_diagnostics_contract_spec --test logging_runtime -q`
  - `cargo test --test runtime_profile --test bootstrap_profile_smoke -q`
- 内存测量命令 / 清单
  - 用 packaged build 复现并从日志中筛选：
    ```powershell
    Select-String -Path .\logs\system-error.log* -Pattern "app.memory","startup-snapshot","close-shrink","idle-shrink","trim-request","trim-executed"
    ```
  - 检查每条事件是否同时带有：
    - renderer/path
    - active / no-surface 状态
    - cache stats（若可得）
    - `working_set_bytes`
    - `private_usage_bytes`
    - `pagefile_usage_bytes`
- 回滚标准
  - 若 diagnostics 只能说明“发生了 trim”，却不能说明“发生前后 `private/commit` 是否变化”，则不算完成。

## 6. 任务 3：观测 session close / no-surface idle 后的 cache 与 retained state

- 目的
  - 证明 close session、clear surface、延时 no-surface idle 后，terminal / session / backend 各层到底释放了什么、保留了什么。
- 可能涉及文件
  - `src/app/bootstrap.rs`
  - `src/app/terminal_renderer/host.rs`
  - `src/app/terminal_presenter.rs`
  - `src/app/terminal_renderer/wgpu_renderer.rs`
  - `src/app/terminal_renderer/platform/windows.rs`
  - `src/app/ssh/session_manager.rs`
  - `src/app/ssh/runtime/terminal.rs`
  - `tests/bootstrap_smoke.rs`
  - `tests/ssh_session_manager_spec.rs`
  - `tests/terminal_renderer_prepare_cache_spec.rs`
- 先补的测试
  - close session 后：
    - session runtime memory release 已发生
    - registry / surface / runtime control 已删
    - presenter / host / backend cache stats 可观测
  - no-surface idle 后：
    - host 被 drop
    - backend purge 已执行
    - `working set` trim 的发生顺序可观测
- 验证命令
  - `cargo test --test bootstrap_smoke --test ssh_session_manager_spec --test terminal_renderer_prepare_cache_spec -q`
- 内存测量命令 / 清单
  - 场景：
    - 开 1 个 session，大输出，关闭该 session
    - 开 3 到 5 个 session，分别大输出，再全部关闭
  - 记录时间点：
    - 关闭立即
    - 关闭后约 1 秒
    - 关闭后 30 秒
    - 关闭后 60 秒
  - 重点核对：
    - scrollback 是否真被 release
    - renderer host 是否被 drop
    - `private/commit` 是否回到接近基线
- 回滚标准
  - 若 close-path 改动影响 SSH / SFTP / session 关闭语义，立即回滚。
  - 若只能让 `working set` 掉下去、`private/commit` 无明显变化，则不进入默认优化结论。

## 7. 任务 4：复核 glyph / shape / bitmap cache 的边界与 clear 生命周期

- 目的
  - 在不破坏滚动、输入、重绘体验的前提下，复核有界 cache 与 bitmap atlas retained state 是否存在“边界过松”问题。
- 可能涉及文件
  - `src/app/terminal_presenter.rs`
  - `src/app/terminal_renderer/wgpu_renderer.rs`
  - `src/app/terminal_atlas.rs`
  - `src/app/terminal_renderer/platform/windows.rs`
  - `tests/terminal_renderer_prepare_cache_spec.rs`
  - `tests/terminal_runtime_perf_contract_spec.rs`
  - `tests/terminal_session_spec.rs`
- 先补的测试
  - 先让以下契约失败后再实现：
    - bitmap atlas `sprite_cache` 的 clear 或边界契约
    - Windows CPU glyph payload cache 在安全生命周期下的 retained contract
    - cache bound 变更后 scrollback reuse 不倒退
- 验证命令
  - `cargo test --test terminal_renderer_prepare_cache_spec --test terminal_runtime_perf_contract_spec --test terminal_session_spec -q`
- 内存测量命令 / 清单
  - 场景：
    - 首次打开终端
    - 重滚动
    - 大量输出
    - 回看 scrollback
    - 关闭全部 session
  - 重点对比：
    - shaped-row cache 条目数 / 上限
    - mono / color glyph cache 条目数
    - glyph raster cache 条目数
    - prepared-row cache 条目数
    - bitmap sprite / pixel retained state
    - `private/commit` before/after
- 回滚标准
  - 若滚动明显卡顿、输入抖动、selection / cursor 闪烁、首次重开终端首帧明显退化，则回滚。

## 8. 任务 5：复核 startup eager path

- 目的
  - 分段测量 startup 路径，确认主因到底是 UI font fallback、`AppWindow::new()` / Slint，还是 bootstrap service eager init。
- 可能涉及文件
  - `src/main.rs`
  - `src/app/bootstrap.rs`
  - `src/app/runtime_profile.rs`
  - `tests/bootstrap_smoke.rs`
  - `tests/bootstrap_profile_smoke.rs`
- 先补的测试
  - 先锁住以下行为：
    - presenter 仍保持 lazy-init
    - welcome-first startup 不被破坏
    - 新增分段 measurement hooks 时，默认保持静默
- 验证命令
  - `cargo test --test bootstrap_smoke --test bootstrap_profile_smoke --test runtime_profile -q`
- 内存测量命令 / 清单
  - 分段打点：
    - `configure_ui_font_fallbacks`
    - `AppWindow::new()`
    - `log_ui_shell_font_diagnostics`
    - `bind_top_status_bar...` 完成
  - packaged mainline / software compatibility 分开记录
  - 重点观察：冷启动空载、欢迎页空载 30 秒 / 60 秒
- 回滚标准
  - 若任何 lazy-init 尝试影响欢迎页首帧、window startup、top status bar、vault / SSH prompt 行为，则回滚。

## 9. 任务 6：复核 font fallback lazy path

- 目的
  - 在证据足够时，谨慎验证 UI shared collection / system font database 的 lazy path 是否值得推进。
- 可能涉及文件
  - `src/app/font_diagnostics.rs`
  - `src/app/system_font_database.rs`
  - `src/app/terminal_font/windows_dwrite.rs`
  - `Cargo.toml`（若后续决定引入额外 vendor patch）
  - 可能新增 vendored `i-slint-common` / `i-slint-core` patch（仅在 measurement 证明值得时）
  - `tests/startup_font_memory_regression.rs`
  - `tests/terminal_renderer_dwrite_spec.rs`
- 先补的测试
  - 先让以下契约失败后再实现：
    - startup 时不急切扫描 system font DB
    - shared collection / fallback 触发时机受控
    - CJK / emoji / explicit family fallback 仍工作
- 验证命令
  - `cargo test --test startup_font_memory_regression --test terminal_renderer_dwrite_spec -q`
  - 若碰 vendored Slint patch，再补相关 source-contract tests
- 内存测量命令 / 清单
  - 重点测量：
    - 冷启动空载
    - 欢迎页空载
    - 首次打开终端
  - 重点对比：
    - system font database 是否被触发
    - DirectWrite context / emoji renderer 是否延后到真正需要时
    - `private/commit` 是否真实下降
- 回滚标准
  - 若出现 shell UI 字重、布局、中文、emoji、Nerd glyph、font fallback 任一回归，立即回滚。
  - 若收益只体现在 `working set`，没有 `private/commit` 改善，则仅保留为观测性实验，不并入默认行为。

## 10. 任务 7：dead-code / dependency audit（只做候选清单，不马上删除）

- 目的
  - 把“瘦身”严格限制为候选清单与证明过程，不让它抢占真实内存主线，也不允许先删再看。
- 可能涉及文件
  - `Cargo.toml`
  - 候选模块本身
  - 对应 tests / docs / build scripts
  - 建议新增或更新候选审计文档，而不是直接删文件
- 先补的测试
  - source-contract / 引用审计测试，证明候选：
    - 不在 production path
    - 不被 build features / Windows wrappers / tests / scripts / docs 引用
- 验证命令
  - `cargo check --all-targets`
  - `cargo test --all-targets`
  - `bash tests/build_win_x64_script_smoke.sh`
  - `bash tests/build_win_x64_software_script_smoke.sh`
  - 必要时再做 package build smoke
- 内存测量命令 / 清单
  - 只有在依赖移除已通过编译、测试、打包验证后，才比较：
    - binary size
    - startup `private/commit`
    - package size
  - 若无显著变化，必须明确记录“只是代码清理，不是主线 memory win”。
- 回滚标准
  - 只要证明链不完整，就不允许删除。
  - 任何删减一旦触及 package、fonts、Windows native renderer、SSH / SFTP / vault 主链，立即停止并回滚。

## 11. 正式实现提示

后续正式实现阶段必须明确执行：

- 新开 Codex 窗口。
- 进入 `.worktrees/feature-memory-footprint-reduction`。
- 使用 `superpowers:executing-plans`。
- 每个任务前使用 `superpowers:test-driven-development`。
- 异常时使用 `superpowers:systematic-debugging`。
- 完成前使用 `superpowers:verification-before-completion`。

## 12. 推荐起手任务

后续正式实现建议从 **任务 1：建立可复现的内存 baseline** 开始，而不是直接改 cache 或删代码。

原因：

- 当前 450 MB 仍缺少按场景拆开的证据；
- `working set` 与 `private commit` 很容易被混淆；
- mainline / software compatibility / runtime fallback 需要先分清；
- 没有 baseline，任何“优化成功”都不可信。

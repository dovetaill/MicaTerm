# dead-code / dependency audit 候选清单

日期：2026-06-10  
工作目录：`/home/wwwroot/mica-term/.worktrees/feature-memory-footprint-reduction`

## 1. 边界

- 本文档只做候选清单，不马上删除。
- 本文档不把“看起来像历史遗留”直接包装成可删项。
- 任何后续删除都必须先补完整证明链，而不是先删再看。

## 2. 删除证明链

某项代码、依赖或 vendored patch 只有同时满足以下条件，才允许从“候选”升级为“可删”：

1. 已确认不在生产路径。
2. 已确认不被 build features 引用。
3. 已确认不被 Windows / desktop wrappers 引用。
4. 已确认不被 tests 引用。
5. 已确认不被 docs 保留为现行合同或回归说明。

当前结论：当前没有满足证明链的 runtime 删除候选。

## 3. 已审计项

| 项目 | 状态 | 当前结论 |
| --- | --- | --- |
| `vendor/i-slint-renderer-skia` | 不是候选 | `Cargo.toml` 仍通过 `[patch.crates-io]` 指向该 vendored patch；`build-win-x64.sh` 仍导出 `slint-renderer-skia,terminal-native-renderer`；`build-desktop.sh` 仍按 `slint-renderer-skia` 分支打包；多条 source-contract tests 仍直接读取该目录验证文本质量与 purge 合同。 |
| `vendor/i-slint-backend-winit` | 不是候选 | `Cargo.toml` 仍用该 patch 驱动桌面 backend；`src/app/bootstrap.rs` 的 backend purge 仍通过该 patch 暴露的 `purge_winit_renderer_memory()` 接线；`tests/slint_backend_purge_contract_spec.rs` 明确锁住该 purge 合同。 |
| `fontdb` | 不是候选 | `Cargo.toml` 仍声明依赖；`src/app/system_font_database.rs`、`src/app/font_diagnostics.rs`、`src/app/terminal_font/windows_dwrite.rs`、`src/app/terminal_font/windows_locator.rs`、`src/app/terminal_emoji.rs` 仍直接使用；当前 task 5/6 的启动与字体观测本身就依赖它。 |
| `windows-sys` | 不是候选 | `Cargo.toml` 仍为进程内存计量与部分 Win32 路径声明 `Win32_System_ProcessStatus`、`Win32_System_Threading` 等 feature；当前内存诊断与 Windows 外壳行为仍需要这组 API。 |
| `tests/support/retired_windows_subsystem.rs` | 待补证明 | 它是 test-only helper，不属于 runtime memory 主线；但当前仍被 `bootstrap_profile_smoke.rs`、`runtime_profile.rs`、`native_terminal_surface_contract_spec.rs`、`terminal_renderer_dwrite_spec.rs` 等多条契约测试导入。若未来要删，应在单独的测试清理任务里先改写这些断言，再评估是否保留。 |

## 4. 为什么本轮不删

- `vendor/i-slint-renderer-skia` 和 `vendor/i-slint-backend-winit` 仍处在 live startup / packaged / purge 主链上，贸然删除会直接破坏 mainline package 与 task 2-5 的内存观测。
- `fontdb` 与 `windows-sys` 不是“瘦身噪音”，而是当前字体回退、Windows native renderer、memory diagnostics 的基础依赖。
- `tests/support/retired_windows_subsystem.rs` 即使未来可以清理，也只会减少测试辅助代码，不会形成本轮关注的 runtime memory win。

## 5. 后续动作

- 若未来要继续 dead-code / dependency audit，优先从 test-only helper 或纯文档遗留开始，不要先碰 packaged Windows、字体回退或 vendored Slint patch。
- 只有在某项同时满足“脱离生产路径、脱离 build features、脱离 wrappers、脱离 tests、脱离 docs”后，才允许把状态从“待补证明/不是候选”改成真正候选。

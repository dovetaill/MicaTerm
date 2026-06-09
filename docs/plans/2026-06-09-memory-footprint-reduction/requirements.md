# 内存占用缩减需求文档

日期：2026-06-09  
执行者：Codex  
状态：仅文档阶段，本轮不改代码

## 1. 背景与用户问题

用户反馈当前 MicaTerm 在 Windows packaged build 下的内存占用偏高，肉眼可见到约 450 MB，且体感上有“越来越臃肿”的趋势。用户希望优先降低真实内存占用；如有必要，可在严格证明安全的前提下整理冗余代码或依赖，但绝不能影响任何既有功能。

结合本地仓库核查结果，当前问题不能被简化为“任务管理器数字太大”，而应拆成多个可能叠加的来源：

- 启动期的 eager initialization；
- UI 字体与系统 fallback 初始化；
- terminal renderer 的 glyph / shape / prepared-row / bitmap atlas 缓存；
- session / scrollback 生命周期；
- Skia / Direct3D / Slint backend 持有的缓存或原生资源；
- `EmptyWorkingSet` 带来的 `working set` 表象回落。

因此，本轮需求的首要目标不是“先做一个看上去变小的数字”，而是先建立正确的度量口径与问题归因边界。

## 2. Superpowers 使用记录

本轮已实际按要求使用并落实以下 superpowers 技能：

- `superpowers:using-superpowers`：先确认本轮必须先走技能流程，再开始调研与产出。
- `superpowers:brainstorming`：先做事实核查、方案探索、边界收敛，不直接进入实现。
- `superpowers:using-git-worktrees`：明确正式实现必须新开窗口，并进入 `.worktrees/feature-memory-footprint-reduction`；本轮不在 worktree 中改代码。
- `superpowers:test-driven-development`：本轮不写实现代码，但已为后续实现阶段设计“先补测试、再改逻辑”的任务结构。
- `superpowers:systematic-debugging`：本轮对内存问题做归因分层，明确区分真实释放与 `working set` 表象，不接受猜测式优化。
- `superpowers:verification-before-completion`：文档完成前核对路径、事实来源、约束符合性与“未改业务代码”边界。
- `superpowers:executing-plans`：仅留给后续正式实现窗口使用，本轮不进入执行阶段。

## 3. 主目标

本轮确认的主目标如下：

- 降低 **真实内存占用**，重点关注 Windows packaged build 下的：
  - 冷启动；
  - 欢迎页空载；
  - 首次打开终端；
  - 大量输出；
  - 重滚动；
  - 多 session；
  - 关闭 session 后的回落；
  - 重启应用后的重复表现。
- 以内存真实性指标为主：`private bytes / private commit / commit size` 为主轴，`working set` 为辅。
- 优化结论必须建立在 measurement baseline 与 before/after 证据之上，不允许只凭主观体感或单张截图下结论。

## 4. 非目标

以下内容明确 **不是** 本轮目标：

- 本轮不改代码，只产出需求、设计、任务三份文档。
- 不删除任何无法证明安全的代码、依赖、打包入口或兼容分支。
- 不牺牲任何功能、视觉、字体、终端渲染、输入行为、SSH/SFTP、同步/vault、窗口行为。
- 不改变 Windows mainline packaging 或 software compatibility packaging 的默认语义。
- 不把更激进的 `EmptyWorkingSet` / trim `working set` 当成核心优化方案。
- 不承诺固定数字目标，例如“450 MB 一定降到某个数”；只能提出“先建基线，再分阶段优化”的目标表达。

## 5. 必须保留的功能面

后续任何实现都必须完整保留并验证以下功能面：

- Windows mainline packaging：`./build-win-x64.sh` 对 `skia + native + rendering-notifier` 的约束。
- Windows software compatibility packaging：`./build-win-x64-software.sh` 对 `software host + native + rendering-notifier` 的约束。
- `build-desktop.sh` 的统一打包入口语义，包括可移植包布局、`.mica-term-portable`、字体许可证、`logs/`、`data/assets.redb` 等分发结构。
- welcome-first startup：欢迎页优先启动，不能因为“省内存”而提前把终端重路径强行拉进首帧。
- terminal rendering：native / bitmap 回退链、终端刷新、滚动、输入、选区、光标、重建行为都必须保持。
- SSH / SFTP / session 行为：打开、关闭、重连、tab 切换、scrollback、close 后释放语义都必须不变。
- 字体 fallback：JetBrains Maple Mono、Sarasa Term SC Nerd、Windows emoji / system fallback、Nerd glyph、CJK 宽度与显示必须保持。
- 日志与诊断开关：`MICA_TERM_LOG`、`MICA_TERM_MEMORY_DIAGNOSTICS` 及其 runtime/config 行为必须保留。
- Windows native presenter / notifier / DPI 矩阵行为：不能为了省内存破坏 packaged Windows 现有行为。
- Skia 与 software compatibility 行为：不能偷换 renderer 合同来换取表面的内存数字改善。

## 6. 已核对的本地事实边界

后续实现必须以以下本地事实为准，而不是以用户 prompt、旧计划或主观猜测为准：

- `src/app/memory.rs` 当前会采样 `working_set_bytes`、`pagefile_usage_bytes`、`private_usage_bytes`，并通过 `K32EmptyWorkingSet` 执行 `working set` trim。
- `docs/wikis/2026-04-08-windows-working-set-trim-findings.md` 已给出明确证据：`working set` 可明显下降，但 `private/commit` 不一定同步下降。
- `src/app/bootstrap.rs` 当前已经存在：
  - active-idle shrink；
  - no-surface idle shrink；
  - `clear_workspace_terminal_transient_caches()`；
  - `release_workspace_terminal_renderer_resources()`；
  - renderer/backend purge；
  - 最后一步的 process `working set` trim。
- `src/app/terminal_presenter.rs`、`src/app/terminal_renderer/wgpu_renderer.rs` 已存在 cache 统计与 clear hook；native path 已有有界 cache 设计。
- `src/app/terminal_font/windows_dwrite.rs` 已把部分 DirectWrite 相关 helper 改为 lazy-init，但 UI 侧 `shared_collection()` 与 system fallback 仍可能在 startup 被触发。
- `readme.md`、`build-win-x64.sh`、`build-win-x64-software.sh`、`src/app/runtime_profile.rs` 一致表明：当前两个 Windows packaged 路径都保留 native terminal renderer，不能把 software compatibility package 误判成“只走 bitmap terminal”。
- `build-win-x64.sh` 与 `build-win-x64-software.sh` 最终都依赖 `build-desktop.sh` 完成统一的打包与产物布局，因此任何后续验证都不能只看局部 wrapper。

## 7. 测量要求（Measurement Requirements）

后续正式实现阶段，至少必须覆盖以下场景：

### 7.1 启动与欢迎页

- 冷启动空载：新进程启动后，窗口稳定显示但尚未进行交互。
- 欢迎页空载：欢迎页停留 30 秒、60 秒，确认是否仍有异步初始化继续推高 `private/commit`。
- 重启应用：完整退出后再次启动，确认第二次启动是否存在额外 retained state。

### 7.2 首次进入终端与高压场景

- 首次打开终端：确认 native presenter、DirectWrite、glyph cache、system fallback 的首次增量。
- 大量输出：例如长日志、`history`、大文本输出，观察峰值与回落。
- 重滚动：PageUp/PageDown、拖拽滚动条、回看历史，观察 cache 增长、复用与回落。
- 多 session：至少 3 到 5 个 session 并发时的增量曲线。

### 7.3 关闭 session 后的回落

- 关闭 session 后立即记录。
- 关闭后 30 秒记录。
- 关闭后 60 秒记录。
- 关闭全部 session 后再次重启应用，对比进程内 retained state 与真正冷启动行为。

### 7.4 打包矩阵

- Windows mainline package：`./build-win-x64.sh` 产物。
- Windows software compatibility package：`./build-win-x64-software.sh` 产物。
- 每类 package 都必须记录实际 renderer 落点与 runtime fallback 结果，避免把不同 renderer/path 的数字误当成同类比较。

## 8. 指标定义

后续文档、日志、测试与验收必须统一术语：

### 8.1 `working set`

- 含义：当前驻留在物理内存中的页面。
- 作用：用于描述任务管理器“看起来是否下降”。
- 限制：可能因为 `EmptyWorkingSet` 大幅下降，但这 **不等于** 应用真正释放了私有提交内存。

### 8.2 `private bytes` / `private commit`

- 本仓库当前最接近的口径是 `PROCESS_MEMORY_COUNTERS_EX.PrivateUsage`，即 `private_usage_bytes`。
- 这是后续内存优化的主 KPI。
- 必须配合场景、时间点与 before/after 对比来解读，不能只看单点快照。

### 8.3 `commit size`

- 需要与 `PagefileUsage` / `PrivateUsage` 一起对照。
- 用于确认进程是否真正减少了已承诺的私有内存，而不只是把 resident pages 移出了工作集。

### 8.4 句柄数与线程数

- 作为辅助指标，用于判断是否有额外 runtime/service/host 线程或句柄泄漏。
- 不是本轮唯一目标，但必须进入 baseline 记录。

### 8.5 renderer cache 统计

至少包括：

- shaped-row cache 条目数 / 上限；
- mono glyph cache 条目数；
- color glyph cache 条目数；
- glyph raster cache 条目数；
- prepared-row cache 条目数；
- bitmap atlas 路径的 sprite / pixel buffer retained state（若当前未暴露，则后续先补观测）。

### 8.6 字体与系统 fallback 初始化标记

至少包括：

- UI `shared_collection()` 是否已触发 system fallback；
- `load_system_font_database()` 是否已触发；
- DirectWrite system font collection 是否已建立；
- emoji renderer / fallback resolver 是否已初始化；
- 首次打开终端时，是否触发额外字体链增长。

## 9. 验收原则

后续正式实现必须满足以下验收原则：

- 先有 baseline，再做优化。
- 每一个优化项都必须有 before/after 对照。
- 每一个优化项都必须有功能回归验证。
- 不能只用任务管理器截图或主观体感宣称成功。
- 不能把 `EmptyWorkingSet` / trim `working set` 当成真实优化结论。
- 不能为了“看起来省内存”破坏 fallback font、emoji、CJK、Nerd glyph、cursor、selection、SSH/SFTP/session 行为。
- 不能在未确认 renderer 实际落点时比较不同 package 的数字。
- 只能提出“measurement baseline + 分阶段优化目标”，不能承诺固定降幅。

## 10. 本轮文档阶段交付边界

本轮交付严格限定为：

- `docs/plans/2026-06-09-memory-footprint-reduction/requirements.md`
- `docs/plans/2026-06-09-memory-footprint-reduction/designs.md`
- `docs/plans/2026-06-09-memory-footprint-reduction/tasks.md`

本轮不进行任何业务代码、构建脚本、测试代码或资源文件修改。

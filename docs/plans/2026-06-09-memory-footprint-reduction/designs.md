# 内存占用缩减设计文档

日期：2026-06-09  
执行者：Codex  
状态：仅文档阶段，本轮不改代码

## 1. Superpowers 使用记录

本轮设计过程按以下顺序实际使用了 superpowers 技能：

1. `superpowers:using-superpowers`
2. `superpowers:brainstorming`
3. `superpowers:using-git-worktrees`
4. `superpowers:test-driven-development`（用于设计后续实现阶段的测试优先策略）
5. `superpowers:systematic-debugging`
6. `superpowers:verification-before-completion`
7. `superpowers:executing-plans` 仅保留给后续正式实现窗口

补充记录：

- 当前仓库 `.worktrees/` 已存在，且 `git check-ignore` 已确认其被 `.gitignore` 忽略；正式实现应在 `.worktrees/feature-memory-footprint-reduction` 中进行。
- 本轮还实际启动了多位 `gpt-5.4 xh` subagents，覆盖 Windows 内存 profiling、Rust/Slint、terminal renderer、功能回归、代码瘦身、测试与发布等角色。

## 2. 仓库事实快照（Repository Reality Snapshot）

以下内容只基于本地仓库事实，不直接照搬用户 prompt，也不直接照搬旧计划。

### 2.1 构建与打包入口现状

- `readme.md` 当前把 Windows 正式入口区分为：
  - `./build-win-x64.sh`：Windows mainline，`skia + native + rendering-notifier`
  - `./build-win-x64-software.sh`：Windows software compatibility，`software host + native + rendering-notifier`
- `build-win-x64.sh` 会注入：
  - `MICA_TERM_BUILD_FLAVOR=windows-mainline`
  - `MICA_TERM_PACKAGE_RENDERER=skia`
  - `MICA_TERM_PACKAGE_TERMINAL_RENDERER=native`
  - `MICA_TERM_PACKAGE_NATIVE_PRESENT_PATH=rendering-notifier`
- `build-win-x64-software.sh` 会注入：
  - `MICA_TERM_BUILD_FLAVOR=windows-software-compat`
  - `MICA_TERM_PACKAGE_RENDERER=software`
  - `MICA_TERM_PACKAGE_TERMINAL_RENDERER=native`
  - `MICA_TERM_PACKAGE_NATIVE_PRESENT_PATH=rendering-notifier`
- 两个 wrapper 最终都调用 `build-desktop.sh` 完成统一打包；后者还负责 portable 包结构、字体许可证、Windows 产物布局等共通逻辑。
- 这意味着：**当前两个 Windows package 都保留 native terminal renderer**，不能把 software compatibility package 误写成“只走 bitmap terminal path”。

### 2.2 Runtime profile 与 renderer fallback 现状

- `src/main.rs` 当前统一使用 `AppRuntimeProfile::packaged()`。
- `src/app/runtime_profile.rs` 表明：
  - Windows mainline renderer fallback chain：`skia -> skia-software -> software`
  - software compatibility chain：`software`
  - 但 terminal render mode 在两个 package 中都可为 `Native`
- 因此任何 baseline 都必须同时记录：
  - build flavor；
  - host renderer；
  - terminal render mode；
  - native present path；
  - 是否发生 runtime fallback。

### 2.3 启动路径的真实现状

- `run_with_profile()` 在首帧前仍会做：
  - `configure_ui_font_fallbacks()`
  - `AppWindow::new()`
  - `log_ui_shell_font_diagnostics()`
  - `bind_top_status_bar_with_profile_and_async_handle(...)`
- `src/app/bootstrap.rs` 中，当前最大的 eager 区域已经不只是 terminal presenter，而是 shell / service bootstrap：
  - UI prefs
  - asset catalog
  - keychain
  - quick-launch preferences
  - transfer store
  - vault bootstrap state
  - vault sync service
  - SFTP browser controller
  - timer / channel / async runtime bridge
- 这说明“startup memory 主要来自 terminal presenter”已经不是完整事实。

### 2.4 Terminal presenter 与 renderer 的现状

- `docs/plans/2026-04-03-windows-startup-memory-design.md` 里提出的 presenter lazy-init 已基本落地：
  - bootstrap 初始阶段 `WORKSPACE_TERMINAL_RENDERER_HOST` 为 `None`
  - 只有出现 active terminal surface 时才 `ensure_workspace_terminal_presenter()`
  - 无 host 时使用 fallback cell size 维持 welcome-first 启动
- `src/app/terminal_presenter.rs` 表明：
  - 仍同时保留 bitmap atlas presenter 与 Windows native presenter
  - presenter 已暴露 `cache_stats()` 与 `clear_transient_caches()`
- `src/app/terminal_renderer/wgpu_renderer.rs` 表明：
  - native path 已有有界 cache
  - 默认 mono glyph / color glyph / glyph raster cache 上限均为 `1024`
  - prepared-row cache 与 previous prepared rows 已存在
  - 超限后主要通过延后 reset / clear，而不是每帧暴力清空
- `src/app/terminal_atlas.rs` 表明：
  - bitmap atlas 路径存在 `sprite_cache`、`pixels`、`row_hashes`
  - 可以在缩放变化时清空，但从快速核查看，尚未看到与 native path 对等的显式 LRU / 条目上限约束
- 结论：native path 不是“完全没做 cache 控制”，而 bitmap atlas retained state 更值得复核边界是否偏松。

### 2.5 Scrollback 与 session 生命周期现状

- `src/app/ssh/runtime/terminal.rs`：`release_memory()` 会把真实 terminal core 替换成 `ReleasedTerminalCoreAdapter`，丢弃大部分 scrollback 与 visible content。
- `src/app/ssh/session_manager.rs`：真正 `close_session()` 时，会先释放 terminal memory，再清 registry、surface、revision、cwd、runtime control、SFTP 绑定等状态。
- 但 `disconnect_session()`、失去 active surface、真正 close tab，并不是同一个生命周期点。
- 因此“关掉 session 后为什么还保留不少内存”不能只看 session runtime，也必须同时看 presenter/renderer/backend 是否仍持有缓存或原生资源。

### 2.6 字体与 fallback 路径现状

- Shell UI 主字体：JetBrains Maple Mono。
- Terminal 主字体：Sarasa Term SC Nerd。
- Windows emoji fallback：`Segoe UI Emoji`。
- `src/app/terminal_font/windows_dwrite.rs` 当前已 lazy-init：
  - system font database
  - locator
  - emoji renderer
  - DirectWrite context
- 但 UI 侧 `src/app/font_diagnostics.rs` 仍会在 startup 主动使用 `shared_collection()` 并设置 generic fallback families。
- `src/app/system_font_database.rs` 当前实现仍是直接 `fontdb.load_system_fonts()`。
- 结论：terminal font helper 的 lazy-init 已有进展，但 UI shared collection / system fallback 仍是 startup private/commit 的高嫌疑点。

### 2.7 Memory diagnostics / trim / purge 的现状

- `src/app/memory.rs` 当前采样：
  - `WorkingSetSize`
  - `PeakWorkingSetSize`
  - `PagefileUsage`
  - `PrivateUsage`
- `src/app/memory.rs` 的 `trim_process_working_set()` 直接调用 `K32EmptyWorkingSet`。
- `src/app/logging/config.rs` 仍解析 `MICA_TERM_MEMORY_DIAGNOSTICS`；`readme.md` 也仍保留该环境变量说明。
- `src/app/ssh/runtime/pump.rs`：大输出空闲后仍会执行 `trim_process_working_set()`。
- `src/app/bootstrap.rs`：
  - active-idle shrink 只清 transient caches，不主动 drop 可见 host
  - no-surface idle shrink 会 clear caches、drop renderer host、调用 backend purge，最后再做 `working set` trim
- `vendor/i-slint-backend-winit` 与 `vendor/i-slint-renderer-skia`：当前已经接通 purge hook，包含 renderer memory purge、layer cache clear、surface purge、Skia global cache purge 与 D3D deferred cleanup。
- 结论：旧 wiki 中“更深层 purge 还没接入”的判断已经部分过时；但“`EmptyWorkingSet` 只是表象优化”的判断仍然成立。

### 2.8 历史文档与当前实现的偏差

- `docs/wikis/2026-04-08-windows-working-set-trim-findings.md` 对“`EmptyWorkingSet` 只能明显拉低 `working set`，不代表真实释放 `private/commit`”的结论仍有效。
- 但它关于“backend 更深 purge 尚未接入”的描述，已经与当前 vendored backend 不完全一致。
- `docs/plans/2026-04-03-windows-startup-memory-design.md` 的 presenter lazy-init 已基本落地，不能再当成本轮首要未完成项。
- `docs/plans/2026-04-09-startup-ui-font-fallback-design.md` 提出的“primary collection + lazy system collection”方向，目前在本地仓库的接线层面未见已落地证据。

## 3. 外部调研摘要（Prior Art / MCP Tavily Research）

本轮使用了 Tavily、Exa 与 GitHub 搜索；所有外部资料只作参考，最终决策仍以本地代码为准。

### 3.1 Windows 内存口径：可以直接借鉴的部分

参考方向：

- Microsoft Learn：`Working Set`
- Microsoft Learn：`EmptyWorkingSet`
- Microsoft Learn：`PROCESS_MEMORY_COUNTERS_EX`
- Microsoft Learn：Memory Performance Information

可借鉴结论：

- `working set` 是 resident 视角，不等于真实已提交私有内存。
- `PrivateUsage` / `PagefileUsage` 更接近 `private commit / commit size` 的判断主轴。
- `EmptyWorkingSet` 的语义就是“尽量把页面移出工作集”，不是“释放应用对象”。

对 MicaTerm 的适配结论：

- 后续 KPI 不能只盯任务管理器；必须把 `private_usage_bytes` 作为主轴。
- 这与本地 `src/app/memory.rs` 的实现口径正好一致。

### 3.2 DirectWrite 与系统字体枚举：高度相关

参考方向：

- Chromium DirectWrite font cache 设计资料
- Microsoft DirectWrite 文档

可借鉴结论：

- system font collection / family 枚举本身就可能带来明显的 I/O、metadata 与内存成本。
- 成熟桌面程序通常会把“枚举字体集合”和“按需读具体 glyph / fallback”拆开。

对 MicaTerm 的适配结论：

- 这与本地 `load_system_font_database()`、`shared_collection()`、DirectWrite fallback 路径高度相关。
- 因此 UI startup 与 terminal first-open 都必须把“system font catalog 何时被触发”纳入测量。

### 3.3 Slint shared font collection：可借鉴，但不能照抄

参考方向：

- Slint `fontique_08` / `shared_collection()` 文档
- Slint upstream `sharedfontique` 相关实现
- Slint 关于中文字体、内存、fallback 的 issue / discussion

可借鉴结论：

- `shared_collection()` 是进程级共享集合，配置会影响整个进程。
- system font fallback 与应用注册字体都可能推高 startup 内存。
- 大型字体与 CJK fallback 一旦在错误时机急切初始化，会显著放大私有内存成本。

不应直接照抄的部分：

- 上游讨论更多是现象与思路，不等于本仓库可直接抄用的修复。
- 本仓库当前没有本地 patch `i-slint-common` / `i-slint-core` 的既成事实，所以是否值得引入更深 vendor 变更，必须以 measurement 先行。

### 3.4 Skia / D3D 缓存治理：可借鉴且已部分接入

参考方向：

- Skia `GrDirectContext` 文档
- Skia `SkGraphics` 文档
- Skia 全局 cache purge / deferred cleanup 资料

可借鉴结论：

- `setResourceCacheLimit()`、`performDeferredCleanup()`、`purgeUnlockedResources()`、`purge_all_caches()` 都属于正规缓存治理路径。
- 真正值得关心的是这些动作对 `private/commit` 的影响，以及对首帧恢复成本的影响。

对 MicaTerm 的适配结论：

- 本地 vendored backend 已接入一部分 purge 路径。
- 但它是否真正降低 `private/commit`、是否会带来首帧卡顿，仍必须用 packaged baseline 验证。

### 3.5 Terminal emulator 缓存策略：高度可借鉴

参考方向：

- WezTerm scrollback 文档
- Alacritty `glyph_cache.rs`
- Contour `text-stack` 内部文档

可借鉴结论：

- 现代终端普遍依赖 glyph atlas、shape cache、prepared row cache、scrollback 上限等机制。
- 正确方向不是“关闭 cache”，而是“有界 cache + 生命周期 clear / purge + 复用策略”。
- close session、no-surface、长时间 idle 往往是最安全的 shrink 触发点。

对 MicaTerm 的适配结论：

- 本仓库当前 native path 已接近这一思路；优先工作应是复核边界与生命周期，而不是推翻渲染架构。
- bitmap atlas 路径因为 retained state 更直白、边界未见明确上限，更值得先做观测。

## 4. Subagent 辩论摘要（Subagent Debate Summary）

### 4.1 参与角色

本轮实际启动了至少 6 位 `gpt-5.4 xh` subagents：

1. Windows 内存 profiling 专家
2. Rust/Slint 架构专家
3. Terminal renderer 专家
4. 功能回归守门员
5. 代码瘦身 / 依赖治理专家
6. 测试与发布专家

### 4.2 第一轮：各自独立观点

#### Windows 内存 profiling 专家

- 坚持把 `working_set_bytes` 与 `private_usage_bytes` 严格分开。
- 明确指出 `K32EmptyWorkingSet` 属于 cosmetic trim，不应被包装成真实内存优化。
- 认为 baseline 必须同时覆盖 package 类型、renderer 落点、close-path 与 purge-path。

#### Rust/Slint 架构专家

- 认为 presenter lazy-init 与 terminal DirectWrite helper lazy-init 已基本落地。
- 认为当前 startup 更可疑的是 UI shared collection / system font database 与 bootstrap 中的大量 eager service 初始化。
- 反对在没有 measurement 的前提下，把 vault / SFTP / session bridge 一口气大规模懒加载。

#### Terminal renderer 专家

- 认为大头更可能出在 scrollback 生命周期、glyph / shape / prepared-row cache，以及 bitmap atlas retained state。
- 认为 `session.close -> release_memory()`、`no-surface -> clear/drop host` 更可能真实降低 `private/commit`。
- 反对把 clear cache 提到滚动、输入等热路径。

#### 功能回归守门员

- 强调 welcome-first startup、native fallback、两类 Windows package 的 native 合同、SSH/SFTP/session 生命周期都不能坏。
- 明确反对为了省内存而偷偷修改 package 语义、字体 ownership 或 active surface 热路径行为。

#### 代码瘦身 / 依赖治理专家

- 认为“冗余清理”只能是次级候选，不能抢在真实内存主线之前。
- 真正可能值得审计的是少数重量级依赖与看似死文件，但必须先证明不在 production path。
- 反对触碰 terminal/font/windows 渲染主链做先手删减。

#### 测试与发布专家

- 坚持先补 failing tests 与 diagnostics contract，再谈实现。
- 强调 packaged build smoke、runtime profile、memory baseline 的执行顺序必须明确。
- 认为任何只让 `working set` 下降的候选，都只能先进入观测阶段。

### 4.3 第二轮交叉质询：哪些是真降 `private/commit`，哪些只是 `working set` 表象

综合交叉质询后，subagents 形成以下共识：

#### 更可能真实降低 `private/commit` 的候选

- `session.close -> release_terminal_memory()` 与 session registry 清理。
- no-surface idle shrink 下的：
  - `clear_workspace_terminal_transient_caches()`
  - `release_workspace_terminal_renderer_resources()`
  - backend renderer purge
- 复核 bitmap atlas 路径的 `sprite_cache` / pixel buffer retained state。
- 复核 Windows native surface detach 后保留的 CPU glyph payload cache。
- 针对 startup 的 `shared_collection()` / system font database 触发时机优化。
- 在严格证明后，对真正未使用的重量级依赖做删减。

#### 更像表象优化的候选

- `K32EmptyWorkingSet` 本身。
- 任何只显示 `working_set_bytes` 下降、但 `private_usage_bytes` 基本不变的动作。
- 没有 before/after `private_usage_bytes` 证据的“内存变小了”体感结论。

### 4.4 第三轮交叉质询：哪些会伤害行为，哪些应先停留在观测阶段

综合交叉质询后，subagents 形成以下共识：

#### 必须先停留在 measurement / diagnostics 阶段的候选

- startup UI font fallback / lazy system collection：先证明 `shared_collection()` 是主因，再决定是否引入更深 vendor 变更。
- deeper Skia / D3D purge：先测 `private/commit` 回落与首帧重建代价，再决定是否默认启用。
- active surface 场景下的 cache shrink：先测滚动、输入、cursor、selection、TUI 行为，再决定是否进入默认路径。
- 任何依赖删减：先做“无 runtime path、无 build/test/docs 引用”的证明，再进入真正删除。

#### 高风险、当前不推荐直接做的候选

- 强制 trim `working set` 冒充优化。
- 删除字体或缩减 fallback 链，导致 CJK / emoji / Nerd glyph 回归。
- 关闭 renderer cache，换取静态数字更好看但滚动/输入显著变差。
- 一次性把 vault / keychain / SFTP / session bridge 大规模改成 lazy-init。
- 改 package 语义，尤其把 software compatibility package 变成非 native terminal 路径。
- 未验证就删 vendored backend patch、terminal font chain 或 Windows surface chain。

### 4.5 综合结论

subagent 多轮辩论后的保守共识是：

1. 主线不应是“更激进 trim”。
2. 主线不应是“先删代码看看”。
3. 主线应是：
   - 先补 measurement baseline 与 diagnostics；
   - 先解释 450 MB 的构成；
   - 再做低风险 lifecycle shrink / bounds review；
   - 然后复核 startup / font lazy path；
   - 最后才做 dead-code / dependency audit，而且只从候选清单开始。

## 5. 内存归因假设分层

### A 类：启动期 eager initialization

- 证据：`run_with_profile()` 前后会主动触发 UI font fallback、window creation、font diagnostics 与完整 bootstrap；bootstrap 中还会 eager 创建 prefs、asset、keychain、vault、SFTP、timer/channel 等对象。
- 需要核查的本地文件：`src/main.rs`、`src/app/bootstrap.rs`、`src/app/runtime_profile.rs`、`readme.md`
- 测量方法：在 `configure_ui_font_fallbacks`、`AppWindow::new()`、`log_ui_shell_font_diagnostics`、`bind_top_status_bar...` 等阶段做分段记录，并分开比较 mainline / software compatibility。
- 风险：如果把 service bootstrap 轻率拆成 lazy-init，可能破坏 vault / SFTP / session / prompt 行为。
- 预期影响：中到高，但前提是先证明确实主要来自 startup eager work，而不是字体或 renderer fallback。
- 失败回退方案：若分段测量显示主因不在 service bootstrap，则停止触碰该层，转回字体或 renderer 方向。

### B 类：字体系统枚举与缓存

- 证据：UI 侧 `shared_collection()` 在 startup 被主动触碰；`load_system_font_database()` 当前仍是完整 `fontdb.load_system_fonts()`；历史设计与外部资料都提示 system font collection 可能是显著启动成本。
- 需要核查的本地文件：`src/app/font_diagnostics.rs`、`src/app/system_font_database.rs`、`src/app/terminal_font/windows_dwrite.rs`、`Cargo.toml`
- 测量方法：记录 UI shared collection、system font DB、emoji renderer、DirectWrite context 的触发时机，并对比欢迎页空载与首次打开终端。
- 风险：字体链最容易造成 CJK / emoji / Nerd glyph / 字重 / 宽度回归。
- 预期影响：对 startup `private/commit` 而言，属于高优先级嫌疑点。
- 失败回退方案：若证明 shared collection 不是主因，则保留现状，只补 diagnostics，不引入更深 vendor patch。

### C 类：terminal renderer / glyph / shape cache

- 证据：native path 已有 shaped-row cache、prepared-row cache、glyph atlas/raster cache；bitmap atlas 路径保留 `sprite_cache`、`pixels`、`row_hashes`；Windows native surface detach 时还会保留部分 CPU-side glyph payload。
- 需要核查的本地文件：`src/app/terminal_presenter.rs`、`src/app/terminal_renderer/wgpu_renderer.rs`、`src/app/terminal_atlas.rs`、`src/app/terminal_renderer/platform/windows.rs`
- 测量方法：每个场景记录 presenter cache stats，并对比首次打开终端、大输出、重滚动、关闭 session、no-surface 30 秒 / 60 秒。
- 风险：过度清 cache 容易带来滚动卡顿、输入抖动、首帧重建与 glyph 闪烁。
- 预期影响：中到高，尤其对应大输出 / 重滚动 / close 后 retained memory。
- 失败回退方案：若优化后命中率下降或交互退化，则回滚 bounds / shrink 改动，仅保留观测能力。

### D 类：session / scrollback 生命周期

- 证据：`release_memory()` 已能丢弃 scrollback / visible content；真正 `close_session()` 还会继续清 session manager 侧状态；但 disconnect、surface clear、close tab 并不等价。
- 需要核查的本地文件：`src/app/ssh/runtime/terminal.rs`、`src/app/ssh/runtime.rs`、`src/app/ssh/session_manager.rs`、`src/app/bootstrap.rs`
- 测量方法：对比 close one session、close all sessions、final no-surface idle 后的 `private/commit` 回落，并同步记录 session count、surface state、cache stats。
- 风险：会话生命周期一旦改坏，很容易伤害 SSH / SFTP / reconnect / tab close 语义。
- 预期影响：高，尤其对应“关闭 session 后为什么还不掉”的主诉。
- 失败回退方案：若 close-path 语义过于复杂，则先只补观测点，不提前重构 session manager。

### E 类：Skia / D3D / GPU / 原生资源缓存

- 证据：vendored backend 已有 purge hook、Skia global purge 与 D3D deferred cleanup；旧 wiki 的方向仍然成立，只是“尚未接入”的前提已变化。
- 需要核查的本地文件：`src/app/bootstrap.rs`、`vendor/i-slint-backend-winit/lib.rs`、`vendor/i-slint-renderer-skia/lib.rs`、`vendor/i-slint-renderer-skia/d3d_surface.rs`
- 测量方法：记录 no-surface idle shrink 前后 `private/commit` 与 `working set`，必要时配合 VMMap / Process Explorer / PerfMon / ETW。
- 风险：可能带来首帧重建卡顿、窗口恢复抖动、不同 renderer 行为不一致。
- 预期影响：中；有机会解释“terminal 都关了为什么还高”，但不能提前承诺收益。
- 失败回退方案：若收益只体现在 `working set`，则不把该路径包装成主线优化，只作为后台清理或观测辅助。

### F 类：真正冗余代码或依赖

- 证据：存在少量候选看起来像死文件、脚手架或未充分使用的重量级依赖；但当前仍缺“无 production path、无 build/test/docs 引用”的最终证明。
- 需要核查的本地文件：`Cargo.toml`、`build.rs`、候选模块本身、相关 tests/docs/build scripts
- 测量方法：先做引用审计，再在独立 worktree 中做移除后的 `cargo check`、`cargo test`、打包 smoke，最后才比较二进制大小与 startup `private/commit`。
- 风险：收益不确定，但回归面可能很大。
- 预期影响：低到中；只能当末位候选。
- 失败回退方案：保留候选清单，不做即时删除。

## 6. 推荐阶段化方案

### 阶段 0：只补 measurement / diagnostics 计划

目标：

- 统一 measurement 口径；
- 明确 packaged repro 步骤；
- 明确 renderer 实际落点；
- 明确 `working set` 与 `private/commit` 的区分。

产出：

- 基线记录模板；
- 场景矩阵；
- 日志与外部工具使用清单。

### 阶段 1：确认 450 MB 的构成与可复现路径

目标：

- 先知道 450 MB 来自哪里：startup、UI font、terminal first-open、scrollback、backend retained cache，还是 renderer fallback。
- mainline / software compatibility 分开复现与记录。

本阶段不做：

- 不先改 cache bounds；
- 不先删代码；
- 不先调 trim aggressiveness。

### 阶段 2：低风险 lifecycle cache shrink / bounds 复核

目标：

- 复核现有 presenter / renderer / bitmap atlas / Windows CPU glyph payload cache 的 clear 与 bounds 行为；
- 优先做低风险、可回滚的小步修改。

重点场景：

- close session；
- no-surface idle；
- active idle 的温和 shrink。

### 阶段 3：startup / font lazy path 复核

目标：

- 若证据继续指向 startup / UI font / shared collection，再决定是否推进更深层的 lazy system collection 方案。
- 不与 session/vault 生命周期重构混做。

### 阶段 4：谨慎的 dead-code / dependency audit

目标：

- 只做候选清单与逐项证明；
- 只在收益明确、验证充分时才进入真正删除。

### 阶段 5：正式实现计划入口

目标：

- 新开 Codex 窗口；
- 进入 `.worktrees/feature-memory-footprint-reduction`；
- 使用 `superpowers:executing-plans` 按 task-by-task 执行；
- 每个 task 前先做 TDD，每次异常先做 systematic debugging，每次宣告完成前做 verification。

## 7. 明确不推荐方案

以下方案本轮明确不推荐：

- 强制 trim `working set` 冒充真实优化。
- 删除字体或缩减 fallback 链，导致 CJK / emoji / terminal glyph 回归。
- 关闭 renderer cache，只换来“数字更好看”但滚动 / 输入明显变差。
- 大规模重构 terminal subsystem。
- 未验证就删代码、删依赖、删 vendored backend patch。
- 修改 packaging 默认语义，尤其是 mainline / software compatibility 的 renderer/native 合同。
- 把 active surface 热路径变成频繁 clear / purge 点。

## 8. 安全边界

后续正式实现必须遵守：

- 所有实现必须新开窗口。
- 所有实现必须在 `.worktrees/feature-memory-footprint-reduction` 中进行。
- 每个 task 都必须小步、可测、可回滚。
- 任何单项改动都必须同时提交：
  - 先失败的测试或 source contract；
  - 实现；
  - before/after 测量记录；
  - 明确的回滚标准。
- 若某项优化只能改善 `working set`，不能改善 `private/commit`，文档里必须直说，不能包装成主线成功。

## 9. 当前最优保守方案

综合本地事实、外部资料与 subagent 多轮辩论，本轮推荐的保守主线是：

1. 先用 packaged baseline 证明 450 MB 到底由哪些层组成。
2. 先确认 `MICA_TERM_MEMORY_DIAGNOSTICS` 当前是否真的能输出足够的 `app.memory` 事件；若不够，先补 observability，而不是先改逻辑。
3. 优先看 close session / no-surface / active idle 下的真实 retained cache 与 `private/commit` 回落。
4. 再审 glyph / shape / bitmap / Windows CPU glyph payload cache 的边界与生命周期。
5. 只有在证据持续指向 startup / UI font 时，才推进 shared collection / lazy system collection 方向。
6. dead-code / dependency audit 只做候选清单，绝不抢在真实内存主线之前。

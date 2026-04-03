# Bootstrap ViewModel Runtime Refactor Design

日期: 2026-04-03
执行者: Codex
状态: 方案已确认，处于 design-only 阶段，未进入业务实现

## 背景

本轮任务聚焦三个已经明显超出单文件维护舒适区的 Rust 文件：

- `src/app/bootstrap.rs`
- `src/shell/view_model.rs`
- `src/app/ssh/runtime.rs`

当前问题不是单纯“行数多”，而是三个文件都已经同时承担了：

- 公共契约
- UI / 状态同步
- 运行时编排
- 多业务域辅助函数
- 较重的流程路由

这种结构在短期内仍可继续加功能，但会持续放大以下成本：

- 新功能落点越来越依赖作者记忆，而不是稳定模块边界；
- 热点文件 merge conflict 概率持续上升；
- 回归排查时需要跨越过长的上下文窗口；
- 局部优化难以做到“只看一个功能域”。

Git 历史也验证了这一点：`d7c0e73`、`c5565ca`、`84b3fd3`、`1f5817c` 显著放大了 `bootstrap.rs` / `view_model.rs`；`19cfa3d`、`bce7de8`、`8de48d4`、`32c8238`、`4e74759`、`4348081` 持续放大了 `src/app/ssh/runtime.rs`。

## 目标

### 本轮目标

- 在不改变业务行为、不改变已有 UI 交互语义的前提下，拆分三个超大文件；
- 保持外部稳定入口，优先避免对现有调用方产生路径级震荡；
- 让 `bootstrap`、`view_model`、`runtime` 的职责边界回到“可解释、可导航、可扩展”的状态；
- 降低重复辅助逻辑、超长函数和巨型 binder 的维护成本；
- 为后续 Windows 11 首发体验继续迭代留出更清晰的演进面；
- 仅在真正不自解释的复杂 orchestration / ownership 边界处增加少量备注，不把注释当作结构问题的替代品。

### 结构目标

- `bootstrap` 回到“应用装配与回调编排入口”的角色，而不是所有子域 helper 的永久堆积点；
- `ShellViewModel` 保持单一状态中心，但把高密度行为簇拆出清晰的文件边界；
- `ssh::runtime` 保持稳定 facade，同时把 transport / auth / terminal engine 等重块拆开；
- 后续开发者可以根据功能域直接定位代码，而不是先在超大文件里全文检索。

## 非目标 / 边界

本轮设计明确不包含：

- 新增任何业务功能；
- 修改 Slint 回调契约语义；
- 修改 session / sync / SFTP / keychain 的产品行为；
- 强行把所有状态再拆成多个 owner struct；
- 为了拆分而改 public API 路径；
- 直接产出 implementation plan；用户未要求时不进入该阶段。

补充边界约束：

- `src/app/bootstrap.rs` 的公开 `bind_*` / `run*` 入口应保持现有调用面稳定；
- `src/shell/view_model.rs` 仍以 `ShellViewModel` 为状态所有者；
- `src/app/ssh/runtime.rs` 仍保留 `crate::app::ssh::runtime` 这一稳定入口；
- 本轮允许内部模块化与少量 helper / dispatcher 抽离，但不允许演变成新的抽象层迷宫。

## 当前实现现状

### 1. `src/app/bootstrap.rs`

- 文件总长约 11039 行；
- `bind_top_status_bar_with_store_and_profile_and_effects_and_session_bridge` 从 `src/app/bootstrap.rs:7077` 开始，是当前超大的总 binder；
- 文件内部大致混合了：依赖装配、`ShellViewModel -> AppWindow` 投影、workspace terminal 协调、SFTP panel 协调、vault sync 与 recovery、Windows window hook；
- 单文件内约有 111 个 `window.on_*` 回调绑定；
- 明显热点包括：`sync_workspace_projection_from_manager`、`forward_active_workspace_*`、`sync_local_vault`、`update_sync_modal_for_local_state`、`sync_asset_modal_state`。

### 2. `src/shell/view_model.rs`

- 文件总长约 5347 行；
- `ShellViewModel` 定义位于 `src/shell/view_model.rs:501`，当前约有 63 个字段；
- 文件中约有 240 个 `pub fn`，且大量方法都在同一所有者上直接改写多组状态；
- 主要状态域包括：shell chrome、assets / snippets / keychain、saved ssh picker / quick launch、SFTP session 状态、workspace tabs / terminal surface、asset modal / ssh modal / host key prompt / context menu；
- 最大维护热点不是单一小函数，而是几条跨域流程：`confirm_asset_modal`、`confirm_delete_asset`、`update_ssh_modal_field`、`begin_ssh_modal_action`、`handle_context_menu_leaf_action`。

### 3. `src/app/ssh/runtime.rs`

- 文件总长约 3345 行；
- 已经不是单纯 runtime，而是 transport / proxy、auth、host key 校验、progress reporter、channel pump、terminal engine、SFTP backend、public contracts 的混合总装；
- `connect_target_handle_for_profile` 位于 `src/app/ssh/runtime.rs:165` 左右，是 transport / proxy 主入口；
- `SshSessionRuntime` 与一组 surface / key / mouse DTO 位于 `src/app/ssh/runtime.rs:791-1568`；
- `run_channel_pump` 位于 `src/app/ssh/runtime.rs:1869` 左右；
- `TerminalSession` 及其 projection / input / filter / color helpers 位于 `src/app/ssh/runtime.rs:2201-3240`，是当前最大的单块职责域。

### 4. Git 历史结论

- `bootstrap.rs` 近期主要由 native terminal surface recovery、vault sync background service、keychain completion 等功能持续放大；
- `view_model.rs` 的膨胀主要来自 keychain、snippet、SFTP right panel、SSH modal / runtime tabs / quick launch 这些 feature 持续挂到同一个状态中心；
- `runtime.rs` 的膨胀主要来自 terminal 输入渲染契约、SOCKS5 / HTTP / multi-hop proxy、connection progress timeline、SFTP 适配和 enhanced session；
- 这说明本轮问题的根因是“多功能域共同堆积在单文件”，而不是少数几段代码没有格式化好。

## 设计要点拆分

### 设计要点 1：全局拆分形态与路径稳定策略

#### 方案 A：保留现有入口文件路径，对内拆为子模块

- 保持外部入口文件名与调用路径稳定；
- 入口文件变成薄 facade / composition root；
- 真实实现拆到内部子模块；
- 优点是迁移风险低、现有调用点基本不动、与项目当前目录模块化风格一致；
- 缺点是第一轮需要精心设计内部模块边界，避免共享依赖横飞。

#### 方案 B：直接切换到 `mod.rs` 目录模块形态

- 直接把 3 个文件都改成目录模块；
- 优点是形态更标准；
- 缺点是路径 churn 更大、Git 历史连续性更弱、回滚成本更高。

#### 最终决策

采用方案 A。

### 设计要点 2：`src/app/bootstrap.rs` 的拆分方式

#### 方案 A：按功能域拆分

建议边界：

- `api / composition_root`
- `shell_chrome / windowing`
- `workspace_terminal`
- `sftp`
- `assets_keychain`
- `vault_sync`

优点：最贴近 Git 演进和当前代码聚类，后续阅读与并行维护最自然。

#### 方案 B：按 binder / sync / action / services 分层

- 优点是读写方向清晰；
- 缺点是单条业务链路会横跨多个文件，不利于快速排障。

#### 最终决策

采用方案 A。

### 设计要点 3：`src/shell/view_model.rs` 的拆分粒度

#### 方案 A：仅按域拆成多个 `impl ShellViewModel` 文件

- 保持单一 owner；
- 把 `validation`、`sftp`、`workspace`、`quick_launch`、`assets`、`context_menu`、`modals`、`projection` 分文件；
- 优点是风险最低；
- 缺点是 `confirm_asset_modal`、SSH modal 全链路、context menu dispatcher 仍然会太重。

#### 方案 B：域拆分 + 选择性引入内部 helper / dispatcher

- 保持 `ShellViewModel` 为唯一状态中心；
- 除了分文件，还允许把最肥的流程抽成少量内部 helper：
  - `asset modal executor`
  - `ssh modal state machine`
  - `context menu dispatcher`
- 优点是既不打散状态所有权，又能真正削减交通枢纽函数的复杂度；
- 缺点是比纯搬文件多一层内部协作接口，需要控制抽象数量。

#### 最终决策

采用方案 B。

### 设计要点 4：`src/app/ssh/runtime.rs` 的边界与 facade 策略

#### 方案 A：保留 `crate::app::ssh::runtime` facade，对内拆模块

建议边界：

- `contracts`
- `transport`
- `auth`
- `pump`
- `terminal`
- `sftp_backend`

优点：既保留现有调用面，也能把 transport / auth / terminal engine 分开维护。

#### 方案 B：先只抽 `TerminalSession` 一大块

- 优点是第一刀最保守；
- 缺点是 transport / auth / progress 仍旧混在主文件里，结构收益不完整。

#### 最终决策

采用方案 A。

## 方案对比结论

- 全局模块化策略：选 A，不做大规模路径震荡；
- `bootstrap`：选 A，按功能域拆，而不是按抽象层拆；
- `view_model`：选 B，在域拆分基础上允许少量内部执行器 / dispatcher；
- `ssh::runtime`：选 A，保留 facade，内部按职责重构。

这组选择的核心原则是：

- 优先稳定外部路径；
- 优先按功能域聚合阅读面；
- 只在 `view_model` 里对真正失控的流程引入选择性 helper；
- 不把“拆分”误做成“重新发明一套新架构”。

## 最终决策

### 1. `src/app/bootstrap.rs`

- 保留文件入口；
- 对内拆成功能域子模块；
- 主文件只保留公共入口、装配入口、少量必须留在 root 的共享定义；
- 大型 `window.on_*` 绑定按域拆成多个 binder 函数；
- `vault_sync`、`workspace_terminal` 作为优先拆分对象。

### 2. `src/shell/view_model.rs`

- 升级为 `view_model/` 子模块目录；
- `ShellViewModel` 结构体定义和最核心共享类型保留在主入口；
- 各域 `impl ShellViewModel` 按文件拆开；
- 对 `confirm_asset_modal`、SSH modal 全链路、context menu leaf dispatch 允许引入内部 helper / dispatcher；
- 暂不把状态拆成多个 owner struct。

### 3. `src/app/ssh/runtime.rs`

- 保留 `crate::app::ssh::runtime` 作为稳定 facade；
- surface / input / event DTO 保持稳定导出；
- transport、auth、pump、terminal、SFTP backend 拆到内部模块；
- `TransportChainGuard` 的保活语义必须显式保留，避免重构后出现 jump host 生命周期回归。

## 实施步骤

1. 先建立 3 组目标模块骨架与 re-export 结构，但不改业务语义；
2. 优先移动 `bootstrap` 中最独立的 `vault_sync` 与 `workspace_terminal`；
3. 再拆 `bootstrap` 的 `assets_keychain`、`sftp`、`shell_chrome / windowing`；
4. 将 `ShellViewModel` 按域拆成多个 `impl` 文件；
5. 在 `view_model` 中追加内部 helper / dispatcher，仅处理最肥大的执行流程；
6. 将 `ssh::runtime` 拆成 facade + internal modules，并先固定 contracts 导出面；
7. 最后回到 root 文件，删除重复 helper、补少量备注、收敛 import 与 re-export。

## 风险与回滚策略

### 主要风险

- `bootstrap` 回调绑定拆分后，闭包捕获集合发生变化，可能引入生命周期或行为回归；
- `view_model` 在 helper / dispatcher 抽离时，如果边界设计不当，容易出现状态修改顺序变化；
- `runtime` 在拆 transport / auth / terminal 时，可能误伤 `TransportChainGuard`、`RusshSftpBackend`、event contract；
- 过度抽象会让“文件变小了，但理解路径更绕”。

### 回滚策略

- 每个文件群分阶段拆分，避免一次性大迁移；
- 每一阶段都保持 facade 与 public contract 稳定；
- 若某一域拆分后验证成本明显高于收益，可回退到“仅文件级拆分，不引入额外 helper”的保守形态；
- 若 `view_model` helper 抽离造成理解成本升高，可只保留文件拆分，撤回额外 dispatcher。

## 验证清单

- `bootstrap` 对外公开入口名称、参数、可调用路径保持兼容；
- `ShellViewModel` 仍是唯一状态 owner，没有因为拆分引入多 owner 漂移；
- `ssh::runtime` 的 public contracts 仍可被 `bootstrap`、`session_manager`、terminal 相关模块直接使用；
- `window.on_*` 回调注册职责按域分组后，仍能完整覆盖现有交互面；
- `confirm_asset_modal`、SSH modal、context menu leaf dispatch 的复杂度相较当前实质下降；
- 重构后新增注释只出现在复杂 ownership、异步编排、隐式保活等非显然位置；
- Git diff 应体现为“结构收敛”，而不是掺杂新功能。

## 参考

- 代码现状：`src/app/bootstrap.rs`、`src/shell/view_model.rs`、`src/app/ssh/runtime.rs`
- 周边模块：`src/app/ssh/mod.rs`、`src/app/sftp/mod.rs`、`src/shell/mod.rs`
- 关键演进提交：`d7c0e73`、`c5565ca`、`84b3fd3`、`1f5817c`、`19cfa3d`、`bce7de8`、`8de48d4`、`32c8238`、`4e74759`、`4348081`、`76d1fd4`、`4f974fd`

## 最终落地备注

本设计文档只确认结构化拆分策略，不批准任何业务行为变更。
后续若进入实施阶段，应另开 implementation plan，并以本设计文档作为唯一结构边界依据。

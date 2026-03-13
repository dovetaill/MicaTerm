# Mica Term FemtoVG WGPU Experimental Build Design

日期: 2026-03-12  
执行者: Codex  
状态: 已确认方案，待进入实现规划

## 背景

当前仓库已经回到单一正式 renderer 路线：

- `Cargo.toml` 默认只保留 `slint-renderer-software`
- `src/main.rs` 固定走 `AppRuntimeProfile::formal()`
- `src/app/runtime_profile.rs` 当前只表达 `Formal + Software`
- `build-release.sh` 继续只服务正式链：
  - `x86_64-unknown-linux-gnu`
  - `x86_64-pc-windows-gnu`

与此同时，仓库中已经保留了一份旁路探索文档：

- `docs/plans/try-winit-femtovg-wgpu.md`

该文档明确把 `winit-femtovg-wgpu` 定义为后续可单独验证的方向，而不是当前正式默认路线。

本轮任务不是替换正式链，也不是接入真实 terminal renderer，而是在现有 `Rust + Slint + winit` 壳层上，为：

- `Linux x64`
- `Windows x64 MSVC`

新增一条严格独立、严格纯净的 `winit-femtovg-wgpu experimental` 构建方式，用于观察：

- renderer 是否能稳定启动
- 现有 shell / titlebar / sidebar / theme / resize 交互是否保持正常
- Linux 与 Windows 的实验结果是否存在明显差异

本轮还有一个强约束：

- 实验链不能混入 `winit-software`
- 不能保留或借用现有 `software + workaround` 语义
- 一旦不满足 renderer 条件，必须明确失败退出，而不是静默回退

## 目标

- 在不修改正式发布链语义的前提下，为当前主应用新增一条纯 `winit-femtovg-wgpu experimental` 构建路径
- 让实验链同时覆盖：
  - `Linux x64`
  - `Windows x64 MSVC`
- 保证实验链继续复用现有 `mica-term` 主 binary，而不是新起一个独立 app
- 保证 experimental renderer 的真源在程序内部，而不是脚本或环境变量
- 明确实验包的窗口标题、失败文案、日志元数据，避免与正式链混淆
- 保持实验包与正式包在打包产物名称上完全可区分
- 为后续是否继续投资 `FemtoVG WGPU` 路线提供可复核证据

## 边界

### 本文档覆盖

- `Cargo.toml` feature 拓扑设计
- experimental runtime profile 语义
- `BackendSelector` 锁定策略
- Linux / Windows experimental wrapper 组织方式
- experimental 包命名与打包产物命名
- experimental 启动失败路径
- 实验边界内的验证项、风险与回滚策略

### 本文档不覆盖

- `wezterm-term` / `termwiz` 真实 terminal surface 接入
- `russh` / `SFTP` 连接逻辑
- `Tokio runtime` 结构调整
- Fluent 动效、视觉 polish 或 GPU 调优
- macOS / Android / iOS 的 experimental 扩展
- 正式链默认 renderer 替换
- 逐文件 implementation plan 与命令级落地步骤

## 调研结论

### 近期 Git 历史

本轮设计直接参考了以下近期提交与文档脉络：

- `2cff7af feat: implement build chain and renderer strategy`
- `b503515 fix: disable theme redraw recovery for skia experimental`
- `6c5b45e docs: add windows theme repro and skia removal design`
- `fefd806 旁路探索文档 Try Winit FemtoVG WGPU`
- `c94d6a0 fix: harden frameless resize and titlebar drag`
- `934b225 Remove windows theme repro and recovery path`

这些记录说明仓库已经经历过一轮：

- `Skia experimental` 的 feature / script / runtime profile 设计
- 随后又把主线重新收敛回单一正式 `software` 路线
- 并把 `winit-femtovg-wgpu` 留在 try 文档，而不是主方案

因此，本轮最合理的做法不是“恢复旧 experimental 分支”，而是基于已经验证过的模式，重新定义一条更严格的 `FemtoVG WGPU experimental` 链。

### 当前源码事实

当前仓库中可确认的关键事实如下：

- `Cargo.toml` 默认只有 `slint-renderer-software`
- `src/main.rs` 没有任何 renderer 选择逻辑
- `src/app/runtime_profile.rs` 当前只保留 `Formal + Software`
- `src/app/logging/runtime.rs` 已经具备输出 `build_flavor / renderer_mode / forced_backend` 的日志元数据能力
- `ui/app-window.slint` 的主工作区仍然是 `WelcomeView` 占位，不是实际 terminal renderer
- `build-release.sh` 继续只构建正式链，不包含任何 experimental 语义
- `build-desktop.sh` 已经具备通过：
  - `CARGO_NO_DEFAULT_FEATURES`
  - `CARGO_FEATURES`
  - `TARGET`
  控制构建产物的通用能力

这意味着本轮实验可以聚焦在：

- renderer 构建链
- runtime lock
- 壳层窗口交互

而不需要假装自己已经在验证终端渲染本体。

### 外部资料结论

根据 Slint 官方文档与上游源码：

- Slint 官方支持 `renderer-femtovg-wgpu`
- 启用该路线需要配合 `unstable-wgpu-28`
- `BackendSelector` 是 `SLINT_BACKEND` 的程序内替代物
- 当 `BackendSelector` 已经显式设置 `backend` 与 `renderer` 时，不会再被外部 `SLINT_BACKEND` 覆盖

参考：

- https://docs.slint.dev/latest/docs/slint/guide/backends-and-renderers/backends_and_renderers/
- https://docs.slint.dev/latest/docs/slint/guide/backends-and-renderers/backend_winit/
- https://docs.slint.dev/latest/docs/rust/slint/struct.BackendSelector
- https://github.com/slint-ui/slint/blob/03ce660629568927f9eb6b779dd9aa2e0a9aeb7e/api/rs/slint/Cargo.toml
- https://github.com/slint-ui/slint/blob/03ce660629568927f9eb6b779dd9aa2e0a9aeb7e/internal/backends/selector/api.rs

## 设计原则

- `Formal Release Must Stay Untouched`  
  正式链继续只承担稳定发布职责，不混入实验路线。
- `Experimental Must Be Pure`  
  experimental 包要么真以 `winit-femtovg-wgpu` 启动，要么明确失败退出。
- `Runtime Truth Lives Inside The App`  
  renderer 选择真源必须在程序内部，而不是脚本或环境变量。
- `One App Identity, Two Build Flavors`  
  继续复用 `mica-term` 主 binary，但要用 app-level feature 明确 formal / experimental 身份。
- `Verification Must Be Comparable Across Linux And Windows`  
  Linux 与 Windows 的实验链必须拥有相同的 feature 语义、标题标识和日志契约。
- `No Workaround Leakage`  
  实验链不能夹带 `software` workaround，否则结论失真。

## 方案对比

### 1. 平台范围

#### 方案 A：只做 Linux

优点：

- 最保守
- 与 `try-winit-femtovg-wgpu.md` 早期建议一致

缺点：

- 无法同时观察 Windows 行为

#### 方案 B：只做 Windows

优点：

- 更贴近 Fluent / Windows 11 产品目标

缺点：

- 失去 Linux 这一主使用平台的直接证据

#### 方案 C：Linux x64 + Windows x64 MSVC 同时纳入实验链

优点：

- 同时覆盖主使用平台与产品关注平台
- 更容易比较跨平台差异

缺点：

- 实验矩阵更大

**最终选择：`方案 C`**

### 2. renderer 路线

#### 方案 A：`pure winit-femtovg`

优点：

- 更保守
- WGPU 变量更少

缺点：

- 与现有 try 文档不完全对齐
- 对未来 GPU 路线的验证力度不足

#### 方案 B：`pure winit-femtovg-wgpu`

优点：

- 与仓库中的 try 文档方向一致
- 更接近未来 GPU renderer 验证目标

缺点：

- 对平台与驱动环境要求更高

**最终选择：`方案 B`**

### 3. renderer 锁定方式

#### 方案 A：通过 `SLINT_BACKEND` 环境变量锁定

优点：

- 写法简单

缺点：

- 语义不够硬
- 容易把程序内真源重新退化为脚本约定

#### 方案 B：通过 `BackendSelector` 程序内锁定

优点：

- 真源在程序内部
- 可以明确失败并返回错误
- 不再依赖外部环境变量约定

缺点：

- 启动逻辑比 `set_var` 略复杂

**最终选择：`方案 B`**

### 4. binary 组织方式

#### 方案 A：继续复用 `mica-term` 主 binary

优点：

- 与之前的 experimental build 思路一致
- 后续若实验成功，更容易回灌主线

缺点：

- 必须额外设计 build flavor 标识，避免混淆

#### 方案 B：新起独立 binary

优点：

- 边界天然干净

缺点：

- 会把“实验构建”升级成“另一款 app”

**最终选择：`方案 A`**

### 5. 构建入口组织

#### 方案 A：新增两个显式 experimental wrapper

- `build-linux-x64-femtovg-wgpu.sh`
- `build-win-x64-femtovg-wgpu.sh`

优点：

- 使用语义最清晰
- 与现有 wrapper 风格一致

缺点：

- repo-root 会多两个很薄的脚本

#### 方案 B：只新增一个通用 experimental wrapper

优点：

- 文件更少

缺点：

- Linux / Windows 入口不够直观

**最终选择：`方案 A`**

### 6. feature 拓扑

#### 方案 A：单一 experimental feature

优点：

- 定义最简单

缺点：

- renderer 能力与 app 身份耦合过紧

#### 方案 B：分层 feature

做法：

- `slint-renderer-femtovg-wgpu`
- `femtovg-wgpu-experimental`

优点：

- 语义清晰
- renderer 能力与 app 身份分层
- 更符合后续扩展

缺点：

- 比单一 feature 多一层定义

**最终选择：`方案 B`**

### 7. 正式链总控是否纳入 experimental

#### 方案 A：实验链完全独立于 `build-release.sh`

优点：

- 正式 / 实验边界最干净

缺点：

- 需要单独记住 experimental wrapper

#### 方案 B：给 `build-release.sh` 增加 experimental mode

优点：

- 总入口更统一

缺点：

- 会重新污染正式总控语义

**最终选择：`方案 A`**

### 8. 产物命名

#### 方案 A：保留可执行文件名 `mica-term`，但 archive / stage dir 带 experimental 后缀

优点：

- 仍然是同一个 app
- 打包产物不与正式链撞名

缺点：

- 需要额外设计产物命名规则

#### 方案 B：产物也继续完全叫 `mica-term`

优点：

- 改动最少

缺点：

- 极易与正式包混淆

**最终选择：`方案 A`**

## 最终决策

本轮最终确认的方案如下：

- 正式链不变：
  - `build-release.sh` 继续只服务正式包
  - 默认 renderer 继续是 `software`
- 新增 experimental 链，仅覆盖：
  - `x86_64-unknown-linux-gnu`
  - `x86_64-pc-windows-msvc`
- experimental 构建形态为：
  - `pure winit-femtovg-wgpu`
  - `--no-default-features`
  - `--features femtovg-wgpu-experimental`
- `Cargo.toml` 采用分层 feature：
  - `slint-renderer-femtovg-wgpu`
  - `femtovg-wgpu-experimental`
- experimental runtime profile 使用 `BackendSelector` 硬锁定：
  - `backend = winit`
  - `renderer = femtovg-wgpu`
  - `require_wgpu_28(WGPUConfiguration::default())`
- experimental 不启用任何 `software` workaround
- experimental 标识显式暴露：
  - 窗口标题带 `FemtoVG WGPU Experimental`
  - 启动失败文案明确指向 `winit-femtovg-wgpu`
  - runtime metadata 日志明确记录 build flavor 与 renderer mode
- 新增两个 experimental wrapper：
  - `build-linux-x64-femtovg-wgpu.sh`
  - `build-win-x64-femtovg-wgpu.sh`
- 可执行文件名仍为 `mica-term`
- 打包产物名显式带 experimental 后缀，例如：
  - `mica-term-femtovg-wgpu-experimental-x86_64-unknown-linux-gnu-release.tar.gz`
  - `mica-term-femtovg-wgpu-experimental-x86_64-pc-windows-msvc-release.zip`

## 实施步骤

1. 定义 `Cargo.toml` 分层 feature 拓扑
2. 扩展 `AppRuntimeProfile`，引入 formal / experimental 两类 build flavor
3. 在 `main` 启动阶段增加 profile 选择与 experimental selector 入口
4. 通过 `BackendSelector` 硬锁定 `winit + femtovg-wgpu + wgpu-28`
5. 为 experimental profile 增加标题、失败文案与日志元数据契约
6. 明确关闭 experimental profile 下的 workaround 语义
7. 新增 Linux / Windows 两个 experimental wrapper，并限制 target 边界
8. 调整 stage dir / archive naming，确保不与正式链冲突
9. 增补测试、README 与 verification 文档，使实验链可被重复验证

## 风险与回滚

### 风险 1：Linux 成功但 Windows MSVC 失败

这是允许结果，不代表设计错误。

处理策略：

- 记录为平台差异结论
- 保留实验链
- 不影响正式链

### 风险 2：实验链启动成功但窗口交互回归

处理策略：

- 保留实验链仅作 try
- 不提升为正式方案

### 风险 3：WGPU 相关依赖或上游 API 组合不稳定

处理策略：

- 明确记录实验失败
- 回滚 experimental 增量
- 不允许把回滚方式变成 mixed renderer 或 software fallback

### 风险 4：experimental 与正式包混淆

处理策略：

- 标题显式标识
- 启动失败文案显式标识
- archive / stage dir 显式标识
- 日志元数据显式标识

### 回滚原则

- 只回滚 experimental 相关增量
- 不改正式链默认路径
- 不恢复“实验链偷偷回退到 software”的妥协实现

## 验证清单

- [ ] `Cargo.toml` 中存在分层 feature：
  - `slint-renderer-femtovg-wgpu`
  - `femtovg-wgpu-experimental`
- [ ] 正式链默认仍然只走 `software`
- [ ] experimental 构建必须使用 `--no-default-features`
- [ ] experimental runtime profile 能明确区分 build flavor 与 renderer mode
- [ ] experimental 在创建任何 Slint window 前完成 selector
- [ ] experimental selector 明确锁定：
  - `winit`
  - `femtovg-wgpu`
  - `wgpu-28`
- [ ] external `SLINT_BACKEND` 不会覆盖 experimental 选择
- [ ] experimental 不启用 workaround
- [ ] 正式链 `build-release.sh` 不包含任何 experimental 逻辑
- [ ] 新增两个 experimental wrapper：
  - `build-linux-x64-femtovg-wgpu.sh`
  - `build-win-x64-femtovg-wgpu.sh`
- [ ] wrapper 只允许：
  - `x86_64-unknown-linux-gnu`
  - `x86_64-pc-windows-msvc`
- [ ] 可执行文件仍然叫 `mica-term`
- [ ] archive / stage dir 带 experimental 后缀
- [ ] experimental 窗口标题带 `FemtoVG WGPU Experimental`
- [ ] experimental 启动失败时输出明确失败文案
- [ ] runtime metadata 可见：
  - `build_flavor`
  - `renderer_mode`
  - `forced_backend`
- [ ] Linux x64 启动成功率、resize、maximize / restore、theme toggle 可人工验证
- [ ] Windows x64 MSVC 启动成功率、resize、maximize / restore、theme toggle 可人工验证

## 参考资料

- Slint backends and renderers  
  https://docs.slint.dev/latest/docs/slint/guide/backends-and-renderers/backends_and_renderers/
- Slint winit backend  
  https://docs.slint.dev/latest/docs/slint/guide/backends-and-renderers/backend_winit/
- Slint `BackendSelector`  
  https://docs.slint.dev/latest/docs/rust/slint/struct.BackendSelector
- Slint Rust crate features  
  https://github.com/slint-ui/slint/blob/03ce660629568927f9eb6b779dd9aa2e0a9aeb7e/api/rs/slint/Cargo.toml
- Slint backend selector source  
  https://github.com/slint-ui/slint/blob/03ce660629568927f9eb6b779dd9aa2e0a9aeb7e/internal/backends/selector/api.rs
- 本仓库旁路探索文档  
  `docs/plans/try-winit-femtovg-wgpu.md`

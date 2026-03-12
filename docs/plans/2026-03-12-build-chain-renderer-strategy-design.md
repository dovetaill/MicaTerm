# Mica Term Build Chain And Renderer Strategy Design

日期: 2026-03-12  
执行者: Codex  
状态: 已确认方案，待进入实现规划

## 背景

当前仓库已经围绕 `Theme Toggle Offscreen Bug` 做过一轮定位和兜底修复，但围绕 renderer 与发布链的策略仍未最终收敛。

本轮讨论中，产品约束被进一步明确为：

- 第一优先级不是单独追求 Windows 上的 Skia 视觉效果
- 第一优先级是让 `Debian Linux` 成为正式构建机，稳定产出：
  - `Linux x64` 正式包
  - `Windows x64 GNU` 正式包
- 同时保留一条独立的 `Windows MSVC + Skia` 实验链，用于验证 Slint `Skia` renderer 对当前 `offscreen` 历史问题的真实影响

结合仓库现状，这不是单一 UI bug 修复问题，而是一次构建链、renderer、实验边界与后续平台迁移策略的收敛设计。

## 目标

- 明确正式发布链和实验链的边界，避免一条链路同时承担“稳定发布”和“renderer 试验”两种职责
- 保持 `Debian` 为正式链唯一总控构建机
- 为 `Windows x64 GNU` 正式包定义稳定 renderer 策略
- 为 `Windows MSVC pure Skia experimental` 包定义可验证、可追踪、可失败即退出的实验策略
- 保留当前 `offscreen workaround`，直到 Skia 实验在真实环境中证明它可以被安全移除
- 保持该设计只作用于当前 `shell / window / theme / build` 层，不提前耦合未来的 `wezterm-term`、`termwiz`、`russh`、`SFTP`

## 边界

### 本文档覆盖

- 正式发布链与实验链的构建职责划分
- `Linux x64` 与 `Windows x64 GNU` 正式 renderer 选择
- `Windows MSVC pure Skia experimental` 包的身份、启动方式、失败策略
- `offscreen workaround` 在正式链和实验链中的保留策略
- 正式总控脚本的行为边界
- 旁路尝试方案 `winit-femtovg-wgpu` 的文档组织方式

### 本文档不覆盖

- `wezterm-term` / `termwiz` 真实 terminal surface 接入
- `russh` / `SFTP` 会话逻辑
- Slint 组件视觉重绘或 Fluent 动效重设计
- CI 接入细节
- 逐文件实现命令级 implementation plan

## 调研结论

### Git 历史

与本轮决策直接相关的近期提交包括：

- `991d032 修复 Light / Dark 切换时窗口超出屏幕区域出现不完全切换的问题`
- `1d35840 feat: sync theme toggle with native window appearance`
- `240ab67 fix: stabilize theme toggle redraw recovery`
- `2e4f9fb merge: theme toggle redraw recovery`

这些提交说明，仓库已经完成了：

- 主题切换与原生 `window appearance` 的桥接
- 一个面向 Windows 的 `offscreen redraw recovery workaround`
- 一个 `windows-skia-experimental` feature 与 `build-win-x64-skia.sh` 入口

但这些提交尚未完成“正式链与实验链的长期策略收敛”。

### 当前源码证据

当前仓库的关键事实如下：

- `Cargo.toml` 默认 feature 仍然是 `slint-renderer-software`
- `windows-skia-experimental` 只是额外启用 `slint/renderer-skia`
- `build-win-x64-skia.sh` 只设置：
  - `TARGET=x86_64-pc-windows-msvc`
  - `CARGO_FEATURES=windows-skia-experimental`
- `build-desktop.sh` 当前只是把 feature 传给 `cargo build`，默认不会自动 `--no-default-features`
- `src/app/bootstrap.rs` 里的 `ThemeRedrawRecovery` 与 `request_inner_size(+1 -> restore)` 逻辑按 `target_os = "windows"` 编译，而不是按“Skia 实验包”分支
- `ui/app-window.slint` 的 `render-revision` 仍然是 workaround 闭环的一部分
- 当前主内容区仍然是 `WelcomeView` 占位，真实 terminal runtime 尚未接入

这意味着：

- 现有 `build-win-x64-skia.sh` 更接近“让二进制具备 Skia 能力”
- 它还不是“纯 Skia、程序内部真锁定、拥有独立实验身份”的完整实验链

### 外部资料

官方和上游资料给出了几条关键约束：

- Slint `winit` backend 支持：
  - `winit-software`
  - `winit-femtovg`
  - `winit-femtovg-wgpu`
  - `winit-skia`
  - `winit-skia-software`
- Slint 支持通过 `SLINT_BACKEND` 或程序内 `BackendSelector` 指定 renderer
- 当前仓库自己已验证：
  - `windows-skia-experimental` 不适用于 `Debian Linux -> x86_64-pc-windows-gnu` 正式打包链
  - 更现实的 Skia 实验目标是 `Windows MSVC`

参考：

- https://docs.slint.dev/latest/docs/slint/guide/backends-and-renderers/backends_and_renderers/
- https://docs.slint.dev/latest/docs/slint/guide/backends-and-renderers/backend_winit/
- https://docs.slint.dev/latest/docs/rust/slint/struct.BackendSelector
- https://github.com/slint-ui/slint/pull/7934
- https://github.com/slint-ui/slint/pull/6833
- https://github.com/rust-skia/rust-skia

## 设计原则

- `Debian Owns Formal Release`
  正式发布链必须由 Debian 总控，不依赖 Windows 机器参与正式产物生成。
- `Formal And Experimental Must Be Separated`
  正式链追求稳定，实验链追求验证，不让同一条链承担双重目标。
- `Renderer Truth Lives Inside The App`
  renderer 的最终真源必须在程序内部，而不是依赖 shell 环境变量。
- `Experimental Results Must Be Clean`
  一份标记为 `Skia Experimental` 的包，要么真以 `Skia` 启动，要么明确失败退出，不能静默回退。
- `Workaround Stays Until Proven Unnecessary`
  在 Skia 实验未完成真实机验证前，不移除现有 `offscreen workaround`。
- `Do Not Couple Future Terminal Runtime Early`
  当前决策只收敛构建链与 renderer 策略，不把未来 terminal runtime 一起绑进来。

## 设计要点与方案对比

### 1. 发布链结构

#### 方案 A：Debian 主发布链 + 独立 Windows Skia 实验链

做法：

- Debian 负责正式包：
  - `Linux x64`
  - `Windows x64 GNU`
- Windows MSVC 机器负责独立的 `Skia experimental` 包

优点：

- 最符合当前产品约束
- 正式链不被 Skia 构建限制卡住
- 可以持续验证 Skia，而不污染正式发布策略

缺点：

- 发布矩阵变成“正式链 + 实验链”双层结构

#### 方案 B：Debian 同时承担正式链和 Skia 实验链

优点：

- 全部产物都由一台构建机统一产出

缺点：

- 与当前仓库已验证事实冲突
- `windows-skia-experimental` 不适合现有 `Linux -> Windows GNU` 路径

**最终决策：选择 `方案 A`**

### 2. 正式包 renderer 策略

#### 方案 A1：`Linux x64` 正式包使用 `winit-software`

优点：

- 与当前仓库默认 feature 一致
- 依赖最少
- 最适合作为 Debian 正式链的保守默认值

缺点：

- 视觉和动效上限一般

**最终决策：选择 `A1`**

#### 方案 A2：`Windows x64 GNU` 正式包使用 `winit-software + workaround`

优点：

- 与当前仓库现状一致
- 不受 Skia 构建链限制
- `offscreen` 问题已有明确兜底

缺点：

- 正式链仍保留 renderer-software 路径的历史包袱

**最终决策：选择 `A2`**

#### 方案 B：正式包尝试 `winit-femtovg-wgpu`

优点：

- 更符合未来 Fluent 动效方向

缺点：

- GPU / 驱动 / Wayland / X11 变体更多
- 没有证据表明它能直接替代当前正式链
- 不适合作为本轮正式默认值

**结论：不作为当前正式方案，另存 try 文档**

### 3. Skia 实验包的编译形态

#### 方案 A：实验包同时编入 `software + skia`

优点：

- 出问题时更容易临时回旋

缺点：

- 实验结果不纯
- 容易让“当前到底是不是在测 Skia”失去确定性

#### 方案 B：实验包是 `pure Skia`

做法：

- 实验包不再携带默认 `software renderer`
- 将 `Skia experimental` 定义为一个纯实验二进制形态

优点：

- 实验语义最干净
- 结果最有说服力

缺点：

- 一旦 Skia 初始化失败，包无法像多 renderer 包那样自行回退

**最终决策：选择 `方案 B`**

### 4. 实验包身份识别

#### 方案 A：仅按 `target` 识别

缺点：

- `Windows MSVC` 不等于 `Skia experimental`
- 不能精准区分正式 Windows 包和实验 Windows 包

#### 方案 B：按 `app-level feature` 识别

做法：

- 引入应用层实验 feature，作为“这是一份 Skia experimental 包”的唯一身份标记

优点：

- 语义清晰
- 方便日志、诊断、窗口标题、关于页等统一标注
- 为未来实验包差异化行为提供稳定入口

缺点：

- 比单纯靠 `target` 多一层 feature 设计

#### 方案 C：按环境变量识别

缺点：

- 易误用
- 打包产物不自描述

**最终决策：选择 `方案 B`**

### 5. Skia renderer 锁定策略

#### 方案 A：只靠外层脚本设置 `SLINT_BACKEND`

优点：

- 改动轻

缺点：

- 一旦用户直接启动 exe 或环境变量丢失，实验语义就不再可信

#### 方案 B：程序内部锁定 renderer，外层不参与

优点：

- 真源清晰

缺点：

- 外层缺乏可观测标记，不利于快速确认包身份

#### 方案 C：程序内部真锁定，外层脚本只做标记/校验

做法：

- 程序内部主动选择 `winit-skia-software`
- 外层脚本、包名、日志元数据只负责说明这是 `Skia Experimental`

优点：

- renderer 真源清晰
- 包身份和排查线索也清晰
- 最适合长期维护

缺点：

- 比单层策略多一层设计

**最终决策：选择 `方案 C`**

### 6. 外部变量冲突与启动失败策略

#### 方案 A1：实验包遇到冲突 `SLINT_BACKEND` 时，忽略外部变量并继续强制 Skia

优点：

- 符合“如果是 Skia 模式，那就只有 Skia”的前提
- 避免实验结果被外部环境污染

缺点：

- 需要在日志中明确提示覆盖行为

**最终决策：选择 `A1`**

#### 方案 B1：Skia 初始化失败时，明确提示后退出

优点：

- 比静默退出更易定位
- 比自动降级更符合纯实验包定义

缺点：

- 需要补一层启动失败展示逻辑

**最终决策：选择 `B1`**

#### 方案 C1：初始化失败后自动降级到其他 renderer

缺点：

- 直接破坏 `pure Skia experimental` 的语义

**结论：拒绝采用**

### 7. `offscreen workaround` 策略

#### 方案 A：正式链与实验链都保留当前 workaround

优点：

- 最稳
- 不会把“renderer 差异”和“应用逻辑差异”同时引入

缺点：

- 第一阶段实验无法直接证明“没有 workaround 时 Skia 是否天然解决问题”

#### 方案 B：实验链先关闭 workaround

优点：

- 实验信号更纯

缺点：

- 风险过高
- 当前证据不足

**最终决策：选择 `方案 A`**

### 8. Debian 正式总控入口

#### 方案 A：单命令总控

做法：

- Debian 上提供一个总入口
- 该入口顺序产出：
  - `Linux x64` 正式包
  - `Windows x64 GNU` 正式包

优点：

- 最符合“Debian 一机发正式链”的心智模型
- 更适合后续 release automation

缺点：

- 需要定义失败时的策略

**最终决策：选择 `方案 A`**

#### 失败模式：方案 C

做法：

- 默认 `fail-fast`
- 提供显式参数切换到 `best-effort`

优点：

- 正式发版时严格
- 本地排查时灵活

缺点：

- 比固定单模式略复杂

**最终决策：选择 `方案 C`**

### 9. `winit-femtovg-wgpu` 的组织方式

#### 方案 A：主设计文档承载正式链 + Skia 实验链，try 方案另存旁路文档

优点：

- 主文档聚焦当前已确认路线
- `winit-femtovg-wgpu` 不会污染正式决策

缺点：

- 信息分成两份文档维护

#### 方案 B：所有备选方案都塞进主设计文档

缺点：

- 主文档变重
- 当前路线与旁路尝试边界不清

**最终决策：选择 `方案 A`**

## 最终决策汇总

### 正式发布链

- 构建机：`Debian Linux`
- 总控模式：单命令入口
- 默认失败策略：`fail-fast`
- 可选失败策略：`best-effort`

### 正式产物矩阵

- `Linux x64`
  - backend: `winit`
  - renderer: `winit-software`
  - 备注：作为最稳正式默认值，不引入 GPU 依赖波动
- `Windows x64 GNU`
  - backend: `winit`
  - renderer: `winit-software`
  - 备注：继续保留当前 `offscreen workaround`

### 实验产物矩阵

- `Windows x64 MSVC`
  - 身份：`Skia Experimental`
  - 编译形态：`pure Skia`
  - backend: `winit`
  - renderer: 强制 `winit-skia-software`
  - renderer 选择真源：程序内部
  - 外部 `SLINT_BACKEND` 冲突处理：忽略并写日志
  - 初始化失败处理：明确提示后退出
  - workaround：第一阶段继续保留

### 旁路探索

- `winit-femtovg-wgpu`
  - 不作为本轮正式或实验默认路径
  - 单独记录在 `docs/plans/try-winit-femtovg-wgpu.md`

## 高层实施步骤

1. 重组 Cargo feature 语义

- 保持正式链默认 feature 对应 `software renderer`
- 为实验包引入明确的 `app-level feature`
- 让实验包成为 `pure Skia` 二进制，而不是“带 Skia 能力的混合包”

2. 明确实验包身份与可观测性

- 在运行时日志中写出：
  - package profile
  - active backend
  - active renderer
- 在窗口标题、关于信息或诊断输出中标出 `Skia Experimental`

3. 把 renderer 真锁定迁入程序内部

- 实验包启动时由程序内部显式选择 `winit-skia-software`
- 外层脚本不再承担 renderer 真源职责

4. 定义变量冲突处理

- 若实验包检测到外部 `SLINT_BACKEND` 与预期冲突
- 忽略外部值
- 记录覆盖日志

5. 定义实验包失败路径

- 若 `winit-skia-software` 初始化失败
- 显示明确错误信息
- 立即退出
- 不做自动降级

6. 保持 workaround 不变

- 正式链继续沿用当前 Windows `offscreen redraw recovery`
- 实验链在第一阶段也继续保留，避免把变量一次改太多

7. 增加 Debian 正式总控入口

- 一个入口负责：
  - `Linux x64` 正式包
  - `Windows x64 GNU` 正式包
- 默认 `fail-fast`
- 可切换 `best-effort`

8. 旁路 try 文档化

- 记录 `winit-femtovg-wgpu` 的目标、触发条件、验证方式与退出标准
- 但不把它纳入当前正式或实验默认路线

## 风险与回滚

### 风险 1：正式链与实验链长期分叉

风险：

- 不同 renderer 和不同构建机可能导致行为差异长期存在

应对：

- 让正式链和实验链的职责从一开始就明确分离
- 不允许实验链策略污染正式链默认值

回滚：

- 如果实验链长期不稳定，可以停止发布 `Skia Experimental` 包
- 正式链不受影响

### 风险 2：Skia 实验包启动失败率高于预期

风险：

- 即使在 Windows MSVC 环境，`pure Skia` 也可能在部分真实机环境中无法稳定初始化

应对：

- 采用“明确提示后退出”的失败策略
- 通过中等识别策略把错误信息、renderer 选择和 profile 记录清楚

回滚：

- 暂停实验包分发
- 保留正式链与文档，不影响正式发布

### 风险 3：保留 workaround 会降低实验纯度

风险：

- 第一阶段无法直接证明“Skia 自身是否已经天然解决问题”

应对：

- 先确保实验链的 renderer 结果可信
- 等第一阶段稳定后，再决定是否进行“关闭 workaround”的第二阶段实验

回滚：

- 不存在额外回滚成本，因为 workaround 本来就保留在正式链

### 风险 4：`winit-femtovg-wgpu` 诱导过早切换正式 renderer

风险：

- 如果把 try 方案过早拉入正式讨论，容易打乱当前已确认路线

应对：

- 只保留 try 文档，不纳入当前正式或实验默认值

## 验证清单

- [ ] Debian 总控入口能在默认模式下产出：
  - `Linux x64` 正式包
  - `Windows x64 GNU` 正式包
- [ ] Debian 总控入口在 `best-effort` 模式下能输出清晰的分链结果汇总
- [ ] `Linux x64` 正式包运行时明确落在 `winit-software`
- [ ] `Windows x64 GNU` 正式包运行时明确落在 `winit-software`
- [ ] `Windows x64 GNU` 正式包继续保留现有 `offscreen workaround`
- [ ] `Windows MSVC Skia Experimental` 包为 `pure Skia` 形态
- [ ] 实验包启动时程序内部强制 `winit-skia-software`
- [ ] 实验包在日志或诊断输出中明确标记 `Skia Experimental`
- [ ] 若外部设置冲突的 `SLINT_BACKEND`，实验包会忽略它并写日志
- [ ] 若 `winit-skia-software` 初始化失败，实验包会明确提示后退出
- [ ] 第一阶段实验包继续保留现有 workaround
- [ ] `winit-femtovg-wgpu` 仅出现在 try 文档，不进入当前默认路线

## 参考链接

- Slint Backends & Renderers  
  https://docs.slint.dev/latest/docs/slint/guide/backends-and-renderers/backends_and_renderers/
- Slint Winit Backend  
  https://docs.slint.dev/latest/docs/slint/guide/backends-and-renderers/backend_winit/
- Slint `BackendSelector`  
  https://docs.slint.dev/latest/docs/rust/slint/struct.BackendSelector
- Slint PR: Skia use software rendering by default on Windows  
  https://github.com/slint-ui/slint/pull/7934
- Slint PR: partial rendering with Skia software renderer  
  https://github.com/slint-ui/slint/pull/6833
- rust-skia repository  
  https://github.com/rust-skia/rust-skia

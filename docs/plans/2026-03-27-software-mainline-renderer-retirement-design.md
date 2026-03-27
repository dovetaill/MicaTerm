# Software Mainline Renderer Retirement Design

日期: 2026-03-27  
执行者: Codex  
状态: 已确认，进入实现

## 背景

当前仓库主线被锁定到 `slint + winit + femtovg-wgpu + wgpu-28`，Windows 上还额外显式请求
`DX12`。这条路线在视觉上成立，但对当前产品目标出现了明显冲突：

- SSH/TUI 场景下内存占用偏高，和“小而美、低占用”的定位不一致
- 主线被 GPU backend、驱动栈和 vendored renderer patch 绑死，维护成本高
- 仓库里已经有多处测试、脚本和文档把 GPU 路线写成唯一事实，导致后续收缩困难

同时，最近的 SSH terminal 调试也已经证明，当前高占用不只是单点 bug，更与渲染策略本身
有关。即使继续修补局部同步逻辑，主线仍不适合继续以 GPU renderer 为默认。

## 目标

- 将正式主线恢复为 `winit + software renderer`
- 移除主线里对 `wgpu-28`、`DX12`、`femtovg` 的强绑定
- 不再保留任何可编译的 GPU experimental 入口
- 仅保留历史文档，作为已归档的尝试记录
- 保持最近 SSH terminal 的内存优化改动继续有效，不被这次 renderer 退役回滚

## 非目标

- 本轮不重做 terminal 组件结构
- 本轮不引入新的 GPU/Skia 替代实验线
- 本轮不删除所有历史文档中对 `femtovg-wgpu` 的提及，只要求它们转为 archive 语义

## 业界做法

轻量终端或偏生产力的桌面壳层，通常把“稳定、低占用、跨机器行为一致”的 renderer 作为主线，
把 GPU 强化路线留在实验分支或特定产品线中。终端本体对持续刷新、密集文本、整屏 TUI 的压力极高，
如果 UI 层再叠一层 GPU scene graph 和逐项对象树，内存和复杂度很容易失控。

对当前仓库而言，更合理的做法不是继续把 `femtovg-wgpu + DX12` 打磨成唯一主线，而是把它收回
到历史文档，正式路线回到软件渲染，再单独持续优化 terminal surface 同步与 UI 表达方式。

## 方案对比

### 方案 A：主线切回 software，GPU 路线仅保留 archive 文档

做法：

- `Cargo.toml` 默认 feature 切回 `slint-renderer-software`
- 删除 `slint-renderer-femtovg-wgpu` feature、相关 patch 和主线 selector 代码
- 删除 GPU wrapper、smoke、运行时契约
- 把旧文档保留为历史记录，但不再让 README / verification / 现行文档描述它是主线

优点：

- 最符合产品目标
- 可以彻底去掉 `wgpu-28 / DX12 / femtovg` 的主线耦合
- 构建、调试、验证路径都显著变简单

缺点：

- 会放弃当前 GPU 路线的视觉优势
- 需要同步清理较多测试和文档契约

### 方案 B：主线默认 software，但保留可编译 GPU experimental feature

优点：

- 保留后续回退空间

缺点：

- 仍然保留 `wgpu-28 / femtovg` 依赖树和维护负担
- 与“只留文档历史，不保留任何可编译 GPU 实验入口”的已确认约束冲突

### 方案 C：保留 GPU 主线，只继续做内存优化

优点：

- 代码改动最少

缺点：

- 不解决主线策略错误
- 与用户明确要求冲突

## 最终决策

选择 `方案 A`。

正式主线恢复为 `winit + software renderer`。`femtovg-wgpu + wgpu-28 + DX12` 不再作为任何可编译
入口存在，只保留历史文档，用于解释这条路线为什么被采用过、为什么被归档。

## 实施边界

### 必须变更

- `Cargo.toml`
- `src/app/runtime_profile.rs`
- `src/main.rs`
- 与 runtime profile / renderer 绑定相关的测试
- README 与 verification 中仍把 GPU 写成当前主线的内容
- build / smoke / contract 中仍暴露 GPU experimental 的入口

### 允许保留但要改语义

- `docs/plans/try-winit-femtovg-wgpu.md`
- `docs/plans/2026-03-13-windows-femtovg-wgpu-dx12-retrospective.md`
- 其他历史设计 / 实现文档

这些文档可以继续存在，但只能作为 archive 或 retrospective，不能再代表当前主线事实。

## 风险

### 风险 1：仍有遗漏的 GPU 绑定残留在脚本或测试里

应对：

- 用全文检索清点 `femtovg`、`wgpu-28`、`DX12`、`renderer-femtovg-wgpu`
- 最终验证时确认这些词仅出现在 archive 文档或第三方锁文件中

### 风险 2：软件渲染主线暴露出原先被 GPU 路线掩盖的 UI 假设

应对：

- 只调整 renderer/profile 契约，不顺手改 UI 结构
- 通过现有 smoke 和 targeted tests 确认启动链与 terminal 投影仍正常

## 验证标准

- 默认构建不再要求 `wgpu-28`
- `src/main.rs` 不再出现 `DX12`、`wgpu_28`、`femtovg-wgpu`
- `AppRuntimeProfile::mainline()` 明确返回 `Software`
- README 不再把 GPU 路线描述为当前主线
- GPU wrapper / smoke / contract 不再保留为 live 入口
- 近期 SSH terminal 相关测试继续通过

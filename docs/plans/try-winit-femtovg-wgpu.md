# Try Winit FemtoVG WGPU

日期: 2026-03-12  
执行者: Codex  
状态: 旁路探索文档，未纳入当前主方案

## 背景

在正式讨论 `Debian -> Linux x64 + Windows x64 GNU` 主发布链，以及 `Windows MSVC pure Skia experimental` 实验链时，`winit-femtovg-wgpu` 被明确提出作为一个后续可探索方案。

它没有被选为当前默认路线，不是因为它一定错误，而是因为当前阶段有更高优先级的约束：

- 正式链必须先稳定
- 当前 `Theme Toggle Offscreen Bug` 已经在 `software + workaround` 路径有可用兜底
- `winit-femtovg-wgpu` 会更早把正式链带入 GPU / 驱动 / 图形后端差异

因此，本文件只作为旁路探索记录，不代表当前已确认路线。

## 当前定位

`winit-femtovg-wgpu` 在当前整体策略中的位置是：

- 不是正式链默认 renderer
- 不是当前 Skia 实验链的替代物
- 是一个后续可单独验证的现代 GPU 渲染尝试

## 为什么本轮没有采用

### 原因 1：正式链优先级更高

当前已经确认：

- 正式 `Linux x64` 使用 `winit-software`
- 正式 `Windows x64 GNU` 使用 `winit-software + workaround`

这两条路线都服务于一个更强约束：

- `Debian` 必须先成为稳定正式构建机

`winit-femtovg-wgpu` 虽然在视觉和动效方向上更有潜力，但它不适合在这个阶段替代正式默认值。

### 原因 2：它不能直接回答当前 Skia 问题

当前最核心的验证问题是：

- `Skia` 是否能改善或消除当前 `offscreen` 历史问题

`winit-femtovg-wgpu` 不是这个问题的直接答案，所以不应该抢占 `Skia experimental` 的位置。

### 原因 3：GPU 路径变体更多

一旦引入 `wgpu` 路线，变体会显著增加：

- Linux 上的 Wayland / X11 差异
- 不同 GPU 驱动和 Vulkan / Metal / Direct3D 映射差异
- Windows GNU 上的图形栈兼容性问题

这会让当前正式链稳定目标变得更难达成。

## 适合在什么条件下启动这条 try

建议在满足以下条件后再启动：

- `Debian` 正式总控入口已经稳定
- `Linux x64` 与 `Windows x64 GNU` 正式包可以持续产出
- `Windows MSVC Skia Experimental` 的第一阶段验证已经完成
- 团队明确需要验证更现代的 GPU 动效方向，而不是只验证 Skia

## 建议的试验顺序

### 第一阶段：先试 `Linux x64`

建议优先在 Linux 上做 `winit-femtovg-wgpu` 尝试，原因是：

- Debian 本身就是主构建机
- Linux 正式链当前默认值最保守
- 在 Linux 上验证该方案，更容易观察：
  - 启动成功率
  - 基础布局稳定性
  - 简单动画与交互顺滑度

### 第二阶段：再决定是否扩展到 `Windows x64 GNU`

只有当 Linux 试验结果足够稳定时，才建议继续评估 Windows GNU 侧尝试。

当前不建议把它直接并入 Windows 正式链。

## 建议的试验边界

### 覆盖

- `shell / titlebar / sidebar / main workspace placeholder` 的启动与交互稳定性
- renderer 与 backend 日志可观测性
- 基础窗口交互：
  - resize
  - maximize / restore
  - theme toggle

### 不覆盖

- 真实 terminal surface 接入
- SSH / SFTP runtime
- 正式链默认 renderer 切换
- 当前 `offscreen workaround` 的替换决策

## 试验假设

当前 try 的主要假设是：

- `winit-femtovg-wgpu` 可能在 Linux 桌面观感和动效方向上优于 `winit-software`
- 它可能更适合后续更强调 Fluent 运动感的桌面终端壳层

但本轮没有证据表明：

- 它一定比 `winit-software` 更稳定
- 它能直接替代当前 Windows GNU 正式链
- 它能直接解决当前 `offscreen` 历史问题

## 建议的试验记录内容

一旦启动 try，建议至少记录：

- 目标平台
- backend / renderer 真实值
- 启动是否成功
- 主题切换是否稳定
- resize / maximize / restore 是否正常
- 是否出现 GPU 相关异常
- 是否有明显视觉收益值得后续继续投资

## 进入主方案的门槛

只有满足以下条件，才有资格进入后续主方案讨论：

- 在目标平台上连续多次稳定启动
- 关键窗口交互没有明显回归
- 日志与诊断能力足够清晰
- 视觉或动效收益明显高于 `winit-software`
- 不会显著破坏 Debian 正式链的稳定性目标

## 失败退出标准

出现以下任意情况时，应停止该 try，不继续扩散：

- 多平台启动不稳定
- 明显增加驱动兼容性问题
- 对正式链维护成本造成持续压力
- 没有形成足够清晰的视觉收益

## 结论

`winit-femtovg-wgpu` 是一个值得保留的旁路探索方向，但它当前只适合作为：

- 后续 Linux 优化路线的候选项
- 与 `software`、`Skia` 进行横向对比的参考线

它不属于当前已经确认的正式默认方案，也不替代当前已经确认的 `Windows MSVC pure Skia experimental` 路线。

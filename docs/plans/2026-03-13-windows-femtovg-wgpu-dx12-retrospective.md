# Windows FemtoVG WGPU DX12 Retrospective

日期: 2026-03-13  
执行者: Codex  
状态: 已验证修复，现象消失

## 目标

在不回退到软件渲染、不保留 app 层 mask/recovery/nudge 之类补丁的前提下，修复
Windows 上 `slint + winit + femtovg-wgpu` 的真实显示异常。

这里的核心要求不是“绕过去”，而是找出真正的渲染/后端根因，并让主线运行路径本身稳定。

## 初始现象

用户在 Windows 上使用 `winit + femtovg-wgpu + wgpu-28` 主线时，仍然可以稳定复现显示异常。

从当时现象看：

- 应用能启动
- 渲染循环持续进行
- 拖动、恢复、最小化、主题切换等窗口交互期间，异常现象仍然存在
- 问题不是单纯“没有渲染”，而是“渲染路径存在，但结果不对”

## 约束

- 不接受回退到 `software`
- 不接受保留 app 层恢复实验
- 不接受用额外 redraw、mask、recover、nudge 等策略掩盖问题
- 必须保留主线为 `winit + femtovg-wgpu`

## 第一阶段：清掉错误方向和旁路复杂度

### 1. 收敛到单一路径

先把运行路径收敛到纯主线：

- runtime profile 固定到 `Mainline`
- backend 固定为 `winit`
- renderer 固定为 `femtovg-wgpu`
- 移除 formal/software/experimental 分叉
- 删除 app 层恢复实验

这样做的目的，是避免多个 runtime route 干扰根因判断。

### 2. 加渲染链路诊断日志

加入了几类日志：

- `winit` 窗口事件
- Slint `RenderingNotifier` 生命周期
- WGPU adapter / surface capabilities / surface config

目的是把问题拆成几层：

- 窗口事件是不是正常到达
- Slint 是不是正常发起渲染
- WGPU 是不是正常拿到 surface 和 adapter
- surface 的 present mode / alpha mode 到底是什么

## 第二阶段：先修掉诊断链本身的问题

在渲染 notifier 里直接读取 window 几何信息时，曾触发：

- `RefCell already mutably borrowed`

根因不是窗口显示异常本身，而是调试代码在 suspend/minimize teardown 路径里重入借用了
`WinitWindowAdapter`。

修复方式：

- 不再在 rendering notifier 内部直接访问 `window.window().position()/size()`
- 改成在 `winit` 事件回调中缓存几何信息
- rendering notifier 只读取缓存快照

这个修复很重要，因为它把“调试代码引入的新崩溃”从真实图形问题中分离掉了。

## 第三阶段：验证 surface 参数是否就是根因

后续依次验证了几个明显可疑点。

### 1. 透明窗口与 opaque alpha 是否冲突

观察到 surface capabilities 中：

- `alpha_modes=[Opaque]`

于是把 Windows `winit` window attributes 显式改成：

- `transparent_window=false`

目的：

- 避免窗口属性要求透明，但 surface 只支持 opaque

### 2. present mode 是否导致异常

初始化 surface config 时，显式优先：

- `present_mode=Fifo`

并修正 resize 路径，避免重新配置时偷偷回到别的 mode。

### 3. alpha mode 是否选择错误

初始化 surface config 时，显式优先 blended alpha，没有则回退 opaque。

在用户后续日志中，已经能看到：

- `transparent_window=false`
- `present_mode=Fifo`
- `alpha_mode=Opaque`

但现象仍然存在。

这一步带来的关键结论是：

- 问题不是简单的透明窗口设置错误
- 也不是 `present_mode` 初始选择错误
- 也不是 app 层 redraw 频率不够

## 第四阶段：怀疑真正的差异在 WGPU backend

此前日志始终显示：

- `backend=Vulkan`

而在 Windows 上，特别是在 AMD RX550 这台机器上，`Vulkan` 是最值得怀疑的变量，因为：

- 透明窗口与 alpha 已经对齐了
- present mode 已经切到 `Fifo`
- 主渲染生命周期日志是连续的
- 问题仍旧存在

于是开始把调查重点转向：

- “应用试图要求 DX12，但这个要求到底有没有真正传到 Slint/WGPU 初始化链路？”

## 第五阶段：沿调用链核对 DX12 请求是否真的生效

沿着下面这条链逐层确认：

- `src/main.rs`
- `BackendSelector::require_wgpu_28(...)`
- `i-slint-backend-selector`
- `i-slint-backend-winit`
- `renderer/femtovg`
- `i_slint_core::graphics::wgpu_28::init_instance_adapter_device_queue_surface(...)`

当时一个非常关键的矛盾是：

- `transparent_window=false` 的日志出现了
- 但原本期望中的 `requested_backends=DX12` 没有出现在用户日志里
- 最终 adapter 仍然是 `backend=Vulkan`

这说明：

- 窗口属性 hook 的链路是生效的
- 但 WGPU backend 配置没有真正落地

## 第六阶段：在 renderer 边界补直接证据

为了判断是 selector 丢参，还是 renderer/Slint 核心没有按参数初始化 WGPU，在
`vendor/i-slint-renderer-femtovg/wgpu.rs` 里新增了边界日志：

- `femtovg renderer received requested graphics api`

它会直接打印：

- `requested_api`
- `requested_backends`
- `backends_to_avoid`

这样就能明确知道，FemtoVG renderer 实际收到的 graphics API 请求到底是什么。

## 第七阶段：发现真正阻止 DX12 生效的代码问题

最终发现问题不是 Slint 丢参，也不是 WGPU 忽略配置，而是应用代码里使用了错误的公开 API 路径。

错误写法：

- `slint::wgpu_28::api::WGPUSettings::default()`

正确写法：

- `slint::wgpu_28::WGPUSettings::default()`

原因是 `slint::wgpu_28` 顶层 re-export 了 `api::*`，但没有暴露 `api` 子模块本身。

这意味着：

- 代码里“想要求 DX12”的意图是对的
- 但真正进入 Windows 构建时，这段代码并没有以正确公开 API 形态工作
- 修正到正确路径后，DX12 请求才真正落到运行时初始化链路

## 最终验证日志

修正后，用户日志第一次同时满足了以下几个关键证据：

- `configuring wgpu backend preferences ... requested_backends=Backends(DX12)`
- `femtovg renderer received requested graphics api requested_api="wgpu28-automatic" requested_backends=Some(Backends(DX12))`
- `wgpu adapter initialized for femtovg renderer backend=Dx12`

并且此时用户确认：

- “这次修复了”

## 为什么这次修复真的有效

从证据上看，最强结论是：

1. `transparent_window=false`、`present_mode=Fifo`、`alpha_mode=Opaque` 是必要的收敛动作，但它们不是决定性修复。
2. 在这些条件已经成立时，`backend=Vulkan` 的运行仍然会复现异常。
3. 当且仅当运行时真正切换到 `backend=Dx12` 后，现象消失。

所以高置信度结论是：

- 真正的根因位于 Windows 上这台 AMD 机器的 `Vulkan` 路径，而不是 app 层状态同步、窗口事件、redraw 频率或 surface 基础配置本身。

更准确地说：

- 问题是 `Windows + AMD RX550 + Slint/winit/femtovg-wgpu + Vulkan` 这条图形后端组合的不稳定或错误表现
- 把主线固定到 `DX12`，是把 renderer 初始化到正确且稳定的 Windows 原生后端
- 这不是“掩盖现象”，而是把渲染系统初始化到了正确后端

## 这次修复包含了什么

### 主线约束

- 主线固定为 `winit + femtovg-wgpu + wgpu-28`
- 不再保留 formal/software/experimental 运行分叉

### 运行时收敛

- Windows 下显式 `transparent_window=false`
- surface 配置显式优先 `present_mode=Fifo`
- surface 配置与 reported alpha 能力对齐

### 诊断与防回归

- 为定位问题，曾临时加入 `winit` 事件、Slint rendering lifecycle、WGPU adapter /
  surface config、renderer 边界日志
- 在确认 DX12 才是决定性修复后，这些临时日志和只为日志存在的辅助测试已经从 live 代码移除
- 当前保留的是更小的防回归契约：
  - Windows 主线仍显式请求 `DX12`
  - vendor 里仍显式配置 `surface_config.format/present_mode/alpha_mode`
  - runtime profile 与脚本 smoke test 会阻止旧的 software/experimental 路线回流

## 经验教训

### 1. 先把“猜测”变成“证据”

如果没有 adapter/backend 级别日志，很容易一直在 app 层围绕 redraw、theme sync、窗口事件打转。

### 2. 能解释一部分现象的参数，不一定是根因

`transparent=false`、`Fifo`、`Opaque` 都看起来合理，但它们只能解释潜在不一致，不能解释“为什么修完这些仍然坏”。

### 3. backend 选择必须做运行时证据闭环

“代码里写了 DX12”不等于“运行时真的用上了 DX12”。只有当日志同时证明：

- 请求发出去了
- renderer 收到了
- adapter 确实是 `Dx12`

才算闭环。

### 4. 诊断代码也可能引入假问题

此前 `RefCell already mutably borrowed` 并不是原始显示异常，而是 tracing 代码引入的新崩溃。先把它清掉，才不会混淆判断。

## 现在的结论

当前这条 Windows 主线之所以稳定，是因为：

- 它已经不是“想用 DX12”
- 而是“已经被证据证明，实际在用 DX12”

并且对这次机器/驱动组合来说，DX12 是稳定路径，Vulkan 不是。

## 建议的后续守则

- Windows `femtovg-wgpu` 主线继续保持显式 `DX12`
- 如果未来需要再次调查 backend 差异，重新临时加日志可以，但不要把排障 trace 长期留在主线
- 如果未来要重新尝试 Vulkan，必须在单独实验分支里做，不能直接替换当前 Windows 主线

# Windows `K32EmptyWorkingSet` 内存回落排查结论

## 背景

这次排查的核心现象是：

- 启动后常驻大约 90-100 MB。
- 打开终端标签、执行 `history`、滚动大输出后，工作集会涨到 150-200 MB 左右。
- 在某些“大输出 + 空闲 + 再按一次回车”的场景里，进程占用会突然掉到 20-30 MB，随后继续使用时又回到 30-80 MB。

最关键的问题不是“为什么能掉下去”，而是“这个掉下去到底是真释放了内存，还是只是 Windows 把驻留页赶走了”。

## 本次代码路径

### 1. 大输出触发工作集 trim

- `src/app/ssh/runtime.rs` 定义了 trim 门槛：
  - `WORKING_SET_TRIM_IDLE_INTERVAL = 2s`
  - `WORKING_SET_TRIM_MIN_OUTPUT_BYTES = 1 MB`
- `src/app/ssh/runtime/pump.rs` 在 SSH 输出进入终端时累计 `pending_output_bytes`。
- 当累计输出超过 1 MB，且后续空闲 2 秒，`run_channel_pump()` 会调用 `crate::app::memory::trim_process_working_set()`。
- 这次排查期间我们临时加过 trim 前后快照和详细日志；排查完成后，这些诊断打印已经移除，不再默认输出。

### 2. 真正执行的是 `K32EmptyWorkingSet`

- `src/app/memory.rs` 的 `trim_process_working_set()` 直接调用 `K32EmptyWorkingSet(GetCurrentProcess())`。
- 同文件里的 `current_process_memory_snapshot()` 使用 `K32GetProcessMemoryInfo()` 读取：
  - `WorkingSetSize`
  - `PeakWorkingSetSize`
  - `PagefileUsage`
  - `PrivateUsage`

### 3. 现有“真实释放”路径与“工作集压缩”路径不是一回事

此前已经合入的两次修复是真正有意义的释放路径：

- `499f88c`：空闲时释放 scene-image 工作像素缓冲和终端缓存。
- `0aeb3f4`：最后一个终端关闭后，释放共享 terminal presenter host / native surface。

对应代码主要在：

- `src/app/bootstrap.rs`
- `src/app/terminal_presenter.rs`
- `src/app/terminal_renderer/wgpu_renderer.rs`
- `src/app/terminal_scene_image.rs`

这些路径会清空或丢弃应用自己持有的缓存/对象；而 `K32EmptyWorkingSet` 只是请求 Windows 尽量把当前驻留页移出工作集。

## 关键日志证据

这次用户提供的诊断日志已经足够说明问题。下面这些日志来自排查期间临时加入的诊断版本，当前正式代码已不再打印这些行：

```text
2026-04-08T13:55:14.776942Z DEBUG app.memory: terminal memory trim threshold crossed ... pending_output_bytes=1076667 trim_threshold_bytes=1048576 idle_interval_ms=2000
2026-04-08T13:55:17.512397Z DEBUG app.memory: terminal memory trim request ... before_working_set_bytes=205586432 ... before_pagefile_usage_bytes=206917632 before_private_usage_bytes=206917632
2026-04-08T13:55:17.539095Z DEBUG app.memory: terminal memory trim executed ... trim_succeeded=true ... after_working_set_bytes=659456 ... after_pagefile_usage_bytes=206917632 after_private_usage_bytes=206917632
```

这三行说明了非常关键的一点：

- trim 前：
  - `working_set_bytes ≈ 205.6 MB`
  - `private_usage_bytes ≈ 206.9 MB`
- trim 后：
  - `working_set_bytes ≈ 0.63 MB`
  - `private_usage_bytes ≈ 206.9 MB`

结论非常直接：

- **工作集（resident / 常驻页）几乎被清空了**
- **私有提交内存（private commit）基本没变**

所以“按一下回车后内存突然掉到 20-30 MB”这个现象，本质上不是应用突然真的只剩下 20-30 MB 了，而是 **Windows 把大量页从 working set 里赶走了**。

## `private_usage_bytes` 和“常驻内存”的区别

### `working_set_bytes`

`working_set_bytes` 对应 Windows 的 **Working Set**：

- 表示“当前驻留在物理内存里的、属于该进程工作集的可分页页”。
- 更接近任务管理器里常见的“内存 / Working Set / Resident”视角。
- 它会随着页面换入换出快速波动。

### `private_usage_bytes`

`private_usage_bytes` 对应 `PROCESS_MEMORY_COUNTERS_EX.PrivateUsage`：

- Microsoft 文档说明它等同于该进程的 **Commit Charge** / **Private Bytes**。
- 它表示“这个进程专有、且已经被内存管理器承诺（committed）的内存”。
- 这些内存即使当前不驻留在 RAM、已经被换出，也仍然算在这里面。

### 直观理解

可以把它理解成：

- `working_set_bytes`：现在“正坐在桌面上”的页。
- `private_usage_bytes`：这个进程“已经租下来的房间总面积”。

`K32EmptyWorkingSet` 更像是把暂时不用的东西从桌面搬进仓库；  
它**不等于**退租，也**不等于**把应用自己的对象、缓存、GPU 资源真正释放掉。

## 为什么 trim 后程序还能正常跑，而且内存会慢慢回升

因为很多页只是被移出了工作集，并没有被真正释放：

- 一旦继续交互、滚动、重绘、访问历史数据，这些页又会被重新触发 page fault；
- 被访问到的热页会重新进入 working set；
- 于是你会看到占用从 20-30 MB 再回到 30-80 MB，甚至更高。

这和“程序真的缩小成 20-30 MB 常态运行态”是两件事。

## 为什么不建议把这种低工作集状态直接做成默认常态

因为它主要优化的是 **resident 指标观感**，而不是真正的 committed memory：

- 频繁 trim 容易带来更多 page fault；
- 首次继续交互时更容易卡顿、抖动；
- 渲染缓存和字体缓存被 OS 赶出 RAM 后，下一次命中成本更高；
- 对真实私有提交的帮助非常有限。

官方和成熟实践也更偏向把这类动作放在“确实要 idle / suspend / background”的时机，而不是做成高频前台常态策略。

## 对当前代码的进一步判断

### 已经做到的真实优化

当前代码里，已经有几类真正有效的释放：

- 终端 presenter 的 transient caches 会清空；
- scene-image 的 `working_pixels` / `last_base_pixels` 会丢弃；
- 最后一个终端关闭后，共享 presenter host 与 native surface 会被释放。

这些都是真正能让应用自己少持有内存的路径。

### 还没有做到的更深层优化

从当前代码搜索结果看，应用层还**没有**显式接入这些更强的 renderer/cache 释放钩子：

- `SkGraphics::PurgeAllCaches` / `PurgeFontCache` / `SetFontCacheLimit`
- `GrDirectContext::performDeferredCleanup`
- `GrDirectContext::purgeUnlockedResources`
- `GrDirectContext::freeGpuResources`
- `IDXGIDevice3::Trim`
- `ID3D11DeviceContext::ClearState + Flush`

也就是说，后续若还想继续压低“真实 committed memory”，更值得做的方向不是更激进地 `EmptyWorkingSet`，而是：

1. 给应用层补一套“前台空闲 / 全部终端关闭 / 进入后台”时的 Skia / D3D 资源回收钩子；
2. 对字体缓存、资源缓存、GPU resource cache 做限额与按场景 purge；
3. 区分“必须常驻的 renderer 基础设施”和“可以在无终端时销毁后重建的对象”；
4. 继续用实际指标区分：
   - `working_set_bytes`
   - `private_usage_bytes`
   - 终端 presenter cache stats
   - scene-image 像素缓存大小

## 能不能把 20-30 MB 变成启动后一段时间的常态

**初步结论：不能直接把这次观测到的 20-30 MB 当作真实目标值。**

原因不是程序做不到继续运行，而是：

- 这个状态主要是 OS 主动把工作集压薄；
- 它不能证明应用真实提交内存也只剩 20-30 MB；
- 一旦重新交互，热页还是会回来。

如果要把“更低内存”变成常态，应该追求的是：

- **让 `private_usage_bytes` 也明显下降**
- 而不是只让 `working_set_bytes` 暂时掉下去

这就要求后续优化继续聚焦：

- 应用自己的 terminal/presenter/scene caches
- Skia 全局缓存
- Direct3D / DXGI 驱动内部缓存
- Slint/Skia renderer 生命周期

## 和 Zed 的比较该怎么看

不能只拿“任务管理器当下看到的常驻内存”横向对比：

- 不同应用的 renderer 栈不同；
- 字体/图形缓存策略不同；
- 是否预热 GPU / 文本系统不同；
- 是否做了前后台专门 trim / purge 不同；
- 统计口径也可能不同（working set、private bytes、private working set、commit size）。

更公平的比较方式应该是同时比较：

- 空闲 working set
- private usage / commit
- 打开一个标签后的增量
- 大输出后的峰值
- 关闭全部终端后的回落速度和回落终值

## 这次新增诊断日志的价值

这次日志已经把以前“看起来像神秘掉内存”的现象解释清楚了：

- `trim-threshold-crossed`：说明超过 1 MB 大输出门槛；
- `trim-request`：记录 trim 前真实工作集/提交；
- `trim-executed`：确认 `K32EmptyWorkingSet` 是否成功，以及前后差值；
- `trim-skipped`：说明没达到门槛，只是正常 idle。

这套日志已经完成排查使命，并且不适合长期默认打开，所以当前主线代码里已经移除了这部分 trim 诊断打印。

## 当前结论

一句话总结：

> 这次 `cat` 大文本后出现的“内存突然掉到 20-30 MB”，主要是 `K32EmptyWorkingSet` 把 working set 压下去了，不是应用真正把 private committed memory 释放到了 20-30 MB。

因此：

- 这个现象是“真实存在”的；
- 但它更多是 **resident-set trim**，不是 **true memory release**；
- 真正还值得继续优化的方向，应该放在 Skia / D3D / renderer 生命周期和缓存释放，而不是把 `EmptyWorkingSet` 做成更激进的默认策略。

## 参考资料

- Microsoft Learn: `EmptyWorkingSet`
  - https://learn.microsoft.com/en-us/windows/win32/api/psapi/nf-psapi-emptyworkingset
- Microsoft Learn: `PROCESS_MEMORY_COUNTERS_EX`
  - https://learn.microsoft.com/en-us/windows/win32/api/psapi/ns-psapi-process_memory_counters_ex
- Microsoft Learn: `Working Set`
  - https://learn.microsoft.com/en-us/windows/win32/memory/working-set
- Microsoft Learn: `Memory Performance Information`
  - https://learn.microsoft.com/en-us/windows/win32/memory/memory-performance-information
- Microsoft Learn: `ID3D11DeviceContext::Flush`
  - https://learn.microsoft.com/en-us/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-flush
- Microsoft Learn: `IDXGIDevice3::Trim`
  - https://learn.microsoft.com/en-us/windows/desktop/api/dxgi1_3/nf-dxgi1_3-idxgidevice3-trim
- Skia API: `GrDirectContext`
  - https://api.skia.org/classGrDirectContext.html
- Skia API: `SkGraphics`
  - https://api.skia.org/classSkGraphics.html
- Microsoft TechCommunity: working-set trimming can hurt performance
  - https://techcommunity.microsoft.com/blog/askperf/using-xperf-to-troubleshoot-working-set-trimming/374749

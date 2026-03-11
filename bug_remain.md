# Theme Toggle Offscreen Recovery Notes

日期: 2026-03-11

## 问题背景

当前 Windows 主题切换缺陷表现为:

- 窗口部分离开屏幕时切换 `Light / Dark`
- 屏幕内可见区域会切到新主题
- 屏幕外区域拖回后，可能短暂保留旧主题像素

从本轮调试日志可以确认:

- `ThemeMode -> Slint dark-mode` 同步正常
- 原生窗口 `set_theme(...)` / backdrop 同步正常
- `request_redraw()` 正常发出
- 问题不在“主题状态没切过去”，而在“部分离屏区域重新可见后没有可靠得到新像素”

## 当前保留的修复方案

当前仓库保留的是一个 `Windows-only` recovery workaround，目标是让窗口从离屏重新回到可见区域时，强制重新生成内容层像素。

### 方案组成

1. 在主题切换瞬间记录窗口当前可见面积
2. 监听 `Moved / Resized / ScaleFactorChanged`
3. 只要窗口重新变得更可见，就触发一次恢复动作
4. 恢复动作包含两部分:
   - `request_inner_size(+1px -> restore)`，逼窗口 surface 重新走一轮 resize
   - bump `render-revision`，逼 Slint 内容树重新失效

### 当前代码落点

- recovery 状态机: `src/app/bootstrap.rs`
- Slint revision 失效通道: `ui/app-window.slint`
- 调试期曾临时加入主题切换诊断日志与 Win32 full redraw 尝试，当前已收敛移除，默认构建只保留必要 error 日志

### 当前实际效果

从用户在 Windows 上的最新日志看:

- recovery 会在窗口逐步拖回屏幕时连续触发
- 可见面积最终回到 `1296000`
- 用户反馈“这次拖回来看起来恢复了”

因此当前 workaround 是有效的，可以保留作为现阶段修复方案。

## 为什么这不是最优解

这个方案的问题不在“能不能用”，而在“层级不对”。

它本质上是应用层 workaround，而不是 renderer 层根修复:

- 业务层需要感知“窗口是否部分离屏”
- 业务层需要跟踪“可见面积是否继续增大”
- 业务层需要反复触发 `size nudge`
- 业务层还要人为制造一个 `render-revision` 来逼 Slint 全树失效

这说明真正的问题更可能在:

- `renderer-software` 的 partial invalidation / backing buffer 复用路径
- 或 Windows + Slint software renderer 在部分离屏重新可见时的像素刷新策略

换句话说，当前方案是“可靠兜底”，不是“优雅根治”。

## 本轮评估过的备选方案

### 1. 只做原生 full redraw

做法:

- `request_redraw()`
- Win32 `RedrawWindow(...)`

结论:

- 已验证不足，且相关尝试代码已删除
- 原生消息级 full redraw 不能稳定抹掉残留旧像素

### 2. 单次 `size nudge`

做法:

- 第一次重新可见时只触发一次 `+1px -> restore`

结论:

- 不足
- 日志证明窗口经常只是先恢复一小部分可见面积
- 如果 recovery 只触发一次，会过早消耗恢复机会

### 3. 多次 `size nudge` + Slint render revision

做法:

- 只要窗口继续变得更可见，就反复触发 recovery

结论:

- 当前有效
- 但仍然属于 workaround

### 4. 切换到 Skia renderer

这是本轮评估后最值得继续的长期方向。

理由:

- 当前问题高度像 `renderer-software` 路径的离屏失效 / 缓冲复用问题
- 如果换成 Skia 后天然消失，就能移除现有 workaround

## Skia 实验结论

仓库里已经补了一个实验 feature 与脚本入口:

- Cargo feature: `windows-skia-experimental`
- wrapper script: `build-win-x64-skia.sh`

但当前结论非常明确:

- 这个实验路径 **不能** 在当前 Linux -> `x86_64-pc-windows-gnu` cross-build 环境中完成
- 原因不是仓库脚本本身，而是 `rust-skia 0.90` 的上游限制

### 已验证到的事实

1. `cargo check --workspace --features windows-skia-experimental` 可以在当前 Linux 环境通过
2. `cargo clippy --workspace --features windows-skia-experimental -- -D warnings` 可以通过
3. 但真正的 Windows GNU 打包失败

### 失败原因

`rust-skia` 在 `x86_64-pc-windows-gnu` 下:

- 先尝试下载预编译 Skia binaries
- 对应 URL 返回 `404`
- 随后回退到完整 Skia 源码构建
- 源码构建要求 VC / MSVC toolchain

因此结论是:

- `windows-skia-experimental` 当前只适合在 **Windows MSVC shell**
- 不适合当前这条 Linux -> Windows GNU 打包链路

## 当前建议

短期建议:

- 保留现有 workaround，不再继续在业务层叠更多 hack
- 不再保留主题切换诊断 debug 埋点，问题复核以必要 error 日志和定向实验分支为主

中期建议:

- 如果后续有 Windows MSVC 构建环境，优先验证 `windows-skia-experimental`
- 如果 Skia 下问题天然消失，再考虑删除:
  - 可见面积跟踪
  - `size nudge`
  - `render-revision` workaround

长期建议:

- 若要彻底优雅解决，应优先推动 renderer 层根因确认
- 必要时整理最小复现，提交给 Slint / 上游渲染路径排查

## 当前结论

当前仓库中的 Windows 主题切换离屏恢复问题:

- 已有一个有效 workaround
- 该 workaround 已在用户环境下观察到可恢复效果
- 但它不是最优解
- 真正更优的方向仍然是 Skia 或上游 renderer 层修复

# Windows Terminal Text Rendering Design

日期: 2026-04-04
执行者: Codex
状态: 方案已确认，处于 design-only 阶段，未进入业务实现

## 背景

当前项目已经具备独立的终端渲染骨架，但 Windows 终端文本观感仍明显偏离目标：

- 深色背景下文字偏灰、偏虚，缺少 Windows 原生终端那种稳定、饱满的“墨水感”
- 英文、数字、符号、中文混排时存在字体风格割裂感
- `W`、`w`、`4`、`%`、`M`、`@`、`#`、`]`、`)`、`}` 等字符右侧存在疑似被裁掉一列像素的现象
- cursor、selection、baseline、cell 布局在不同路径上存在所有权分裂和对齐漂移风险

本轮设计的核心不是继续在通用 UI 文本路径上“微调参数”，而是确认 Windows 首发主线的终端文本渲染架构，使其满足以下产品方向：

- Windows 11 首发优先，观感尽量接近原生 Windows Terminal
- 外层窗口与应用壳继续由 Slint 承担
- 终端文本区在 Windows 下走独立、稳定、可控的 native text pipeline
- 保留跨平台扩展余地，不把终端模型和平台渲染彻底耦死

## 目标

### 视觉目标

- 让英文、数字、符号、中文在同一行内更统一，不再呈现“临时拼接两套字体”的观感
- 让小字号深色背景下文本更实、更锐、更稳定，不再发灰发虚
- 消除宽字符右侧的裁切感，保证实际可见边界不被 clip、atlas 或 overlay 意外吞掉
- 统一 baseline、cell、高亮、selection、cursor、underline 的几何对齐关系

### 架构目标

- 将 `WindowsMainline` 的终端主路径切换为真正的 native surface，而不是继续以 scene bitmap 为主
- 在 Windows 下把终端主文本渲染目标确定为 `DirectWrite + Direct2D`
- 保持终端自己的 cell/grid 模型，不退化为普通富文本控件
- 把文本、selection、cursor、underline、IME preview 统一收敛到同一条 Windows native 渲染链
- 为后续 macOS / Linux / Android / iOS 保留共享 terminal model、共享 frame contract、平台分层扩展能力

## 非目标 / 边界

本轮设计明确不包含：

- 不重写 `wezterm-term`、SSH runtime 或整个 terminal model
- 不把整个 UI 改造成 WinUI / WPF / WebView
- 不在本轮为 Linux / macOS 同步实现同等 native text path
- 不引入普通文本控件的自动换行、段落布局、富文本语义
- 不在本轮直接产出实现代码；仅确认架构与实施边界
- 不额外生成 implementation plan，除非用户后续明确要求

## 当前实现现状

### 1. Windows 终端主显示并非 Slint `Text`，但主线仍偏向 scene bitmap

当前终端文本主体并不是由 Slint 文本控件直接绘制。宿主层在终端区域显示的是：

- 一张 bitmap `Image`，见 `ui/shell/terminal-session-host.slint:861`
- 或同 HWND 上的 retained native surface

但 Windows 主线运行时仍默认优先使用 `SceneImage`：

- `src/app/runtime_profile.rs:222`
- `src/app/bootstrap.rs:2021`

这意味着打包口径虽然标称 native terminal renderer，但 Windows mainline 的默认实际行为仍偏向“先渲染成 bitmap，再进 Slint scene”。

### 2. 当前所谓 `DirectWriteFontSystem` 并非真实 DirectWrite 字体管线

`src/app/terminal_font/windows_dwrite.rs` 当前的实现本质上是：

- 内置 `Fusion JetBrains Maple Mono` 字体，见 `src/app/terminal_font/windows_dwrite.rs:26`
- 使用 `rustybuzz` 做 shaping，见 `src/app/terminal_font/windows_dwrite.rs:209`
- 使用 `swash` 做 rasterization，见 `src/app/terminal_font/windows_dwrite.rs:115`

也就是说，当前 Windows 字体系统命名上叫 `DirectWriteFontSystem`，但实际上并未真正使用 DirectWrite 负责 font face、glyph metrics、fallback shaping 和主文本绘制。

### 3. fallback 解析存在，但非 emoji 路径没有真正切换到对应字形来源

当前 fallback family 的发现逻辑已经存在：

- `src/app/terminal_font/windows_fallback.rs:18`
- `src/app/terminal_font/windows_dwrite.rs:227`

但实际 monochrome shaping / raster 仍基本回到默认 bundled face：

- `src/app/terminal_font/windows_dwrite.rs:332`
- `src/app/terminal_font/windows_dwrite.rs:115`

这会直接导致中英文混排、符号 fallback、CJK fallback 的实际输出不稳定，也与目标字体链不一致。

### 4. Windows native renderer 目前是 bitmap mask blit，而不是真正的 DWrite text drawing

当前 Windows retained path 中：

- render target 创建为 `DXGI_FORMAT_B8G8R8A8_UNORM + D2D1_ALPHA_MODE_PREMULTIPLIED`，见 `src/app/terminal_renderer/platform/windows.rs:217`
- monochrome glyph 使用 `FillOpacityMask` 绘制，见 `src/app/terminal_renderer/platform/windows.rs:574`
- color glyph 使用 `DrawBitmap` 绘制，见 `src/app/terminal_renderer/platform/windows.rs:625`

当前代码中没有看到终端主文本直接走 `DrawGlyphRun`、`IDWriteTextLayout`、`GetOverhangMetrics` 之类的真实 DirectWrite 文本绘制链。

### 5. 右侧裁切风险是结构性的，而不只是字体“长相”问题

当前 glyph atlas 与 clip 策略存在多处结构性风险：

- atlas entry 没有安全 padding，见 `src/app/terminal_renderer/atlas.rs:75`
- glyph draw 在 WGPU 准备阶段会被 clamp 回 cell span，见 `src/app/terminal_renderer/wgpu_renderer.rs:417`
- scene-image 路径按 cell span 做 clip，见 `src/app/terminal_scene_image.rs:349`
- scene-image blit 时还会再次约束 glyph origin，见 `src/app/terminal_scene_image.rs:564`

因此当前“宽字符右边像少一列像素”的问题，更像是 visible bounds、overhang、atlas padding、dest rect、cell clip 多因素叠加造成的渲染 bug。

### 6. overlay ownership 当前存在分裂

Windows native renderer 会绘制 cursor overlay：

- `src/app/terminal_renderer/platform/windows.rs:746`

但 Slint host 仍然会根据 window 属性画一层 cursor：

- `ui/shell/terminal-session-host.slint:1005`
- `src/app/bootstrap.rs:2225`

这意味着 native path 下 cursor 可能存在双绘、遮挡、几何漂移或职责重复。

### 7. 默认字体与排版参数不符合本轮目标

当前默认配置为：

- 默认字体：`Fusion JetBrains Maple Mono`，见 `src/app/terminal_font/backend.rs:11`
- 默认字号：`18.0px`，见 `src/app/terminal_font/backend.rs:55`
- Windows fallback 候选优先 `Microsoft YaHei UI`，且未纳入 `Cascadia Mono`，见 `src/app/terminal_font/windows_fallback.rs:18`

这与本轮目标字体链和字号区间不一致。

## 设计要点拆分

### 设计要点 1：Windows 主渲染路径

Windows 首发主线是否继续以 `SceneImage` 为默认，还是切换到真正的 native surface。

### 设计要点 2：Windows 文本引擎目标形态

Windows 终端主文本是否继续以自定义 shaping+raster+bitmap blit 为主，还是切到 `DirectWrite + Direct2D` 原生绘制。

### 设计要点 3：字体链与分发策略

是否继续依赖系统 fallback，还是显式建立并随包分发稳定字体链。

### 设计要点 4：visible bounds / clip / atlas / overlay ownership

是否继续使用严格 cell containment 与分裂 overlay，还是切到 visible bounds 驱动、统一 native overlay 所有权。

### 设计要点 5：调试与验收能力

是否只保留静态日志，还是提供专门的调试开关与可视化信息暴露。

## 方案对比

### 设计要点 1：Windows 主渲染路径

#### 方案 A：继续以 `SceneImage` 为主线

优点：

- 变更面最小
- 与当前运行时分流最贴近

缺点：

- 仍处于透明合成与 scene bitmap 约束下
- 很难稳定达到接近 Windows Terminal 的文本质感
- 会继续保留双路径妥协代码

#### 方案 B：`WindowsMainline` 默认切到 `PostRenderNativeSurface`

优点：

- 更符合 Windows 首发体验目标
- 与当前 native presenter / retained surface 骨架契合
- 便于把文本与 overlay 统一到一条链路

缺点：

- 需要进一步收紧 handoff、geometry sync、fallback 策略
- 初期调试复杂度更高

#### 最终决策

采用方案 B。`SceneImage` 不再作为 Windows mainline 默认主路径，仅作为 fallback / compatibility 路径保留。

### 设计要点 2：Windows 文本引擎目标形态

#### 方案 A：继续使用 `rustybuzz + swash + atlas + FillOpacityMask`

优点：

- 最大化复用现有实现
- 改造面较低

缺点：

- ClearType 与原生文本观感上限明显受限
- fallback、visible bounds、真实 metrics 的控制力不足
- 长期会继续把“字体引擎问题”堆在业务 renderer 层

#### 方案 B：混合方案，DirectWrite 负责 face / fallback / metrics，最终仍 blit bitmap

优点：

- 可先解决 metrics 与 fallback 失真
- 能兼容既有 atlas / prepared frame 结构

缺点：

- 终端主文本依然不是原生 DWrite 直接绘制
- 视觉上限仍低于真正 native text

#### 方案 C：Windows 主文本以 `DirectWrite + Direct2D` 原生绘制为目标

优点：

- 与目标视觉最一致
- metrics、overhang、fallback、AA 条件控制力最强
- 能从根上减少 bitmap-mask 文本导致的发灰、发虚问题

缺点：

- 改造面最大
- 需要更清晰的 renderer 分层与 overlay 收口

#### 最终决策

采用方案 C。Windows 终端主文本的目标架构确定为 `DirectWrite + Direct2D` 原生绘制；如实施过程中需要降低切换风险，可以在内部使用短期过渡层，但不改变最终目标架构。

### 设计要点 3：字体链与分发策略

#### 方案 A：单字体优先，`Sarasa Term SC + Segoe UI Emoji`

优点：

- 中英文字形统一度高
- 实现简单

缺点：

- 英文终端感不如 `Cascadia Mono`
- powerline / Nerd Font 适配空间较小

#### 方案 B：系统字体链，`Cascadia Mono -> Sarasa Term SC -> Segoe UI Emoji`

优点：

- 风格合理
- 不增加包体

缺点：

- 机器间安装情况不同，观感不可完全控

#### 方案 C：显式字体链并随包分发主字体

字体链：

- `Cascadia Mono`
- `Sarasa Term SC`
- `Segoe UI Emoji`

优点：

- 观感最稳定
- 中英文混排控制力最高
- 便于后续形成可复制的默认终端视觉

缺点：

- 增加包体
- 需要处理字体许可、更新与打包策略

#### 最终决策

采用方案 C。Windows 默认显式使用 `Cascadia Mono -> Sarasa Term SC -> Segoe UI Emoji`，并将 Latin/CJK 主字体作为随包资产提供，避免依赖系统“随缘 fallback”。

### 设计要点 4：visible bounds / clip / atlas / overlay ownership

#### 方案 A：继续 strict cell containment，仅放宽部分 clip 与 padding

优点：

- 最小侵入
- 复用现有 cell 约束模型

缺点：

- 很难根治右侧 overhang 裁切
- selection / cursor 仍可能遮挡真实 glyph visible bounds
- 仍保留 host / native 双边 ownership

#### 方案 B：visible bounds 驱动，native path 统一拥有 overlay

关键原则：

- 使用 overhang / visible bounds，而不是纯 advance 或 cell rect 作为最终可见边界依据
- glyph 仅受行级或 viewport 级 clip 约束，不做过紧字符级 clip
- Windows native path 统一负责 text、selection、cursor、underline、IME preview

优点：

- 最符合终端文本真实显示需求
- 能系统性解决右侧裁切与遮挡问题
- 所有终端可视元素共享同一套 grid 坐标与像素对齐规则

缺点：

- 需要重新梳理 glyph visible bounds 与 overlay 排序合同
- 调试工作量更大

#### 最终决策

采用方案 B。

### 设计要点 5：调试与验收能力

#### 方案 A：仅保留日志

优点：

- 实现简单

缺点：

- 很难快速定位字体、clip、atlas、AA、alpha mode、DPI 之间的关系

#### 方案 B：提供 debug 开关，支持日志与可视化 overlay

调试信息至少包含：

- 当前 renderer path
- 当前 text antialias mode / alpha mode
- 当前字体链
- cell 逻辑宽度与 glyph 最终绘制宽度
- glyph bitmap bounds / atlas rect / screen rect
- baseline
- 是否像素对齐
- 当前 DPI / scale factor

优点：

- 更利于定位文本发虚、裁切、对齐漂移
- 能直接支撑回归验证

缺点：

- 需要控制 debug 入口，避免污染 release 体验

#### 最终决策

采用方案 B。

## 最终决策

本轮确认的 Windows 终端文本渲染方向如下：

1. `WindowsMainline` 默认终端主路径切换为 `PostRenderNativeSurface`
2. Windows 终端主文本目标架构为 `DirectWrite + Direct2D`
3. Windows 默认显式字体链为 `Cascadia Mono -> Sarasa Term SC -> Segoe UI Emoji`
4. `Cascadia Mono` 与 `Sarasa Term SC` 作为随包资产提供，不依赖系统随机 fallback
5. Windows native path 统一拥有 text、selection、cursor、underline、IME preview 的绘制权
6. glyph 可见边界以 visible bounds / overhang 为准，仅受行级或 viewport 级 clip 约束
7. bitmap atlas 路径不再作为 Windows 主文本最终方案，只保留为 fallback / compatibility 能力
8. 必须提供可选 debug overlay / log，暴露字体、metrics、clip、AA、alpha、DPI 相关信息

## 实施步骤

以下步骤为高层实施顺序，用于约束落地边界，不等同于详细 implementation plan。

### 阶段 1：切换 Windows 主线到 native surface

- 调整 runtime profile / presenter 选择逻辑，让 `WindowsMainline` 默认进入 `PostRenderNativeSurface`
- 保留 `SceneImage` 作为 fallback 开关，不直接删除旧路径
- 收口 native surface geometry / visibility / damage 同步逻辑

### 阶段 2：重建 Windows 文本引擎边界

- 为 Windows 新增真实 DirectWrite 字体访问与 fallback/metrics 能力
- 终端主文本测量、font face 选择、visible bounds 获取改由 DWrite 主导
- 明确 cell model 与 native text layout 的边界：terminal 仍控制 cell/grid，native text 只负责 glyph metrics 与绘制

### 阶段 3：统一字体链与默认排版

- 把默认字体切换为 `Cascadia Mono -> Sarasa Term SC -> Segoe UI Emoji`
- 默认字号调整到 `13px` 或 `14px`
- 默认 line height 调整到 `1.35 ~ 1.45`
- 保持 `letter spacing = 0`
- 以 `Regular` 为默认 weight，不依赖 synthetic bold 伪装“更实”

### 阶段 4：修复 visible bounds / overhang / clip / atlas 问题

- 在 native path 下以 visible bounds / overhang metrics 驱动 glyph draw bounds
- 取消过紧字符级 clip，改为行级或 viewport 级 clip
- fallback bitmap / atlas 路径增加安全 padding，避免边缘采样损失
- 检查 selection、cursor、背景块的遮挡顺序，确保不吞掉 glyph 实际可见像素

### 阶段 5：统一 overlay ownership

- Windows native path 下，由 native renderer 统一绘制 selection、cursor、underline、IME preview
- Slint host 不再在 native mode 下重复绘制 cursor 或其他终端 overlay
- 保持这些 overlay 继续使用同一套 terminal cell/grid 坐标

### 阶段 6：建立调试与验证能力

- 增加 debug 开关与可选 overlay/log
- 暴露 renderer path、font chain、baseline、glyph bounds、AA mode、alpha mode、DPI / scale factor
- 增加针对宽字符、CJK、emoji、Nerd Font 的回归验证用例

## 风险与回滚策略

### 风险 1：native 路径切换引发空白、闪烁或 surface handoff 回归

应对：

- 保留 `SceneImage` fallback 开关
- 切换过程采用 feature gate 或 runtime gate
- 每个阶段保持可回退到上一个可见稳定版本

### 风险 2：DirectWrite 集成后，现有 terminal prepare contract 与平台 backend 边界失衡

应对：

- 坚持 terminal model 负责 cell/grid 和 overlay 语义
- DWrite 只接管 font face、metrics、fallback、glyph visible bounds 与最终文本绘制
- 避免把 terminal layout 彻底下沉成平台私有逻辑

### 风险 3：字体随包分发带来许可、包体和更新管理成本

应对：

- 在实施前确认字体 license 与分发策略
- 把 bundled font 与系统 override 分离
- 保留开发期替换与排查入口

### 风险 4：overlay ownership 收口时出现双绘或状态同步遗漏

应对：

- 明确 native mode 下 Slint host 不负责 cursor / selection 等终端 overlay
- 把“模式切换时谁拥有可视层”写成显式条件，而不是依赖隐式 window 属性同步

### 风险 5：visible bounds 放宽后出现跨 cell 溢出与 hit-test 复杂度提升

应对：

- 仅放宽绘制可见边界，不改变 terminal 的逻辑占位规则
- clip 仍受行级和 viewport 级约束
- 调试模式下提供 bounds overlay，方便快速识别异常

## 验证清单

### 基础视觉验证

- 深色背景下文本明显比当前实现更实、更清晰，不再发灰发虚
- 英文、数字、符号、中文混排观感统一，不再像两套字体强行拼接
- `W`、`w`、`M`、`m`、`%`、`4`、`@`、`&`、`#`、`>`、`<`、`]`、`)`、`}`、`A`、`V`、`Y` 右侧不再出现裁切感

### 布局与覆盖验证

- baseline、cell width、cell height、cursor、selection、underline 几何关系一致
- selection 与 cursor 不遮挡 glyph 实际可见边界
- IME preview、underline、hyperlink highlight 在 native path 下仍与 cell/grid 一致

### DPI 与字号验证

- 在 `100%`、`125%`、`150%` DPI 下验证
- 在 `12px`、`13px`、`14px`、`15px` 字号下验证
- 检查整数像素对齐、scale factor 传播、baseline 稳定性

### 字体与字符集验证

- 英文
- 中文
- 中英混排
- box drawing
- powerline / Nerd Font glyph
- emoji

### 调试能力验证

- 能查看当前 renderer path
- 能查看当前字体链
- 能查看 glyph bitmap bounds / visible bounds / screen rect
- 能查看 baseline 与像素对齐状态
- 能查看当前 antialias mode / alpha mode / DPI / scale factor

### Implementation closure note

- 当前仓库内自动化验证以源码级与脚本级钩子为主，不能在本地 smoke 中完整替代真实 Windows 多显示器 DPI 视觉验收。
- 因此本轮实现将 `100%`、`125%`、`150%` DPI 与 `12px`、`13px`、`14px`、`15px` 字号矩阵拆成两层：
  - 自动化：要求 `build-win-x64.sh` 与相关 smoke 脚本显式枚举这些 DPI/字号目标，确保打包、默认参数与验证入口不会漂移。
  - 手工验证：在真实 Windows 环境下记录 dark background、CJK/Latin mixed、Nerd Font、emoji 等观感结果。
- Diagnostics ship as trace/log hooks only；本轮不引入默认常驻的 release UI overlay。
- 已实现的 native diagnostics 需要持续提供 `renderer path`、`font chain`、`baseline`、`glyph bounds`、`text antialias mode`、`alpha mode`、`DPI / scale factor`。
- 当前 Windows 首发路径的对外口径固定为 `directwrite-d2d` 主文本渲染路径；若后续 fallback 或 stop-loss 路线调整，必须同步更新打包脚本与 smoke 断言。

## 备注

- 本文档仍以设计方向为主，但已补充 Task 7 收口时确认的 implementation closure note。
- 若后续继续调整 Windows packaged text path、fallback 口径或 DPI/字号验证矩阵，必须同步更新本文件、implementation plan 与打包 smoke。

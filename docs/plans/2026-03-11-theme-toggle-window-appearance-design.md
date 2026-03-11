# Mica Term Theme Toggle Window Appearance Design

日期: 2026-03-11  
执行者: Codex  
状态: 已确认方案，待进入实现规划

## 背景

当前仓库的窗口壳层、标题栏和主题切换能力主要来自以下提交：

- `a1357ce feat: implement overall style shell baseline`
- `fb7ab7b feat: implement top status bar shell chrome`
- `5c5b95e feat: implement top status bar style bugfix2`

本轮问题聚焦在一个明确缺陷：

- 黑白模式切换时，若窗口有部分区域超出屏幕边界，超出屏幕的那部分会出现颜色未完整切换、残留旧主题、局部异常混色等问题

结合截图与源码，当前问题更接近“原生窗口外壳/系统合成层”和“Slint 内容层”没有同步切换主题，而不是单一 Slint 控件配色错误。

相关实现主要位于：

- `src/app/bootstrap.rs`
- `src/app/windowing.rs`
- `src/shell/view_model.rs`
- `ui/app-window.slint`
- `ui/theme/tokens.slint`
- `ui/shell/titlebar.slint`

## 目标

- 修复 `Light / Dark` 切换时窗口部分超出屏幕所导致的不完全切换问题
- 保留当前 `frameless + transparent + self-drawn shell` 的总体结构
- 保持 Windows 11 优先的 `Fluent + Mica Alt` 视觉方向
- 将主题切换从“只切 Slint token”升级为“Slint 内容层 + 原生窗口外壳”双层同步
- 为后续 macOS / Linux / Android / iOS 迁移预留干净的平台抽象边界

## 边界

### 本文档覆盖

- 窗口外壳主题同步策略
- 透明根窗口与原生 backdrop 的职责划分
- Windows 原生 theme / backdrop / redraw 的协同设计
- 跨平台抽象边界
- 高层实施步骤
- 风险、回滚与验证清单

### 本文档不覆盖

- SSH / SFTP / Terminal 功能实现
- `wezterm-term` / `termwiz` 接入细节
- Welcome / Sidebar / TabBar 的视觉重构
- 数据持久化后端调整
- 逐文件命令级 implementation plan

## 当前现状与根因判断

### 1. 主题切换目前只影响 Slint 属性

当前主题切换链路的核心动作是更新 `AppWindow.dark-mode`，进而驱动 `ThemeTokens.dark-mode`。

这意味着当前切换已覆盖：

- `shell-surface`
- `shell-stroke`
- `command-tint`
- `panel-tint`
- `terminal-surface`
- `text-primary`

但它没有覆盖原生窗口的：

- Windows theme override
- system backdrop / Mica
- 非客户区或系统合成相关重绘

### 2. 根窗口是透明窗口

当前 `AppWindow` 使用：

- `no-frame: true`
- `background: transparent`

实际可见的主界面是内部自绘 `Rectangle`。这种结构本身没有问题，但只要窗口背景保持透明，就不能把“窗口外壳”的主题一致性完全寄托在 Slint 内容区上。

### 3. 原生窗口桥接能力存在，但未用于外观同步

当前 `WindowController` 已经能通过 `with_winit_window(...)` 拿到 `winit::window::Window`，并已经用于：

- drag
- minimize
- maximize / restore
- close

但还没有真正用于：

- `winit::Window::set_theme(...)`
- Windows backdrop 同步
- 主题切换后的原生重绘

### 4. 仓库已有 Windows Mica 依赖，但未真正落地

`Cargo.toml` 已引入：

- `window-vibrancy = "0.7.1"`

但当前源码中没有实际调用 `apply_mica()` 或其他原生 backdrop API。这说明“设计意图是 Mica”，但“实现状态仍停留在纯 Slint token 切换”。

### 5. 根因结论

本轮问题的高概率根因是：

- `Slint content layer` 已切换主题
- `native window shell / system composition layer` 未同步切换或未被可靠重绘

在窗口部分超出屏幕边界时，这种不同步更容易暴露，因为超出区域更依赖系统窗口表面的合成与裁剪行为，而不是单纯依赖应用内容区的重新绘制。

## 设计原则

- `Single Theme Source`
  主题模式只能有一个权威状态源，即 `ThemeMode`。
- `Dual-Layer Sync`
  主题切换必须同时同步到 Slint 内容层和原生窗口外壳层。
- `Windows-First, Cross-Platform-Ready`
  Windows 11 先完整落地，其他平台通过抽象层先占位，不在本轮假实现。
- `Preserve Mica Direction`
  不因为短期 bug 直接放弃 `transparent + Mica` 路线。
- `Explicit Redraw`
  对主题切换后的窗口刷新不能依赖“系统可能会帮我刷新”，必须设计显式刷新步骤。

## 设计要点与方案对比

### 1. 窗口外壳主题归属

#### 方案 A1：继续纯 Slint 主题切换

做法：

- 仅更新 `ThemeTokens.dark-mode`
- 不处理原生 `winit::Window` theme / backdrop

优点：

- 改动最小

缺点：

- 与当前问题高度同源
- 超出屏幕区域仍可能保留旧主题
- 不是真正的 Windows 11 Mica 路线

#### 方案 A2：Hybrid Window Appearance

做法：

- Slint 负责内容区、圆角壳层、内部组件 token
- 原生 `winit::Window` 负责 `theme + backdrop + redraw`

优点：

- 最符合本轮问题根因
- 与 Windows 11 Fluent / Mica 目标一致
- 保留现有自绘壳层结构

缺点：

- 需要新增一个薄的平台外观桥接层

#### 方案 A3：完全退回纯自绘实体背景

做法：

- 放弃原生 Mica 托底
- 用不透明实体背景彻底覆盖透明窗口问题

优点：

- 最稳

缺点：

- 明显偏离当前产品视觉目标
- 后续若恢复 Mica 需要再次返工

**最终决策：选择 `A2`**

### 2. 主题切换触发链路

#### 方案 B1：只更新 Slint，等待系统被动刷新

优点：

- 简单

缺点：

- 当前问题已经证明该方案不可靠

#### 方案 B2：双通道同步 + 显式原生重绘

做法：

1. 更新 `ShellViewModel.theme_mode`
2. 更新 `AppWindow.dark-mode`
3. 同步原生 `winit::Window::set_theme(...)`
4. 同步 Windows backdrop
5. 主动请求一次原生重绘/失效刷新

优点：

- 可同时覆盖内容层和系统合成层
- 对窗口超出屏幕边界的场景更稳

缺点：

- 需要定义切换时序

#### 方案 B3：切换后重建窗口

优点：

- 最强兜底

缺点：

- 交互粗糙
- 后续接终端 session 时风险高

**最终决策：选择 `B2`**

### 3. 根窗口透明策略

#### 方案 C1：保留透明根窗口 + 原生 Mica 托底

做法：

- 继续保留 `Window.background: transparent`
- 内部 Slint 壳层继续自绘
- 原生窗口负责 backdrop 材质与系统主题同步

优点：

- 保留当前圆角与分层结构
- 与 Windows 11 Mica 方向一致

缺点：

- 依赖原生桥接实现正确

#### 方案 C2：改为不透明根窗口

优点：

- 更稳

缺点：

- 会削弱 Mica 真实感
- 与当前设计方向不一致

**最终决策：选择 `C1`**

### 4. 平台抽象边界

#### 方案 D1：直接把 Windows 逻辑写进 `bootstrap/windowing`

优点：

- 落地快

缺点：

- 平台分支会迅速污染主链路
- 后续迁移成本高

#### 方案 D2：新增薄的 `PlatformWindowEffects` 抽象

做法：

- 定义一个很薄的平台窗口外观接口
- Windows 提供真实实现
- 其他平台先提供 `no-op` 或最小实现

优点：

- 符合跨平台演进目标
- 不会大改现有架构

缺点：

- 需要多一个模块与接口定义

**最终决策：选择 `D2`**

## 最终设计决策

本轮最终采用以下组合：

- `DP1-B`：`Hybrid Window Appearance`
- `DP2-B`：`Dual-Layer Sync + Explicit Native Redraw`
- `DP3-A`：`Transparent Root Window + Native Mica Backdrop`
- `DP4-B`：`Thin PlatformWindowEffects Abstraction`

可概括为：

- `ThemeMode` 继续作为唯一主题源
- Slint 继续掌管内容与 token
- 原生 `winit::Window` 正式接管窗口 theme / backdrop / redraw
- Windows 11 首先实现真实外壳同步
- 其他平台不在本轮假装支持，但接口边界先建立

## 架构草图

### 目标链路

`Titlebar theme toggle`
-> `ShellViewModel.theme_mode`
-> `UiPreferences persistence`
-> `AppWindow.dark-mode / ThemeTokens.dark-mode`
-> `PlatformWindowEffects.apply_theme(...)`
-> `winit::Window::set_theme(...)`
-> `Windows backdrop sync`
-> `request native redraw`

### 模块职责

`ShellViewModel`

- 继续持有当前主题模式
- 不感知具体平台 API

`bootstrap`

- 仍然是 UI 事件绑定入口
- 在主题切换时调用平台外观桥接

`PlatformWindowEffects`

- 接收统一的主题模式和外观意图
- 内部决定如何作用于原生窗口

`Windows implementation`

- 负责 `ThemeMode -> winit::Theme`
- 负责 `ThemeMode -> Mica / Alt Mica / fallback`
- 负责切换后的显式刷新

## Windows 侧设计约束

### 1. 主题映射

`ThemeMode::Dark`

- Slint token 切到 dark
- 原生窗口 theme 映射到 `winit::window::Theme::Dark`

`ThemeMode::Light`

- Slint token 切到 light
- 原生窗口 theme 映射到 `winit::window::Theme::Light`

### 2. Backdrop 策略

默认目标保持 `Mica Alt` 倾向，与既有文档一致。

设计上允许以下行为：

- Windows 11 支持目标 backdrop 时，应用对应 backdrop
- 若系统版本或系统设置不满足条件，则降级为无 backdrop 或更基础的外观同步

本轮不要求为了“必须看到 Mica”而牺牲主题切换正确性。

### 3. 重绘策略

主题切换完成后，设计上必须显式触发一次原生刷新。

接受的高层策略包括：

- 请求窗口 redraw
- 请求窗口失效重绘
- 在必须时做极轻量的外观刷新调用

本轮原则：

- 禁止采用 `hide/show`
- 禁止采用“销毁并重建窗口”

## 跨平台准备

本轮只要求把接口边界定干净，不要求其他平台同步实现材质。

抽象层需要满足：

- 输入是平台无关的主题/外观意图
- 输出是平台特定的窗口效果行为
- Windows 之外的平台可以先是 `no-op`

这样后续可以按平台逐步补：

- macOS：vibrancy / material
- Linux：尊重 compositor 条件，最小化假设
- Android / iOS：只保留主题语义，不假设桌面窗口材质

## 实施步骤

### Step 1

梳理并固化主题切换的单一状态源，确认 `ThemeMode` 是唯一权威主题输入。

### Step 2

为窗口外观新增平台抽象层，例如：

- `PlatformWindowEffects`
- `WindowAppearanceBridge`

命名实现阶段再定，但职责必须保持薄。

### Step 3

在 Windows 实现中补齐：

- 原生 `winit::Window` 主题同步
- Mica / Alt Mica backdrop 同步
- 切换后的显式重绘

### Step 4

在 `bootstrap` 的主题切换回调中，把当前“只改 Slint 属性”的逻辑升级为：

- 更新 view model
- 更新 Slint 属性
- 保存偏好
- 同步原生窗口外观

### Step 5

补充验证与必要日志，确保能够观察到：

- 当前主题模式
- 原生窗口外观同步是否被调用
- 降级分支是否发生

## 风险

### 风险 1：Windows 版本或系统设置差异

不同 Windows 11 版本、系统透明效果开关、显卡驱动环境，都会影响 backdrop 呈现。

应对：

- 优先保证主题正确切换
- backdrop 允许降级
- 在日志中记录降级事实

### 风险 2：Slint software renderer 与透明窗口组合的边界行为

当前项目使用 `renderer-software`。即使原生外壳同步补齐，某些设备上仍可能存在透明窗口特殊行为差异。

应对：

- 保持显式 redraw 设计
- 把“是否需要进一步调整 renderer 策略”留到后续独立议题，不在本轮扩散

### 风险 3：过早把 Windows 细节写死到通用链路

若直接把所有 DWM / backdrop 逻辑散落到 `bootstrap`，后续跨平台会很快变脏。

应对：

- 保持平台桥接层独立
- 通用层只传递主题与外观意图

## 回滚策略

若 Windows backdrop 同步引入新的稳定性问题，回滚顺序如下：

1. 保留平台抽象层
2. 先关闭 backdrop 应用，仅保留原生 theme 同步和显式 redraw
3. 若仍异常，再暂时退回为“Slint + 原生 theme”，但不删除平台桥接结构

明确禁止的回滚方式：

- 直接删掉主题切换能力
- 直接重建窗口作为常规切换方案
- 直接放弃所有跨平台抽象边界

## 验证清单

- [ ] 正常状态下从 `Dark -> Light -> Dark` 连续切换，窗口整体颜色始终完整一致
- [ ] 窗口底部超出屏幕时切换主题，超出区域不残留旧主题色
- [ ] 窗口左侧超出屏幕时切换主题，超出区域不残留旧主题色
- [ ] 窗口右侧超出屏幕时切换主题，超出区域不残留旧主题色
- [ ] 窗口顶部超出屏幕时切换主题，超出区域不残留旧主题色
- [ ] 最大化后切换主题，窗口外壳和内容区一致
- [ ] 还原后切换主题，窗口外壳和内容区一致
- [ ] 重启应用后，持久化主题值与窗口实际外观一致
- [ ] Windows 支持目标 backdrop 时，Mica / Alt Mica 呈现正常
- [ ] Windows 不支持或系统关闭透明效果时，应用能平稳降级，不影响主题正确性

## 参考

- Microsoft Win32 Mica: <https://learn.microsoft.com/en-us/windows/apps/desktop/modernize/ui/apply-mica-win32>
- Winit `Window::set_theme`: <https://docs.rs/winit/latest/winit/window/struct.Window.html#method.set_theme>
- Winit Windows `set_system_backdrop`: <https://docs.rs/winit/latest/winit/platform/windows/trait.WindowExtWindows.html>
- window-vibrancy `apply_mica`: <https://docs.rs/window-vibrancy/latest/window_vibrancy/fn.apply_mica.html>

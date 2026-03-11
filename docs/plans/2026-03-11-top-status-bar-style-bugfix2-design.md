# Mica Term Top Status Bar Style Bugfix2 Design

日期: 2026-03-11  
执行者: Codex  
状态: 已确认方案，待进入实现规划

## 背景

当前仓库的顶部状态栏基线主要来自以下提交：

- `fb7ab7b feat: implement top status bar shell chrome`
- `b832f42 feat: polish top status bar style`

相关实现集中在以下文件：

- `ui/shell/titlebar.slint`
- `ui/components/titlebar-icon-button.slint`
- `ui/components/window-control-button.slint`
- `ui/components/titlebar-tooltip.slint`
- `ui/app-window.slint`
- `src/app/bootstrap.rs`
- `src/app/windowing.rs`
- `src/shell/view_model.rs`

当前版本已经具备以下基础能力：

- `no-frame: true` 的自绘窗口标题栏路线
- `drag / minimize / maximize-toggle / close` 的窗口桥接
- 基于 `PopupWindow` 的全局菜单与 tooltip 原型
- 顶栏基础分区和本地 Fluent SVG 资产接入

但当前界面仍存在明显产品问题：

- 左侧菜单按钮不在最左侧，且图标语义不对
- 品牌呈现仍像临时文本拼接，缺少成熟的 header logotype
- `Workspace` 与 `SSH` 都是低价值占位信息
- 右侧工具区层级不清，面板按钮没有处于正确位置
- 自绘 caption controls 仍偏小，最小化和最大化语义不够清楚
- 缺少 `Pin / Always On Top`
- 缺少真正闭环的日间/夜间模式切换
- tooltip 虽已有原型，但当前可发现性仍不达标

这说明本轮不是单纯“换几个图标”，而是一次围绕 `titlebar information architecture + branding + state bridge + caption semantics` 的定向重构设计。

## 目标

- 将左侧菜单重构为唯一、明确、固定在最左的 `Navigation` 入口
- 将品牌区从普通文本升级为适合标题栏的 `Navigation-Led Logotype`
- 删除 `Workspace` 与 `SSH` 两类无效占位
- 将右侧工具顺序固定为：
  - `theme -> panel-toggle -> divider -> pin -> min -> maximize/restore -> close`
- 放大按钮与图标命中区，修复 caption controls 语义不清的问题
- 为 `theme` 与 `pin` 建立真实可工作的状态模型与持久化约束
- 为所有顶栏按钮补齐统一、稳定、低噪音的 tooltip 体验
- 保持当前 `Rust + Slint + Tokio`、`frameless shell` 与跨平台演进路线不变

## 边界

### 本文档覆盖

- 顶栏布局和按钮编排
- 顶栏品牌表达方式
- 右侧 utility cluster 与 caption controls 的边界
- `theme` 与 `pin` 的状态语义
- tooltip 策略
- 高层实施步骤
- 风险、回滚与验证标准

### 本文档不覆盖

- SSH / SFTP 连接逻辑
- Terminal 渲染逻辑
- Welcome / TabBar / Sidebar 的样式重构
- 设置页完整产品设计
- 持久化后端的最终落地选型
- 逐文件、逐测试命令级别的 implementation plan

## 当前现状与根因

### 1. 左侧入口层级错误

当前 `menu-button` 被放在 `brand-zone` 的最右侧，而不是窗口最左；这与“全局导航入口”的语义冲突，也破坏了 Fluent 风格下左侧主入口的稳定心智。

### 2. 品牌表达过于临时

当前左侧仍是 `Text + MT chip` 组合，不像成熟桌面工具的顶栏品牌锁定方式。仓库虽然已有 `mica-term-logo.svg` / `mica-term-mark.svg`，但现有那套几何 `M` 图形不适合作为本轮标题栏主品牌表达。

### 3. 右侧信息架构失真

当前 `Workspace` 文本占用了拖拽区注意力，`SSH` pill 又占用右侧 utility 区预算，两者都没有形成实际价值，只会压缩真实工具的空间。

### 4. Caption controls 仍像占位控件

虽然窗口控制能力已经有桥接，但视觉上仍显得偏小，且与 utility tools 紧挨，造成“最小化像丢失”“最大化像横线”“中间两个按钮容易误判”的观感问题。

### 5. 新状态还没有建模闭环

当前仓库虽然已经有：

- `AppWindow.dark-mode`
- `ThemeTokens.dark-mode`
- `ShellViewModel`
- `WindowController`

但 `dark-mode` 还没有真正串到 UI token 系统里，`always-on-top` 也还没有进入 view model 或窗口桥接状态，因此 `theme` 与 `pin` 不能只做图标，必须做状态建模。

## 设计原则

- `Navigation-Led Branding`
  顶栏品牌重心放在导航入口及其后的 logotype，而不是孤立图形徽标。
- `Utility First`
  右侧只保留真实工具与窗口控制，不再保留无效文本。
- `Windows 11 Fluent Semantics`
  自绘按钮需接近 Windows 11 caption / toolbar 的 hit area、图标语义与 hover 节奏。
- `Cross-Platform Ready`
  Windows 11 是首发主目标，但状态模型和窗口能力桥接要为 macOS / Linux 保持可迁移性。
- `Low Noise Discoverability`
  所有按钮都要可发现，但不能靠即时闪烁 tooltip 或文本堆砌破坏顶栏克制感。

## 方案对比与最终决策

### 1. 左侧 Navigation 入口

#### 方案 A

保留当前品牌区骨架，只把图标替换为 `Navigation`。

优点：

- 改动最小

缺点：

- 按钮仍不在窗口最左
- 结构语义仍然错误

#### 方案 B

拆出独立 `leading-nav zone`，让 `Navigation` 按钮固定在窗口最左，品牌区整体右移。

优点：

- 最符合 Fluent 工具型桌面的导航语义
- 与用户预期完全一致
- 便于后续继续扩展左侧品牌节奏

缺点：

- 需要重排顶栏左侧布局

**最终决策：选择 `B`**

### 2. 顶栏品牌表达

#### 方案 A

继续使用 Slint `Text`，通过字重、字距、大小写制造“艺术字”效果。

优点：

- 不需要新增 SVG 资产

缺点：

- 跨平台字形一致性差
- 顶栏品牌容易像普通标签

#### 方案 B

重做新图形徽标，再配紧凑字标，形成新的 `header lockup`。

优点：

- 品牌完整度最高

缺点：

- 左侧已经有 `Navigation` 按钮，再加图形容易抢主次

#### 方案 C

采用 `Navigation-Led Logotype`，不再在顶栏放单独品牌图形，而是做一版新的顶部专用 `Mica Term` 字标 SVG，让它与左侧 `Navigation` 按钮形成统一语言。

优点：

- 最符合仓库已有的整体风格文档方向
- 视觉更成熟、克制
- 避免“左侧两个图标互相竞争”

缺点：

- 顶栏独立小图形记忆点会减弱

**最终决策：选择 `C`**

补充约束：

- 不复用当前那枚几何 `M` header 方案
- 新字标必须使用 SVG 资产而不是文本拼排
- 新字标保持与现有 app/taskbar 品牌系统同色系，但允许是 header-only 变体

### 3. 右侧按钮顺序

候选阶段曾讨论过将 `panel-toggle` 放在最右或将 `pin` 放在 divider 前后，但最终用户明确指定右侧顺序，因此本项不再保留多解。

**最终决策：**

`theme -> panel-toggle -> divider -> pin -> min -> maximize/restore -> close`

附加决策：

- 删除 `Workspace`
- 删除 `SSH`
- 删除旧的右侧重复入口语义

### 4. 按钮比例与尺寸策略

#### 方案 A

保持现有按钮尺寸，只替换图标。

优点：

- 侵入最小

缺点：

- 无法真正解决“小、挤、像重叠”的问题

#### 方案 B

保持顶栏总高度仍在当前 `48px` 级别，但增大 utility button 与 caption button 的容器与图标尺寸。

优点：

- 可以明显改善命中区与图标语义
- 仍属于 bugfix 级别，不会牵动整窗比例

缺点：

- 需要重新校准右侧预算和间距

#### 方案 C

连 titlebar 总高度一起提升到 `52-56px`。

优点：

- 空间最宽裕

缺点：

- 更像整窗视觉改版，不像定向修复

**最终决策：选择 `B`**

建议尺寸基线：

- titlebar 高度保持 `48px`
- 左侧导航与右侧 utility button 命中区提升到约 `36x36`
- utility icon 目标显示尺寸提升到约 `20px`
- caption controls 横向 hit area 继续保持接近系统标题栏节奏，但图标提升到约 `20px`
- 允许实现阶段在 `±2px` 范围内做视觉校准

### 5. 状态架构

#### 方案 A

纯 Slint 属性路线。

优点：

- UI 侧简单

缺点：

- 状态容易散落

#### 方案 B

纯 Rust 控制器路线。

优点：

- 状态集中

缺点：

- UI 主题状态也全压进 Rust，不够自然

#### 方案 C

混合路线：状态归 `ShellViewModel`，主题通过全局 token / window property 生效，置顶通过窗口能力桥接实现。

优点：

- 最贴合当前仓库现有分层
- 便于后续接轻量持久化
- 跨平台适配空间更好

缺点：

- 需要补完整的状态桥接

**最终决策：选择 `C`**

### 6. Theme Toggle

#### 方案 A

`Light / Dark` 两态，直接切换，且持久化。

优点：

- 交互最直接
- 与当前需求完全一致

缺点：

- 需要补轻量持久化入口

#### 方案 B

`Light / Dark` 两态，但仅会话有效。

优点：

- 实现较轻

缺点：

- 产品完成度明显不足

#### 方案 C

三态 `Follow System / Light / Dark`，标题栏按钮只打开菜单。

优点：

- 长期更完整

缺点：

- 超出本轮范围

**最终决策：选择 `A`**

图标与语义决策：

- Dark 模式显示 `darkTheme`
- Light 模式显示 `weatherSunny`
- tooltip 显示下一步动作，而不是仅复述当前状态

### 7. Pin / Always On Top

#### 方案 A

`Pin Off / Pin On` 两态，直接切换，且持久化。

优点：

- 桌面端语义最自然
- 与 theme toggle 的交互模型一致

缺点：

- 需要补轻量持久化入口

#### 方案 B

两态，但仅会话有效。

优点：

- 实现较轻

缺点：

- 用户体验偏弱

#### 方案 C

做成一次性动作，而不是状态。

优点：

- 逻辑最轻

缺点：

- 与 `pin / pinOff` 图标语义冲突

**最终决策：选择 `A`**

补充技术决策：

- Windows 首选 `always-on-top` 窗口能力
- 状态由 view model 持有
- 持久化记录“期望状态”
- 对不支持的后端允许优雅降级，但不改变 Windows 11 首发目标

### 8. Tooltip

#### 方案 A

鼠标进入立即显示。

优点：

- 响应最直接

缺点：

- 在密集图标区过于吵闹

#### 方案 B

全顶栏共享一个 tooltip popup，短延迟显示，离开或点击立即关闭。

优点：

- 最适合密集 icon toolbar
- 视觉稳定
- 与当前仓库已有 shared tooltip 方向一致

缺点：

- 需要校准延迟和锚点

#### 方案 C

不同按钮采用不同显示时机。

优点：

- 理论上更精细

缺点：

- 规则不统一

**最终决策：选择 `B`**

建议行为：

- hover 延迟建议约 `180-280ms`
- 点击按钮前先关闭 tooltip
- 打开 global menu 前先关闭 tooltip
- tooltip 不应遮挡 `Navigation` 主菜单锚点

### 9. Divider

#### 方案 A

使用 Fluent `Divider Tall`

优点：

- 风格统一

缺点：

- 容易受图标边界框影响

#### 方案 B

使用 Fluent `Divider Short`

优点：

- 更克制

缺点：

- 分组感可能不足

#### 方案 C

直接自绘一条半透明竖线。

优点：

- 最稳定
- 最像桌面标题栏里的结构分隔
- 不依赖图标 bounding box

缺点：

- 不属于 Fluent icon 资产的一部分

**最终决策：选择 `C`**

## 最终布局

### 顶栏最终结构

从左到右：

`Navigation` -> `Mica Term header logotype` -> `flex drag zone` -> `theme` -> `panel-toggle` -> `divider` -> `pin` -> `min` -> `maximize/restore` -> `close`

### 明确删除项

- `Workspace`
- `SSH`
- 旧的品牌文本拼接方案
- 顶栏中与主菜单重复的无效入口语义

### 图标语义

- `Navigation`: Fluent `navigation` 系列，优先 `20` 或 `24` 版本
- `theme-dark`: Fluent `darkTheme`
- `theme-light`: Fluent `weatherSunny` 或同语义 light counterpart
- `panel-toggle`: Fluent `panel-right`
- `pin-on`: Fluent `pin`
- `pin-off`: Fluent `pinOff`
- `minimize`: Fluent `subtract`
- `maximize`: Fluent `maximize`
- `restore`: Fluent `restore`
- `close`: Fluent `dismiss`

## 状态模型建议

### UI / ViewModel 层

建议补充以下状态：

- `theme_mode`
  - `Light`
  - `Dark`
- `is_always_on_top`
- 现有 `show_right_panel`
- 现有 `show_global_menu`
- 现有 `is_window_maximized`
- 现有 `is_window_active`

### Window / Token 桥接层

- `AppWindow.dark-mode` 必须真正驱动 `ThemeTokens.dark-mode`
- `AppWindow` 或窗口根级必须具备 `always-on-top` 桥接
- `WindowController` 继续承接 `drag / minimize / maximize / close`
- `always-on-top` 优先使用 Slint 自身窗口能力，必要时保留 `winit` 能力兜底

### 持久化约束

本轮设计要求 `theme` 与 `pin` 都是“可持久化状态”，但**不在本设计文档中锁死存储后端**。实现阶段只需要满足：

- 重启后恢复上次选择
- 配置缺失时回退到默认值
- 状态读取失败时不阻塞窗口启动

## 实施步骤

### 阶段 1：结构重排

- 移除 `Workspace` 与 `SSH`
- 将左侧导航按钮拆成独立最左区域
- 将右侧工具顺序调整为确认后的结构
- 为右侧新增 `theme`、`pin` 和自绘 `divider` 的空间预算

### 阶段 2：品牌资产落地

- 新增顶部专用 `Navigation-Led Logotype` SVG
- 让顶栏品牌区改用 SVG 资产而非文本拼排
- 保持顶栏字标与现有项目品牌色系统一致

### 阶段 3：状态桥接

- 补 `theme_mode` 与 `is_always_on_top`
- 完成 `AppWindow -> ThemeTokens` 主题闭环
- 完成 `Pin` 与窗口 `always-on-top` 能力闭环
- 补持久化入口

### 阶段 4：交互修正

- 放大 utility 与 caption controls 的 hit area
- 校准 maximize / restore / minimize 的视觉语义
- 修正右侧 cluster 与 caption controls 的间距与 hover 面

### 阶段 5：Tooltip 收口

- 保留 shared tooltip 架构
- 统一各按钮 tooltip 文案
- 统一显示/关闭时机
- 校准 popup 锚点，避免遮挡主菜单

## 风险与回滚

### 主要风险

- 顶栏右侧按钮数量增加后，如果宽度预算控制不好，可能重新出现拥挤感
- 新字标如果处理不当，可能与现有 app/taskbar 品牌体系脱节
- `always-on-top` 在不同平台后端支持程度不同，跨平台行为可能不完全一致
- `PopupWindow` tooltip 在高 DPI 或多屏条件下可能出现锚点偏差
- 自绘 caption controls 如果 hover / pressed 区域处理不好，可能出现误触或误读

### 回滚策略

- 如新字标效果不达标，可暂时回退到纯 wordmark 临时版，但不回退到旧 `MT + Text` 组合
- 如 `pin` 在非 Windows 后端不稳定，可先限制 UI 生效平台，同时保留状态模型
- 如 shared tooltip 在真实桌面行为异常，可先保留共享模型，仅缩减显示延迟和锚点复杂度
- 如右侧按钮过密，可优先微调 utility 区宽度与间距，避免先动整体 titlebar 高度

## 验证清单

- [ ] `Navigation` 按钮固定在窗口最左
- [ ] 顶栏品牌已替换为新的 `Navigation-Led Logotype`
- [ ] `Workspace` 已删除
- [ ] `SSH` 已删除
- [ ] 右侧顺序严格为 `theme -> panel-toggle -> divider -> pin -> min -> maximize/restore -> close`
- [ ] `panel-toggle` 明确位于 `theme` 后、`divider` 前
- [ ] `pin` 明确位于 `divider` 后、caption controls 前
- [ ] 最小化按钮视觉明确，不再与最大化/还原混淆
- [ ] 最大化与还原图标语义明确
- [ ] 右侧按钮整体比例较当前版本明显放大
- [ ] 所有按钮 hover 时均显示 tooltip
- [ ] tooltip 采用 shared popup + short delay 行为
- [ ] 点击按钮时 tooltip 立即关闭
- [ ] 打开 global menu 时 tooltip 不会遮挡菜单入口
- [ ] `theme` 可在 `Light / Dark` 间切换并持久化
- [ ] `theme` 切换后 `ThemeTokens` 实际发生变化
- [ ] `pin` 可在 `Pin Off / Pin On` 间切换并持久化
- [ ] Windows 11 上 `Pin` 能真实控制窗口始终置顶
- [ ] 整个顶栏在默认窗口宽度下不再出现拥挤、粘连或错读

## 后续衔接

如果进入实现阶段，下一份文档应补成：

- `docs/plans/2026-03-11-top-status-bar-style-bugfix2-implementation-plan.md`

该 implementation plan 需要进一步锁定：

- 精确文件清单
- 状态字段与回调命名
- 持久化入口
- 图标资产命名
- 逐步验证命令

# Mica Term Overall Style Design

日期: 2026-03-10  
执行者: Codex

## 背景

当前仓库仍处于空白起步阶段, 只有初始 `init` 提交, 尚无 Rust / Slint / Tokio 业务代码与现成 UI 原型。本设计文档用于先定义整机视觉与交互风格基线, 作为后续 Windows 11 首发版本的样式契约, 避免在工程初始化后边做边改。

产品定位为现代 SSH / SFTP 桌面终端客户端, 首发平台聚焦 Windows 11, 但底层风格体系需为后续迁移 macOS、Linux、Android、iOS 留出空间。视觉目标不是做展示型 landing page, 而是做一套以终端效率为核心、以 Fluent 为底、带有限品牌识别度的桌面工具外观。

## 目标

- 建立可执行的整机视觉方向, 为后续 Slint 组件、design tokens、窗口与面板样式提供统一依据
- 明确哪些区域承担品牌表达, 哪些区域必须保持系统一致性与中性
- 约束首版动效、层级、色彩、字体与尺寸节奏, 防止过度设计
- 为后续模块设计提供全局样式前提, 包括 `Custom Titlebar`、`Activity Bar`、`Terminal Tabs`、`Right Panel`

## 边界

### 本文档覆盖

- 整体视觉骨架
- 色彩与材质方向
- 层级、圆角、边框、阴影与密度
- 字体、图标、状态反馈与转场语言
- 品牌表达边界
- Welcome / Empty State 风格
- 首版微亮点范围

### 本文档不覆盖

- Rust / Slint 具体实现代码
- 终端渲染技术方案
- SSH / SFTP 连接架构
- 模块级详细交互流程
- 完整 implementation plan

## 前提与约束

- Slint 样式基础使用 `fluent`, 不锁死 `fluent-dark`
- 自定义 token 从一开始定义 `dark/light` 两套
- 首版验收优先级为 `dark > light`
- `light` 首版要求完整、清晰、不过曝
- 高对比模式首版只要求不崩、不失读, 完整适配放后续路线图
- 终端主工作区不得承担装饰性品牌表达

## 方案对比

### 1. 整体视觉骨架

- `A. Calm Fluent Shell`
  - 更偏系统原生与克制
- `B. Command Console Fluent`
  - 更偏工具型, 适合 Tab、侧栏、右侧面板、命令中枢
- `C. Premium Card Studio`
  - 更偏展示型与品牌化

最终选择: `B. Command Console Fluent`

选择原因:
- 最匹配 SSH / SFTP + Terminal Tabs 的桌面工具定位
- 适合 `Mica Alt` 倾向的分层式 Fluent 体验
- 比 `A` 更有专业工具感, 比 `C` 更稳更克制

### 2. 色彩与材质

- `A. Quiet Neutral`
  - 最稳, 但品牌记忆点偏弱
- `B. Tinted Console`
  - 中性终端区 + 顶部与右侧轻微 tint
- `C. Accent Forward`
  - 记忆点强, 但容易压过终端内容

最终选择: `B. Tinted Console`

选择原因:
- 主工作区终端背景保持中性, 不染色
- 顶部命令区与右侧面板承担轻微 tint
- accent 只保留一个主色, 保持识别度与克制平衡

### 3. 层级、边框与密度

- `A. Crisp Utility`
  - 信息密度高, 但偏硬
- `B. Layered Fluent Tooling`
  - 轻描边 + 明度差 + 局部阴影
- `C. Soft Glass Depth`
  - 更高级, 但更接近展示型

最终选择: `B. Layered Fluent Tooling`

选择原因:
- 主窗口外轮廓与右侧 Drawer 共用圆角体系
- 终端区只保留弱分割, 不做卡片浮起
- 阴影集中给 `Command Palette / Popup / Right Panel`
- 顶部栏、侧栏、右侧面板靠轻描边与明度差建立层级

### 4. 字体、图标与动效语言

- `A. Native Fluent`
  - `Segoe UI Variable` + `Cascadia Mono` + Fluent 线性图标
- `B. Modern Devtool`
  - 偏开发工具个性化
- `C. Hybrid Premium`
  - 更强品牌化, 更靠展示型

最终选择: `A. Native Fluent`

选择原因:
- 最贴近 Windows 11 与 Fluent 的系统一致性
- UI 字体与终端字体分离, 兼顾桌面感与终端可读性
- 动效只保留必要四类:
  - hover: 颜色 / 明度轻变
  - active: accent 强化 + 轻微背景填充
  - focus: 清晰但不刺眼的单色 focus ring
  - drawer / popup: `160-220ms` 的 width / opacity / translate 过渡

### 5. 页面比例与布局节奏

- `A. Compact Pro`
  - 偏紧凑
- `B. Balanced Desktop`
  - 平衡效率与 Fluent 呼吸感
- `C. Spacious Workbench`
  - 更偏品牌化工作台

最终选择: `B. Balanced Desktop`

基础尺寸:
- 顶部自绘栏: `48px`
- Activity Bar: `48px`
- Assets Sidebar 默认宽度: `256px`
- Tab Bar: `38px`
- Right Panel 默认展开宽度: `392px`
- 全局节奏基线: `8px`
- 常用间距: `8 / 12 / 16 / 24`

### 6. 主题策略

- `A. Dark-Only First Release`
- `B. Dual-Mode Spec From Day One`
- `C. Dark-Primary, Light-Compatible`

最终选择: `C. Dark-Primary, Light-Compatible`

选择原因:
- 终端产品首版主战场仍然是 dark
- 但从设计层面排除 light 会导致后续返工明显
- 因此从一开始定义双套 token, 首版优先打磨 dark, 保证 light 完整可用

### 7. 品牌表达边界

- `A. System-First Accessibility`
- `B. Balanced Brand Guardrail`
- `C. Brand-Forward Accessibility`

最终选择: `B. Balanced Brand Guardrail`

可品牌化区域:
- 顶部命令区背景轻微 tint
- Active tab / segmented control / panel accent
- 空状态插画或辅助图形

明确保持克制的区域:
- 终端正文区
- host 列表正文
- 普通文本
- focus / danger / warning 状态

### 8. 品牌记忆点分布

- `A. Command-Center Branding`
- `B. Navigation-Led Branding`
- `C. Ambient Branding`

最终选择: `B. Navigation-Led Branding`

第一品牌触点:
- Active tab
- 顶部命令区入口
- 右侧 panel 的 segmented control / active state

第二品牌触点:
- Activity Bar 当前选中项
- Welcome / empty state 的辅助图形

### 9. 首版记忆点组件集

- `A. Minimal Signature Set`
- `B. Balanced Signature Set`
- `C. Extended Signature Set`

最终选择: `B. Balanced Signature Set`

首版重点打磨组件:
- 顶部命令区入口
- Active tab
- 右侧 panel segmented control
- Welcome / empty state
- Command Palette

首版仅做功能性完成:
- host 列表正文
- 普通表单
- 普通按钮
- 普通状态文本

首版明确不做:
- 大面积背景纹理
- 强动态插画
- 复杂 page transition
- 终端区任何装饰性品牌表达

### 10. Welcome / Empty State

- `A. Quiet System`
- `B. Guided Console Welcome`
- `C. Showcase Landing`

最终选择: `B. Guided Console Welcome`

具体落点:
- 视觉中心: 欢迎标题 + 一句副标题
- 操作区: `New Connection`、`Open Recent`、`Snippets`、`SFTP`
- 辅助区: 最近主机列表或空状态提示
- 插画: 只做轻量辅助图形, 不做大面积品牌背景

### 11. 状态反馈

- `A. Quiet System Status`
- `B. Fluent Operational Status`
- `C. Monitor Dashboard Status`

最终选择: `B. Fluent Operational Status`

具体落点:
- `connecting`: 轻微动态, 不持续抢注意力
- `connected`: 稳定、低噪音的正向状态
- `disconnected / error`: 清楚, 但不使用夸张品牌色
- `toast`: 只承担动作反馈, 不承担主视觉
- `inline status` 优先于全局弹层

### 12. 转场语言

- `A. Near-Instant Utility`
- `B. Fluent Contextual Motion`
- `C. Stage-Like Motion`

最终选择: `B. Fluent Contextual Motion`

具体落点:
- `Welcome -> Terminal`: `140-180ms`, 轻微 `opacity + translate`
- `Tab` 切换: 只做 active state 与内容轻过渡, 不做水平滑屏
- `Right Panel`: 作为最主要的结构性动效
- `Command Palette / Popup`: 出现更明确, 关闭更快
- 连接状态变化: 只在状态点、pill、inline label 上局部变化, 不带动整页

## 最终决策

整机样式最终定义为:

> 一个以 Windows 11 Fluent 为底、以终端效率为核心、品牌表达集中在导航骨架和命令入口、但对终端正文与系统状态保持克制的桌面 SSH / SFTP 工具外观。

它不是极简系统壳, 也不是展示型品牌工作台, 而是一套“工具优先、品牌有限聚焦”的 Fluent 终端产品语言。

### 设计原则

- 终端工作区优先于装饰
- 导航层承担品牌记忆点
- 系统一致性优先于自定义炫技
- 动效服务于上下文切换, 不服务于表演
- 首版只打磨少数高价值识别点

## 实施步骤

### 阶段 1: 建立设计 token 基线

- 定义 `dark/light` 两套色彩 token
- 定义圆角、描边、阴影、间距、字体与动效时长 token
- 先确定 `accent` 单色系统, 不引入多色品牌体系

### 阶段 2: 完成 Shell 层样式骨架

- 建立自绘顶栏、Activity Bar、Assets Sidebar、Tab Bar、Right Panel 的尺寸基线
- 完成外轮廓、分层、弱描边与局部阴影策略
- 确保主终端区始终保持中性和低装饰

### 阶段 3: 打磨首版记忆点组件

- 顶部命令区入口
- Active tab
- Right Panel segmented control
- Command Palette
- Welcome / empty state

### 阶段 4: 校准状态反馈与动效

- 统一 `hover / active / focus / connecting / connected / error` 反馈语言
- 校准 `Welcome -> Terminal`、Right Panel、Popup 的统一转场时长
- 确保动效不影响高频终端操作效率

### 阶段 5: 完成主题与可访问性基线验证

- 验证 `dark` 首版主验收效果
- 验证 `light` 模式完整可用且不过曝
- 验证高对比模式下不崩、不失读
- 验证键盘可达性与 focus 清晰度

## 风险与回滚

### 主要风险

- 品牌表达过强, 导致终端主区被视觉噪音干扰
- 顶部命令区、Tab、右侧面板同时强化后, 导航骨架显得过忙
- `light` 模式若仅靠 dark 反推, 可能出现泛白、边界弱、层级塌陷
- 动效若分散定义, 最终会显得不一致
- 后续模块若不遵循本契约, 整机会出现风格断裂

### 回滚策略

- 若品牌感过强: 回退到 `Quiet Neutral` 的色彩强度, 降低 tint 与 accent 占比
- 若层级过厚: 减少局部阴影, 保留描边与明度差
- 若 `light` 观感不稳: 先保留结构一致性, 进一步压缩 tint 和填充强度
- 若动效过多: 回退到只保留 Right Panel 与 Popup 的主结构动画
- 若欢迎页过于展示化: 回退到更纯工具型的 `Guided Console Welcome`

## 验证清单

### 视觉一致性

- [ ] 顶栏、侧栏、Tab、右侧面板遵循统一圆角与分层体系
- [ ] 终端主区保持中性, 无装饰性品牌表达
- [ ] accent 仅为单一主色, 未形成多色竞争
- [ ] 首版重点打磨组件具有清晰识别度, 但不破坏整体克制感

### 主题与可读性

- [ ] `dark` 模式达到主验收标准
- [ ] `light` 模式完整、清晰、不过曝
- [ ] 普通文本不因品牌色染色而影响可读性
- [ ] `focus / danger / warning` 状态遵循系统一致性

### 交互与动效

- [ ] hover / active / focus 反馈清晰且不突兀
- [ ] `Welcome -> Terminal` 转场时长在 `140-180ms` 区间
- [ ] Right Panel 动效为主要结构性动效, 其余动画克制
- [ ] 连接状态变化只影响局部状态组件, 不影响整页稳定性

### 可访问性底线

- [ ] 所有主要操作可通过键盘到达
- [ ] `dark/light` 下 hover/active/focus 清晰可辨
- [ ] 高对比模式下不崩、不失读

## 后续路线

以下内容不进入首版默认目标, 仅作为后续路线图:

- `Premium Card Studio` 视觉升级分支
- 高对比主题完整适配
- 更系统化的 accessibility 审查
- 更强品牌图形系统与展示性增强

## 参考资料

- Slint Widget Styles: https://docs.slint.dev/latest/docs/slint/reference/std-widgets/style/
- Slint Window: https://docs.slint.dev/latest/docs/slint/reference/window/window/
- Slint Animations: https://docs.slint.dev/latest/docs/slint/guide/language/coding/animation/
- Fluent 2 Design System: https://fluent2.microsoft.design/
- Windows Mica: https://learn.microsoft.com/en-us/windows/apps/design/style/mica
- Windows Layering: https://learn.microsoft.com/en-us/windows/apps/design/signature-experiences/layering

# Mica Term Sidebar Navigation Design

日期: 2026-03-11  
执行者: Codex

## 背景

当前仓库已经完成第一轮 shell 外观基线与顶部状态栏交互修整, 但左侧导航仍停留在空壳阶段。

- `ui/app-window.slint` 已将 `Sidebar` 作为主布局左侧区域插入, 但当前只是单一占位组件
- `ui/shell/sidebar.slint` 只有 `48px` 宽度、背景与描边, 没有任何导航项、回调、状态或动画
- `src/shell/metrics.rs` 与 `tests/window_shell.rs` 已明确保留 `Activity Bar = 48px` 与 `Assets Sidebar = 256px` 的双层左区尺寸契约
- `src/shell/view_model.rs` 当前没有左侧导航选中态、折叠态或模块模型
- 中央区域仍然是 `WelcomeView`, 真实 terminal runtime 尚未接入

这意味着当前任务不应被理解为“给 48px 空栏塞几个按钮”, 而应被理解为“恢复并落定左侧双层导航骨架, 为后续 SSH / Snippets / Keychain / SFTP 承载稳定入口”。

## 目标

- 为左侧导航建立明确的信息架构, 满足 `Folder / Folder Open`、`Window Console`、`Snippets`、`Keychain` 三个一级业务入口
- 保持与已落地的 shell 视觉基线一致, 继续遵循 Windows 11 Fluent + Mica Alt 语言
- 保持底层结构对未来 `wezterm-term`、`russh`、`SFTP`、跨平台迁移友好
- 保留未来扩展位, 避免首轮实现后很快推翻左侧导航架构

## 边界

### 本文档覆盖

- 左侧导航整体骨架
- `Folder / Folder Open` 的语义与交互
- `Window Console`、`Snippets`、`Keychain` 的承载方式
- 左侧导航的数据建模边界
- 图标资源策略
- 预留功能位
- 风险、回滚与验证清单

### 本文档不覆盖

- 真实 terminal 控件渲染接入
- SSH / SFTP runtime、会话复用与连接生命周期
- Snippet 执行引擎与变量模板系统
- Keychain 的加密、同步与存储策略
- 逐文件 implementation plan

## 源码与历史结论

### 当前源码结论

- `Sidebar` 当前只是一条 `48px` 空栏, 还不是可工作的 navigation shell
- `AppWindow` 目前没有独立的 `Assets Sidebar` 内容区组件, 左侧结构被临时压缩成单层
- Rust 侧尚未存在 `SidebarDestination` 或类似导航枚举, 也没有 `ModelRc` 驱动的导航模型

### Git 历史结论

- `a1357ce feat: implement overall style shell baseline` 建立了 `Titlebar / Sidebar / TabBar / RightPanel / WelcomeView` 的静态骨架
- 之后的提交主要集中在 `Titlebar`、tooltip overlay、theme / window appearance, 没有进入 sidebar 信息架构
- 因此本轮可以直接围绕既有 shell 基线定义左侧导航, 不需要兼容历史业务逻辑

## 设计要点与方案对比

### 1. 左侧整体骨架

#### 方案 A: 单层 `48px icon rail`

优点:

- 实现最简单
- 视觉最克制
- 对当前 `Sidebar` 占位改动最少

缺点:

- `Window Console / Snippets / Keychain` 都缺少稳定的二级信息承载区
- 很快会把列表、详情、过滤塞回主区或右侧面板, 架构会失衡
- 与现有 `Activity Bar + Assets Sidebar` 尺寸契约冲突

#### 方案 B: 双层 `Activity Bar + Assets Sidebar`

优点:

- 与现有 `48 + 256` 指标完全一致
- 一级导航与模块内容解耦, 最适合 terminal 工具
- 可以同时保留主工作区与左侧浏览区
- 后续扩展 `Transfers / Tunnels / SFTP Browser` 成本最低

缺点:

- 需要补回左侧第二层内容面板
- 状态建模比单层稍复杂

#### 方案 C: `48px rail + hover/flyout`

优点:

- 默认更轻
- 不必长期占据 `256px`

缺点:

- 桌面端 hover/flyout 容易抖动
- 不适合高频浏览列表
- 对触控与跨平台一致性不友好

最终选择: `方案 B`

### 2. `Folder / Folder Open` 的语义

#### 方案 A: 作为左侧内容栏开关

优点:

- 完全符合本轮需求描述
- 心智模型直接, `Folder Open = 左侧内容栏已展开`
- 不与业务模块争抢一级导航位

缺点:

- 需要额外定义“关闭后仍显示哪一个当前模块”的规则

#### 方案 B: 作为普通业务模块

优点:

- 扩展空间大
- 可承载项目树、工作区、资产浏览

缺点:

- 不符合当前需求的核心语义
- 会把“结构控制”与“业务导航”混在一起

最终选择: `方案 A`

### 3. `Window Console` 的承载方式

#### 方案 A: 切主区

优点:

- 实现最直接
- 无需额外左侧内容布局

缺点:

- 每次查看会话列表都要打断主工作区
- 不符合现代终端工具的多区域工作流

#### 方案 B: 展开左侧 `Assets Sidebar` 承载主机与会话列表

优点:

- 最符合 SSH 终端工具的浏览习惯
- 主区可以继续保留 terminal tabs / welcome / active session
- 后续接最近连接、分组、收藏、搜索都自然

缺点:

- 需要定义内容面板内部层级

#### 方案 C: 放到右侧面板

优点:

- 能复用现有 `RightPanel`

缺点:

- 与 inspector/detail 面板角色冲突
- 布局重心错误

最终选择: `方案 B`

### 4. `Snippets` 的承载方式

#### 方案 A: 与 `Window Console` 共用左侧 `Assets Sidebar`

优点:

- 架构一致
- 既适合“快速收藏命令”, 也适合未来扩展到分类、标签、变量模板
- 右侧面板未来仍可承担详情与预览

缺点:

- 需要明确内容区切换规则

#### 方案 B: 通过 `Command Palette / Modal`

优点:

- 轻量
- 对少量 snippets 足够快

缺点:

- 不适合浏览与管理
- 很难承接后续结构化能力

#### 方案 C: 全放右侧面板

优点:

- 细节编辑时靠近主区

缺点:

- 不适合作为一级导航模块的主承载区

最终选择: `方案 A`

### 5. `Keychain` 的承载方式

#### 方案 A: 作为一级模块, 内容显示在左侧 `Assets Sidebar`

优点:

- 与 Termius 类产品心智一致
- 适合后续扩展 `Accounts / Identities / SSH Keys / Vault Groups`
- 保持与 Console / Snippets 同层级

缺点:

- 需要提前规划列表与详情边界

#### 方案 B: 收纳到 `Settings / Modal`

优点:

- 首轮实现更简单

缺点:

- 会削弱 Keychain 的产品权重
- 不符合本轮需求中“一级导航模块”的定位

最终选择: `方案 A`

### 6. 左侧导航的数据建模

#### 方案 A: 在 Slint 中硬编码 4 个按钮

优点:

- 视觉实现最快
- 初期心智负担最低

缺点:

- 后续增加 badge、禁用态、分组、底部 utility 区几乎必然返工
- UI 与 Rust 状态边界不清晰

#### 方案 B: Rust 枚举 + `ModelRc` 驱动导航项

优点:

- 与 Slint 官方推荐的数据列表模式一致
- 后续扩展成本最低
- 可自然支持选中态、可见性、预留模块、底部 utility 区

缺点:

- 首轮比硬编码多一层模型定义

最终选择: `方案 B`

### 7. 图标资源策略

#### 方案 A: 继续 vendoring Fluent SVG, 用 `@image-url(...)`

优点:

- 与现有 `Titlebar` 图标策略一致
- 构建链路简单
- 不引入新的运行时依赖

缺点:

- 需要手动维护 SVG 资源

#### 方案 B: icon font 或第三方图标 crate

优点:

- 资源集中管理

缺点:

- 偏离当前项目资产策略
- 增加额外依赖与适配成本

最终选择: `方案 A`

### 8. 预留功能位策略

#### 方案 A: 先在模型层预留, UI 不默认展示

优点:

- 当前界面更完整克制
- 后续启用新模块不需要推翻结构

缺点:

- 预留能力对用户不可见

#### 方案 B: 现在就显示 disabled ghost icons

优点:

- 能传达 roadmap

缺点:

- 会让首版界面显得未完成
- 容易制造“按钮点不开”的负反馈

最终选择: `方案 A`

## 最终决策

最终确认方案为:

`1B + 2A + 3B + 4A + 5A + 6B + 7A + 8A`

对应决策如下:

- 左侧采用双层结构: `Activity Bar 48px + Assets Sidebar 256px`
- 顶部 `Folder / Folder Open` 不属于业务模块, 只负责控制左侧 `Assets Sidebar` 展开与折叠
- 分隔线位于 `Folder / Folder Open` 下方, 用来区分“结构控制”与“业务导航”
- 一级业务导航为:
  - `Window Console`
  - `Snippets`
  - `Keychain`
- `Window Console`、`Snippets`、`Keychain` 的主要内容都承载在左侧 `Assets Sidebar`
- 左侧导航项由 Rust 模型驱动, 而不是在 Slint 中手写固定按钮
- 图标继续使用 vendored Fluent SVG 资源
- 未来扩展位只在模型层预留, 首版 UI 不直接暴露

## 信息架构

### 左侧区域分层

#### `Activity Bar`

- 宽度固定 `48px`
- 顶部是 `Folder / Folder Open`
- 下方 `1px` 分隔线
- 中段是一级业务导航按钮
- 底部预留 utility 区, 后续用于 `Settings / Logs`

#### `Assets Sidebar`

- 默认展开, 宽度 `256px`
- 根据当前一级导航切换内容
- 关闭后宽度收至 `0`, 但 `Activity Bar` 始终保留

### 默认状态

基于产品主目标, 默认激活模块推断为 `Window Console`。

说明:

- 这是基于 SSH 终端工具定位做出的设计推断
- 若后续产品希望以 `WelcomeView` 为更强的初始入口, 仍可保留 `Window Console` 选中态但不强制立即展示会话详情

## 模块承载定义

### `Window Console`

左侧内容区承载:

- 主机分组
- 最近连接
- 收藏会话
- 打开的终端会话列表

后续可扩展:

- 搜索
- 标签过滤
- 会话状态徽标

### `Snippets`

左侧内容区承载:

- Snippet 分组
- 收藏命令
- 最近使用
- 模板入口

后续可扩展:

- 变量模板
- 参数化执行
- 右侧预览 / 编辑详情

### `Keychain`

左侧内容区承载:

- 账号列表
- Identity / SSH Key 列表
- Vault / Group 分组
- 快速切换入口

后续可扩展:

- 凭据搜索
- 关联 host
- 权限标签

## 交互规则

### 折叠与展开

- 默认显示 `Folder Open`
- 点击后收起 `Assets Sidebar`, 图标切换为 `Folder`
- 再次点击后恢复展开, 图标切回 `Folder Open`
- 折叠时保留当前一级模块选中态, 不重置当前 destination

### 模块切换

- 点击一级业务导航时:
  - 若内容栏已关闭, 先自动展开
  - 再切换到对应模块内容
- 一级导航只负责切换左侧内容区上下文, 不直接重排主工作区

### 视觉反馈

- 图标按钮保持与现有 `TitlebarIconButton` 同一套 Fluent 风格语言
- `hover` 使用低噪音 tinted 背景
- `active` 使用更明确的背景层级与选中指示
- 保持对 terminal 主工作区克制, 品牌记忆点集中在导航骨架

## 预留功能建议

模型层优先预留以下能力:

- `Transfers / SFTP Browser`
- `Port Forwarding / Tunnels`
- `Settings`
- `Logs`

布局建议:

- `Transfers / SFTP Browser`、`Port Forwarding / Tunnels` 预留为未来一级业务导航
- `Settings`、`Logs` 更适合作为 `Activity Bar` 底部 utility 区能力

## 实施步骤

### 阶段 1: 恢复左侧双层结构

- 将现有 `Sidebar` 从单层占位恢复为 `Activity Bar + Assets Sidebar` 架构
- 对齐现有 `48 + 256` 设计契约

### 阶段 2: 建立导航状态模型

- 在 Rust 侧定义左侧导航 destination、折叠态、选中态
- 用 `ModelRc` 驱动 Slint 导航项渲染

### 阶段 3: 接入三个首发模块

- `Window Console`
- `Snippets`
- `Keychain`

### 阶段 4: 接入折叠交互与动效

- `Folder / Folder Open` 状态切换
- 左侧内容栏宽度与透明度过渡
- 模块切换时的低干扰过渡

### 阶段 5: 验证视觉与架构一致性

- 核对尺寸契约
- 核对选中态、折叠态、图标状态
- 核对未来扩展位不会破坏当前结构

## 风险与回滚

### 主要风险

- 若继续沿用单层 `48px` rail, 后续 `Console / Snippets / Keychain` 会争抢主区与右侧面板
- 若 `Folder / Folder Open` 被设计成业务模块, 结构控制语义会混乱
- 若首轮直接硬编码按钮, 很快会在扩展 badge、预留模块、utility 区时返工
- 若把 `Window Console` 放进主区或右侧, 会削弱桌面终端产品的多区域效率

### 回滚策略

- 若双层结构在首轮实现中复杂度超出预期, 允许先交付静态双层骨架与空内容面板
- 不回退到“把所有模块都塞进单层 48px rail”的方向
- 不回退到“把 Keychain 收进 Settings”的方向

## 验证清单

- [ ] 左侧宽度契约保持 `48px + 256px`
- [ ] 默认图标为 `Folder Open`
- [ ] 收起内容栏后图标切为 `Folder`
- [ ] 分隔线位于结构控制与业务导航之间
- [ ] 一级业务导航仅包含 `Window Console`、`Snippets`、`Keychain`
- [ ] 点击业务导航时, 若内容栏关闭会自动展开
- [ ] 当前选中模块在折叠 / 展开之间保持不丢失
- [ ] 导航项来源于 Rust 模型而非 Slint 硬编码
- [ ] 图标资源继续使用 Fluent SVG + `@image-url(...)`
- [ ] 模型层已为 `Transfers / Tunnels / Settings / Logs` 预留扩展位


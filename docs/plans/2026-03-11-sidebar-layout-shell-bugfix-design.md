# Sidebar Layout Shell Bugfix Design

日期: 2026-03-11  
执行者: Codex

## 背景

当前 `Sidebar` 导航骨架已经在 `fcc313d feat: implement sidebar navigation shell` 中落地，但界面在真实窗口场景下出现了明显的壳层布局异常：

- 默认打开时，主工作区下半部分缺失，实际可见高度明显小于窗口高度
- 最大化后，上半部分布局与默认尺寸几乎一致，下半部分变成大面积空白
- 异常同时影响左侧 `Sidebar / Assets Sidebar`、中间主内容区与整体视觉完整性

本轮任务不是继续扩展业务功能，而是修复 shell 级布局契约，使窗口在默认尺寸、恢复尺寸和最大化状态下都能正确填满 client area，并为后续 terminal、SSH、SFTP 模块保留稳定外壳。

## 目标

- 让启动窗口尺寸成为真实生效的 runtime 契约，而不是仅存在于文档和测试中的常量
- 重构 `AppWindow` 的 `titlebar + body` 布局层级，使 `body` 成为唯一负责吃满剩余高度的容器
- 为左侧 `Activity Bar / Assets Sidebar`、中间主区、右侧面板定义清晰的响应式优先级
- 补齐能阻止同类回归的几何契约测试，而不是只验证状态和尺寸常量

## 边界

### 本文档覆盖

- `AppWindow` / shell 布局层级调整
- 启动窗口默认尺寸策略
- 左中右三区域的响应式收缩优先级
- 最大化、恢复、默认打开三类窗口状态的验证策略

### 本文档不覆盖

- `wezterm-term` / `termwiz` 终端接入
- `russh` / `SFTP` 业务逻辑
- `Snippets` / `Keychain` 真实数据结构和交互细节
- 视觉风格重新设计
- 持久化“上次窗口大小与位置”的产品策略

## 调研结论

### Git 历史

- `a1357ce feat: implement overall style shell baseline` 建立了最初的 `Titlebar / Sidebar / TabBar / RightPanel / WelcomeView` 壳层骨架
- `fb7ab7b feat: implement top status bar shell chrome` 到 `9e9941b fix: lift titlebar tooltip to window overlay` 主要在修顶部状态栏、tooltip overlay 与 window appearance
- `fcc313d feat: implement sidebar navigation shell` 把左侧空栏替换为 `Activity Bar + Assets Sidebar`，但没有同步补齐整窗 body 的拉伸契约

### 当前源码证据

- `ui/app-window.slint` 目前通过一个 `VerticalLayout` 组织 `Titlebar` 与下方 `HorizontalLayout`，但 body 没有被抽象成单独的 fill container
- `ui/shell/sidebar.slint` 与 `ui/shell/assets-sidebar.slint` 目前更偏向“按内容高度参与布局”，缺少“吃满剩余垂直空间”的壳层约束
- `ui/welcome/welcome-view.slint` 仍然只是两个绝对定位文本，不构成对主区高度的显式占满
- `src/app/bootstrap.rs` 暴露了 `default_window_size() -> (1440, 900)`，但当前运行路径里没有看到它被作为真实窗口 restored size 明确应用
- 当前测试主要验证状态切换、尺寸常量和回调连通性，尚未覆盖默认尺寸、最大化和 body 几何契约

### 外部参考

- Slint 布局文档说明：布局子项如果没有显式 fill / stretch 约束，会按最小尺寸和首选尺寸参与布局，而不是自动撑满可用空间
- 因此本问题更接近“布局契约缺失”，不是单纯的样式或渲染抖动

参考：

- https://docs.slint.dev/latest/docs/slint/guide/language/coding/positioning-and-layouts/
- https://docs.slint.dev/latest/docs/slint/reference/layouts/overview/

## 设计要点与方案对比

### 1. 启动窗口尺寸契约

#### 方案 A：仅依赖 `.slint` 中的 `preferred-width / preferred-height`

优点：

- 声明式最简单
- 改动面最小

缺点：

- 当前现象表明该策略在本项目现状下不足以保证真实启动尺寸
- Rust 侧 `default_window_size()` 会继续成为未被消费的孤立常量

#### 方案 B：Rust runtime 显式设置 restored size，`.slint` 保留 preferred size 作为设计默认值

优点：

- 启动窗口尺寸有单一真源，行为可预测
- 更适合 `no-frame + Mica Alt` 的 Windows 桌面场景
- 后续扩展“记住上次窗口尺寸”时演进成本低

缺点：

- 需要在 bootstrap / windowing 层显式接管一次窗口尺寸同步

#### 方案 C：直接上“记住上次非最大化窗口尺寸与位置”

优点：

- 产品体验最好

缺点：

- 超出本轮 bugfix 范围
- 会把布局修复和持久化策略耦合在一起

最终选择：`方案 B`

### 2. 主内容区拉伸策略

#### 方案 A：在现有组件树上逐层补 `height: 100%`、`preferred-height: 100%`、`vertical-stretch`

优点：

- 改动最小
- 可以快速验证问题是否来自单层约束缺失

缺点：

- 结构仍然模糊，后续继续加 terminal surface、split pane、overlay 时容易再次失衡
- 需要记住很多局部规则，维护成本高

#### 方案 B：重构为明确的 `ShellFrame / ShellBody` 层级，`ShellBody` 作为唯一吃满剩余空间的容器

优点：

- 结构清晰，职责边界明确
- `Titlebar` 与 `Body` 的几何关系可以被稳定验证
- 适合后续接入 terminal host、overlay、split panes、cross-platform shell 容器

缺点：

- 比局部修补多一次壳层重组
- 需要同步审查 tooltip overlay、right panel、sidebar 在新层级中的挂载关系

#### 方案 C：用 `ScrollView / Flickable` 包一层规避空白

优点：

- 覆盖表象问题很快

缺点：

- 只是掩盖，不是修复
- 不适合桌面终端主工作区

最终选择：`方案 B`

### 3. Sidebar 与主区的响应式边界

#### 方案 A：左右栏全部固定宽度，仅通过最小窗口尺寸防止挤压

优点：

- 规则简单
- 最接近视觉稿

缺点：

- 对恢复窗口和中小屏设备不够友好
- 一旦宽度不足，整体会显得僵硬

#### 方案 B：优先级响应式

规则：

- `Activity Bar` 永远固定 `48px`
- `Assets Sidebar` 是第一个可折叠区域
- `RightPanel` 是第二个可收起区域
- 主区保留最低可用宽度
- 垂直溢出由局部面板自身处理，不让整窗主布局失真

优点：

- 最符合终端工具的桌面产品逻辑
- 兼顾默认打开、恢复窗口和最大化
- 为后续终端标签、多会话、inspectors 保留稳定行为

缺点：

- 规则更多，测试矩阵也更大

#### 方案 C：左右栏按比例缩放

优点：

- 小窗口下也能全部显示

缺点：

- 会破坏 Fluent 桌面命中区和图标比例
- 不适合桌面终端类工具

最终选择：`方案 B`

### 4. 回归验证策略

#### 方案 A：补几何契约测试

验证内容：

- 默认启动尺寸是否等于约定的 restored size
- 最大化后 `ShellBody` 是否吃满 `Window - Titlebar`
- 左侧、中间、右侧容器是否都匹配新的壳层几何规则
- `Assets Sidebar` 折叠与 `RightPanel` 收起时主区是否按预期扩展

优点：

- 能直接拦住本次这类“看起来像样式，实则是布局契约”的问题
- 比纯视觉截图更稳

缺点：

- 需要补一层 UI geometry 观测能力

#### 方案 B：截图 golden test

优点：

- 最直观

缺点：

- 易受平台、字体、缩放、渲染细节影响
- 维护成本高

#### 方案 C：继续只测状态和尺寸常量

优点：

- 最轻量

缺点：

- 已被当前 bug 证明不足

最终选择：`方案 A`

## 最终决策

本轮确认方案为：`1B + 2B + 3B + 4A`

即：

1. 由 Rust runtime 显式接管 restored window size，`.slint` 的 preferred size 保留为设计默认值
2. 将当前窗口壳层重构为更清晰的 `ShellFrame / ShellBody` 结构，确保 `ShellBody` 成为唯一吃满剩余高度的容器
3. 使用优先级响应式策略：固定 `Activity Bar`，优先折叠 `Assets Sidebar`，其次收起 `RightPanel`，最后保护主区最小可用宽度
4. 通过几何契约测试建立回归防线，不再只依赖状态类测试

## 建议实施步骤

### 阶段 1：确立窗口尺寸真源

- 在 Rust 运行时为窗口显式设置 restored size
- 保持 `default_window_size()` 与 `.slint preferred-size` 数值一致
- 明确“默认尺寸”和“最大化状态”各自的职责边界

### 阶段 2：重组 shell 根布局

- 在 `AppWindow` 中引入清晰的 `ShellFrame / ShellBody` 结构
- `Titlebar` 只负责顶部壳层
- `ShellBody` 负责承载 `Sidebar + Main Content + RightPanel`
- tooltip overlay 继续保持独立 overlay 层，不混入 body 布局流

### 阶段 3：为三列布局定义响应式优先级

- 固定 `Activity Bar`
- `Assets Sidebar` 依据显式状态和宽度预算折叠/展开
- `RightPanel` 保持独立可见性，不挤占主区到底
- 主区成为剩余空间的最终承接者，并在恢复/最大化时都要跟随窗口高度变化

### 阶段 4：补齐验证

- 为默认启动、恢复窗口、最大化窗口补几何契约测试
- 为 sidebar 折叠和右侧面板切换补主区扩展验证
- 保留现有 state smoke tests，但不再把它们视为主要防线

## 风险与回滚

### 主要风险

- `no-frame` 窗口与自绘 `Titlebar` 的几何关系在 Windows 11 下可能与测试 backend 表现存在差异
- `ShellBody` 重组后，tooltip overlay、right panel 边角半径和分割线可能出现新的视觉偏差
- 如果响应式规则一次引入过多，可能同时影响 sidebar、main area、right panel 三个区域

### 风险控制

- 先把“默认尺寸真源”和“ShellBody 吃满剩余空间”做成最小可验证变更
- 再叠加响应式收缩优先级
- 保持 overlay 与业务内容区分层，不让 tooltip 成为布局参与者

### 回滚策略

- 若 `ShellBody` 重构引入新回归，允许先保留 runtime window size 修复，再回退局部响应式策略
- 若响应式优先级引发连锁问题，可暂时退回为“固定 `Activity Bar + Assets Sidebar`，隐藏 `RightPanel`”的中间态
- 若测试 backend 与 Windows 实际行为不一致，以几何契约为主、人工截图为辅进行差异收敛

## 验证清单

- 默认打开时窗口恢复为约定尺寸，不再出现下半区缺失
- 最大化后主内容区高度随窗口增长，不再出现大面积下方空白
- `Titlebar` 高度保持固定，`ShellBody` 高度等于 `window height - titlebar height`
- `Activity Bar` 始终保持固定宽度
- `Assets Sidebar` 折叠后主区横向扩展正确
- `RightPanel` 打开和关闭时不破坏 `ShellBody` 的垂直填充
- 现有 sidebar 状态切换测试继续通过
- 新增几何契约测试能够覆盖默认、恢复、最大化三种窗口状态

## 结论

这次问题的核心不是单个控件样式错误，而是 shell 根布局缺少稳定的窗口几何契约。只有同时确立 runtime window size 真源、重组 `ShellFrame / ShellBody`、定义响应式优先级，并用几何测试守住边界，才能避免 Sidebar 之后继续在 terminal host、split panes、overlay 等阶段重复出现同类问题。

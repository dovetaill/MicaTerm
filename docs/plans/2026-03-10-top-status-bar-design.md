# Mica Term Top Status Bar Design

日期: 2026-03-10  
执行者: Codex

## 背景

当前仓库已经完成第一轮整体壳层样式基线, 但顶部区域仍处于静态占位阶段。

- `ui/app-window.slint` 已启用 `no-frame: true`, 说明窗口已经进入自绘标题栏路线
- `ui/shell/titlebar.slint` 当前只包含一个 `CommandEntry`, 还不是完整 `title bar`
- `src/app/windowing.rs` 仅声明 `MicaAlt` 偏好和 `no_frame`
- `src/app/bootstrap.rs` 只负责创建并运行 `AppWindow`, 尚未提供窗口命令桥接
- `src/shell/view_model.rs` 仅有 `show_welcome` / `show_right_panel` 静态状态, 尚未绑定到 Slint UI
- 中央区域当前仍是 `WelcomeView`, 真实 terminal 控件尚未接入

这意味着本轮设计不需要迁就已有 terminal widget, 可以围绕现有 `frameless shell` 基座直接定义顶部状态栏的结构、交互边界和状态模型, 为后续接入真实终端区、连接状态和全局命令入口预留稳定接口。

## 目标

- 将顶部状态栏定义为完整可工作的 `custom title bar`
- 满足左侧品牌区 / 中间拖拽区 / 右侧状态与窗口控制区三段需求
- 满足窗口拖动、双击最大化/还原、最小化/关闭、设置菜单、右侧侧栏开关等验收要求
- 保持 Windows 11 Fluent + Mica Alt 的一体化视觉
- 保持底层架构对 macOS / Linux / Android / iOS 的迁移友好性

## 边界

### 本文档覆盖

- 顶部状态栏的布局骨架
- 拖拽区与双击行为策略
- 右上角三大金刚键的交互方案
- 设置按钮下拉菜单的承载方式
- 顶栏相关状态模型与窗口命令桥接边界
- 风险、回滚策略与验证清单

### 本文档不覆盖

- 真实 terminal 渲染接入
- SSH / SFTP 会话逻辑
- Command Palette 的完整产品设计
- 实现级 API 细节与逐文件改动列表
- 详细 implementation plan

## 前提与约束

- 保持 `Slint + Rust` 为主, 不引入新的桌面 UI 框架
- 保持当前 `no-frame: true` 路线, 不回退到系统默认标题栏
- 视觉上保持 Fluent / Mica Alt 一体化, 不接受明显割裂的原生菜单外观
- Windows 11 为首发优先级, 但设计不能把核心行为永久锁死在 Win32-only 细节上
- 当前阶段允许引入最少必要的 `winit` 访问层, 但只作为窗口命令适配层, 不反向污染整个 UI 架构

## 当前现状判断

### Git 与源码结论

最近与当前壳层直接相关的提交是 `a1357ce feat: implement overall style shell baseline`。该提交完成了 `frameless shell`、`MicaAlt` 外观偏好、`Titlebar / Sidebar / TabBar / RightPanel / WelcomeView` 基础结构, 但尚未进入运行时交互阶段。

当前可以明确视为“已存在但未落地”的能力:

- 自定义标题栏路线已经确定
- `Right Panel` 已有视觉占位, 但没有展开/收起状态联动
- `Window` 已经可通过 Slint API 控制最大化/最小化
- 顶栏尺寸已和设计基线对齐为 `48px`

当前仍缺失的关键能力:

- 拖拽区与双击最大化逻辑
- 窗口控制按钮
- 设置菜单弹出层
- 顶栏状态模型
- UI 与 Rust 侧窗口命令桥接

## 设计要点与方案对比

### 1. 顶栏骨架布局

#### 方案 A: 简单三段式

结构:

- 左侧固定宽度品牌区
- 中间 `stretch` 拖拽区
- 右侧固定宽度动作区

优点:

- 最容易在当前 `HorizontalLayout` 上演进
- 代码和布局约束最少
- 小步实现成本低

缺点:

- 窄窗口下中间拖拽区容易被左右内容压缩
- 后续增加状态图标、同步状态、连接指示时弹性不足
- 不够像成熟桌面工具的标题栏

#### 方案 B: Windows-aware 五段式

结构:

- 左侧品牌区
- 左中主拖拽区
- 右中动作区
- 靠近 caption controls 的最小拖拽安全区
- 最右窗口控制区

优点:

- 更接近 Windows 自定义标题栏最佳实践
- 始终保留可发现的拖拽空白
- 适合后续扩展状态图标和更多全局入口
- 有利于控制交互区域与拖拽区域的边界

缺点:

- 布局定义和状态响应比三段式复杂

最终选择: `B. Windows-aware 五段式`

选择原因:

- 更符合首发平台 Windows 11 的使用习惯
- 能在不牺牲拖拽体验的前提下容纳更多全局动作
- 对后续状态图标、同步状态、连接态扩展最稳

### 2. 中间拖拽区与双击最大化

#### 方案 A: Slint 手势 + winit 原生拖拽

结构:

- `TouchArea` 负责命中、点击、双击
- Rust 侧窗口命令适配层通过 `WinitWindowAccessor` 获取底层 `winit::window::Window`
- 单击拖拽调用 `drag_window()`
- 双击调用 `set_maximized(!is_maximized())`

优点:

- 拖拽行为最接近系统原生
- 更适合窗口吸附、拖到屏幕边缘最大化等桌面行为
- 双击逻辑清晰, 且仍可留在跨平台窗口服务抽象内

缺点:

- 需要引入 Slint 的 `winit` 访问层
- 需要多一层 desktop adapter 来隔离平台依赖

#### 方案 B: 纯 Slint / Rust 手动拖拽

结构:

- `TouchArea` 记录按下位置
- Rust 侧在 `moved` 中不断调用 `Window.set_position(...)`
- 双击仍走 Slint Window API

优点:

- 不依赖底层 `winit` 访问接口
- 理论上更“框架中立”

缺点:

- DPI、多显示器、窗口吸附、最大化恢复前位置等边界更容易出问题
- Windows 11 手感通常不如原生拖拽
- 实现成本并不低, 只是把复杂性转移到了手动逻辑里

最终选择: `A. Slint 手势 + winit 原生拖拽`

选择原因:

- 符合首发平台 Windows 11 的交互优先级
- 能以最少逻辑换取最自然的拖拽体验
- 对后续无边框 resize / snap 能力预留更好延展点

### 3. 右上角窗口控制按钮

#### 方案 A: 完全自绘 caption controls

结构:

- `Minimize`
- `Maximize / Restore`
- `Close`

全部由 Slint 绘制, 点击后通过 Rust 窗口命令服务执行。

优点:

- 视觉一致性最好
- hover / pressed / inactive / danger 状态可完全纳入 Fluent 语言
- 不受系统标题栏外观约束
- 对未来跨平台统一抽象更友好

缺点:

- 所有交互态都要自己定义
- 测试面比系统 caption buttons 更大

#### 方案 B: 混合方案

结构:

- 顶栏主体自绘
- 三大金刚键尽量保留系统 caption buttons

优点:

- 系统按钮行为天然正确
- 部分系统状态不需自绘

缺点:

- 与当前 `no-frame: true` 路线不一致
- 很容易造成视觉割裂
- 跨平台抽象会更难统一

最终选择: `A. 完全自绘 caption controls`

选择原因:

- 这是最符合产品视觉目标的路线
- 与当前 frameless shell 一致
- 后续迁移到其他平台时, 只需替换窗口命令适配层, 无需推倒 UI 结构

### 4. 设置按钮下拉菜单与右侧侧栏联动

#### 方案 A: Slint `PopupWindow` + Rust ViewModel

结构:

- 设置按钮点击后弹出 `PopupWindow`
- 菜单项点击通过 callback 回 Rust
- `show_settings_menu` / `show_right_panel` / `window_active` 等状态统一由 Rust view model 驱动

优点:

- 视觉与动效统一
- 菜单圆角、阴影、分组、hover 状态都可纳入现有 token
- 与未来 `Command Palette`、状态菜单、更多全局入口共享弹层语言

缺点:

- 焦点管理、Esc 关闭、点击外部关闭需要设计清楚
- 需要补充基础键盘导航策略

#### 方案 B: 原生菜单

结构:

- 设置按钮调用平台原生菜单

优点:

- 一部分系统语义天然可用
- 某些无障碍和键盘行为开箱即用

缺点:

- 外观与当前 Fluent / Mica 壳层割裂
- 平台分支提前变多
- 不利于保持产品化一致体验

最终选择: `A. Slint PopupWindow + Rust ViewModel`

选择原因:

- 能保住产品视觉完整性
- 更符合当前技术栈
- 后续扩展到更多顶栏弹层时复用价值最高

## 最终决策

本轮顶部状态栏正式采用以下组合:

- `1B`: Windows-aware 五段式布局
- `2A`: Slint 手势 + winit 原生拖拽
- `3A`: 完全自绘 caption controls
- `4A`: Slint `PopupWindow` + Rust ViewModel

## 最终设计

### 布局结构

顶部状态栏维持 `48px` 高度, 采用五段式结构:

1. 左侧品牌区
2. 主拖拽区
3. 右侧全局动作区
4. 最小拖拽安全区
5. 窗口控制区

建议的视觉内容分布:

- 左侧品牌区: App icon、`Mica Term`、全局菜单按钮
- 主拖拽区: 空白为主, 可允许低信息密度状态文案或轻量连接态, 但不放高频交互控件
- 右侧动作区: 全局状态图标占位、右侧侧栏开关、设置按钮
- 最小拖拽安全区: 始终为空白, 保证窗口缩小时仍可拖动
- 窗口控制区: 最小化、最大化/还原、关闭

### 交互边界

- 拖拽仅发生在主拖拽区和最小拖拽安全区
- 左侧品牌按钮、设置按钮、右侧侧栏开关、窗口控制按钮均为明确交互区, 不可落入拖拽区
- 双击仅对拖拽区生效
- 设置菜单展开时:
  - 点击按钮可切换展开/收起
  - 点击外部区域关闭
  - `Esc` 关闭
- 右侧侧栏开关只控制 `RightPanel` 的可见性, 不影响标题栏结构

### 窗口行为

- `Minimize`: 调用窗口命令服务最小化窗口
- `Maximize / Restore`: 根据当前窗口状态切换
- `Close`: 关闭窗口
- `Double Click Drag Region`: 最大化 / 还原
- `Drag Start`: 通过底层 `winit` 原生拖拽发起

### 状态模型

建议把当前 `ShellViewModel` 扩展为“壳层状态”与“内容状态”两个层次。

本轮新增的壳层状态应至少包括:

- `show_right_panel`
- `show_settings_menu`
- `is_window_maximized`
- `is_window_active`

建议原则:

- 顶栏展示状态由 Rust 持有
- Slint 负责渲染和事件发出
- 窗口命令由单独的 `window command adapter` 执行

这样可以避免把平台行为直接塞进 Slint 组件, 也避免把 UI 细节回推到 `bootstrap.rs`。

### 分层建议

建议形成以下职责边界:

- `ui/shell/titlebar.slint`
  - 负责结构、视觉、手势入口、菜单弹层
- `src/shell/view_model.rs`
  - 负责顶栏可观察状态
- `src/app/windowing.rs`
  - 负责窗口命令抽象、平台窗口行为适配
- `src/app/bootstrap.rs`
  - 负责绑定 Slint callbacks 与 Rust 状态

这样后续若迁移到 macOS / Linux:

- `title bar` UI 结构大概率仍可复用
- 只需替换窗口命令适配层中的平台实现

## 实施步骤

### 阶段 1: 标题栏结构重构

- 将当前单一 `CommandEntry` 标题栏升级为五段式布局
- 为左侧按钮、右侧状态位、侧栏开关、窗口按钮预留稳定槽位
- 保证窄窗口下最小拖拽安全区仍存在

### 阶段 2: 顶栏状态模型接入

- 在 Rust 侧扩展壳层状态
- 将 `show_right_panel` 从静态默认值改为真正驱动 UI 的状态
- 增加设置菜单和窗口状态相关字段

### 阶段 3: 窗口命令适配层

- 在 `windowing` 层定义统一窗口命令边界
- 接入 `maximize / restore / minimize / close / drag`
- 用平台适配隔离 `winit` 细节

### 阶段 4: 设置菜单弹层

- 使用 `PopupWindow` 实现下拉菜单
- 固化点击外部关闭、Esc 关闭、二次点击切换行为
- 菜单项先以全局入口占位为主, 保留后续扩展

### 阶段 5: 顶栏与右侧面板联动

- 右侧切换按钮控制 `RightPanel`
- 保持面板展开/收起时标题栏布局稳定
- 避免按钮跳位和拖拽区塌陷

## 风险与回滚

### 风险 1: `winit` 访问层引入后平台边界变脆

表现:

- `drag_window()` 绑定方式侵入 UI 层
- 后续平台扩展时需要大量改动

控制策略:

- 只在 `windowing` 适配层持有 `winit` 细节
- Slint 只发“drag requested”之类的意图事件

回滚策略:

- 如访问层耦合失控, 可临时退回到纯 Slint 双击最大化 + 后续再评估拖拽实现

### 风险 2: 拖拽区与交互区命中冲突

表现:

- 设置按钮、侧栏按钮偶发触发拖拽
- 双击按钮区域错误触发最大化

控制策略:

- 强制把所有按钮从拖拽区结构中独立出来
- 保留单独最小拖拽安全区

回滚策略:

- 如命中边界复杂度过高, 可先缩小拖拽区到中间主区域, 暂时牺牲少量拖拽面积

### 风险 3: 自绘窗口按钮状态不够像系统

表现:

- hover / pressed / inactive 对比度不稳定
- `Close` 按钮危险态不够明确

控制策略:

- 单独定义 `Close` 的 hover / pressed danger 色
- 保持 `Minimize / Maximize` 中性, `Close` 使用系统感更强的危险色逻辑

回滚策略:

- 若首轮视觉验证不过关, 优先调整 token 和 hit area, 不改架构路线

### 风险 4: 设置菜单后续可扩展性不足

表现:

- 菜单一旦加分组或更多入口就需要重写

控制策略:

- 从一开始按“通用顶栏弹层”建模, 不按一次性临时菜单设计

回滚策略:

- 若 `PopupWindow` 限制超出预期, 仍可保留状态模型并替换弹层载体, 不影响顶栏主结构

## 验证清单

- [ ] 顶部状态栏保持 `48px` 高度并与现有 shell 风格一致
- [ ] 左侧包含图标 / 名称 / 全局菜单按钮
- [ ] 中间存在明确拖拽区, 且窄窗口下仍保留最小拖拽安全区
- [ ] 右侧包含状态图标占位、右侧侧栏开关、窗口控制按钮
- [ ] 拖拽区按下后窗口可正常拖动
- [ ] 双击拖拽区可最大化 / 还原
- [ ] `Minimize / Maximize / Close` 均工作正常
- [ ] 设置按钮点击后出现下拉菜单
- [ ] 设置菜单支持点击外部关闭
- [ ] 设置菜单支持 `Esc` 关闭
- [ ] 右侧侧栏按钮能正确切换 `RightPanel`
- [ ] 右侧面板切换不影响窗口按钮命中与拖拽区可用性
- [ ] 顶栏交互区不会误触发拖拽
- [ ] `Close` 按钮 hover / pressed 态在视觉上明显区别于其他按钮

## 参考资料

- Slint `Window`: https://docs.rs/slint/latest/slint/struct.Window
- Slint `TouchArea`: https://docs.slint.dev/latest/docs/slint/reference/gestures/toucharea/
- Slint `PopupWindow`: https://docs.slint.dev/latest/docs/slint/reference/window/popupwindow/
- Slint `winit_030` / `WinitWindowAccessor`: https://docs.rs/slint/latest/slint/winit_030/
- Windows Title bar customization: https://learn.microsoft.com/en-us/windows/apps/develop/title-bar
- Windows TitleBar control: https://learn.microsoft.com/en-us/windows/apps/develop/ui/controls/title-bar
- Windows Mica guidance: https://learn.microsoft.com/en-us/windows/apps/design/style/mica

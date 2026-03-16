# Assets Sidebar Toolbar Design

日期: 2026-03-16  
执行者: Codex  
状态: 已完成方案确认，待进入实现规划

## 背景

当前仓库已经完成 shell 外框、顶部状态栏、左侧 `Activity Bar + AssetsSidebar` 双层骨架，以及右侧面板的布局契约，但 `AssetsSidebar` 仍然停留在占位阶段。

现状可以从以下代码直接确认：

- 左侧仍是固定双层布局，`Activity Bar = 48px`，`Assets Sidebar = 256px`
- `AssetsSidebar` 目前只根据 `active-panel` 展示静态占位文本
- Rust 侧已经存在 `ShellViewModel -> bootstrap callback -> Slint property` 的状态驱动链路
- 主工作区仍是 `TabBar + WelcomeView` 占位，真实 terminal runtime 尚未接入
- 当前 renderer 已锁定为 `winit + femtovg-wgpu + wgpu-28`，本轮不涉及 renderer 变更

相关文件：

- `ui/shell/assets-sidebar.slint`
- `ui/shell/sidebar.slint`
- `ui/app-window.slint`
- `src/shell/view_model.rs`
- `src/app/bootstrap.rs`
- `src/shell/metrics.rs`

相关 Git 历史：

- `fcc313d feat: implement sidebar navigation shell`
- `ae48182 fix: repair sidebar shell layout contracts`
- `4f9fc9e feat: flatten shell chrome — Titlebar/RightPanel 收敛为 flat internal chrome`

这意味着当前任务不应被理解为“补几个按钮”，而应被理解为“为左侧资产区建立一个可持续扩展的 header / toolbar 交互层”，为后续真实 SSH Host、Folder、Session、SFTP 目录、Snippet Group 和 Keychain Group 的接入提前定好结构。

## 目标

- 为 `AssetsSidebar` 增加顶部 `Toolbar`
- 支持以下首轮能力：
  - 左侧标题文本 `资产列表`
  - 搜索按钮，点击后在 toolbar 下方展开搜索框
  - 搜索框在“无输入内容且点击空白处”时收起
  - “展开全部 / 收起全部” 按钮，状态不同图标不同
  - “平铺 / 树形” 按钮，状态不同图标不同
  - `Create` 按钮，下拉菜单包含 `新建文件夹`、`新建 SSH 连接`
- 保持 Fluent / Mica 工具型视觉语言一致
- 保持状态模型可扩展到未来真实资产树、过滤、持久化视图模式与上下文菜单

## 边界

### 本文档覆盖

- `AssetsSidebar` 顶部 toolbar 的组件边界
- toolbar 与 Rust view model 的状态归属
- 搜索框展开/收起规则
- 树形 / 平铺模式的模型语义
- 展开全部 / 收起全部的状态语义
- `Create` 下拉菜单的载体与交互
- 风险、回滚与验证要求

### 本文档不覆盖

- 真实 terminal widget 接入
- `wezterm-term` / `termwiz` / `russh` / `russh-sftp` runtime
- 真实资产数据源、数据库 schema、SFTP 浏览器
- 搜索算法优化
- 资产树虚拟滚动
- 创建文件夹 / SSH 连接的业务表单与提交逻辑

## 现状与约束

### 1. 结构约束

- 左侧结构已经是 `Activity Bar + AssetsSidebar` 的双层模型，不能退回单层 icon rail
- `AssetsSidebar` 宽度当前固定为 `256px`
- `AppWindow` 已经通过 `effective-show-assets-sidebar` 做了 layout 裁决，toolbar 设计不能破坏这一层级

### 2. 状态约束

- 现有壳层交互统一走 Rust `ShellViewModel`
- 顶栏、右侧面板、侧栏切换都已采用 `callback -> bootstrap -> set_* property` 模式
- 本轮若把业务状态塞回纯 Slint 本地属性，后续接入真实资产树时会再次回迁

### 3. 视觉约束

- 现有工具按钮统一为 `36px`
- 已存在可复用的自绘 `TitlebarIconButton`
- 已存在自绘 `PopupWindow` 菜单样式
- 当前 token 已定义 `ThemeTokens.shell-surface / command-tint / panel-tint / shell-stroke / text-primary`

### 4. 官方能力确认

基于 Slint 官方文档，以下能力已确认可用：

- `PopupWindow` 支持相对父元素定位与 `close-policy`
- `close-on-click-outside` 可用于菜单的点击外部关闭
- `TextInput` 支持 `focus()` / `clear-focus()`
- `FocusScope` 和键盘事件可用于后续补充 `Esc`、方向键与快捷键
- `ContextMenuArea` + `Menu` 也可用于菜单系统，但平台渲染一致性不如当前自绘 `PopupWindow`

参考：

- <https://docs.slint.dev/latest/docs/slint/reference/window/popupwindow/>
- <https://docs.slint.dev/latest/docs/slint/reference/window/contextmenuarea/>
- <https://docs.slint.dev/latest/docs/slint/reference/keyboard-input/textinput/>
- <https://docs.slint.dev/latest/docs/slint/guide/development/focus/>
- <https://docs.slint.dev/latest/docs/slint/reference/keyboard-input/overview/>

## 设计要点与方案对比

### 1. Toolbar 作用域

#### 方案 A: 只为 `console` 做专用 toolbar

优点：

- 首轮实现最直接
- 与当前需求表述完全一致
- 额外抽象最少

缺点：

- 后续 `snippets`、`keychain` 很可能再拆一轮 header
- 结构上会把 `AssetsSidebar` 继续留在“一个容器里塞多种 panel”的不稳定状态

#### 方案 B: 定义 `AssetsSidebar` 通用 header 骨架，首轮只启用 `console` 动作

优点：

- 与现有 `active-panel` 架构一致
- 后续可以为 `snippets`、`keychain` 复用标题区、操作区与搜索交互
- 不需要未来推翻 header 层

缺点：

- 比单一专用实现多一层抽象

最终选择：`方案 B`

### 2. 状态归属

#### 方案 A: toolbar 状态全部留在 Slint 本地属性

优点：

- 接线最少
- 初版看起来实现更快

缺点：

- 真实资产树、过滤、视图模式持久化接入时仍要回 Rust
- 状态测试与跨组件联动能力弱

#### 方案 B: 业务状态归 Rust，临时展示态归 Slint

业务状态包括：

- `search_query`
- `search_expanded`
- `asset_view_mode`
- `asset_tree_expansion_state`
- `create_menu_open`

临时展示态包括：

- hover
- pressed
- focus ring
- 展开/收起动画中的过渡帧

优点：

- 与现有 titlebar / sidebar / right panel 的模式一致
- 便于持久化和测试
- 后续接入真实数据源时不需要迁移状态边界

缺点：

- 首轮 wiring 比纯 Slint 稍多

最终选择：`方案 B`

### 3. 搜索框展开方式

#### 方案 A: `inline collapsible search row`

结构：

- toolbar 本体固定在顶部
- 点击搜索按钮后，在 toolbar 下方展开一行搜索输入区
- 搜索输入区属于 `AssetsSidebar` 内部布局，而不是浮层

优点：

- 最符合“点开之后底部出现搜索框”的描述
- 视觉更稳，更像列表 header 的一部分
- 不会引入额外 popup 层级

缺点：

- 需要自己处理点击空白收起的命中规则
- 需要自己维护展开高度动画

#### 方案 B: toolbar 下方锚定 `PopupWindow`

优点：

- 点击外部关闭更直接
- 弹层边界更清楚

缺点：

- 视觉上更像菜单或浮层，而不是列表自带搜索行
- 与“底部出现搜索框”的产品感知不完全一致

最终选择：`方案 A`

### 4. “展开全部 / 收起全部” 的状态语义

#### 方案 A: 由树的真实展开态驱动

规则：

- 只在 `tree` 模式下可用
- 若存在可展开但未展开节点，显示“展开全部”
- 若已全展开，显示“收起全部”
- `flat` 模式下按钮保留位置但置灰不可用

优点：

- 与用户可见状态一致
- 行为最符合桌面树控件的直觉
- 后续可扩展到部分展开态

缺点：

- 需要维护树的派生状态

#### 方案 B: 仅切换预设策略，不关心当前真实树态

优点：

- 初期实现可能更省事

缺点：

- 用户看到的树态和按钮文案可能脱钩
- 语义偏虚

最终选择：`方案 A`

### 5. “平铺 / 树形” 的模型语义

#### 方案 A: 一份 canonical tree model，两个投影视图

规则：

- `tree` 模式显示缩进和可展开节点
- `flat` 模式对同一份数据做平铺投影
- 搜索、排序、选中态都以同一 canonical 数据为源

优点：

- 为未来真实资产数据结构打好基础
- 搜索、过滤、批量展开不会分裂成两套逻辑
- 更适合 Host Group / Folder / Session / SFTP Browser

缺点：

- 建模上比简单 UI 切换更严谨

#### 方案 B: 仅做 UI 层“看起来像两种模式”

优点：

- 实现速度快

缺点：

- 一旦接入真实数据和过滤，语义容易崩
- 后续返工概率高

最终选择：`方案 A`

### 6. `Create` 下拉菜单载体

#### 方案 A: 自绘 `PopupWindow`

结构：

- `Create` 按钮点击后，打开锚定按钮左下或下方的自绘菜单
- 菜单项首轮仅两项：
  - `New Folder`
  - `New SSH Connection`

优点：

- 与当前 titlebar menu 风格统一
- 可完全遵循 Fluent / Mica 的自定义视觉
- 后续可补图标、快捷键、禁用态、二级菜单

缺点：

- 键盘导航与无障碍语义需要后续补完

#### 方案 B: `ContextMenuArea + Menu`

优点：

- 菜单语义更接近系统标准能力

缺点：

- 不一定与当前自绘壳层视觉完全一致
- 后续平台表现不够稳定可控

最终选择：`方案 A`

## 最终决策

最终采用以下组合：

- `D0B` Toolbar 定义为 `AssetsSidebar` 通用 header 骨架，但首轮只启用 `console` 动作
- `D1B` 业务状态归 Rust `ShellViewModel`，临时展示态归 Slint
- `D2A` 搜索框采用 toolbar 下方 `inline collapsible search row`
- `D3A` “展开全部 / 收起全部” 由真实树展开态驱动
- `D4A` 采用 canonical tree model + `tree / flat` 两个投影视图
- `D5A` `Create` 采用自绘 `PopupWindow` 菜单

## 最终设计

### 1. 组件结构

建议将 `AssetsSidebar` 内部结构重组为四层：

1. `AssetsSidebarHeader`
2. `AssetsSidebarSearchRow`
3. `AssetsSidebarListHost`
4. `AssetsSidebarEmptyOrPlaceholder`

首轮虽然仍可能显示占位内容，但结构上应提前分层：

- `Header` 负责标题、模式切换、创建与搜索入口
- `SearchRow` 负责输入框与搜索相关视觉反馈
- `ListHost` 负责未来真实资产树/平铺列表的宿主
- `EmptyOrPlaceholder` 继续承担当前占位内容

### 2. Header 布局

建议采用如下顺序：

- 左侧：`资产列表`
- 中间：弹性空白
- 右侧：
  - 搜索按钮
  - 展开全部 / 收起全部按钮
  - 平铺 / 树形按钮
  - `Create` 按钮

布局原则：

- 所有工具按钮保持同一尺寸语言
- 标题是 header 的主语义锚点，不参与点击
- 即使按钮在当前模式下不可用，也尽量保留槽位，避免 toolbar 跳位

### 3. 搜索交互

行为定义如下：

- 点击搜索按钮：
  - 若搜索框未展开，则展开并自动 focus
  - 若搜索框已展开且当前为空，则保持展开并 focus
  - 若搜索框已展开且已有输入，则不收起，只 focus
- 点击 toolbar 外部空白：
  - 若搜索框为空，则收起
  - 若搜索框非空，则保持展开
- 切换 panel 时：
  - 若离开 `console`，header 骨架仍存在，但 `console` 专属搜索行为不再激活
- 后续建议支持：
  - `Escape`：清空或收起
  - `Ctrl+F`：展开并 focus

### 4. 树形 / 平铺模式

建议定义：

- `AssetViewMode::Tree`
- `AssetViewMode::Flat`

`Tree` 模式：

- 显示层级关系
- 允许节点展开/收起
- 展开全部按钮可用

`Flat` 模式：

- 以同一 canonical tree model 生成平铺列表
- 去除层级展开交互
- 展开全部按钮置灰

这保证未来引入 `Folder -> SSH Host -> Recent Session`、`Snippet Group -> Snippet`、`Key Group -> Key` 时仍只有一套真实数据模型。

### 5. 展开全部 / 收起全部

建议定义一个派生状态：

- `tree_is_fully_expanded`

按钮逻辑：

- `Tree + 非全展开`：显示“展开全部”
- `Tree + 已全展开`：显示“收起全部”
- `Flat`：按钮保留但 disabled

这样按钮图标与语义始终反映用户当前能执行的动作，而不是静态切换器。

### 6. Create 菜单

建议：

- `Create` 作为文字按钮或带下拉箭头的文字按钮
- 使用自绘 `PopupWindow`
- 菜单项首轮固定：
  - `New Folder`
  - `New SSH Connection`

关闭规则：

- 点击按钮切换打开/关闭
- 点击菜单外关闭
- 点击菜单项后关闭
- 后续补充 `Escape` 关闭

不建议首轮采用 split button，原因是当前没有获批的默认主动作，强行指定会制造额外产品语义。

### 7. Rust 状态模型建议

虽然本文件不是 implementation plan，但为保证架构一致性，建议状态层最少预留以下概念：

- `asset_view_mode`
- `asset_search_query`
- `asset_search_expanded`
- `asset_create_menu_open`
- `asset_tree_is_fully_expanded`

其中：

- `query / mode / expansion` 属于业务状态
- hover、pressed、临时 focus ring 留在 Slint

### 8. 与现有架构的整合方式

- 继续复用 `AppWindow callback -> bootstrap -> ShellViewModel -> set_* property`
- 不引入新的全局状态总线
- 不改变 `effective-show-assets-sidebar` 的 layout 判定
- 不触碰 renderer 选择、window frame、titlebar chrome 与 right panel contract

## 实施步骤

### 阶段 1: Header 结构落位

- 在 `AssetsSidebar` 内加入 header / search row / list host 的层次骨架
- 保留当前 panel placeholder 内容，但迁移到 `ListHost` 或 placeholder 宿主中

### 阶段 2: 状态模型扩展

- 在 Rust 侧补齐 toolbar 所需状态
- 增加对应 callbacks 与 Slint property 绑定

### 阶段 3: 搜索交互

- 实现搜索按钮切换与下方搜索行展开
- 实现“空值点击外部收起，非空保持展开”
- 加入 focus 与后续键盘行为预留

### 阶段 4: 视图模式与树态语义

- 接入 `tree / flat` 状态
- 为“展开全部 / 收起全部”定义可验证派生规则

### 阶段 5: Create 菜单

- 复用当前自绘菜单语言落地 `PopupWindow`
- 打通两项菜单动作的回调出口

### 阶段 6: 测试与契约固化

- 增加 UI contract smoke
- 增加 Rust 状态单测
- 增加搜索与菜单关闭行为的 smoke / interaction 测试

## 风险与回滚

### 风险 1: 过早把 toolbar 写死为 `console` 专用

表现：

- `snippets`、`keychain` 后续再做 header 时结构冲突

控制策略：

- 保持 header 是 `AssetsSidebar` 的通用骨架
- 首轮只让 `console` 启用动作

回滚策略：

- 若首轮实现复杂度超出预期，可暂时只启用 `console` 的 header 内容，但不要删除通用 header 容器

### 风险 2: 搜索框点击空白收起逻辑误伤内部交互

表现：

- 点列表、点菜单、点输入框边缘时被错误判定为外部点击

控制策略：

- 明确“外部区域”只包括 toolbar/search/list 之外的 sidebar 空白
- `Create` 菜单打开时，点击菜单内部不触发搜索收起

回滚策略：

- 若首轮命中规则复杂，可先收窄为“点 search button 再次切换 + Esc 收起”，再补完整 click-away

### 风险 3: `flat` 与 `tree` 早期仅有占位数据，状态看似多余

表现：

- 初期看起来像“为了未来而设计”

控制策略：

- 只保留真正必要的状态位
- 不提前引入完整资产树数据结构实现

回滚策略：

- 若首轮验证表明状态过多，可保留 `view_mode` 和 `search_query`，将 `tree_is_fully_expanded` 延后到真实树接入时再实装

### 风险 4: `Create` 菜单若改用原生菜单会破坏当前壳层风格

表现：

- 视觉语言与 titlebar menu 不一致

控制策略：

- 首轮统一使用自绘 `PopupWindow`

回滚策略：

- 若 `PopupWindow` 在特定平台出现明确缺陷，再局部退回菜单载体，不改动业务状态模型

## 验证清单

- `AssetsSidebar` 在结构上存在独立 header、search row 与 list host 层次
- 标题 `资产列表` 固定在 header 左侧
- 搜索按钮点击后，搜索框出现在 toolbar 下方，而不是浮层
- 搜索框为空时点击外部区域会收起
- 搜索框非空时点击外部区域不会强制收起
- `Tree` 模式下存在“展开全部 / 收起全部”互斥语义
- `Flat` 模式下“展开全部 / 收起全部”按钮 disabled
- 平铺 / 树形切换按钮存在明确状态差异
- `Create` 菜单为自绘菜单，而非平台原生菜单
- `Create` 菜单包含且仅包含：
  - `New Folder`
  - `New SSH Connection`
- toolbar 交互不破坏现有 `Sidebar` 宽度契约和 `effective-show-assets-sidebar` 布局逻辑
- toolbar 交互不影响 titlebar、right panel、window frame 与 renderer 相关行为

## 参考

源码参考：

- `ui/shell/assets-sidebar.slint`
- `ui/shell/sidebar.slint`
- `ui/components/titlebar-icon-button.slint`
- `ui/components/titlebar-menu.slint`
- `ui/shell/titlebar.slint`
- `src/shell/view_model.rs`
- `src/app/bootstrap.rs`
- `src/shell/metrics.rs`
- `src/main.rs`

外部参考：

- Slint `PopupWindow`: <https://docs.slint.dev/latest/docs/slint/reference/window/popupwindow/>
- Slint `ContextMenuArea`: <https://docs.slint.dev/latest/docs/slint/reference/window/contextmenuarea/>
- Slint `TextInput`: <https://docs.slint.dev/latest/docs/slint/reference/keyboard-input/textinput/>
- Slint Focus Handling: <https://docs.slint.dev/latest/docs/slint/guide/development/focus/>
- Slint Key Handling Overview: <https://docs.slint.dev/latest/docs/slint/reference/keyboard-input/overview/>

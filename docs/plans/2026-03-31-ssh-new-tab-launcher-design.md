# SSH New Tab Launcher Design

日期: 2026-03-31
执行者: Codex
状态: 方案已确认，待进入实现

## 背景

当前快速启动页已经有一套 `Quick Launch Dashboard`，但信息密度和交互路径都偏重：

- 首页同时展示 `Recent Connections / Favorites / Groups / Group Focus / Connection Detail`
- 顶部还有搜索框
- 最近连接需要先选中，再去右侧操作区点击连接
- 当前工作区没有一个明确的、像浏览器那样的 tab 尾部新建入口

这和用户目标冲突。用户想要的是：

- 快速启动页更简单、更像成熟 SSH 客户端的新建标签入口
- 只保留最近连接作为主入口
- 去掉右侧 `Connection Detail`
- 最近连接做成更轻的卡片式
- 点击卡片就直接连接，不再走“选中 -> 详情区 -> Connect”
- tab 区域末尾增加 Fluent 风格的新建标签图标按钮
- 点击新建按钮后，应出现一个更像 `Xshell` 的启动入口，可以快速打开已有 SSH 连接

## 外部参考

本设计参考了 2026-03-31 当日可访问的官方资料与产品行为：

- `Xshell 8 Manual`
  - 会话列表支持选中后直接打开，整体心智模型是“session launcher -> 新 tab 进入真实会话”
  - 链接: <https://www.netsarang.com/docs/Xshell8_manual.pdf>
- `Windows Terminal`
  - tab 尾部使用新建标签按钮，`+` 是一级入口，附加入口通过旁侧菜单承载
  - 链接: <https://learn.microsoft.com/is-is/windows/terminal/install>
- `iTerm2 Profiles Window`
  - profile launcher 独立于 terminal session，自身是启动入口而不是长期保留的空白 tab
  - 链接: <https://iterm2.com/documentation-one-page.html>
- `Termius Desktop Changelog`
  - 2025-02-07 引入 `New Tab`
  - 2025-02-24 调整为打开后可直接搜索主机，说明其也把新标签入口当成 launcher surface
  - 链接: <https://termius.com/changelog/desktop-changelog>

结论：业界主流做法并不是长期保留一个空白启动页 tab，而是把它当作一次性的 launcher。连接建立后，当前 tab 进入真实 session，或者直接由真实 session 接管。

## 目标

### 本轮目标

- 将 welcome / quick launch 页面收敛为极简的 `New Tab Launcher`
- 首页只保留 `Recent Connections`
- 去掉搜索框、收藏、分组、右侧详情 pane
- 最近连接使用更轻的卡片布局，单击即连接
- 在 tab strip 尾部新增 Fluent 风格的新建标签按钮
- 点击该按钮后打开 launcher tab
- launcher tab 中提供一个 `Open Saved SSH Connections` 入口
- 该入口打开 SSH 资产选择 modal，使用树形列表展示已有 SSH 连接
- modal 中支持双击打开 SSH 连接
- 从 launcher 发起连接后，当前 launcher tab 直接被真实 SSH session 替换

### 体验目标

- 气质更接近 `Xshell / Termius / Windows Terminal` 的新标签入口，而不是信息看板
- 视觉上保持现有项目的 flat / square / Fluent-ish 方向
- `New Tab` surface 不做大而全，不承担资产管理职责
- 用户从打开 launcher 到进入真实 SSH session 的路径控制在两步以内

## 非目标

本轮不包含：

- 资产编辑器改造
- 收藏 / 分组 / 搜索逻辑的彻底删除或数据层移除
- 终端 session 生命周期模型重写
- 通过 `+` 直接弹出复杂多级菜单
- workspace 多窗口或多 pane 新模型

说明：

- 现有 quick-launch 相关的 projection / preference 数据可先保留，只在 UI 与交互层收敛
- 如果后续重新引入 favorites 或 search，应通过新的 launcher 设计再评估，而不是保留当前 dashboard 结构

## 现状分析

### 1. Welcome 页过重

当前 `ui/welcome/welcome-view.slint` 同时承载：

- `Recent Connections`
- `Favorites`
- `Groups`
- `Group Focus`
- `QuickLaunchDetailPane`
- 顶部搜索框

这导致首页视觉重心被切散，也让“快速连接”变成一个需要先选择再确认的多步流程。

### 2. 卡片交互不符合快速连接预期

当前 `QuickLaunchCard` 的主交互是：

- 单击: 选中
- 双击: 激活连接

这更像列表浏览器，不像启动器。对 launcher 来说，默认动作应当是单击即连接。

### 3. Tab strip 缺少显式新建入口

当前 `ui/shell/tabbar.slint` 仅渲染 session tabs，没有尾部新建标签按钮。  
虽然 terminal host 内部已有 `new-tab` local action，但它不是可见主入口，也不符合桌面终端对 tab strip 的常规预期。

### 4. Workspace 的 welcome 语义可承接 launcher

当前 `ShellViewModel` 在 `workspace_tabs.is_empty()` 时展示 welcome。  
这说明 welcome surface 已经天然承担“无活动 session 时的工作区入口”角色，可以演进为 launcher tab，而不需要引入第二套 workspace surface。

## 方案比较

### 方案 A：只精简 welcome，不增加 tab 尾部新建按钮

内容：

- welcome 仅保留 recent cards
- 去掉 detail pane 和其他 section
- 继续只在“无 tab”时显示

优点：

- 改动最小

缺点：

- 没有解决“用户需要显式新建 tab 入口”的问题
- 不能承接 `Xshell` 式 `+ -> launcher -> connect` 路径

结论：

- 不采用

### 方案 B：`+` 直接打开 SSH 资产树 modal，不做 launcher tab

内容：

- tab strip 增加 Fluent 新建按钮
- 点击后直接弹出树形 SSH 列表 modal
- 双击列表项直接连接

优点：

- 交互直接
- UI 改动较小

缺点：

- 最近连接卡片没有合适承载面
- 不够像 `Termius` 的 `New Tab` surface
- 无法同时满足“极简 recent 首页”和“打开现有 SSH 列表”两种入口

结论：

- 不采用

### 方案 C：tab strip 增加 Fluent 新建按钮，打开 launcher tab，再从 launcher 打开 recent 或树形 modal

内容：

- tab strip 尾部放置 Fluent 新建标签按钮
- 点击后创建一个 launcher tab
- launcher tab 只显示 `Recent Connections` 卡片区和一个 `Open Saved SSH Connections` 按钮
- 点击 recent card 直接连接
- 点击按钮打开树形 SSH 资产 modal
- modal 中双击连接
- 一旦发起连接，当前 launcher tab 被真实 SSH session 替换

优点：

- 最贴近 `Xshell + Termius + Windows Terminal` 的混合体验
- 快速路径和完整路径都存在
- 首页结构极简，视觉更干净
- 明确区分“启动入口”和“真实会话”

缺点：

- 需要在 tab/session 模型里区分 launcher tab 与真实 session tab
- 需要新增一个 SSH picker modal

结论：

- 采用方案 C

## 最终设计

## 设计要点 1：New Tab 入口

- 在 `ui/shell/tabbar.slint` 的所有 session tabs 之后、填充 spacer 之前增加一个独立按钮
- 按钮必须使用 Fluent 图标，不使用裸文本 `+`
- 按钮风格与现有标题栏 / tab strip 图标系统保持一致：
  - square hit target
  - hover / pressed surface 与现有 token 对齐
  - 不引入圆角 pill
- 按钮语义是 `New Tab`，不是 `Connect`

交互：

- 单击按钮 -> 创建并激活一个 launcher tab
- 如果当前已经有 launcher tab，则直接激活该 launcher tab，不重复堆积多个空 launcher

## 设计要点 2：Launcher Tab 内容

launcher tab 是一个轻量启动页，不是完整 dashboard。

页面结构：

- 页面标题：`New Tab`
- 次级说明：一句短文案，例如 `Open a recent connection or browse saved SSH targets`
- `Recent Connections` 卡片区
- `Open Saved SSH Connections` 主按钮

明确删除：

- 顶部搜索框
- `Favorites`
- `Groups`
- `Group Focus`
- `Connection Detail`
- `Connect / Connect in New Tab / Reveal in Assets` 详情动作区

最近连接卡片规则：

- 保留 title / subtitle / 少量 meta
- 视觉更轻，允许采用 2~4 列自适应卡片网格
- 单击卡片直接连接
- 不再存在“选中态”
- 不再要求双击才生效

空状态：

- 如果没有最近连接，显示简洁空态文案
- 同时保留 `Open Saved SSH Connections` 按钮作为主入口

## 设计要点 3：Open Saved SSH Connections Modal

modal 定位参考 `Xshell session launcher` 与 `iTerm2 Profiles Window`：

- 独立 modal
- 主体是 SSH 资产树
- 只展示 saved SSH assets
- 支持展开 / 折叠文件夹
- 支持搜索过滤
- 支持键盘上下移动与 Enter 打开
- 支持鼠标双击打开

modal 结构：

- 标题：`Open Saved SSH Connections`
- 顶部搜索输入框
- 左侧或主体树形区域
- footer 仅保留 `Cancel`

交互：

- 双击连接项 -> 在当前 launcher tab 中打开该 SSH session
- Enter -> 同上
- 单击仅选中，不连接

本 modal 不承担：

- 编辑资产
- 收藏管理
- 详情预览侧栏

## 设计要点 4：Launcher Tab 替换语义

采用 `Xshell` 风格：

- launcher tab 是一次性启动入口
- 从 launcher 发起连接后，当前 launcher tab 被真实 SSH session 替换
- 不额外保留一个空 launcher tab

理由：

- 更符合桌面 SSH 客户端预期
- 避免产生无意义空 tab
- 用户完成连接后，tab strip 中只保留真实工作会话

边界规则：

- 如果连接失败，launcher tab 进入连接错误视图或被错误 session 接管，但不额外再生一个 tab
- 如果用户取消 modal，launcher tab 保持原样
- 如果用户关闭 launcher tab，不影响已有真实 session

## 设计要点 5：与现有数据和状态的关系

为了控制改动范围，本轮不立即删除现有 quick-launch projection 数据。

保留：

- recent preference 数据
- quick launch records 的投影逻辑
- 当前 SSH 资产树数据源

调整：

- UI 只消费 `recent-items`
- selected detail、favorite、group projections 可继续存在，但 launcher surface 不再使用
- `connect-requested` 与 `connect-in-new-tab-requested` contract 收敛为单一路径：
  - launcher 只需要“在当前 launcher tab 打开连接”

## 设计要点 6：标签栏行为

tab strip 最终包含两类可见元素：

- 真实 session tabs
- 尾部 `New Tab` Fluent 图标按钮

不在本轮引入：

- `+` 旁边的下拉菜单
- 新建终端 / 新建 snippet / 新建其他 profile 的多入口菜单

键盘语义：

- 现有 `Ctrl+Shift+T` 暂不强制改成打开 launcher
- 本轮先新增可见入口，不主动破坏现有快捷键契约

## 风险与约束

### 1. 当前 workspace tab 数据结构可能默认只容纳真实 session

如果 `workspace_tabs` 当前完全以真实 session 为中心建模，需要补一层 launcher projection，或者引入一个轻量 pseudo-session。  
实现上必须避免让 launcher 污染真实 SSH runtime 管理器。

### 2. 现有 quick-launch 测试契约会失效

已有 smoke/spec 多数默认 welcome dashboard 包含：

- QuickLaunchDetailPane
- favorite / group / reveal 相关 contract

这些测试需要同步收敛，否则会形成“旧 dashboard 契约绑架新 launcher”的问题。

### 3. 单击即连接需要避免误触

recent cards 改为单击即连接后，卡片的 hover、pressed、focus 状态必须清晰。  
同时要避免在 launcher 中保留会让人误认为可先选中的视觉样式。

## 验证标准

- tab strip 尾部出现 Fluent 风格新建标签按钮
- 点击该按钮后可进入 launcher tab
- launcher tab 不再显示 favorites、groups、detail pane、搜索框
- launcher tab 最近连接卡片单击即可连接
- launcher tab 中存在 `Open Saved SSH Connections` 按钮
- 点击按钮后弹出树形 SSH picker modal
- modal 中双击某个 SSH 连接后，当前 launcher tab 被真实 SSH session 替换
- 关闭 modal 不产生额外 tab
- launcher tab 不会无限制重复创建
- 现有真实 SSH session tab 的切换与关闭不回归

## 实施建议

实现顺序建议如下：

1. 先定义 launcher tab 的状态与投影契约
2. 再把 tab strip 尾部 Fluent 新建按钮接上
3. 收敛 welcome UI 为 launcher
4. 新增 SSH picker modal
5. 最后把 launcher -> open session -> replace current tab 语义接通

这样可以先稳定 tab/navigation contract，再处理 modal 和连接替换语义，降低回归面。

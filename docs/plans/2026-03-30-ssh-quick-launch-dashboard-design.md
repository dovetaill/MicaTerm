# SSH Quick Launch Dashboard Design

日期: 2026-03-30
执行者: Codex
状态: 方案已确认，待进入 implementation plan

## 背景

当前 `mica-term` 在没有打开 SSH 标签页时，会把工作区保持在 `welcome` 模式，但现有
`ui/welcome/welcome-view.slint` 只有最小欢迎文案，无法承接 SSH-first 产品的核心场景：

- 快速重开最近使用的 SSH 连接
- 直达常用连接或高频分组
- 在不展开左侧资产栏的前提下理解连接上下文
- 从空白工作区直接进入生产级远程操作

用户明确要求：

- 终端区域默认打开不能是一片空白
- 要做成类似 Xshell 那种快捷启动窗口
- 以 SSH 连接为主，不做临时连接输入
- 样式、图标、信息层级都要美观、大方、方便、耐用

## 外部调研结论

本轮通过 `exa` 和 `tavily` 对成熟 SSH / terminal 产品进行了定向调研，结论一致：

### 共同模式

- 空工作区不会只放欢迎文案，而是会承接“最近连接 / 已保存会话 / 搜索 / 一键打开”
- 会话管理会和标签页模型打通，避免“管理页”和“终端页”割裂
- 视觉上偏桌面工具，不是网页登录页；重点在信息层级、密度和状态可读性
- 图标与分组会被用来压缩认知成本，但不会把所有元数据塞进卡片

### 参考产品

- Xshell
  - 强调 `Session Manager`、`Quick Start`、`Quick Command Manager`、`Tab Manager`
  - 支持自定义会话图标和快速启动
  - 来源: <https://www.netsarang.com/en/xshell/>
  - 来源: <https://www.netsarang.com/en/xshell-all-features/>
- SecureCRT
  - `Connect Dialog` 以已保存会话树、查找、在当前窗口开 tab 为核心
  - 来源: <https://www.vandyke.com/support/tips/connectdialog.html>
- MobaXterm
  - 会话自动保存，主界面围绕 session manager 和 tabbed environment 展开
  - 来源: <https://mobaxterm.mobatek.net/documentation.html>
- Termius
  - 最近版本持续加强 `New Tab`、`recent connections`、`workspaces`、状态恢复
  - 来源: <https://termius.com/changelog/desktop-changelog>
- Warp
  - 虽然不是 SSH-first，但 `Launch Configurations` 证明“启动面 = 快速恢复高频工作集”
  - 来源: <https://docs.warp.dev/features/sessions/launch-configurations>

## 目标

### 本轮目标

- 将 `welcome` 空态升级为 SSH-first 的快捷启动台
- 让工作区空态直接展示最近连接、收藏连接、连接分组和当前选中详情
- 保持“只从已保存 SSH 连接启动”，不支持首页临时连接输入
- 与现有 `SessionManager`、workspace tabs、资产树结构对齐
- 保持桌面工具质感，不做网页式欢迎页

### 体验目标

- 第一眼就能理解“这里是连接启动台”，而不是占位页
- 高频连接应该在一次点击或双击内进入 SSH tab
- 连接卡片视觉清晰，但信息不过载
- 首页在连接数量增长后依然整洁，不变成资产树的重复副本
- 卡片、详情、分组之间要有明确主次，不靠炫技效果

## 非目标 / 边界

本轮不包含：

- 临时 SSH 连接输入框
- 最近断开会话的跨重启恢复
- 团队共享收藏或 vault 级共享星标
- SFTP 启动页
- workspace 模板、pane 布局恢复
- 会话健康检查、在线状态探测、远端 OS 实时识别

本轮允许使用的现有元数据：

- 资产标题 `title`
- SSH 连接 `host / user / port`
- `environment`
- `remark`
- proxy / auth 摘要

## 当前实现现状

### 1. Welcome 仍是最小文案页

当前 `WelcomeView` 仅包含：

- 标题 `Welcome to Mica Term`
- 副标题 `Command-first SSH and SFTP workspace`

这不足以支撑 SSH-first 启动场景。

相关文件：

- `ui/welcome/welcome-view.slint`

### 2. 工作区 welcome / terminal / progress 切换已经稳定

当前 `WorkspacePane` 通过 `TerminalSessionHost` 的 `mode` 切换：

- `welcome`
- `terminal`
- `connection-progress`
- `session-error`

这意味着首页改造不需要推翻 workspace host 模型，只需要把 `welcome` 的内容升级。

相关文件：

- `ui/shell/workspace-pane.slint`
- `ui/shell/terminal-session-host.slint`
- `src/shell/view_model.rs`
- `src/app/bootstrap.rs`

### 3. 资产模型里没有 recent / favorites 的持久化语义

当前持久化资产目录只包含：

- root ids
- asset nodes
- SSH 连接 payload
- snippet payload

SSH 连接 payload 有：

- `environment`
- `remark`
- auth / proxy / credential ref

但没有：

- `last_used_at`
- `favorite`
- `pin`
- `usage_count`

这意味着最近连接与收藏不能直接塞进当前 asset catalog 主 schema。

相关文件：

- `src/app/assets_catalog/model.rs`
- `src/app/assets_catalog/redb_store.rs`

### 4. 现有 UI preferences 只适合轻量窗口配置

当前 `UiPreferences` 仅保存：

- theme mode
- always on top
- right panel view

不适合继续塞复杂 quick launch 数据，因此应该新增独立的 quick launch 偏好存储。

相关文件：

- `src/app/ui_preferences.rs`

## 方案选择

### 方案 A：Classic Session Manager

做成传统左树右列表的会话管理器。

优点：

- 学习成本最低
- 最接近 Xshell / SecureCRT 老牌 SSH 客户端

缺点：

- 工作区会更像“会话管理窗口”，不够一体
- 和现有 `welcome` host 的融合感较弱

### 方案 B：Quick Launch Dashboard

主工作区直接变成 SSH 快捷启动台：最近连接大卡片、收藏连接、分组入口、右侧详情。

优点：

- 兼顾美观与效率
- 与现有 workspace content host 更贴合
- 最符合“空白区变快捷启动窗”的要求

缺点：

- 需要引入一层本地 quick launch 偏好模型
- 需要新增少量 welcome 专属交互状态

### 方案 C：Dense Ops Board

以高密度表格 / 行项目为主，强调环境、备注、跳板链等运维信息。

优点：

- 信息密度最高
- 适合重度运维用户

缺点：

- 首页气质偏硬，不够大方
- 第一屏观感容易压抑

### 结论

采用方案 B：`Quick Launch Dashboard`。

原因：

- 最适合当前 `mica-term` 的 SSH-first 工作区定位
- 能复用现有 `welcome` host 模式而不是强拆 workspace
- 既能靠近 Xshell 的 quick start，又能吸收 Termius / Warp 的现代启动体验

## 目标布局

首页划分为四个功能区：

### 1. 顶部标题区

- 标题：`Quick Start`
- 副标题：简短说明“Open saved SSH connections fast”
- 右侧保留全局搜索入口，只搜索已保存 SSH 连接

### 2. 最近连接区

- 位于首屏最显眼位置
- 展示 6 到 8 个大卡片
- 按最近使用倒序排列
- 作为默认键盘焦点列表

### 3. 收藏连接 / 连接分组区

- 收藏连接：中密度卡片或行项目
- 连接分组：从 console asset tree 的 folder roots 或常用 folder 投影而来
- 点击分组后在 welcome 内联显示该分组下的 SSH 连接

### 4. 详情侧栏

- 展示当前选中连接的摘要
- 包含：
  - 名称
  - `user@host`
  - port
  - environment
  - auth 摘要
  - proxy 摘要
  - remark
  - 最近连接时间
- 主动作：
  - `Connect`
  - `Connect in New Tab`
  - `Edit`
  - `Reveal in Assets`

## 交互设计

### 卡片交互

- 单击：选中并刷新详情侧栏
- 双击：打开 SSH 连接
- `Enter`：打开选中连接
- `Ctrl/Cmd+Enter`：以 `ForceNewTab` 打开新标签页
- 右键：打开卡片上下文菜单

### 搜索交互

- 搜索范围只包含已保存 SSH asset
- 匹配：
  - 连接名
  - host
  - user
  - environment
  - remark
- 搜索结果不跳出到资产页，而是在 welcome 页面内联过滤

### 与资产栏的关系

- quick launch 不是左侧资产栏的替代品
- `Reveal in Assets` 会同步选中左侧对应 asset
- `Open Assets Sidebar` 仍保留为次级动作

### 与 session manager 的关系

- 首页启动动作最终仍走现有 `runtime_profile_for_saved_asset(...)` 与 `open_session(...)`
- 默认主动作使用当前产品现有语义
  - `Connect`: `OpenSessionMode::ActivateExisting`
  - `Connect in New Tab`: `OpenSessionMode::ForceNewTab`

## 视觉样式

### 总体方向

- 保持桌面 SSH 控制台气质
- 延续当前 `ThemeTokens` 的深灰蓝层次
- 不做营销页式 hero，不做强圆角卡片墙

### 表面层次

- 启动面底色基于 `workspace-surface`
- 区块卡片使用接近 `assets-surface / inspector-surface / terminal-canvas-surface` 的分层
- 选中态使用 `accent` 和更强的边界线表达

### 信息密度

卡片严格只显示三层信息：

- 第一行：连接名
- 第二行：`user@host`
- 第三行：环境 / 端口 / 最近使用时间中的压缩组合

其余信息进入详情侧栏。

### 动效

- 仅使用轻量 hover、focus、selected 过渡
- 避免弹跳、缩放等网页式动效
- 目标是“耐用”，不是“惊艳一次”

## 图标设计

### 原则

- 继续使用现有 Fluent 图标体系
- 不引入额外风格库
- 允许增加少量同体系 SVG 资源

### 图标映射

- 默认 SSH：`window-console`
- `bastion / jump / gateway`：更偏导航或防护语义的图标
- `database / db / mysql / postgres / redis`：数据库语义图标
- 普通 folder 分组：沿用 `folder`

### 色彩策略

- 环境标签而不是主图标承担颜色语义
- `prod` 使用更高风险色条
- `staging / test / dev` 使用冷静副色
- 图标主体仍保持整体一致性，不做品牌 logo 墙

## 数据模型设计

### 新增本地 quick launch 偏好存储

新增独立于 `UiPreferences` 的本地 JSON 偏好文件，建议命名为：

- `quick-launch-preferences.json`

字段：

- `favorite_asset_ids: Vec<String>`
- `recent_asset_ids: Vec<String>`
- `last_selected_asset_id: Option<String>`

### recent 规则

- 每次通过已保存 SSH asset 发起打开动作时更新
- 去重
- 仅保留最近 12 到 16 条
- asset 删除后自动清理失效 id

### favorites 规则

- 本地偏好，不写入共享资产目录 schema
- 不伪装为真实 folder
- asset 删除后自动清理失效 id

### 分组来源

- 直接来自现有 console asset tree 的 folder
- 首版只处理 console domain，不纳入 snippets / keychain

## 架构落点

### UI 层

- `TerminalSessionHost` 继续决定 `welcome`/`terminal`/`progress`
- `WelcomeView` 升级为完整 quick launch 页面
- 推荐拆出 welcome 子组件：
  - `ui/welcome/quick-launch-card.slint`
  - `ui/welcome/quick-launch-section.slint`
  - `ui/welcome/quick-launch-detail-pane.slint`

### 状态层

`ShellViewModel` 新增 welcome 专属状态：

- 当前选中的 quick launch asset id
- 搜索 query
- quick launch 过滤结果
- favorite / recent 投影结果

### bootstrap 层

`bootstrap` 负责：

- 将 quick launch 投影同步到 `AppWindow` 属性
- 响应 welcome 卡片动作 callback
- 将 `Connect` / `Connect in New Tab` 路由到现有 SSH session open path
- 在会话打开成功发起前更新 recent 列表

## 测试策略

### 单元测试

- quick launch 偏好存储 roundtrip
- favorite / recent 去重与失效 id 清理
- welcome 投影顺序与过滤逻辑
- view model 中 welcome 选中态与 fallback 行为

### 集成测试

- 从 recent 卡片打开 saved SSH asset
- 从 favorite 卡片打开 saved SSH asset
- `Connect in New Tab` 走 `ForceNewTab`
- `Reveal in Assets` 同步左侧选中
- 搜索结果为空时的空状态展示

### UI contract smoke

- `WelcomeView` 应包含 quick start 结构而不再只是两行文案
- `WorkspacePane` 仍通过 `TerminalSessionHost` 切换 host mode
- quick launch 卡片区、收藏区、详情区的关键结构存在

## 风险与约束

- 如果直接把 recent / favorites 塞进 asset catalog，会污染共享目录模型
- 如果 welcome 页面承载过多字段，会迅速退化为第二个资产栏
- 如果默认点击就强制 `ForceNewTab`，会偏离当前产品既有 reopen 语义
- 如果图标和颜色语义过多，会破坏当前简洁的桌面视觉体系

## 最终决策

本轮设计确认如下：

- 采用 `Quick Launch Dashboard`
- 只服务已保存 SSH 连接，不支持首页临时连接
- welcome 页面展示最近连接、收藏连接、连接分组、详情侧栏
- recent / favorites 使用独立本地偏好存储
- 启动动作复用现有 SSH open-session 链路
- 视觉风格沿用当前深色桌面工具基底，增强层次与图标语义，但保持克制

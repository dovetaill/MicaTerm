# SSH New Tab Productized Design

日期: 2026-05-06
执行者: Codex
状态: 已获用户确认，进入实现

## 目标

将 MCA TERM 右侧 New Tab 主内容区从占位式 welcome/dashboard 改为产品级 SSH launcher surface。只改右侧 New Tab 主内容区域，不改左侧 Assets、sidebar、标题栏、窗口控制或外层软件壳。

## 调研结论

- Termius 把新建/首页心智放在 Hosts、Vaults、Recent connections、Search hosts or tabs 与 workspace restore 上，强调 one-click reconnect。
- Tabby 通过 Profiles & Connections selector 和 recentProfiles 权重让最近 profile 更容易打开。
- Apple Terminal / Windows Terminal 的 New Tab 主要围绕 profile/default session launcher，不承担完整管理面。

结论：terminal / SSH 工具的 New Tab 应服务快速恢复最近连接和进入 saved SSH selector，不应做后台 dashboard、营销 hero 或重复 saved hosts 资产树。

## 设计方案

### 页面结构

- 顶部轻量 intro header：`New Tab` 标题、短文案、`Open Saved SSH` 入口。
- 右上低对比 terminal / connection motif：只减少单调感，不抢 Recent 主视觉。
- 主体 Recent Connections：最多 7 条 modern list rows / list-cards。

### Open Saved SSH

- 入口位置：标题文案下方左对齐。
- 文案：`Open Saved SSH`。
- 行为：打开现有 `OpenSavedSshModal`，继续使用资产树浏览/搜索/选择 saved SSH。
- 不在右侧重复 saved hosts 区块，因为左侧 Assets 和 modal 已是完整 browse surface。

### Recent Connections

每条 recent row 包含：

- 连接名 / host display name。
- secondary info：`user@host` 或 `user@ip`。
- 可选轻量 tertiary info：端口、备注、jump host 等真实配置来源。
- 右侧真实 time label：`2m ago`、`18m ago`、`Yesterday` 等。
- 轻 chevron / open affordance。

不得显示：Environment / Status / Favorite 列，或系统中不存在的 Debian12 / AWS / Prod / PostgreSQL 等标签。

### 图标素材策略

采用重绘生成方案：以 `dist/tmp/5-3.png` / `dist/tmp/5-4.png` 的视觉语言为基准，重绘为项目内可使用的 SVG 或 Slint 矢量组件。优先保持 row server icon、open terminal icon、chevron、terminal motif 的线条风格一致；不直接裁脏 PNG。

## 工程边界

- 保留现有 quick launch / saved SSH modal 的功能链。
- 扩展 recent 数据结构记录真实打开时间，兼容旧 `recent_asset_ids`。
- `QUICK_LAUNCH_RECENT_LIMIT` 调整为 7。
- 点击 recent row 继续调用现有连接流程；缺失配置由现有失败 tab / profile resolution 处理链路承接。

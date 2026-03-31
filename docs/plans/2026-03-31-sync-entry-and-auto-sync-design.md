# Sync Entry And Auto Sync Design

日期: 2026-03-31
执行者: Codex
状态: 已实现并合并到 `master`

## 背景

当前同步交互存在四个直接问题：

- 顶部标题栏把 `Sync` 做成了 “open modal” 入口，而不是同步动作本身。
- `Set up sync` 没有进入可编辑配置流程，当前实现只会重复报错。
- 同步 modal 信息架构混乱，把“同步”“配置”“解锁”“错误”都堆在同一个静态窗里。
- modal 视觉层级差，错误条会把按钮区挤出底部，且拖拽反馈不稳定。

用户目标很明确：

- 顶部状态栏只负责同步，不承担配置。
- 配置入口下沉到左上角菜单。
- 同步应以自动同步为主，手动同步为辅。
- 用户不应该频繁感知 provider、状态机和复杂设置。

## 外部参考

本设计参考了 2026-03-31 当日可访问的官方资料：

- `1Password Support: Sync your 1Password data`
  - 明确写到一台设备上的改动会立即在其他设备可用，体现“自动同步、低感知”。
  - 链接: <https://support.1password.com/sync/>
- `Termius Support: Secure Sync and Sharing Using Vaults`
  - 将 vault 描述为安全的单一数据源，强调同步是底层能力，而不是高频显式任务。
  - 链接: <https://support.termius.com/hc/en-us/articles/18873335262489-Secure-Sync-and-Sharing-Using-Vaults>

结论：

- 成熟产品默认把 sync 做成后台能力。
- 手动 `Sync now` 只应作为补救或显式刷新动作。
- “同步入口”和“同步配置”应分离。

## 目标

### 本轮目标

- 顶部标题栏 `Sync` 按钮改为“立即同步”动作，不再默认打开 modal。
- 左上角 `Open menu` 新增 `Sync Settings` 入口。
- 现有 sync modal 重构为 `Sync Settings` 配置面，不再承担标题栏主入口职责。
- 将同步模型改为“自动同步优先，手动同步兜底”。
- 修复 sync modal 的视觉与布局缺陷：
  - 错误提示不再挤压 footer
  - 按钮区与背景有清晰层级
  - modal 可稳定拖动
  - 长错误文案不再溢出底部边框

### 体验目标

- 用户日常只理解两件事：
  - 顶部 `Sync` 是立即同步
  - 菜单里的 `Sync Settings` 是配置同步
- 首次未配置时，不再出现“点了 Set up sync 但什么都配不了”的死路。
- 配置完成后，绝大多数同步在后台自动完成。

## 非目标

本轮不包含：

- 完整重做 vault 加密模型
- 新增团队共享 vault
- 在标题栏加入复杂的下拉菜单或二级动作
- 引入新的右侧 Sync 面板
- 同时开放过多 provider 的正式 UI 选择器

## 现状分析

### 1. 顶部入口语义错误

当前 `ui/shell/titlebar.slint` 中 `sync-button` 的 tooltip 是 `Open sync`，点击会触发 `open-sync-modal-requested`，本质上是“打开窗口”而不是“执行同步”。

### 2. 配置链路不存在

当前 `src/app/bootstrap.rs` 中：

- `Set up sync` 只是重复抛出 `Configure a Gitee remote first`
- 没有任何配置表单、配置保存或 provider 编辑流程

所以问题不是“文案不好”，而是交互主路径根本未闭环。

### 3. modal 布局把错误和动作混在一起

当前 `ui/components/sync-vault-modal.slint` 把错误 banner 放在 footer 内部。长错误文案出现时，会直接压缩按钮区域并制造越界视觉。

### 4. 自动同步缺席

虽然模型中已经有 `auto_sync_enabled`，但当前正式交互仍然要求用户显式打开 sync modal 再操作，这和用户预期相反。

## 方案比较

### 方案 A：保留顶部 `Open Sync`，只修样式

优点：

- 实现最小

缺点：

- 入口语义仍旧错误
- 用户仍需频繁感知 sync modal
- 不解决配置链路缺失

结论：

- 不采用

### 方案 B：顶部按钮直接同步，配置下沉到菜单，settings modal 承担配置与异常处理

优点：

- 入口语义清晰
- 符合自动同步优先的产品模型
- 与用户预期一致
- 可以复用现有 modal 基础设施

缺点：

- 需要重排标题栏、菜单、modal 状态机和部分 bootstrap 逻辑

结论：

- 采用

### 方案 C：顶部按钮做成左键同步、右键配置的复合动作

优点：

- 入口集中

缺点：

- 标题栏交互复杂
- discoverability 差
- 不适合当前项目已有 titlebar 交互模型

结论：

- 不采用

## 最终设计

## 设计要点 1：标题栏入口

- 标题栏按钮文案语义改为 `Sync`，tooltip 改为 `Sync now`
- 点击按钮时：
  - 若 sync 已配置且 vault 已解锁，直接执行同步
  - 若缺少配置或缺少解锁条件，打开 `Sync Settings`
- 顶部按钮不再作为默认 modal 入口

## 设计要点 2：菜单入口

左上角 `Open menu` 内新增：

- `Sync Settings`
- `Settings`
- `Close menu`

`Sync Settings` 是唯一正式配置入口。

## 设计要点 3：Sync Settings Modal

现有 sync modal 重构为 `Sync Settings`，承担：

- 查看当前 sync 状态
- 配置同步目标
- 开启或关闭自动同步
- 首次启用时输入主密码完成本地 vault 初始化
- 已锁定时输入主密码解锁
- 发生错误时展示恢复路径

modal 结构：

- Header
  - 标题 `Sync Settings`
  - 状态副标题
  - 可拖动标题栏
- Body
  - `Auto sync` 开关
  - `Targets` 配置区
  - `Vault` 解锁/启用区
  - 错误提示区
- Footer
  - `Cancel`
  - 状态相关主按钮，如 `Save and Enable` / `Unlock` / `Save Settings`

## 设计要点 4：同步目标

首轮 UI 收敛为：

- 一个 `Primary` 目标
- 一个可选 `Mirror` 目标

这样既满足“可选择多个同步目标”，又避免一次性把 UI 扩成无限增删列表。

目标字段：

- `Gist ID`
- `Personal Access Token`
- `Enabled`（mirror only）

## 设计要点 5：自动同步

自动同步规则：

- 若 `auto_sync_enabled == true` 且 vault 已解锁：
  - 完成首次启用后触发一次同步
  - 每次成功修改资产/密钥链数据后触发一次防抖同步
- 若 vault 已锁定：
  - 不自动同步
  - 顶部手动 `Sync` 会转入 `Sync Settings`

## 设计要点 6：错误处理

错误处理原则：

- 错误不再塞进 footer，避免挤压按钮
- 可恢复错误留在 body 的固定错误区
- 顶部 `Sync` 点击后的错误才需要打断用户；自动同步失败仅更新状态文案，不立刻弹窗

## 状态流

### 未配置

- 顶部点 `Sync` -> 打开 `Sync Settings`
- 用户填写 target + 主密码
- 点击 `Save and Enable`
- 创建本地 vault，并根据自动同步开关决定是否立即同步

### 已配置但锁定

- 顶部点 `Sync` -> 打开 `Sync Settings`
- 用户输入主密码并 `Unlock`
- 若 auto sync 打开，解锁成功后立即同步

### 已配置且已解锁

- 顶部点 `Sync` -> 直接执行同步
- 菜单里的 `Sync Settings` 仍可修改配置

## 测试要求

- 标题栏按钮 contract 改为 `sync-now-requested`
- 菜单 contract 包含 `sync-settings-selected`
- sync settings modal 渲染需要验证：
  - 错误区不再压缩 footer
  - footer 动作区颜色有明显对比
  - 拖拽交互仍有效
- bootstrap smoke 需要验证：
  - 顶部 `Sync` 在 ready 状态直接触发 sync
  - 未配置/锁定时会回退到 settings modal
  - auto sync 在启用后与资产变更后可触发

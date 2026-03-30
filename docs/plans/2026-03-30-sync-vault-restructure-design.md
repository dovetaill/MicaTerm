# Sync & Vault 重构 Design

日期: 2026-03-30
执行者: Codex
状态: 方案已确认，未进入业务实现

## 背景

当前 `Sync & Vault` 在产品、交互和实现三个层面都处于不可用状态：

- 入口错误。它被放在 `Settings` / 右侧 panel 体系里，而不是独立的产品能力。
- 交互错误。当前是右侧栏内容，不是独立 modal，也没有对齐现有 `SSH` modal 的交互模式。
- 行为错误。UI 中多个按钮没有真实点击接线，导致“看得到但用不了”。
- 能力边界错误。现有 payload 已经覆盖大量资产数据，但恢复、清空、provider 暴露和首次启用流程都没有收敛成可交付产品。

本轮目标不是局部修补，而是把 `Sync & Vault` 从“实验性右侧 panel”重构为“独立 Sync 能力 + 可用 vault/sync modal”，并明确首发平台为 Windows 11 桌面体验优先。

## 目标

### 本轮目标

- 将 `Sync` 从 `Settings` 中拆出，成为独立按钮、独立能力、独立 modal。
- 废弃当前右侧 `vault` panel 作为正式产品入口，不再让 `Sync & Vault` 占用右侧栏。
- modal 交互遵循当前项目已有 `SSH` modal 的桌面式模式：
  - 独立弹出
  - 状态驱动
  - 动作只在当前状态下显示
  - 不展示“想当然”的静态卡片堆砌
- 首发先保证 `Gitee` 路径可真实使用。
- sync 范围覆盖资产域核心数据：
  - `SSH connections`
  - `密码 / 密钥 / keychain`
  - `snippets`
  - `文件与文件夹目录结构`
- 明确排除 `ui_preferences`。
- 内部架构保持通用 provider 能力，为后续 `GitHub Gist`、`S3`、自建服务等保留扩展位，但首发正式 UI 收敛。

### 体验目标

- Windows 11 Fluent 气质，但不做多余装饰。
- `Sync` 作为一级功能，应具备高发现性和明确的任务导向。
- 第一次打开 modal 时，用户看到的是“如何启用同步”，不是“残缺的状态面板”。
- 已启用后，用户看到的是“当前同步健康状态 + 可执行动作”，不是“无效按钮集合”。

## 非目标 / 边界

本轮不包含：

- 实现 `tabby-web` 风格的自建 sync host。
- 首发支持 `OAuth` 浏览器授权闭环。
- 在正式 UI 中同时暴露多 provider 选择器。
- 恢复 `Appearance` 为正式功能入口。
- 同步 `ui_preferences`。
- 将当前右侧 panel 继续演化为正式 vault 管理界面。

本轮边界说明：

- `Appearance` 被视为历史预留入口，并没有真正开发完成；正式 UI 不再将其作为当前能力展示。
- `Settings` 只保留真实可用的本地应用设置，不再承载 `Sync`。
- `known_hosts` 不作为本次确认方案中的首发同步承诺范围；如后续纳入，需要单独定义 SSH trust policy。

## 当前实现现状

### 1. `Sync & Vault` 仍绑定在右侧 panel

当前 `ShellViewModel` 和 Slint UI 仍将 `vault` 视为 `RightPanelView` 的一个变体，而不是独立 modal：

- [view_model.rs](/home/wwwroot/mica-term/src/shell/view_model.rs#L41)
- [right-panel.slint](/home/wwwroot/mica-term/ui/shell/right-panel.slint#L37)

这直接违背了“独立 Sync 能力”的产品语义。

### 2. UI 按钮基本无效

当前右侧 `vault` 面板中，多数动作只是 `Rectangle + Text`，没有实际点击事件与后端绑定：

- [right-panel.slint](/home/wwwroot/mica-term/ui/shell/right-panel.slint#L111)
- [app-window.slint](/home/wwwroot/mica-term/ui/app-window.slint#L644)

所以现状不是“逻辑偶发失败”，而是“入口 UI 从定义上不可操作”。

### 3. Rust 后端 action 已存在，但没有形成产品闭环

vault 相关行为已经在 `bootstrap` 中存在：

- `create`
- `unlock`
- `sync`
- `lock`

关键位置：

- [bootstrap.rs](/home/wwwroot/mica-term/src/app/bootstrap.rs#L4010)
- [bootstrap.rs](/home/wwwroot/mica-term/src/app/bootstrap.rs#L4043)
- [bootstrap.rs](/home/wwwroot/mica-term/src/app/bootstrap.rs#L4076)
- [bootstrap.rs](/home/wwwroot/mica-term/src/app/bootstrap.rs#L4101)

因此当前主要问题不是“后端完全没有能力”，而是信息架构、状态机和 UI 接线没有形成可用产品。

### 4. 当前 vault 面板模型过粗，且误导用户

当前 view model / UI 存在这些问题：

- 一个 label 同时承担多种动作语义。
- UI 硬编码 `1 primary + 1 mirror` 卡片。
- 后端模型实际支持 `1 primary + N mirrors`，但 UI 没有可解释映射。

关键位置：

- [view_model.rs](/home/wwwroot/mica-term/src/shell/view_model.rs#L63)
- [model.rs](/home/wwwroot/mica-term/src/app/vault/model.rs#L351)

这意味着当前正式 UI 既不忠于后端能力，也不忠于首发产品边界。

### 5. snapshot 已覆盖多类资产，但 restore / clear 仍不完整

`VaultSnapshot` 当前已经包含：

- asset catalog
- SSH secret bundles
- keychain catalog
- keychain secret bundles
- `known_hosts`
- sync/ui prefs

关键位置：

- [model.rs](/home/wwwroot/mica-term/src/app/vault/model.rs#L236)

但实际 apply / clear 路径并不完整：

- unlock 后没有把 `keychain_catalog` 完整重建回 shell state
- lock 时只清理了部分资产树，没有清理完整的 keychain UI / catalog 状态

关键位置：

- [bootstrap.rs](/home/wwwroot/mica-term/src/app/bootstrap.rs#L3280)
- [bootstrap.rs](/home/wwwroot/mica-term/src/app/bootstrap.rs#L3296)
- [view_model.rs](/home/wwwroot/mica-term/src/shell/view_model.rs#L2295)

### 6. 当前默认运行时允许“本地已创建，但无法真正 sync”

`VaultRuntimeOptions::default()` 里没有提供完整的 bootstrap remote 初始化模板：

- [bootstrap.rs](/home/wwwroot/mica-term/src/app/bootstrap.rs#L229)
- [bootstrap.rs](/home/wwwroot/mica-term/src/app/bootstrap.rs#L3801)

这导致用户有可能得到一个“本地 vault 已创建，但远端未配置”的半完成状态。对于正式产品，这不是可接受的首发体验。

### 7. provider 成熟度不一致

当前 provider 状态：

- `S3` 可用
- `GitHubGist` 已接线
- `GitLabSnippet` 仍是 placeholder
- `GiteeGist` 仍是 placeholder

关键位置：

- [github_gist.rs](/home/wwwroot/mica-term/src/app/vault/provider/github_gist.rs#L209)
- [gitlab_snippet.rs](/home/wwwroot/mica-term/src/app/vault/provider/gitlab_snippet.rs#L147)
- [gitee_gist.rs](/home/wwwroot/mica-term/src/app/vault/provider/gitee_gist.rs#L91)

结合当前任务要求，首发必须优先补齐 `Gitee`，而不是维持一个“看上去支持很多 provider，实际上大多不可用”的界面。

### 8. 当前菜单结构仍然保留 `Settings` / `Appearance`

titlebar 和全局菜单当前仍保留：

- `Settings`
- `Appearance`
- 右侧 panel toggle

关键位置：

- [titlebar.slint](/home/wwwroot/mica-term/ui/shell/titlebar.slint#L189)
- [titlebar.slint](/home/wwwroot/mica-term/ui/shell/titlebar.slint#L363)
- [titlebar-menu.slint](/home/wwwroot/mica-term/ui/components/titlebar-menu.slint#L33)

这与本轮确认的产品边界冲突：

- `Sync` 应成为独立按钮
- `Appearance` 不应继续作为正式功能入口

### 9. Git 历史表明 vault 是渐进堆叠出来的，而不是一开始就完成了产品化

相关演进提交：

- `a075087 feat: add ssh vault sync workflow`
- `328c012 feat: complete assets snippets workflow`
- `e0ccf6d feat: implement assets keychain and ssh identity integration`
- `5a8772f fix: restore snippet actions and ssh secret persistence`

这些提交说明 vault/sync 已逐步扩展到资产域多个子系统，但入口设计和可用性没有同步收敛，所以现在需要一次产品化重构。

## 设计要点拆分

## 设计要点 1：产品入口与导航边界

### 方案 A：继续放在 `Settings` / 菜单层级

优点：

- 改动较小
- 不需要新增 titlebar 一级入口

问题：

- 仍然会被理解为“附属配置项”
- 与“独立 Sync 能力”的产品语义冲突
- 会继续混淆 `Settings` 与业务功能

### 方案 B：`Sync` 作为 titlebar 常驻独立按钮，点击打开 modal

优点：

- 产品语义最清晰
- 高发现性
- 与现有 `SSH` modal 的全局动作模式一致
- 能直接摆脱右侧 panel 体系

问题：

- titlebar 会新增一个一级入口，需要控制按钮密度

### 最终决策

采用方案 B。

正式产品中：

- `Sync` 是 titlebar 常驻独立按钮
- `Settings` 只保留真实可用的本地应用设置
- `Appearance` 从正式 UI 隐藏，不再作为当前能力入口

## 设计要点 2：modal 信息架构

### 方案 A：固定式 dashboard modal

特征：

- 无论当前状态如何，都展示一组固定卡片
- 所有动作都常驻显示

问题：

- 容易重演当前右侧 panel 的“卡片看起来很多，但逻辑关系混乱”
- 首次启用、已锁定、已启用、错误态不易分辨

### 方案 B：状态驱动 modal，交互参考现有 `SSH` modal

特征：

- 按当前状态切换内容与动作
- 首次启用时展示 setup 流程
- 已启用时展示管理与状态
- 错误态展示可恢复动作和可理解提示

优点：

- 更符合桌面客户端 modal 习惯
- 动作数量与状态匹配，不会出现“无效按钮堆满”
- 能自然承载 remote-first 初始化流程

### 最终决策

采用方案 B。

modal 至少区分以下状态：

- `NotConfigured`
- `Locked`
- `UnlockedButRemoteIncomplete`
- `Ready`
- `SyncError`

每种状态只展示与当前上下文直接相关的动作，不再沿用现在的静态 `primary / secondary / tertiary` 按钮布局。

## 设计要点 3：首次启用流程

### 方案 A：remote-first

流程：

- 打开 `Sync` modal
- 设置 vault master password
- 配置远端 provider
- 校验认证
- 选择或创建远端对象
- 完成首次 sync 启用

优点：

- 用户完成启用时就已经“真正可同步”
- 避免当前“本地已创建，但并不能 sync”的假完成状态
- 更符合行业常见同步产品做法

问题：

- 首次流程比本地先建仓更长

### 方案 B：local-first

流程：

- 先本地创建 vault
- 以后再补绑远端

问题：

- 会继续制造半完成状态
- UI 更难解释当前到底是“已启用”还是“待补全”

### 最终决策

采用方案 A。

正式 UI 不再把“只有本地 vault、没有远端”的状态包装成已完成启用。

## 设计要点 4：同步范围

### 方案 A：只同步最小配置

范围：

- 仅远端连接配置
- 少量 snippets / 偏好数据

问题：

- 与当前产品定位不符
- 与 Git 历史中已经扩展的资产域能力不符
- 用户迁移价值太低

### 方案 B：同步完整资产域核心数据，但排除 `ui_preferences`

范围：

- `SSH connections`
- SSH passwords / identities / keychain secrets
- `snippets`
- 文件与文件夹目录结构

优点：

- 与“资产域同步”的目标一致
- 与现有 `VaultSnapshot` 覆盖方向一致
- 真正有跨设备迁移价值

问题：

- restore / clear 路径必须补齐，不能只靠 payload 包含字段

### 最终决策

采用方案 B。

明确排除：

- `ui_preferences`

本轮文档不把 `known_hosts` 作为首发承诺同步项。

## 设计要点 5：provider 暴露策略与 Gitee 首发形态

### 方案 A：正式 UI 首发只暴露 `Gitee`，内部保留通用 provider 架构

特征：

- 用户只看到首发可用的 `Gitee`
- 内部仍维护 provider registry / auth model / target model
- 后续可扩展 `GitHubGist`、`S3`、自建服务

优点：

- 首发界面清楚
- 范围受控
- 不会暴露 placeholder provider

### 方案 B：正式 UI 同时展示多 provider 壳

特征：

- 一开始就展示多个 provider 选项
- 未完成 provider 以 disabled / coming soon 呈现

问题：

- 会制造“看上去支持很多，实际上还不能用”的印象
- 不利于当前实验产品快速收敛

### 最终决策

采用方案 A。

正式 UI 首发只暴露 `Gitee`。

内部架构仍保持 provider-agnostic，不走一次性写死的 `Gitee-only` 特例实现。

## 设计要点 6：Gitee 认证方式

### 方案 A：首发仅支持 `PAT`

特征：

- 使用 `access token + target reference`
- 配置简单
- 更贴近社区 `Gitee Gist` 同步实践

参考：

- 社区插件 [`starxg/terminus-sync-config`](https://github.com/starxg/terminus-sync-config)
- Tabby 官方对自建 sync 的说明见 [`Eugeny/tabby#5119`](https://github.com/Eugeny/tabby/issues/5119#issuecomment-991722190)

优点：

- 范围可控
- 最快把 `Gitee` 路打通
- 失败面小

### 方案 B：首发同时支持 `PAT + OAuth`

优点：

- 更适合普通用户

问题：

- 需要浏览器跳转、回调处理、token 生命周期设计
- 明显扩大首发复杂度

### 最终决策

采用方案 A。

首发先交付 `PAT` 路径，后续如要增加 `OAuth`，通过内部通用 auth model 扩展，不推翻首发架构。

## 方案对比

本轮最终方案是以下组合，而不是单点修补：

- 入口层面：`Sync` 从 `Settings` 脱离，成为 titlebar 一级按钮
- 容器层面：废弃正式右侧 `vault` panel，改为独立 modal
- 交互层面：modal 采用状态驱动，而不是静态 dashboard
- 生命周期层面：采用 remote-first 启用流程
- 数据层面：同步完整资产域核心数据，但排除 `ui_preferences`
- provider 层面：正式 UI 首发只暴露 `Gitee`
- 认证层面：首发仅支持 `PAT`
- 架构层面：内部保留通用 provider / auth / target 模型

对比结论：

- 相比“继续修右侧 panel”或“继续混在 Settings 里”，该方案更符合产品语义，也更容易保证点击闭环。
- 相比“多 provider 一起展示”，该方案更利于实验产品快速做成真正可用的首发版本。
- 相比“先本地再远端”，该方案更符合业界同步产品的启用语义。

## 最终决策

本轮已确认决策如下：

1. `Sync` 是独立功能，不属于 `Settings`。
2. 正式 UI 中，`Sync` 通过 titlebar 独立按钮打开 modal。
3. 正式产品不再使用右侧 `vault` panel 作为 `Sync & Vault` 入口。
4. modal 交互参考现有 `SSH` modal，采用状态驱动展示与动作控制。
5. `Appearance` 视为历史预留入口，并没有真正开发；正式 UI 隐藏该入口。
6. `Settings` 只保留真实可用的本地设置。
7. 首次启用采用 remote-first，不再把“只有本地 vault”视为已启用。
8. 首发正式 UI 只暴露 `Gitee` provider。
9. 首发 `Gitee` 认证方式仅支持 `PAT`。
10. sync 范围覆盖资产域核心数据，但明确排除 `ui_preferences`。
11. 内部实现保持通用 provider 架构，不做一次性硬编码特例。

## 实施步骤

以下为设计级实施顺序，不展开为 implementation plan：

1. 重定义产品 IA。
   - 将 `Sync` 从 `Settings` / 右侧 panel 体系中抽离
   - 清理正式 UI 中的 `Appearance` 暴露
   - 明确 titlebar 独立按钮与 modal 打开契约

2. 建立新的 modal 状态机。
   - 定义 `NotConfigured`、`Locked`、`UnlockedButRemoteIncomplete`、`Ready`、`SyncError`
   - 定义每个状态下允许展示的字段和动作

3. 收敛远端初始化流程。
   - 以 remote-first 方式重组首次启用流程
   - 让“启用完成”严格等于“远端已可用、sync 可执行”

4. 收敛首发 provider 面。
   - 正式 UI 只做 `Gitee`
   - 内部维持通用 provider registry 和 auth model
   - 首发认证先交付 `PAT`

5. 校正 snapshot contract。
   - 明确哪些资产进入 snapshot
   - 明确 lock / unlock / clear / restore 的完整生命周期
   - 明确 `ui_preferences` 不进入同步 contract

6. 完成 modal 动作闭环。
   - 所有可见按钮都必须有实际 action
   - 错误态必须回到可恢复状态，而不是留下死按钮

7. 废弃旧正式入口。
   - 右侧 `vault` panel 不再作为正式产品入口
   - 旧 UI 结构若暂时保留，只能作为过渡内部路径，不再对正式用户暴露

## 风险与回滚策略

### 风险 1：modal 改造期间，旧 panel 与新 modal 并存导致状态分裂

应对：

- 在实现期只允许一个正式用户入口
- 旧 panel 如需保留，只能作为内部过渡路径，不共享正式入口语义

回滚策略：

- 若新 modal 未完成闭环，不上线正式入口切换
- 先保留现有结构为内部开发态，不对正式用户宣称可用

### 风险 2：snapshot 字段“已包含”但 restore/clear 不完整

应对：

- 以“可还原、可清理、可重复 sync”为标准定义 contract
- 不能只看数据结构是否序列化成功

回滚策略：

- 若某一类资产无法稳定 round-trip，则在首发范围中移出该类资产，而不是保留半工作状态

### 风险 3：`Gitee` provider 首发不稳定

应对：

- 首发认证仅使用 `PAT`
- 正式 UI 只暴露一个 provider，避免同时排查多路故障

回滚策略：

- 如果 `Gitee` 路径在验证期仍不稳定，保留内部通用 provider 架构，但延后正式对外开放 sync 功能，而不是回退到右侧 panel 伪入口

### 风险 4：`Appearance` 隐藏后影响内部实验入口

应对：

- 正式 UI 与内部实验入口分离

回滚策略：

- 如确有内部调试需求，仅恢复 dev-only 暴露，不恢复正式用户可见入口

## 验证清单

以下清单用于后续实现验收：

- `Sync` 不再出现在 `Settings` 信息架构中。
- titlebar 存在独立 `Sync` 入口按钮。
- 点击 `Sync` 打开的是独立 modal，而不是右侧 panel。
- 正式 UI 不再暴露 `Appearance` 入口。
- 首次启用流程必须要求远端可用，不能停留在“仅本地已创建”状态。
- modal 中所有可见按钮都有真实行为，不存在死按钮。
- `Gitee` 首发路径可完成配置、校验、同步。
- `SSH connections` 能完成同步 round-trip。
- SSH passwords / identities / keychain 能完成同步 round-trip。
- `snippets` 能完成同步 round-trip。
- 文件与文件夹目录结构能完成同步 round-trip。
- `ui_preferences` 不进入 sync payload，也不会在恢复时被应用。
- lock / unlock 后，资产状态恢复与清空行为一致。
- 发生认证失败、远端不存在、同步失败时，modal 能展示可理解且可恢复的错误信息。

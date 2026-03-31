# Sync Modal And Auto Sync Hardening Design

日期: 2026-03-31
执行者: Codex
状态: 已批准，待实现

## 背景

当前同步链路还有四个用户可见问题没有收口：

- `Sync Settings` modal 仍然可能在较小窗口里把 footer 挤出可视区，底部按钮和 `Master password` 区域会超出屏幕。
- modal 的 header、body、footer 与页面背景色差不足，动作区和表单区混在一起，错误状态下更难读。
- 自动同步仍然依赖零散入口手动补调用，`SSH / key / snippet` 的部分增删改没有统一进入同步调度。
- 顶部同步缺乏可见反馈，也没有后台定时兜底，用户很难判断同步是否正在运行、成功还是失败。

用户还明确提出两个安全约束：

- 重新打开应用、重新解锁或本地数据暂时缺失时，绝不能把空本地直接覆盖远端。
- 自动同步只应在用户实际发生本地变更后触发；没有真实 mutation 时只能做拉取/校验，不能盲目上传。

## 外部参考

这次实现参考了以下资料，用于约束交互和同步策略：

- `web.dev: Building a dialog component`
  - 对话框应该使用受视口约束的最大尺寸，固定非滚动区域，只让内部内容滚动。
  - 链接: <https://web.dev/building-a-dialog-component/>
- `web.dev: Dialog pattern`
  - dialog 应按不同 viewport 自适应，而不是依赖固定高度。
  - 链接: <https://web.dev/patterns/components/dialog>
- `Primer React PR #5629`
  - 超短视口下 dialog footer 也必须保持可达，不能因为内容增长直接被吞掉。
  - 链接: <https://github.com/primer/react/pull/5629>
- `Evil Martians: Cool frontend arts of local-first: storage, sync, conflicts`
  - local-first 产品应把同步收口成中心协议，在后台做 push/pull 和冲突处理，而不是在每个动作上散落 UI 驱动。
  - 链接: <https://evilmartians.com/chronicles/cool-front-end-arts-of-local-first-storage-sync-and-conflicts>
- `AWS AppSync: Conflict detection and resolution`
  - 并发冲突默认应保守拒绝或要求显式恢复，优于静默覆盖。
  - 链接: <https://docs.aws.amazon.com/appsync/latest/devguide/conflict-detection-and-resolution.html>

## 目标

### 本轮目标

- 让 `Sync Settings` modal 在受限高度下仍能完整显示 header、scroll body、footer。
- 强化 sync modal 的层级与颜色对比，让表单区、错误区、动作区一眼可分。
- 引入统一的 vault sync scheduler，把所有本地 mutation 收口为 `mark dirty -> debounce sync`。
- 增加 `2 分钟` 周期兜底同步，只在已解锁且已配置时运行。
- 顶部同步按钮增加进行中/成功/失败反馈。
- 保持“无真实本地变更不上传”的安全边界，并优先保护远端非空数据。

### 用户体验目标

- modal 底部在普通笔记本高度下始终可见。
- 错误提示不会再把底部按钮顶出边框。
- 用户修改 SSH、密钥、snippets 后，不需要再手动打开同步配置才能触发同步。
- 标题栏的同步行为有明确反馈，不再是“按了像没反应”。

## 非目标

本轮不包含：

- 新增第三种以上远端 provider
- 引入复杂的冲突合并 UI
- 重做 vault 加密格式
- 重构整个标题栏或全局主题系统

## 现状分析

### 1. Sync modal 仍然被固定高度约束

`ui/app-window.slint` 仍把 sync modal shell 固定为：

- `modal-width: 640px`
- `modal-height: 620px`

这意味着即使 `SyncVaultModal` 内部已经拆出了 scroll body，整个 shell 仍然可能在较低 viewport 中把 footer 推到可视区之外。

### 2. modal 内部层级不够明显

`ui/components/sync-vault-modal.slint` 当前大量使用接近的浅色面：

- header
- body canvas
- form field shell
- footer action bar

这些面在 light mode 下差距过小，导致 footer 看起来和外层背景融在一起。

### 3. 自动同步没有中心调度

`src/app/bootstrap.rs` 现有 `sync_local_vault_if_auto_enabled(...)` 只在少数 mutation 入口被调用，属于零散手工挂接：

- 某些资产确认
- rename
- delete
- 某些 SSH 保存入口

这种做法天然会漏掉其它 mutation 路径，也无法统一做防抖、状态反馈和周期兜底。

### 4. 同步状态没有持续反馈

虽然 sync modal 会显示 `status-text` 和 `error-text`，但标题栏没有同步进行中的视觉反馈，用户点击 `Sync` 后缺乏即时确认。

## 方案比较

### 方案 A：继续在现有入口上补 auto-sync 调用

优点：

- 改动最小

缺点：

- 仍旧是零散挂接，长期还会继续漏
- 无法自然接入防抖和周期任务
- modal 高度问题和状态反馈问题仍然存在

结论：

- 不采用

### 方案 B：只修 modal 响应式，不改同步调度

优点：

- 能立即解决界面可见性问题

缺点：

- 数据层行为仍不可靠
- 用户修改 SSH / key / snippet 后仍可能不同步
- 标题栏仍然缺乏同步反馈

结论：

- 不采用

### 方案 C：响应式 modal + 中央调度器 + 顶部反馈（采用）

优点：

- 直接覆盖当前全部核心抱怨
- 符合业界 dialog 和 local-first sync 的成熟做法
- 后续扩展 provider 或更多 mutation 路径时不会继续散架

缺点：

- 需要同时改 Slint 布局、view model 状态和 bootstrap 调度

结论：

- 采用

## 最终设计

### 设计要点 1：modal 改为视口约束布局

`BlockingModalShell` 的 sync 实例不再使用固定 `620px` 高度，而是：

- 保留一个理想高度用于大窗口
- 额外按窗口可用高度减去标题栏和边距做上限约束
- 保证整个 shell 永远留在窗口可视区内

`SyncVaultModal` 保持三段式：

- `Header`: 固定，负责标题、副标题、关闭、拖动
- `Body`: 唯一滚动区域，承载表单、状态和错误内容
- `Footer`: 固定动作栏，始终可见

### 设计要点 2：错误内容进入 body，不再挤压 footer

错误提示必须留在可滚动 body 中，且高度可随着错误增长。footer 只承载动作，不再承担长文本渲染。

这样即使远端错误很长，用户也仍能看到：

- `Master password`
- `Close`
- `Sync now` / `Save`

### 设计要点 3：强化视觉层级

sync modal 改成清晰三层：

- header 使用更高对比的表面色和分隔线
- body 使用独立表单背景
- footer 使用单独动作栏背景、顶部分隔线和更强按钮色差

主按钮与次按钮也要拉开：

- primary 使用清晰 accent surface
- secondary 使用更深或更明确的 outline/filled surface

### 设计要点 4：统一的 sync scheduler

新增一个 vault sync scheduler，状态包含：

- `dirty`: 是否存在待上传的本地变更
- `in_flight`: 是否已有同步任务执行中
- `last_result`: 最近一次同步结果，用于标题栏反馈
- `debounce_timer`: 本地 mutation 后的短延时任务
- `periodic_timer`: `2 分钟` 周期兜底任务

统一规则：

- 本地 mutation 成功提交后：`mark dirty`
- 若已解锁、已配置、auto sync 开启：启动或重置 `1-2 秒` 防抖同步
- 周期定时器仅在已解锁且已配置时运行，用于兜底 pull/retry
- 手动标题栏 `Sync now` 直接绕过防抖，立即请求同步

### 设计要点 5：同步安全边界

自动同步只在 `dirty == true` 时允许 push。

以下场景不允许自动上传空本地：

- 重新打开应用
- 重新解锁
- 本地缓存暂时不存在
- 仅仅打开 sync settings

如果本地缺失但远端存在：

- 优先走拉取/恢复路径
- 保留当前已有的冲突检查
- 不做 silent overwrite

### 设计要点 6：顶部同步反馈

标题栏同步按钮增加状态映射：

- `idle`: 普通图标/文本
- `syncing`: 旋转或帧动画反馈
- `success`: 短暂成功状态
- `error`: 清晰失败状态，并允许再次点击

反馈来自 scheduler 的最近状态，而不是只在 modal 内部显示。

### 设计要点 7：mutation 接入点

所有会改变 vault 数据的写入路径都应调用统一 helper，而不是各自直接发同步：

- SSH 资产新增/编辑
- snippets 新增/编辑
- key / identity 新增/编辑
- folder 新增/rename/delete
- 资产 delete / rename

helper 负责：

- 标记 dirty
- 检查 unlock/config/auto-sync 条件
- 安排防抖同步
- 更新标题栏状态

## 测试策略

### UI 测试

- 更新现有 Slint render/smoke 测试，验证 sync modal footer 在较矮视口下仍有可见像素区域。
- 增加对 footer action zone、footer panel contrast、error-in-body 的断言。
- 增加源码契约测试，约束 sync modal shell 不再使用硬编码 `620px` 高度。

### 调度测试

- 为 scheduler 增加单元测试，验证：
  - mutation 会标记 dirty
  - 防抖期间多次 mutation 只触发一次同步
  - periodic tick 在未 dirty 时不会 push 空本地
  - unlock 本身不会触发 push

### 集成测试

- 复用 `bootstrap_smoke` / `sync_vault_modal_smoke`，覆盖 SSH / rename / delete / snippet 等 mutation 路径最终都进入统一调度。
- 断言同步状态能反映到标题栏和 modal。

## 受影响文件

- 修改 `ui/app-window.slint`
- 修改 `ui/components/sync-vault-modal.slint`
- 修改 `ui/shell/titlebar.slint`
- 修改 `ui/theme/tokens.slint`
- 修改 `src/shell/view_model.rs`
- 修改 `src/app/bootstrap.rs`
- 修改 `tests/assets_modal_render_spec.rs`
- 修改 `tests/assets_modal_smoke.rs`
- 修改 `tests/sync_vault_modal_smoke.rs`
- 修改 `tests/bootstrap_smoke.rs`
- 修改 `tests/top_status_bar_smoke.rs`

## 风险与缓解

### 风险 1：Slint 布局约束互相打架

缓解：

- 先补渲染失败测试，再最小化调整 shell 和 modal 的高度/滚动关系
- 不重写 `BlockingModalShell` 的通用逻辑，只对 sync modal 实例做约束

### 风险 2：中央调度器误把“打开/解锁”当成 mutation

缓解：

- 明确把 `mark dirty` 和 `request sync` 分离
- 只有真实写入路径才设置 dirty

### 风险 3：周期任务造成频繁远端请求

缓解：

- 周期兜底固定为 `2 分钟`
- 未配置、已锁定、已有任务进行中时直接跳过
- 未 dirty 时只允许轻量 pull/retry 分支，不做 push

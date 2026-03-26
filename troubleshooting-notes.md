# 疑难问题排查记录

记录已经定位并解决的疑难问题，重点保留：

- 问题现象
- 真正根因
- 解决方案
- 可复用的排查经验

后续新增记录时，建议继续按日期和主题追加，避免把这份文档写成一次性事故说明。

## 2026-03-26 SSH Tab / Context Menu 问题排查经验

### 问题现象

这次问题集中出现在打开 SSH tab 之后：

- tab 的关闭按钮很难点中
- 鼠标悬浮到 tab 或关闭按钮时出现闪烁
- SSH tab 无法稳定切换
- 已打开 SSH session 后，资产列表右键菜单项点击无响应

这些现象表面上像是：

- `TouchArea` 命中区域有问题
- callback 没有正确绑定
- hover 态样式切换异常
- tab close / switch 的业务逻辑失效

但这些都不是根因。

### 真正根因

根因在 `src/app/bootstrap.rs` 的 session projection timer 刷新链路。

系统每 50ms 会把 `SessionManager` 中的 session 状态投影到 UI 的 `ShellViewModel`。原逻辑里，tab 列表比较使用的是：

- manager 侧重新生成的 `WorkspaceTab`
- state 中当前保存的 `workspace_tabs`

问题在于 `WorkspaceTab.active` 是 UI 派生状态，不是 manager 持有的稳定状态：

- manager 重新投影出来的 tab 默认 `active = false`
- UI 当前激活的 tab 在 state 中会是 `active = true`

这样一来，即使 session 实际没有变化，tab 列表比较结果也会一直不同，导致 `tabs_changed` 长期为 `true`。

后果是：

- tab strip 的 model 被 timer 周期性整体重建
- assets context menu 的 model 也跟着被重建
- 鼠标 hover 期间命中目标不断被替换
- close hit target、tab 本体、context menu row 都会在交互过程中失去稳定引用

最终表现就是：

- hover 闪烁
- 点击点不中
- tab 切换像没生效
- 右键菜单项点击像没反应

所以这不是单个点击事件的问题，而是 UI 交互目标在事件发生期间被不断重建。

### 解决方案

### 1. 先归一化 active tab，再比较 tab 列表

在 `sync_workspace_projection_from_manager(...)` 中新增 active session 推导逻辑：

- 优先使用 `active_workspace_session_id`
- 其次从当前 `workspace_tabs` 中继承仍然存在的 active tab
- 再次回退到第一个 tab

然后在比较 `state.workspace_tabs()` 和 `next_tabs` 之前，先把 `next_tabs.active` 全部按当前 active session 统一写回。

这样 `WorkspaceTab.active` 不再制造伪变化，`tabs_changed` 只会在真实 tab 集合变化时才变成 `true`。

### 2. 把 tab chrome 更新和 session surface 更新拆开

原先逻辑把以下内容绑在一次刷新里：

- tab strip model
- active session 元数据
- terminal surface
- assets context menu state

这会让纯终端内容刷新也触发 tab strip 重建。

修复后拆成两类同步：

- `sync_workspace_tab_items(...)`
  - 只更新 tab strip model
- `sync_workspace_session_state(...)`
  - 只更新 active session title / subtitle / state / surface

这样 terminal surface 更新时，不会再影响 tab 和 context menu 的交互目标。

### 3. timer 只在必要时重建 tab / context menu

在 session projection timer 中引入 `WorkspaceProjectionDelta`：

- `tabs_changed`
- `surface_changed`

刷新策略改为：

- 仅当 `tabs_changed` 时：
  - 更新 `workspace_tab_items`
  - 更新 `assets_context_menu_state`
- 当 `tabs_changed || surface_changed` 时：
  - 更新 active session 的显示状态

这一步直接消除了 50ms 周期性重建 tab / menu model 的问题。

### 4. 增加真实 pointer 回归测试

为避免以后再回归，补了两类测试：

- tab 点击切换与 close hit target 点击关闭
- 已有 session 后，从 assets context menu 点击 `open-connection` 再打开新 tab

同时在 `build.rs` 的非 `release` 构建里开启 Slint debug info，保证测试环境能稳定定位 `ElementHandle`。

### 这次修复涉及的文件

- `src/app/bootstrap.rs`
- `tests/assets_explorer_smoke.rs`
- `build.rs`

### 经验总结

### 1. UI 派生状态不要直接参与“源状态是否变化”的比较

像 `active` 这种字段是 UI 层派生状态，不应直接拿来和 manager 投影结果做原样比较。否则很容易制造“看起来一直在变”的假变化。

### 2. 高频 timer 刷新里要避免整体替换 interactive model

如果某个 model 上承载了：

- hover 状态
- pointer 命中区域
- 点击回调目标

那么高频刷新时必须尽量避免整体重建，否则就会出现“视觉上像有按钮，但实际上很难点中”的问题。

### 3. surface 刷新和 chrome 刷新必须解耦

terminal 内容刷新频率高，tab chrome 刷新频率低，两者不能共用同一套“整块同步”策略，否则交互层一定会被误伤。

### 4. 这类问题不能只看 callback，要看 model 生命周期

如果点击没有反应，不一定是 callback 没有绑定，也可能是：

- 事件目标刚生成就被替换
- hover 过程中 model 被刷新
- pointer press 和 pointer release 命中了不同元素

这类问题应该优先检查：

- model 是否被频繁重建
- timer / async event 是否在交互期间改写 UI
- 交互目标是否具备稳定 identity

### 后续排查建议

以后再遇到类似“hover 闪烁 + 点击无响应 + UI 看起来正常”的问题，可以按下面顺序排查：

1. 看是否存在高频 timer / async projection 在重建 model
2. 看比较逻辑里是否混入了 UI 派生字段
3. 看 pointer 事件期间元素是否被替换
4. 用真实 pointer 注入测试复现，而不是只测 callback 调用
5. 把“内容刷新”和“交互 chrome 刷新”拆开验证

### 结论

这次问题的本质不是 tab close、tab switch 或 context menu action 本身坏了，而是 session projection timer 让 tab 与菜单的交互目标在运行时不断被重建。

修复的关键不在于补更多点击逻辑，而在于：

- 先修正 projection diff 的判定
- 再拆分不同层级的 UI 同步职责
- 最后用真实 pointer 测试锁住行为

这类问题属于“状态投影策略错误导致的交互失稳”，以后应优先从数据流和 model 生命周期入手排查。

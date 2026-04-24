# Modal Input and Select Hardening Design

日期: 2026-04-24
方案名: `modal-input-select-hardening`
状态: 已确认，可进入 implementation planning

## 背景

当前仓库中的编辑类 modal，尤其是 SSH connection modal，在真实鼠标交互下暴露出三类明显问题：

- 文本输入框点击后，光标总是落在文本起始处，无法把插入点放到任意字符之间；
- 文本输入框无法通过鼠标拖选已有内容；
- 带 trailing action 的密码输入框里，现有 `Show` 按钮点击不稳定甚至无法点击；
- modal 底部附近的下拉框展开后会被裁切，选项列表不能完整显示。

用户本次要求不是只修 SSH 一个页面，而是：

- 统一修复所有复用 `DialogTextField` 的编辑类 modal；
- 密码显示/隐藏改成 Fluent 风格眼睛图标；
- 密码显示采用“点击切换显示/隐藏”的持久切换，而不是 press-and-hold peek；
- 对 modal 内部下拉框采用成熟、稳定、可维护的方案，不盲目依赖当前 stock `ComboBox`。

本轮设计聚焦于共享 modal primitive 的交互修正，不改 SSH runtime、资产模型、保存逻辑，也不扩展到非编辑类页面。

## 问题分析

### 1. 文本输入定位和拖选失效的根因

共享输入组件 `ui/components/modal-chrome.slint` 中的 `DialogTextField` 当前由两层交互结构组成：

- 真正可编辑的 `TextInput` 位于 `ui/components/modal-chrome.slint:432`；
- 其上方又覆盖了一个整块 `TouchArea`，位于 `ui/components/modal-chrome.slint:470`。

该 `TouchArea` 点击后只调用 `field-input.focus()`，并没有把鼠标点击位置交回 `TextInput` 自己处理。这会直接导致：

- 光标只能按 `TextInput` 默认聚焦行为落位，而不是落在鼠标点击字符附近；
- 鼠标按下并拖动时，拖选流程无法由 `TextInput` 启动；
- 覆盖层还会抢占 trailing action 区域的命中。

因为 `DialogTextField` 是共享 primitive，这个问题天然会扩散到所有复用它的编辑类 modal，例如：

- `ui/components/assets-ssh-connection-modal.slint`
- `ui/components/assets-rename-modal.slint`
- `ui/components/assets-folder-create-modal.slint`
- `ui/components/assets-keychain-identity-modal.slint`
- `ui/components/assets-keychain-ssh-key-modal.slint`
- `ui/components/sync-vault-modal.slint`

### 2. 密码显示按钮无法点击的根因

SSH modal 里的密码字段目前复用 `DialogTextField` 的 trailing action 文本按钮：

- 主密码字段：`ui/components/assets-ssh-connection-modal.slint:387`
- 代理密码字段：`ui/components/assets-ssh-connection-modal.slint:579`

但这些 trailing action 区域仍在 `DialogTextField` 外层整块 `TouchArea` 的覆盖范围内，因此会出现：

- 点击图形或文字区域时优先触发 field overlay；
- field 被 focus，但 trailing action 不一定收到点击；
- 用户看到按钮，却无法稳定触发显示/隐藏逻辑。

所以密码 reveal 问题不是 SSH 业务状态问题，而是共享 field 命中层问题。

### 3. modal 内部下拉框被裁切的根因

SSH modal 当前直接在 scrollable body 内使用 Slint 标准 `ComboBox`：

- `ui/components/assets-ssh-connection-modal.slint:276`
- `ui/components/assets-ssh-connection-modal.slint:345`
- `ui/components/assets-ssh-connection-modal.slint:477`
- `ui/components/assets-ssh-connection-modal.slint:614`

同时：

- modal body 是 `ScrollView`，定义在 `ui/components/modal-chrome.slint:614`
- modal host 使用 `BlockingModalShell`，其 modal frame 启用了裁剪，见 `ui/components/blocking-modal-shell.slint:105`

这意味着位于 modal 底部的下拉展开时，会同时受到：

- body scroll viewport 的边界约束；
- modal frame 的裁剪约束；
- Slint stock popup 在 bounded container 中的既有限制。

因此“底部展开只显示一部分”不是单个字段样式问题，而是当前 popup 架构与 bounded modal 的结构性不匹配。

## 目标

- 统一修复所有复用 `DialogTextField` 的编辑类 modal 的光标定位、拖选和 trailing action 命中问题；
- 将密码 reveal 改成 Fluent 风格眼睛图标；
- 使用持久切换的显示/隐藏交互；
- 为编辑类 modal 提供稳定的 select/dropdown 方案，避免底部裁切；
- 保持现有业务数据流、字段映射、保存逻辑和 view-model API 尽量不变；
- 将影响范围控制在 modal UI primitive、编辑类 modal 组件和必要的 host wiring 内。

## 非目标

- 本轮不重构 SSH runtime、资产持久化模型或验证逻辑；
- 本轮不重做所有 Slint popup 体系；
- 本轮不统一替换整个应用的全部 `ComboBox`，只覆盖编辑类 modal 内部使用场景；
- 本轮不引入新的全局 design system；
- 本轮不处理 terminal、context menu、titlebar menu 等非编辑类 overlay。

## 方案比较

### 方案 A：只修 `DialogTextField`，不改下拉体系

做法：

- 移除共享 field 顶层命中遮挡；
- 保留现有 modal 内 stock `ComboBox`。

优点：

- 改动最小；
- 立刻解决光标、拖选、密码按钮不可点击的问题。

缺点：

- modal 内下拉裁切问题仍然存在；
- 不能完整覆盖本轮用户诉求。

### 方案 B：共享 field 修正 + modal 专用 select overlay

做法：

- 在 `DialogTextField` 中移除会遮挡 `TextInput` 的整块 overlay；
- 将密码 trailing action 改成 icon toggle；
- 为编辑类 modal 新增共享的 modal 专用 select field，不再直接依赖 stock `ComboBox` 的 popup 展开；
- 在必要处增加 modal host overlay wiring，让下拉列表渲染在 modal body 之上，而不是 scroll content 之内。

优点：

- 一次性解决三类核心问题；
- 共享 primitive 修复收益可覆盖多个 modal；
- 架构上仍然是局部重构，风险可控；
- 更贴近 bounded dialog 里的成熟产品实践。

缺点：

- 比方案 A 多一层 overlay/select 控件实现；
- 需要补充键盘导航、点外关闭、焦点回收和定位逻辑。

### 方案 C：整套 modal 表单控件系统重做

做法：

- 输入框、密码框、下拉框全面换成新系统；
- 同时统一 hover/focus/error/help/tooltip 规范。

优点：

- 长期一致性最好；
- 后续扩展空间最大。

缺点：

- 改动范围明显过大；
- 不符合本轮以 bugfix + focused hardening 为主的目标；
- 回归面太大。

## 最终决策

采用方案 B：共享 `DialogTextField` 修正，加上编辑类 modal 专用的 select overlay。

核心原则：

- `TextInput` 自己负责真实文本区域的命中与交互；
- 密码 reveal 使用 icon toggle，不再使用文字 `Show/Hide`；
- 编辑类 modal 内的下拉不再直接依赖 stock `ComboBox` popup；
- modal 内部的 overlay 一律按 bounded dialog 的空间约束自行定位，上下翻转和高度裁切由本地控件负责。

## 交互设计

### 1. `DialogTextField` 命中与编辑行为

`DialogTextField` 应具备以下行为：

- 鼠标直接点击文本区域时，由 `TextInput` 原生处理插入点定位；
- 鼠标拖拽时，由 `TextInput` 原生处理选区；
- 点击 field padding 时可以聚焦输入框，但 padding hit target 不能覆盖真实文本区域；
- trailing action 区域必须完全可点击，不受 field overlay 抢占；
- multiline 字段与 single-line 字段使用同一命中原则。

具体约束：

- 删除或缩减 `ui/components/modal-chrome.slint:470` 对整块 field 的遮挡；
- 若保留 click-to-focus 辅助区域，只允许作用于非文本 padding 区域；
- 不允许 overlay 再盖住 trailing action 或 text editing viewport。

### 2. 密码 reveal 交互

密码 reveal 统一改为 Fluent 风格 icon toggle，使用眼睛/眼睛关闭图标。

交互规则：

- 默认隐藏；
- 点击一次切换为显示；
- 再点击一次切换为隐藏；
- icon 与状态同步切换；
- accessible label 必须同步变化，例如“显示密码”/“隐藏密码”；
- 键盘 Tab 可聚焦，Space/Enter 可切换。

安全边界：

- modal 初次打开时一律隐藏；
- 保存、取消、关闭 modal 时重置为隐藏；
- 切换认证源、认证方式、代理类型等导致字段重新构建时，也回到隐藏；
- 不跨 modal reopen 持久化显示状态。

### 3. 编辑类 modal 专用 select

新增共享 `DialogSelectField`（名称可在实现阶段最终确定），其视觉风格与 `DialogTextField` 对齐，但展开行为由本地 modal overlay 控制。

关闭状态：

- 显示 label、当前值、下拉箭头；
- focus、hover、invalid 状态与共享 field 风格一致。

展开状态：

- 列表渲染为 modal root 下的 sibling overlay，而不是 scroll content 子项；
- 若 anchor 下方空间足够，则向下展开；
- 若下方不足、上方足够，则向上展开；
- 若两边都不足，则选择空间更大的一侧，并把列表裁成可滚动高度；
- 点击外部关闭；
- `Esc` 关闭；
- 上下键移动高亮；
- `Enter` 提交；
- 关闭后焦点回到原 field。

### 4. 适用范围

本轮编辑类 modal 至少覆盖：

- SSH connection modal：`ui/components/assets-ssh-connection-modal.slint`
- snippet modal：`ui/components/assets-snippet-modal.slint`
- 以及其他复用 `DialogTextField` 的编辑 modal

其中下拉改造的重点对象包括：

- SSH 认证源
- keychain identity 选择
- proxy type
- upstream SSH connection
- snippet package

## 组件与架构设计

### 1. 共享 primitive

需要调整或新增的基础组件：

- 修改：`ui/components/modal-chrome.slint`
  - 修正 `DialogTextField`
  - 为 trailing icon action 提供复用能力
- 新增或扩展：
  - `DialogIconButton` 的 field-sized trailing 变体，或新的 `DialogFieldIconAction`
  - `DialogSelectField` / `ModalSelectField`

### 2. modal host overlay

因为 select dropdown 不能留在 `ScrollView` 内容层里，需要允许 modal host 承载一个局部 overlay。

可接受的架构方向：

- 在具体 modal 内提供 overlay layer；
- 或在 `BlockingModalShell` 内容层上方预留局部 overlay host；
- 或在 `ui/app-window.slint` 的 modal mount 处增加与当前 modal 同层的 select overlay。

本轮不要求做全局 popup framework，但必须保证编辑类 modal 的 select 列表不再被 scroll body 和 modal frame 错误裁切。

### 3. 数据流

保持现有 view-model 协议尽量不变：

- 文本字段继续走 `draft-changed(field, value)`；
- action 按钮继续走 `action-requested(action-id)`；
- select field 继续对外输出标准化 value/label 映射，不重构 SSH draft 模型。

## 错误处理与边界条件

### 1. 文本字段

- 空 helper/error 文案时不留多余布局缝隙；
- multiline 在高内容量下仍允许正确定位和滚动；
- password/icon action 在 disabled 状态不可点击但必须保留明确视觉反馈。

### 2. select 字段

- 无可选项时字段 disabled，并显示解释文案；
- 已选值失效时显示 stale value，同时标记无效；
- body scroll 发生变化时，若 dropdown 仍打开，应跟随更新 anchor 或直接关闭，避免悬浮错位。

### 3. 焦点与关闭

- modal 关闭后焦点回到 primary workspace；
- dropdown 关闭后焦点回到触发字段；
- 多个 select 不允许同时展开。

## 测试与验证范围

### UI 交互验证

- 点击已有文本中间位置，光标落点正确；
- 鼠标拖选已有文本成功；
- 双击/三击行为至少不比当前更差；
- trailing password icon 可点击；
- 键盘可聚焦 reveal icon 并切换状态；
- modal 底部 select 展开时完整可见，不再被底部裁切；
- 上下翻转和可滚动高度逻辑正确。

### 回归验证

至少覆盖以下页面：

- `ui/components/assets-ssh-connection-modal.slint`
- `ui/components/assets-rename-modal.slint`
- `ui/components/assets-folder-create-modal.slint`
- `ui/components/assets-keychain-identity-modal.slint`
- `ui/components/assets-keychain-ssh-key-modal.slint`
- `ui/components/assets-snippet-modal.slint`
- `ui/components/sync-vault-modal.slint`

## 风险与缓解

### 风险 1：共享 field 修改影响多个 modal

缓解：

- 先从 `DialogTextField` 层补 focused regression；
- 再逐个 spot-check 使用方；
- 不混入业务逻辑调整。

### 风险 2：自定义 select overlay 复杂度上升

缓解：

- 仅做编辑类 modal 所需的最小功能集；
- 不提前抽象成全应用 popup framework；
- 保持单选、字符串 model、简单 keyboard nav 即可。

### 风险 3：icon toggle 降低可理解性

缓解：

- 使用 Fluent 常见眼睛图标；
- 保留 accessible label；
- 必要时在 hover 状态提供 tooltip 或 status text。

## 参考依据

外部参考基于 2026-04-24 的检索结果：

- Slint `TouchArea` 文档说明启用的 `TouchArea` 会阻断下层交互：
  - https://slint.dev/latest/docs/slint/reference/gestures/toucharea
- Slint issue #9840 说明上层 `TouchArea` 事件穿透仍有限：
  - https://github.com/slint-ui/slint/issues/9840
- Slint issue #2375 说明 popup 在边界附近被裁切是已知问题：
  - https://github.com/slint-ui/slint/issues/2375
- Slint issue #1143 跟踪 popup/popup window 能力改进：
  - https://github.com/slint-ui/slint/issues/1143
- Microsoft PasswordBox 指南允许自定义 Hidden/Visible reveal 模式：
  - https://learn.microsoft.com/en-us/windows/apps/design/controls/password-box
- Fluent System Icons 官方仓库可作为眼睛图标来源：
  - https://github.com/microsoft/fluentui-system-icons

## 结论

本轮最优路径不是只修 SSH，也不是全面重做 modal 系统，而是：

- 在共享 `DialogTextField` 上修正命中结构；
- 用 Fluent icon toggle 统一密码 reveal；
- 为编辑类 modal 引入局部可控的 select overlay；
- 在不改变现有业务模型的前提下，统一提升所有编辑类 modal 的鼠标交互正确性和 dropdown 稳定性。

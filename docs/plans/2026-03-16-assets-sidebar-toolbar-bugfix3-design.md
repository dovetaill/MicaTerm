# Assets Sidebar Toolbar Bugfix3 Design

日期: 2026-03-16
执行者: Codex
状态: 方案已确认，待后续按设计实施

## 背景

当前 `AssetsSidebar` 顶部工具区的 Search 已经从早先的 root overlay 回收为 sidebar 内部的 inline search row，但在这次 `bugfix3` 收尾后仍有两个明确问题：

- 弹出的搜索框在未输入任何文字时，点击其他区域不会稳定收起。
- 暗色模式下，搜索框内文字仍呈现黑色，和当前输入底色几乎没有对比度。

结合最近提交链路，这两个问题本质上不是 Rust 状态模型缺失，而是 Slint 侧交互与主题 token 收口不完整：

- `2470516 fix: finalize assets sidebar toolbar overlays`
  Search 当时还是 root overlay，并依赖根层 dismiss layer。
- `9252165 fix: finalize assets sidebar toolbar bugfix2`
  Rust 侧明确建立了 `activate_asset_search()`、`close_asset_search()`、`collapse_asset_search_if_empty()` 三条语义。
- `e4a97f3 fix: finalize assets sidebar toolbar bugfix3`
  Search 被改回 inline row，但 root dismiss layer 只保留给 `Create` 菜单，导致 Search 的 click-away 只剩 `TextInput.has-focus` 这一条被动路径。

本轮设计只聚焦这两个 bug 的方案确认，不扩散到 terminal runtime、renderer、SSH/SFTP 或更大范围的 sidebar 重构。

## 目标

- 让空搜索框在点击外部区域时稳定收起。
- 保持现有 `collapse_asset_search_if_empty()` 语义，不把“点击外部”误改成“无条件强制关闭”。
- 修正暗色模式下 Search 输入文字的前景色，确保对比度可用。
- 优先复用现有主题 token 体系，避免在组件内部散落硬编码颜色。
- 不改变当前 `AppWindow -> Sidebar -> AssetsSidebar` 结构，不回退到 root overlay Search 方案。

## 边界

### 本文档覆盖

- Search inline row 的 click-away 触发策略
- Search 外部点击后的关闭语义选择
- 暗色模式下 Search 输入文字的前景色绑定策略
- 主题 token 应归属的层级
- 风险、回滚和验证重点

### 本文档不覆盖

- terminal widget / `wezterm-term` / `termwiz`
- SSH / SFTP / `russh` / `russh-sftp`
- renderer 路线与 `femtovg-wgpu` 配置
- `Create` 菜单布局问题
- 资产树真实过滤算法或数据模型

## 调研摘要

### 当前关键实现

- [ui/shell/assets-sidebar.slint](/home/wwwroot/mica-term/ui/shell/assets-sidebar.slint)
  Search 已经是 `search-row-host` 下的 inline `AssetsSearchPopover`。
- [ui/components/assets-search-popover.slint](/home/wwwroot/mica-term/ui/components/assets-search-popover.slint)
  Search 当前只依赖 `TextInput.changed has-focus` 触发 `collapse-requested()`，且没有显式绑定输入文字颜色。
- [ui/app-window.slint](/home/wwwroot/mica-term/ui/app-window.slint)
  `overlay-dismiss-layer` 当前只服务于 `asset-create-menu-open`，不再覆盖 Search。
- [src/shell/view_model.rs](/home/wwwroot/mica-term/src/shell/view_model.rs)
  已存在：
  - `activate_asset_search()`
  - `close_asset_search()`
  - `collapse_asset_search_if_empty()`

### 当前状态语义结论

- Search 的业务状态语义已经是正确的。
- 空 query 离开焦点时，应走 `collapse_asset_search_if_empty()`。
- 非空 query 离开焦点时，不应自动关闭。
- 只有显式关闭动作，例如 `Esc`，才应该走 `close_asset_search()`。

这一点已被以下测试锁定：

- [tests/assets_sidebar_toolbar_spec.rs](/home/wwwroot/mica-term/tests/assets_sidebar_toolbar_spec.rs)
- [tests/assets_sidebar_toolbar_smoke.rs](/home/wwwroot/mica-term/tests/assets_sidebar_toolbar_smoke.rs)

### 当前终端控件边界

当前主工作区仍是 `WelcomeView` 壳层，真实 terminal widget 尚未接入：

- [ui/app-window.slint](/home/wwwroot/mica-term/ui/app-window.slint)
- [src/main.rs](/home/wwwroot/mica-term/src/main.rs)

`src/main.rs` 已锁定 `winit + femtovg-wgpu` renderer，因此本轮决策可以明确限定在 sidebar 的 Slint 交互层，不影响终端核心和渲染主路径。

### 参考资料

- Slint `PopupWindow`
  <https://docs.slint.dev/latest/docs/slint/reference/window/popupwindow/>
- Slint `TouchArea`
  <https://docs.slint.dev/latest/docs/slint/reference/gestures/toucharea/>
- Slint `TextInput`
  <https://docs.slint.dev/latest/docs/slint/reference/std-widgets/views/textinput/>

## 设计点与方案对比

### 设计点 P1：Search 的点击外部由谁来感知

#### 方案 P1A：继续只依赖 `TextInput.has-focus`

描述：

- 不新增任何显式 click-away 路由。
- 继续依赖输入框失焦时触发 `collapse-requested()`。

优点：

- 改动最少。
- 不新增额外宿主层的交互逻辑。

缺点：

- 行为不稳定，当前 bug 正是这条路径失效的结果。
- 某些点击不会真正夺走 `TextInput` 焦点，空搜索框无法收起。

#### 方案 P1B：由明确宿主区域显式转发 click-away

描述：

- 保留 inline search row 结构。
- 在 Search 之外的明确可点击区域上，显式路由 `collapse-assets-search-requested()`。
- `TextInput.has-focus` 仍保留为兜底路径，但不再作为唯一依据。

优点：

- 行为最可控，和当前 inline row 结构兼容。
- 不需要把 Search 再退回 root overlay。
- 只影响 sidebar 交互层，不改业务状态模型。

缺点：

- 需要梳理哪些区域属于“Search 外部区域”。
- 需要避免误拦截 Search 自身点击。

#### 方案 P1C：把 Search 再退回 `PopupWindow`

描述：

- 重新改回 root overlay / popup 路线。
- 借助 popup 关闭策略处理 click-away。

优点：

- 外部点击关闭机制更直接。

缺点：

- 结构上回退，与这轮 inline row 收敛方向冲突。
- 会重新引入覆盖内容的风险。

最终选择：`P1B`

### 设计点 P2：点击外部后触发哪条关闭语义

#### 方案 P2A：点击外部只走 `collapse_asset_search_if_empty()`

描述：

- click-away 统一只触发 collapse 语义。
- 若 query 为空，则收起。
- 若 query 非空，则保持展开。
- `Esc` 或显式关闭动作才走 `close_asset_search()`。

优点：

- 与现有测试契约完全一致。
- 符合当前产品语义，不会让用户已输入的 query 因误触而直接消失。

缺点：

- 需要明确区分 collapse 和 close 的调用场景。

#### 方案 P2B：点击外部一律 `close_asset_search()`

优点：

- 实现最简单。
- 行为最直观。

缺点：

- 会破坏当前 `empty-only collapse` 语义。
- 与已有测试契约不一致。

#### 方案 P2C：按点击区域区分 collapse 与 close

描述：

- 点击 sidebar 内空白区只 collapse-if-empty。
- 点击主工作区则 force close。

优点：

- 可做更细粒度交互。

缺点：

- 规则复杂，成本高。
- 当前问题规模不足以支持这类复杂交互。

最终选择：`P2A`

### 设计点 P3：暗色模式下 Search 文字如何着色

#### 方案 P3A：显式绑定 `TextInput` 的前景色与相关视觉 token

描述：

- 给 `TextInput` 显式绑定输入文字颜色。
- 同时根据需要补齐 caret / selection 的主题绑定。

优点：

- 最稳定，不再依赖 backend 默认样式。
- 能直接修复当前暗色黑字问题。
- 跨 renderer 行为更可预测。

缺点：

- 需要补充少量输入控件的显式样式字段。

#### 方案 P3B：只调背景，不显式绑定文字颜色

优点：

- 改动更小。

缺点：

- 仍然依赖默认前景色，不稳定。
- 当前问题已经证明这条路线不可靠。

#### 方案 P3C：完全自绘输入文本层

优点：

- 视觉控制力最强。

缺点：

- 成本过高，超出本轮 bugfix 范围。

最终选择：`P3A`

### 设计点 P4：输入颜色 token 放在哪一层

#### 方案 P4A：优先复用全局 `ThemeTokens`

描述：

- 优先复用现有 `ThemeTokens.text-primary`。
- 仅在确有必要时，再最小化新增 input-specific token。

优点：

- 保持主题语义集中。
- 避免组件内部硬编码颜色。
- 与现有 shell 颜色系统一致。

缺点：

- 若后续输入控件需求增长，可能需要再扩 token 体系。

#### 方案 P4B：在 Search 组件内部硬编码 dark/light 颜色

优点：

- 实现最快。

缺点：

- 主题逻辑分叉。
- 后续类似输入控件容易出现颜色漂移。

#### 方案 P4C：现在就扩成完整 form-control token 体系

优点：

- 长期最规整。

缺点：

- 对当前 bugfix 来说过重。

最终选择：`P4A`

## 最终决策

本轮最终确认组合为：`P1B + P2A + P3A + P4A`。

### 决策解释

- Search 保持当前 inline row 结构，不回退 root overlay。
- click-away 由明确宿主区域显式转发，`TextInput` blur 只作为兜底。
- 外部点击只触发 `collapse_asset_search_if_empty()`，保持现有状态语义与测试契约。
- Search 输入文字颜色显式绑定到主题 token，不再依赖默认控件样式。
- 颜色优先复用 `ThemeTokens`，避免在组件内部写死 dark/light 值。

## 实施步骤

1. 明确 Search 的“外部区域”边界，限定为 Search 输入框之外、且确实属于当前 sidebar / shell 可点击宿主的区域。
2. 在这些明确宿主上显式路由 `collapse-assets-search-requested()`，形成稳定 click-away 路径。
3. 保留 `AssetsSearchPopover` 内部的 `changed has-focus` 逻辑作为 blur fallback，不再把它视为唯一关闭依据。
4. 保持 `Esc -> close-assets-search-requested()` 语义不变，不把 click-away 改成 force close。
5. 在 Search 输入控件上显式绑定文字前景色，并按需补齐 caret / selection 的主题绑定。
6. 优先复用 [ui/theme/tokens.slint](/home/wwwroot/mica-term/ui/theme/tokens.slint) 中现有 token；若字段不足，再做最小新增。
7. 更新 UI contract / smoke test，覆盖 click-away 与 dark-mode 文字颜色的契约。

## 风险与回滚

### 风险 1：click-away 宿主边界定义过宽

表现：

- 点击 Search 自身区域也被误判为“外部区域”。

缓解：

- 明确把 Search 本体排除在 click-away 宿主之外。
- 优先选择已有明确分层的宿主区域，而不是做全窗口粗暴覆盖。

回滚：

- 如首轮实现边界难以收敛，可先缩小到 sidebar 内部明确空白区域，再逐步扩展。

### 风险 2：只补文字颜色但遗漏 caret / selection 可见性

表现：

- 暗色模式下文本可见，但 caret 或选区对比仍不足。

缓解：

- 将输入控件相关视觉属性作为一组检查，而不是只修单个 `color`。

回滚：

- 若本轮先只修文本颜色，也必须在验证清单中显式记录 caret / selection 风险。

### 风险 3：组件内颜色硬编码再次回流

表现：

- Search 修好了，但后续其他输入控件继续各写各的颜色。

缓解：

- 明确要求优先复用 `ThemeTokens`。
- 如必须新增 token，也只在全局 token 文件增量扩展，不在组件本地写死颜色。

回滚：

- 若局部 token 扩展引发范围过大，可临时复用 `text-primary` 收口，后续再专门做 form-control token 设计。

## 验证清单

- [ ] Search 展开后，空 query 情况下点击外部区域会收起。
- [ ] Search 展开后，非空 query 情况下点击外部区域不会被误关闭。
- [ ] `Esc` 仍可无条件关闭 Search。
- [ ] Search 与 `Create` 菜单互斥关系不被破坏。
- [ ] 暗色模式下，Search 输入文字有足够对比度。
- [ ] 暗色模式下，caret 与选区可见性未退化。
- [ ] 浅色模式下，输入颜色不发生回归。
- [ ] 现有 `assets_sidebar_toolbar_spec` 与 `assets_sidebar_toolbar_smoke` 语义契约仍成立。


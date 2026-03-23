# Windows Console 资产列表样式优化 Design

日期: 2026-03-19
执行者: Codex
状态: 方案已确认，待按需进入 implementation plan

## 背景

当前 `Windows Console` 资产区已经具备以下壳层能力：

- Rust 真源的 `AssetTree + visible row projection`；
- `AppWindow -> Sidebar -> AssetsSidebar -> AssetNodeRow` 的 Slint 视图链路；
- toolbar、search、blank-area / item context menu、create modal 的基础桥接；
- `winit + femtovg-wgpu` 主线 renderer 下的桌面 shell 样式基线。

最近与本轮任务直接相关的提交主要集中在 `2026-03-17` 到 `2026-03-19`：

- `07692e3 2026-03-17 feat: add windows console assets context menu`
- `24a2aa8 2026-03-17 fix: complete console assets context menu bugfix`
- `a9becfc 2026-03-18 feat: finalize windows console assets context menu bugfix2`
- `7cd28c4 2026-03-18 feat: implement console assets explorer bugfix3`
- `0f22556 2026-03-19 feat: finalize assets explorer modal bugfix4`

用户本轮明确要求：

1. 资产树当前比例、密度与视觉质感都很差，需要明显向 `VS Code Explorer` 靠拢；
2. `New Folder` 与 `New SSH Connection` 两个 create modal 不应再出现中文；
3. 顶部 `Flat` 模式切换后，tree expand/collapse 控件不应“直接消失”；
4. 资产右键菜单偶发点击无反应，需要修复为稳定可触发；
5. 右键菜单 hover 高亮会闪烁且亮暗两套主题对比度都不足，需要系统性优化。

本轮任务不是 terminal runtime、`wezterm-term`、`termwiz`、`russh`、`russh-sftp`、持久化 schema 或 renderer strategy 重构，而是一次聚焦 `Assets Explorer / Toolbar / Context Menu / Modal` 的体验层重构。

## 调研结论

### 1. 当前“丑”的核心不是单一配色问题，而是比例与密度失衡

当前关键尺寸来自以下位置：

- 左侧总宽度为 `48px + 288px`：`ui/shell/sidebar.slint:68`
- `AssetsSidebar` 固定宽度 `288px`：`ui/shell/assets-sidebar.slint:71`
- header 高度 `44px`：`ui/shell/assets-sidebar.slint:82`
- tree row 高度 `36px`、flat row 在带 `path_hint` 时变为 `48px`：`ui/components/asset-node-row.slint:30`
- list host 外边距为 `16px`：`ui/shell/assets-sidebar.slint:285`
- row 缩进步长为 `14px`：`ui/components/asset-node-row.slint:25`

这会直接导致：

- activity rail 视觉上偏笨重；
- explorer pane 的有效内容宽度被两侧边距和较大的缩进吃掉；
- tree row、尤其是 flat row 纵向过胖；
- 文件夹与 SSH 项的比例更像“设置列表”，不像 Explorer。

### 2. Flat 模式下 expand/collapse 控件消失不是偶发 bug，而是当前产品定义如此

当前 toolbar descriptor 明确只在 `Tree` 模式显示 tree controls：

- `src/shell/sidebar.rs:116`
- `ui/shell/assets-sidebar.slint:124`
- `ui/shell/assets-sidebar.slint:125`

也就是说，“点击 Flat 后展开/收缩消失”是当前设计选择，而不是渲染异常。但从用户体验上，这会造成工具栏布局跳变与能力断裂感。

### 3. create modal 当前不仅标题是中文，而是整套 copy 仍是中文壳层

已确认中文文案覆盖：

- `ui/components/assets-folder-create-modal.slint:51`
- `ui/components/assets-folder-create-modal.slint:108`
- `ui/components/assets-folder-create-modal.slint:138`
- `ui/components/assets-ssh-connection-modal.slint:48`
- `ui/components/assets-ssh-connection-modal.slint:102`
- `ui/components/assets-ssh-connection-modal.slint:243`
- `ui/components/assets-ssh-connection-modal.slint:358`
- `ui/components/assets-ssh-connection-modal.slint:379`

因此本轮如果只改标题，不改 tab、按钮与辅助文案，会留下明显的中英混杂。

### 4. 右键菜单 hover 闪烁的根因是“hover 驱动整棵菜单模型重建”

当前链路如下：

- Slint overlay 在 hover 时启动延迟 timer：`ui/components/assets-context-menu-overlay.slint:115`
- 触发 `row-hovered(...)` 回调：`ui/components/assets-context-menu-overlay.slint:123`
- Rust 侧根据 hover path 更新 `context_menu_open_path`：`src/app/bootstrap.rs:996`
- 然后重新同步 menu placement 和三列 menu model：`src/app/bootstrap.rs:1002`、`src/app/bootstrap.rs:1003`

与此同时，row 自身 hover 视觉又直接依赖 `TouchArea.has-hover`：

- `ui/components/assets-context-menu-row.slint:11`
- `ui/components/assets-context-menu-row.slint:16`

因此一旦 hover 导致整棵 model 替换，原 row 实例的 hover 状态会丢失，视觉上就会表现为“闪一下又灭”。

### 5. 右键菜单偶发点击无反应，与当前 hover / open-path / action invoke 语义耦合过深有关

当前 click 与 submenu 逻辑是：

- 点击 action 后，Rust 会先查 `action_id` 对应的 node：`src/app/bootstrap.rs:955`
- 如果它有 children，就改 `open_path`；如果没有 children，才执行 leaf action：`src/app/bootstrap.rs:959` 到 `src/app/bootstrap.rs:964`
- leaf action 的真正业务分发在 `src/shell/view_model.rs:598`

这套机制本身可以工作，但在“hover 导致模型重建”的前提下，容易出现：

- pointer 刚进入 row，row hover 状态被重建；
- click 落点时机恰好与重建冲突；
- 用户感知成“点了没反应”。

### 6. submenu corridor 方案已经设计了一半，但没有真正接完

源码里已经有：

- `pointer-moved` 回调声明：`ui/app-window.slint:136`
- overlay 向上抛出 `pointer-moved(...)`：`ui/app-window.slint:495`
- corridor 算法：`src/shell/context_menu.rs:163`

但 Rust 侧没有实际接 `on_assets_context_menu_pointer_moved(...)` 的 handler，因此 corridor keep-open 只是“预留设计”，并未参与真实交互。这解释了为什么当前菜单在多列与 hover 过渡上稳定性不足。

### 7. 现有 hover / selected token 对比度偏保守，不足以承载 Explorer 菜单的状态提示

当前主要 token：

- `control-hover-surface`：`ui/theme/tokens.slint:14`
- `control-active-surface`：`ui/theme/tokens.slint:15`

这些 token 用在按钮时问题不大，但放到高频密集的 explorer row / context menu row 上，会导致：

- dark mode 下 hover 高亮不够“提起来”；
- light mode 下选中态与普通底色过近；
- 在菜单闪烁时，更难分辨当前 row 是否真实可点击。

### 8. 补充查阅 Slint 官方资料后的架构判断

本轮交互设计已补充参考 Slint 官方资料：

- `TouchArea`：可用于 hover / click / right-button 处理，禁用时事件可透传；
- `FocusScope`：只有拿到 focus 才能稳定承接键盘事件；
- `PopupWindow` 与 `ContextMenuArea` 适合原生 menu/popup 语义，但视觉和多列自绘可控性不如当前自绘 overlay。

参考：

- <https://docs.slint.dev/latest/docs/slint/reference/gestures/toucharea/>
- <https://docs.slint.dev/latest/docs/slint/reference/keyboard-input/focusscope/>
- <https://docs.slint.dev/latest/docs/slint/reference/window/popupwindow/>
- <https://docs.slint.dev/latest/docs/slint/reference/window/contextmenuarea/>

结论：本轮仍保持 `Rust state -> bootstrap bridge -> Slint root overlay` 路线，不切回 Slint 原生 menu 体系，而是在当前自绘体系内做一次更彻底的稳定化重构。

## 目标

### 本轮必须达成

- 资产树视觉、比例、密度显著向 `VS Code Explorer` 靠拢；
- activity rail 更利落，assets pane 有效内容宽度更充足；
- tree 与 flat 切换时 toolbar 布局不再突变；
- `New Folder` / `New SSH Connection` modal 全量英文；
- context menu hover 稳定、不闪烁、不丢点击；
- dark / light 两套主题下 hover 与 selected 状态都足够清晰；
- 变更范围严格限制在 explorer 壳层，不扩散到底层 terminal / SSH runtime。

### 体验目标

- 让 `Windows Console` 资产区更像“代码编辑器里的资源管理器”，少一点 demo tree / settings list 气质；
- 继续保持 Windows 11 Fluent 质感与项目既有方角风格；
- 在保留当前 Slint 自绘自由度的前提下，获得接近 `VS Code` 的视觉克制感与交互稳定性。

## 边界

### 本轮覆盖

- `AssetsSidebar` 的布局比例、header、list host、row 密度；
- `AssetNodeRow` 的 tree / flat 共用行契约；
- toolbar 的 `Tree / Flat / Expand / Collapse / Create` 交互语义；
- context menu overlay、row、column 的 hover / selected / invoke / submenu corridor；
- folder / ssh create modal 的英文 copy、一致性与 focus 体验；
- 亮暗主题下 explorer 层 token 调整。

### 本轮不覆盖

- `wezterm-term` / `termwiz` / custom terminal renderer；
- 真实 SSH 连接测试、隧道、代理、环境变量业务接入；
- `russh` / `russh-sftp` actor 与持久化 schema；
- Snippets / Keychain 的统一 Explorer 重构；
- 拖拽排序、拖拽移动、多选、框选；
- 移动端适配与平台特定交互特化。

## 方案对比与最终决策

## 设计点 1：Explorer 的比例与密度

### 方案 1A：按 `VS Code Explorer` 方向重做比例与密度

方向：

- 收紧 activity rail 的视觉占用；
- 适度放宽 explorer pane；
- 压低 header、row、icon 与 indent 的纵横比；
- flat row 不再使用当前的“胖双行卡片感”，而改成单行主信息 + 轻量路径提示。

优点：

- 能真正解决“高度和宽度配比难看”的主诉；
- 视觉风格最接近用户要求的 `VS Code`；
- 有机会一次性统一 tree / flat 的视觉语言。

缺点：

- 改动面最大；
- 需要联动 row hit-test、text truncation、icon 与 hint 布局。

### 方案 1B：只调现有尺寸，不重做行契约

优点：

- 改动较小；
- 不必大改 row 结构。

缺点：

- 大概率只是“稍微没那么丑”；
- 无法根治当前 row 的按钮感与 flat row 的肥厚感。

### 方案 1C：更激进的信息密度导向，不以 VS Code 为主参考

优点：

- 适合终端工具型产品；
- 信息密度最高。

缺点：

- 容易偏离用户明确要求的 `VS Code` 参考；
- 风险是显得过硬、过窄。

**最终选择：1A**

### 1A 的具体决策

- activity rail 继续保持极窄视觉语言，优先通过缩小内部 padding / button footprint 来变“瘦”，而不是继续放大整体宽度；
- explorer pane 适度放宽，并减少 list host 的左右空耗，让有效内容宽度上升；
- tree row 改为更致密的单行 Explorer row；
- flat row 仍为单行，但在右侧或尾部展示弱化的 path hint，而不是第二行说明；
- disclosure、kind icon、label、path hint 形成稳定的单行节奏，不再出现“图标和文字上下漂”。

## 设计点 2：create modal 的语言策略

### 方案 2A：create modal 全量英文化

优点：

- 最符合用户要求；
- 视觉与命名语气最统一；
- 避免 modal 内部中英混杂。

缺点：

- 需要改动 tab、按钮、提示文案等多处 copy；
- 后续如果做 i18n，还要再抽 key。

### 方案 2B：只改标题与主按钮，保留次级中文文案

优点：

- 改动最小。

缺点：

- 中英混杂会非常明显；
- 违背用户当前明确要求。

### 方案 2C：本轮同步抽 i18n key

优点：

- 后续多语言会更顺。

缺点：

- 明显超出本轮体验优化边界；
- 会把任务从 style / interaction refactor 扩成 copy infra 改造。

**最终选择：2A**

### 2A 的具体决策

- `New Folder` modal：标题、按钮、输入提示统一英文；
- `New SSH Connection` modal：tab、按钮、说明文字统一英文；
- `Test Connection` 作为未来业务动作仍保留英文外壳，但不在本轮补接真实逻辑；
- 本轮不引入 i18n 基础设施，只完成 copy 统一。

## 设计点 3：Flat 模式下 tree controls 的处理方式

### 方案 3A：保留控件位置，但在 Flat 模式改成 disabled / ghost 状态

优点：

- toolbar 布局稳定，不再“消失”；
- 用户理解成本低；
- 仅需调整 descriptor 与视觉状态，不必发明新业务能力。

缺点：

- Flat 模式下会出现一个不可用控件，需要 tooltip 解释语义。

### 方案 3B：Flat 模式下把控件改造成新能力

例如改成 `Reveal Hierarchy` 或 `Group by Folder`。

优点：

- 功能利用率更高；
- 更“聪明”。

缺点：

- 明显超出本轮范围；
- 会引入新的 projection 与行为设计。

### 方案 3C：保留空位但不显示控件

优点：

- 实现最省事。

缺点：

- 视觉上仍像残缺的 toolbar；
- 无法解决用户主诉。

**最终选择：3A**

### 3A 的具体决策

- expand / collapse 按钮的空间位置保持稳定；
- Flat 模式下不再 `visible: false`，改为保留位置但 `enabled: false`；
- 视觉上使用 ghost / muted 状态，不抢主焦点；
- tooltip 明确为类似 `Switch to Tree View to expand folders` 的解释语义；
- 不在本轮给 Flat 模式增加新业务动作。

## 设计点 4：context menu 的交互技术路线

### 方案 4A：保留当前自绘 menu 架构，但拆掉“hover 即整模重建”的耦合

核心策略：

- 保留 `Rust state -> bootstrap -> Slint overlay`；
- leaf hover 只更新前端视觉态，不再每次 hover 都 round-trip Rust；
- 只有进入 submenu、改变 open path、真正 invoke action 时才同步 Rust 真源；
- 把预留的 `pointer-moved` 与 corridor keep-open 算法接完；
- submenu 过渡用稳定 open path，而不是用 hover 抖动驱动整列替换。

优点：

- 最符合当前代码架构；
- 保留 `VS Code` 风格自绘自由度；
- 直接命中“闪烁 + 偶发点不动”的根因。

缺点：

- 需要重构 overlay / bootstrap / open-path 语义；
- 测试面需要覆盖 hover、submenu、click、escape 等多路径。

### 方案 4B：改回 Slint 原生 `ContextMenuArea` / `Menu`

优点：

- 部分点击 / 关闭语义更原生；
- hover / submenu 可少写一部分自定义状态。

缺点：

- 多列、半自定义 VS Code 风格更难做；
- 与当前 overlay / placement 设计割裂较大。

### 方案 4C：继续自绘，但把所有 hover / selection 也全放进 Rust

优点：

- 单一真源绝对一致。

缺点：

- 高频 hover round-trip 会更重；
- 本质上会加剧现在的抖动问题，而不是缓解。

**最终选择：4A**

### 4A 的具体决策

- context menu 的“结构真源”仍在 Rust；
- context menu 的“瞬时 hover 视觉态”下沉到 Slint 组件层，本地维护；
- submenu open path 继续由 Rust 管理，但只在语义变化时更新；
- 完成 `pointer-moved` -> corridor 判断 -> submenu keep-open 这条链路；
- 减少因 hover 造成的 model 替换频率，避免 row 实例频繁重建。

## 设计点 5：context menu 的 hover / selected 视觉策略

### 方案 5A：按 `VS Code` 风格提高整行 hover / selected 对比度

优点：

- 最符合用户要求；
- 与更致密的 Explorer row 更搭；
- 在亮暗主题下都更容易看清当前命中项。

缺点：

- 需要调整 token 与 row background 逻辑；
- 如果状态链不稳，会把闪烁显得更明显，因此必须与 `4A` 配套。

### 方案 5B：维持 Fluent 的克制 hover，只做轻度提亮

优点：

- 风格更偏系统原生；
- 改动保守。

缺点：

- 在高频菜单里辨识度不够；
- 不能满足用户对“高亮明显”的要求。

### 方案 5C：只调 token，不动 row 结构与状态来源

优点：

- 最省事。

缺点：

- 只能治标；
- 对闪烁根因无能为力。

**最终选择：5A**

### 5A 的具体决策

- menu row hover 改为更明确的整行 highlight；
- selected / open-submenu row 与普通 hover 使用不同强度层级；
- 亮色主题提高 hover 与背景的明度差，暗色主题提高表面层级差；
- 保持方角，不回退到圆角菜单视觉；
- divider、icon、label 与 chevron 的对比度重新配平，保证 submenu path 更清晰。

## 最终决策汇总

本轮正式采用以下组合：

- `1A`：按 `VS Code Explorer` 方向重做比例与密度；
- `2A`：create modal 全量英文化；
- `3A`：Flat 模式保留 tree controls 位置，但改为 disabled / ghost；
- `4A`：保留自绘 context menu 架构，拆掉 hover 即整模重建，并补完 pointer corridor；
- `5A`：按 `VS Code` 风格增强 menu hover / selected 对比度。

这是一轮聚焦 explorer 壳层的激进重构，但不是 terminal core 重构。

## 实施步骤

### Step 1：定义新的 Explorer 尺寸与行契约

- 收敛 activity rail、header、list host、row、indent、icon、path hint 的目标尺寸；
- 先统一 tree / flat 的单行 row 节奏，再决定 path hint 的位置与截断方式；
- 让 `AssetNodeRow` 同时适配 tree 与 flat，而不是继续依赖不同高度分支。

### Step 2：重做 toolbar 在 Tree / Flat 下的状态语义

- 调整 `toolbar_descriptor_for(...)` 的 tree controls 策略；
- Flat 模式保留 expand/collapse 槽位，但改为 disabled / ghost；
- tooltip 文案同步改为解释性语义。

### Step 3：重构 context menu 的 hover / open-path 责任边界

- 把瞬时 hover 视觉态从 Rust round-trip 中拆出来；
- 保留 Rust 对 action tree、open path、placement 的真源职责；
- 完成 `pointer-moved` 与 corridor keep-open 接线；
- 避免 hover 时整棵菜单 model 反复替换。

### Step 4：统一 context menu 的视觉层级

- menu column、row、divider、icon、chevron、planned / disabled 状态全部重配色；
- dark / light 两套主题分别校准；
- 确保 hover、submenu open、pressed、disabled 能一眼分清。

### Step 5：完成 create modal 的英文 copy 与焦点体验整理

- 统一 folder / ssh modal 的英文标题、tab、按钮、提示；
- 修正 modal 打开后的 focus 行为；
- 保证 outside click / escape / confirm / cancel 的体验一致。

### Step 6：回归验证 Explorer 关键路径

- Tree / Flat 视觉对比；
- 新建 Folder / SSH modal 英文化；
- blank-area / folder / ssh item 三类右键菜单命中；
- hover 不闪烁、leaf action 不丢失、submenu 过渡稳定；
- light / dark 主题下对比度达标。

## 风险与回滚

### 风险 1：Explorer 尺寸重做可能引发文本截断或命中区偏移

- 缓解：先定义统一 row contract，再做尺寸替换；
- 回滚：可单独回退 `AssetNodeRow` 与 `AssetsSidebar` 的尺寸提交，而不影响 tree model。

### 风险 2：context menu hover 责任拆分后，键盘导航与鼠标导航可能出现状态不一致

- 缓解：明确“结构真源在 Rust，瞬时 hover 在 Slint”的边界，并补测试覆盖；
- 回滚：可回退到当前 open-path 单真源模式，但保留 placement 与视觉改造。

### 风险 3：pointer corridor 接线后，菜单关闭时机可能过于宽松或过于敏感

- 缓解：用固定 corridor margin 和定时器阈值做受控调优；
- 回滚：可以仅关闭 corridor keep-open，保留其它稳定性修复。

### 风险 4：全量英文化可能与未来 i18n 方向重复劳动

- 缓解：本轮只统一 explorer 壳层 copy，不扩展到全局语言系统；
- 回滚：后续若引入 i18n，可将本轮英文 copy 直接抽成 key，不影响交互结构。

### 风险 5：工作区当前存在未提交的外部变更

- 已确认按用户要求忽略现有未提交变更，不主动处理；
- 本轮设计与后续实现只聚焦本任务相关文件，避免误碰无关内容。

## 回滚策略

如果实施后效果不满足预期，建议按以下粒度回滚，而不是整轮全部推翻：

1. 先保留 row 比例与 modal 英文化，仅回滚 context menu 交互重构；
2. 若 toolbar 语义争议较大，可单独回滚 Flat 模式下的 ghost tree controls；
3. 若新 token 在 light / dark 主题下不满意，可仅回滚 explorer token，不回滚交互结构。

## 验证清单

- [ ] activity rail 与 assets pane 的视觉比例更接近 `VS Code Explorer`
- [ ] tree row 明显比当前版本更致密，不再显得过高
- [ ] flat row 不再使用当前的双行肥厚布局
- [ ] `Flat` 模式下 expand / collapse 控件不再“消失”，而是保留为 disabled / ghost 状态
- [ ] `New Folder` modal 无中文残留
- [ ] `New SSH Connection` modal 无中文残留
- [ ] blank-area 右键菜单每次都能稳定触发
- [ ] folder item 右键菜单每次都能稳定触发
- [ ] ssh item 右键菜单每次都能稳定触发
- [ ] context menu hover 不再闪烁
- [ ] context menu 在 dark mode 下 hover / selected 对比清晰
- [ ] context menu 在 light mode 下 hover / selected 对比清晰
- [ ] submenu 过渡时 corridor 行为稳定，不会轻易误关闭
- [ ] 点击 leaf action 不再出现“点了没反应”的体感
- [ ] 本轮变更未扩散到 terminal core / SSH runtime / persistence 层

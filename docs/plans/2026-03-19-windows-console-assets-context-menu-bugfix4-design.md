# Windows Console 资产列表右键菜单 Bugfix4 Design

日期: 2026-03-19
执行者: Codex
状态: 方案已确认，待按需进入 implementation plan

## 背景

当前 `Windows Console` 资产区已经具备以下壳层能力：

- `AssetTree + visible row projection` 的 Rust 真源；
- toolbar、blank-area / item context menu、inline rename 的基础桥接；
- `AppWindow -> Sidebar -> AssetsSidebar -> AssetNodeRow` 的 Slint 结构；
- `winit + femtovg-wgpu` 的当前主线 renderer。

最近相关提交集中在 `2026-03-17` 到 `2026-03-18`：

- `07692e3 2026-03-17 feat: add windows console assets context menu`
- `24a2aa8 2026-03-17 fix: complete console assets context menu bugfix`
- `a9becfc 2026-03-18 feat: finalize windows console assets context menu bugfix2`
- `7cd28c4 2026-03-18 feat: implement console assets explorer bugfix3`

本轮任务不是 terminal runtime、`wezterm-term`、`termwiz`、`russh`、`russh-sftp`、持久化 schema 或 renderer strategy 重构，而是在现有架构上继续收敛 `Windows Console` 资产区的创建体验、树形视觉与交互语义。

用户本轮明确要求：

1. 新建目录或 SSH 连接后，不再使用列表内异常 rename 条，而是改成单独弹框；
2. 当前树形视觉太丑，需要明显向 VS Code Explorer 靠拢；
3. 当前展开 / 收缩按钮点击无效，需要修复为标准 Explorer 交互；
4. 顶部 `Flat` 模式应去除目录显示，只显示 SSH 连接。

## 调研结论

### 1. 当前主工作区仍是壳层，不存在真实 terminal widget

- `src/main.rs` 已将正式链锁定为 `winit + femtovg-wgpu`；
- `ui/app-window.slint` 当前主内容仍挂载 `WelcomeView`；
- `ui/welcome/welcome-view.slint` 仅是欢迎页占位。

因此本轮应严格收敛在 `Assets Explorer shell`，而不是扩散到 terminal core。

### 2. 新建异常的根因是“先插 placeholder，再 inline rename”

当前顶部创建与 context menu 创建都会直接在 `ShellViewModel` 中插入新节点，并立即进入 inline rename：

- `src/shell/view_model.rs:212`
- `src/shell/view_model.rs:417`

这解释了用户截图中的异常：新建目录或 SSH 后，会先在列表里出现一个半成品节点，再通过 inline rename 收名称，视觉和交互都不自然。

### 3. 树模型已有基础，但 UI 仍是原型级

Rust 侧当前已经具备 canonical tree 与 visible rows：

- `src/shell/assets.rs:128`
- `src/shell/assets.rs:252`

但 UI 侧仍然是原型级 explorer：

- `ui/shell/assets-sidebar.slint:256` 仍用 `VerticalLayout + for` 直接线性渲染；
- `ui/components/asset-node-row.slint:52` 仍使用文本 `v` / `>` 作为 disclosure；
- `ui/components/asset-node-row.slint:33` 仍带有偏按钮式 row 视觉。

这就是“数据上已经像树，观感上仍不像 Explorer”的核心原因。

### 4. 展开 / 收缩逻辑大概率不是 Rust 断链，而是 hit-test 冲突

当前回调链路本身是通的：

- `ui/app-window.slint:108`
- `src/app/bootstrap.rs:795`
- `src/shell/view_model.rs:310`

但 `AssetNodeRow` 内同时存在 chevron `TouchArea` 和整行 full-width `TouchArea`：

- `ui/components/asset-node-row.slint:58`
- `ui/components/asset-node-row.slint:126`

这类布局很容易发生整行命中层吞掉 disclosure 点击，因此本轮需要按 Explorer 语义重划命中区，而不是只补状态逻辑。

### 5. 当前 Flat 语义与用户目标不一致

现有 `Flat` 模式只是把全部节点扁平展开：

- `src/shell/assets.rs:305`
- `tests/assets_explorer_projection.rs:37`

这与“Flat 只显示 SSH 连接”的产品语义不一致，因此本轮必须明确改写 `Flat` 的投影定义，而不是继续沿用当前测试契约。

### 6. Slint 官方文档补充确认

本轮涉及的 UI 选择已补充查阅 Slint 官方文档：

- `ListView` 适合长列表，且只实例化可见项；
- `PopupWindow` 适合轻量 popup/menu；
- `Dialog` 更适合标准对话框按钮排布，但不适合当前自绘 shell 直接承接大型 SSH 编辑弹框。

参考：

- <https://docs.slint.dev/latest/docs/slint/reference/std-widgets/views/listview/>
- <https://docs.slint.dev/latest/docs/slint/reference/window/popupwindow/>
- <https://docs.slint.dev/latest/docs/slint/reference/window/dialog/>
- <https://docs.slint.dev/latest/docs/slint/reference/common>

结论：本轮应继续沿用 `AppWindow root overlay` 路线，自定义 modal host，而不是切到另一套原生窗口/弹出体系。

## 目标

### 本轮必须达成

- 新建目录改为单独 modal，不再通过 inline rename 收名称；
- 新建 SSH 连接改为单独大型 modal，表单结构参考用户提供的多 tab 截图；
- `Tree` 模式下的资产列表显著向 VS Code Explorer 风格靠拢；
- 文件夹展开 / 收缩恢复为标准 Explorer 交互；
- `Flat` 模式重新定义为“只显示 SSH 连接”；
- `Flat` 列表中的 SSH 项仍保留足够上下文，避免丢失目录归属信息。

### 体验目标

- 保持 Windows 11 Fluent Design 质感、方角、克制阴影与丝滑交互；
- 让 `Windows Console` 资产区更像“编辑器里的资源管理器”，少一点“设置页列表”的按钮味；
- 明确分离 `Create` 与 `Rename` 两类交互，不再让用户感知到 placeholder 节点。

## 边界

### 本轮覆盖

- `Windows Console` 资产区的创建入口与 modal 宿主；
- `Tree` / `Flat` 投影语义；
- `AssetNodeRow` 的视觉与命中区契约；
- folder/ssh 的 Explorer 风格视觉；
- create / rename / context menu / selection / focus 的交互边界；
- `Flat` 模式的 path hint 设计。

### 本轮不覆盖

- 真实 SSH 连接建立、测试连接、runtime actor；
- `wezterm-term` / `termwiz` 接入；
- SFTP runtime；
- 持久化 schema、数据库迁移；
- 复杂多选、框选、拖拽排序；
- Snippets / Keychain 的树模型统一化。

## 方案对比与最终选择

### 设计点 A：新建弹框宿主形式

#### 方案 A1：统一 `AppWindow` 内部 custom modal host

小型 `New Folder` 和大型 `New SSH Connection` 都挂在 `ui/app-window.slint` 的 root overlay 层，由 Rust modal state 决定内容。

优点：

- 视觉语言统一；
- 与现有自绘 shell 最一致；
- 最容易做出 Fluent / VS Code 混合气质；
- 后续可继续承接编辑连接、批量编辑等大弹框。

缺点：

- 需要自行维护 focus trap、dismiss layer 与 overlay 互斥；
- 需要补 modal state 桥接。

#### 方案 A2：轻量 popup + 原生 dialog 混用

`New Folder` 用 `PopupWindow`，`New SSH Connection` 用 `Dialog` 或第二窗口。

优点：

- 部分行为由 Slint 托管；
- 小弹层场景实现较快。

缺点：

- 会形成两套弹层语义；
- 大型 SSH 编辑界面与当前 shell 风格不统一；
- 不利于后续统一 modal 体系。

**最终选择：A1**

### 设计点 B：新建节点何时真正写入树

#### 方案 B1：Save 后再插入节点

所有 create action 先只打开 modal；用户确认后再 `insert_root` / `insert_child`。

优点：

- 彻底消除 placeholder 节点与异常 rename 条；
- `Cancel` 语义最干净；
- create / rename 职责彻底分离。

缺点：

- 需要新增 modal draft state；
- 需要重新定义保存成功后的 select/focus 语义。

#### 方案 B2：先插 placeholder，再让 modal 驱动 draft

打开 modal 的同时先插一个未完成节点，`Cancel` 时回滚删除。

优点：

- 与现有 inline rename 行为迁移路径更短。

缺点：

- 状态更复杂；
- 仍可能出现闪烁、残留选中、回滚遗漏等边界问题；
- 不能彻底消除“半成品节点”的用户感知。

**最终选择：B1**

### 设计点 C：树形视觉宿主与列表渲染形式

#### 方案 C1：继续使用 `VerticalLayout + for`，只修 icon / spacing / hover

优点：

- 改动最小；
- 对现有模板侵入较低。

缺点：

- 长列表性能与虚拟化不足；
- 更像原型修补，不像正式 Explorer。

#### 方案 C2：切到 `ListView + Explorer row contract`

Rust 继续输出 visible rows，Slint 用 `ListView` 渲染单一 `AssetNodeRow` 契约。

优点：

- 更适合未来长资产树；
- 更像标准 Explorer；
- 便于统一 `Tree` / `Flat` 共享同一行模板。

缺点：

- 需要调整 row anchor、命中区和 context menu 定位；
- UI 改动面较大。

**最终选择：C2**

### 设计点 D：展开 / 收缩交互模型

#### 方案 D1：最小 hit-test 修复

仅修复 disclosure 区域点击不生效的问题，其他交互保持现状。

优点：

- 风险低；
- 修 bug 最快。

缺点：

- 交互模型仍不完整；
- 后续键盘导航、double click、row body selection 仍会返工。

#### 方案 D2：Explorer 交互版

- disclosure 专属命中区只负责 expand/collapse；
- row body 只负责 select/focus；
- folder row 支持 double click 切换展开；
- `Tree` 显示 expand/collapse all；`Flat` 隐藏该控件。

优点：

- 与 VS Code / Explorer 心智模型一致；
- 更利于后续扩展键盘导航；
- 一次性解决命中区职责不清的问题。

缺点：

- 需要重写 row hit-testing 结构；
- 需要同步更新 UI smoke 与交互回归测试。

**最终选择：D2**

### 设计点 E：`Flat` 模式最终语义

#### 方案 E1：保持现状，全部节点拍平

优点：

- 最省改动；
- 兼容现有测试。

缺点：

- 不符合用户要求；
- 目录仍会占据视觉注意力。

#### 方案 E2：只显示 SSH 连接，并给出 path hint

底层树不变，projection 仅输出 SSH 节点；每条 SSH 行补充弱化的 `folder breadcrumb / path hint`。

优点：

- 精准符合“Flat 只看连接”的产品定义；
- 不会丢失目录归属上下文；
- 视觉上更轻、更像连接列表。

缺点：

- 需要扩展 `VisibleAssetRow`；
- 需要同步调整搜索匹配逻辑和测试。

#### 方案 E3：只显示 SSH，但按目录分组显示 section

优点：

- 保留更多结构感；
- 适合连接非常多的情况。

缺点：

- 比真正 flat 更重；
- 会重新引入目录视觉层级，不符合这轮“去除目录显示”的要求。

**最终选择：E2**

## 最终决策

本轮确认采用以下组合方案：

- `A1` 统一 `AppWindow` 内部 custom modal host
- `B1` Save 后再插入节点
- `C2` `ListView + Explorer row contract`
- `D2` Explorer 交互版展开 / 收缩模型
- `E2` `Flat = 只显示 SSH + path hint`

## 最终设计

### 1. 总体架构

保持 `Rust state -> bootstrap bridge -> Slint overlay` 主链不变：

- Rust 继续持有 `AssetTree`、selection、focus、rename、context menu、modal draft state；
- Slint 继续只承担展示、命中与动画；
- modal 以 `AppWindow` 顶层 overlay 实现，不新开额外窗口。

### 2. 新建流程

#### `New Folder`

- 触发来源：toolbar `+`、blank-area context menu、folder context menu；
- 打开方式：统一进入小型 modal；
- 表单字段：仅 `name`；
- 校验：空值 / 全空白时 `确定` disabled；
- 保存：确认后才真正 `insert_root` 或 `insert_child`；
- 关闭：`取消`、`Esc`、点击遮罩可关闭。

#### `New SSH Connection`

- 触发来源：toolbar `+`、blank-area context menu、folder context menu；
- 打开方式：统一进入大型 modal；
- 结构：`标准` / `隧道` / `代理` / `环境变量` / `高级` tabs；
- 本轮只做壳层和字段契约，不接真实网络测试；
- 保存：表单校验通过后再创建 SSH 节点；
- 关闭：右上角关闭、`取消`、`Esc`；不建议点击遮罩直接关闭。

### 3. modal state 建议

建议在 `ShellViewModel` 中新增独立 modal state，语义类似：

```text
None
NewFolder { parent_id, draft_name }
NewSshConnection {
  parent_id,
  active_tab,
  standard_draft,
  tunnel_draft,
  proxy_draft,
  env_draft,
  advanced_draft,
}
```

关键约束：

- modal 与 inline rename 绝不并存；
- 打开 modal 时，必须关闭 create popover、context menu，并结束当前 rename session；
- modal draft 永不直接污染 `AssetTree`。

### 4. row contract

`AssetNodeRow` 升级为统一 Explorer 行契约：

- disclosure slot
- kind icon slot
- title / rename area
- 可选 path hint
- selected / focused / renaming / view-mode 视觉态

`Tree` 与 `Flat` 共用同一组件：

- `Tree`：显示缩进、folder disclosure、folder open/close icon；
- `Flat`：隐藏 disclosure 与目录缩进，仅显示 SSH 行与 path hint。

### 5. 命中区与交互语义

按 `D2` 重划命中区：

- disclosure 命中区：只负责展开 / 收缩；
- row body 命中区：只负责选中 / 聚焦；
- right click：未选中则先选中，再打开菜单；
- folder double click：切换 expanded；
- inline rename：仅服务于现有节点的 `Rename` 动作，不再承接 create。

### 6. `Flat` 投影定义

`Flat` 模式改为“仅输出 SSH 节点”：

- folder 节点在 UI 中完全不显示；
- 每个 SSH 节点追加弱化的 `path hint`，如 `Infra / Prod`；
- 搜索时除连接名外，也可匹配 path hint；
- 顶部 `expand/collapse all` 控件在 `Flat` 下直接隐藏，而不是仅 disabled。

### 7. toolbar 语义

- `Tree`：显示 view toggle + expand/collapse all + create；
- `Flat`：显示 view toggle + create，不显示 expand/collapse all；
- toolbar `+` 仅作为 create 入口，不再直接创建 placeholder 节点。

### 8. context menu 语义

- blank area：`New Folder` / `New SSH Connection` -> root-level create modal；
- folder：`New Folder` / `New SSH Connection` -> child create modal；
- ssh：保留 `Rename` / `Delete` / 其他现有项，但 `New ...` 不再依赖 inline rename 作为后续步骤。

## 实施步骤（高层）

1. 在 `ShellViewModel` 中新增统一 modal state，并定义 create draft 生命周期；
2. 在 `AppWindow` 中增加 modal host overlay 和 dismiss / focus 规则；
3. 实现 `New Folder` 小型 modal；
4. 实现 `New SSH Connection` 大型 multi-tab modal 壳层；
5. 移除 create -> inline rename 这条旧链路，改为 save 后插节点；
6. 将 `AssetsSidebar` 的 console 列表迁移到 `ListView`；
7. 重写 `AssetNodeRow` 命中区与视觉契约；
8. 扩展 `VisibleAssetRow` 支持 `Flat` 模式的 `path_hint`；
9. 重写 `Flat` projection 与搜索匹配规则；
10. 更新 UI contract / projection / smoke / state regression 测试。

## 风险与回滚

### 风险 1：`ListView` 迁移影响 anchor 与 context menu 定位

风险：row absolute-position 计算方式变化后，context menu 锚点和 hover corridor 可能需要同步调整。

回滚策略：若锚点问题在短期内无法稳定，可先保留新 row contract 和 modal 体系，仅临时回退到 `VerticalLayout + for` 宿主，不回退数据模型与 `Flat` 语义。

### 风险 2：modal state 与 inline rename state 串线

风险：若 create modal 打开时没有强制结束 rename session，仍可能出现焦点抢占或残留编辑态。

回滚策略：严格执行“modal 与 rename 不并存”约束；若实现期发现边界复杂，优先暂时禁用 SSH 节点的 inline rename，而不是回退到 placeholder create。

### 风险 3：`Flat` 只显示 SSH 后，用户误以为目录被删除

风险：用户切换到 `Flat` 可能误解为目录丢失。

回滚策略：通过 tooltip、toggle 文案和 row `path hint` 明确 `Flat` 是连接视图，不改动底层树；若必要，再补轻量 secondary hint，但不引入 section header。

### 风险 4：SSH modal 视觉范围过大，拖慢本轮交付

风险：如果把 SSH 编辑弹框做得过满，容易扩散到真实业务表单和运行时校验。

回滚策略：本轮只交付结构、tab、字段布局和本地校验壳层；测试连接、真实连接保存、代理协议深逻辑全部留到后续独立任务。

## 验证清单

### 交互验证

- [ ] toolbar `+` 点击后，不再直接在列表插入 placeholder 节点；
- [ ] `New Folder` 打开小型 modal，输入合法名称后才写入树；
- [ ] `New SSH Connection` 打开大型 modal，保存后才写入树；
- [ ] `Cancel` / `Esc` 后不会残留半成品节点；
- [ ] folder disclosure 点击可稳定展开 / 收缩；
- [ ] folder row double click 可切换展开；
- [ ] right click row 时先选中再开菜单；
- [ ] modal 打开期间，背景列表不可交互。

### 视觉验证

- [ ] `Tree` 模式的行视觉明显更接近 VS Code Explorer；
- [ ] 不再使用文本 `v` / `>` 作为 disclosure；
- [ ] selection / hover / focus 层级清晰，不再有按钮味过重的 pill row；
- [ ] `Flat` 模式只显示 SSH 行，不显示目录节点；
- [ ] SSH 行在 `Flat` 下可显示清晰但克制的 path hint。

### 状态验证

- [ ] create modal draft 不会直接污染 `AssetTree`；
- [ ] modal 与 inline rename 不会并存；
- [ ] 切换 `Tree` / `Flat` 不会破坏底层树结构；
- [ ] `Flat` 搜索可以命中连接名和 path hint；
- [ ] `Tree` 搜索仍保持祖先链可见。

### 回归验证

- [ ] toolbar、search、create popover / modal、context menu 互斥关系正确；
- [ ] context menu anchor 与 hover corridor 在新列表宿主下仍正常；
- [ ] 现有 selection / focus / rename / search regression 不被破坏；
- [ ] 不扩散到 terminal runtime、SSH runtime、SFTP runtime、renderer 逻辑。

## 参考文件

- `src/main.rs`
- `src/app/bootstrap.rs`
- `src/shell/assets.rs`
- `src/shell/view_model.rs`
- `src/shell/sidebar.rs`
- `ui/app-window.slint`
- `ui/shell/assets-sidebar.slint`
- `ui/components/asset-node-row.slint`
- `tests/assets_explorer_projection.rs`
- `tests/assets_sidebar_toolbar_spec.rs`

## 参考资料

- Slint `ListView`: <https://docs.slint.dev/latest/docs/slint/reference/std-widgets/views/listview/>
- Slint `PopupWindow`: <https://docs.slint.dev/latest/docs/slint/reference/window/popupwindow/>
- Slint `Dialog`: <https://docs.slint.dev/latest/docs/slint/reference/window/dialog/>
- Slint Common Properties / `z`: <https://docs.slint.dev/latest/docs/slint/reference/common>

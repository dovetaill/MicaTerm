# Assets Snippets Design

日期: 2026-03-28
功能: `assets-snippets`
状态: 方案已确认，未进入实现

## 背景

当前项目已经完成 `Activity Bar + Assets Sidebar` 的双层左侧壳体，`Window Console` 资产树、SSH 连接表单、`redb` 本地持久化，以及 `VaultSnapshot` 同步主路径也已经落地。

但 `Snippets` 目前仍然只是一级导航占位：

- 导航入口已存在；
- toolbar 文案和 `new-snippet` 直连 action 占位已存在；
- 左侧面板只有静态文案，没有真实数据、表单、持久化或同步；
- 当前本地 catalog 与 vault asset schema 只覆盖 `Folder / SshConnection`。

本轮目标是在不进入实现阶段的前提下，确认 `assets` 模块下 `snippets` 子模块的架构边界、信息架构、交互习惯与数据落盘方向。

外部交互参考只用于校准习惯，不用于机械照搬：

- Termius 官方文档把 `Snippet` 定义为可复用 shell script，把 `Package` 定义为 snippet 集合，新增 snippet 的核心字段是 `Label`、可选 `Package`、`Script`。
- VS Code 官方 UX 指南建议层级数据用 `Tree View`，但不要把 tree item 设计成单击即触发命令的按钮。
- Microsoft `TreeView` 指南建议用缩进、右/下 `chevron`、folder/leaf icon 表达层级关系。

参考来源：

- <https://termius.com/documentation/snippets>
- <https://code.visualstudio.com/api/ux-guidelines/views>
- <https://learn.microsoft.com/en-us/windows/apps/design/controls/tree-view>

## 目标

本轮设计必须覆盖：

- 在 `Snippets` 一级模块下支持管理 `Snippet` 与 `Package`；
- `Snippet` 表单字段为 `name`、`script`、`package`，其中 `package` 可为空；
- `Package` 新建表单只包含名称；
- UI 继续使用当前 `Assets Sidebar` 的树形导航语言与 Fluent 图标体系；
- 交互习惯贴近大众桌面工具，不做激进创新；
- 本地持久化与 vault 同步路径纳入当前主体系，而不是另起完全独立的同步系统；
- 为后续 `Paste`、`Run`、批量运行、启动时执行等能力预留稳定扩展点。

## 非目标 / 边界

本轮不覆盖：

- 任何实现代码、测试代码或持久化迁移代码提交；
- 右侧详情面板的 snippet 专用编辑器；
- 语法高亮、变量模板、参数化执行、snippet marketplace；
- snippet 批量运行、多选、拖拽排序、收藏与最近使用；
- 团队共享 snippet 权限模型；
- snippet 执行引擎与 terminal runtime 的真实联动。

## 当前实现现状

### 1. 左侧导航与 `Snippets` 目前只是壳层占位

`Snippets` 已经作为一级导航存在，并且 toolbar descriptor 为它保留了 `new-snippet` 直连 create action，但并没有真实业务数据：

- [src/shell/sidebar.rs](/home/wwwroot/mica-term/src/shell/sidebar.rs)
- [ui/shell/assets-sidebar.slint](/home/wwwroot/mica-term/ui/shell/assets-sidebar.slint)
- [src/shell/view_model.rs](/home/wwwroot/mica-term/src/shell/view_model.rs)

`ui/shell/assets-sidebar.slint` 里的 `snippets` 面板目前只有占位文案，没有树投影、表单或上下文菜单承载。

### 2. 当前资产真相源仅服务 `Window Console`

`AssetTree` 是当前左侧 explorer 的 Rust 真相源，运行时类型只有：

- `Folder`
- `SshConnection`

对应文件：

- [src/shell/assets.rs](/home/wwwroot/mica-term/src/shell/assets.rs)

当前 `ShellViewModel` 中的真相字段也是 `console_asset_tree`，并不存在 `snippet_tree` 或通用多域 catalog 抽象：

- [src/shell/view_model.rs](/home/wwwroot/mica-term/src/shell/view_model.rs)

### 3. 当前持久化和 vault 同步主体系只支持 SSH 资产

本地 `redb` catalog 模型只支持：

- `PersistedAssetKind::Folder`
- `PersistedAssetKind::SshConnection`

对应文件：

- [src/app/assets_catalog/model.rs](/home/wwwroot/mica-term/src/app/assets_catalog/model.rs)
- [src/app/assets_catalog/redb_store.rs](/home/wwwroot/mica-term/src/app/assets_catalog/redb_store.rs)
- [src/app/assets_catalog/mapper.rs](/home/wwwroot/mica-term/src/app/assets_catalog/mapper.rs)

最新 `VaultSnapshot` 与 `VaultAssetCatalog` 也只支持 `Folder / SshConnection`：

- [src/app/vault/model.rs](/home/wwwroot/mica-term/src/app/vault/model.rs)

因此，如果要让 `Snippets` 进入当前主体系，就必须明确 schema 与 mapper 的扩展边界。

### 4. 当前 Git 演进过程表明 `Snippets` 是预留模块，不是半完成模块

关键历史：

- `fcc313d feat: implement sidebar navigation shell`
- `53c4b1d feat: implement assets sidebar toolbar shell`
- `75a4236 feat: finalize windows console assets explorer workflow`
- `d81b9ac feat: persist windows console assets catalog`
- `a075087 feat: add ssh vault sync workflow`

这条演进线说明：

- `Snippets` 先被放进导航 IA；
- 但当前真正做实的是 `Window Console`；
- 现在补 snippets，不能只做 UI 壳子，必须同步考虑 schema 与同步边界。

## 设计要点拆分

### 设计要点 1：`Snippets` 如何进入当前持久化 / vault 主体系

#### 方案 A：独立 `SnippetCatalog`，完全平行于当前资产体系

做法：

- 新建单独的 `SnippetCatalog`、`SnippetTree`、`SnippetRepository`；
- vault 中新增平行字段，不复用当前 `AssetCatalog`。

优点：

- 与当前 `console_asset_tree` 解耦最彻底；
- 不会污染现有 SSH 资产模型；
- 失败面更小。

缺点：

- 与用户已确认的“进入当前持久化 / vault 主体系”不一致；
- 后续需要维护两套 tree projection、mapper、repository 心智；
- assets 模块会出现同层重复基础设施。

#### 方案 B：扩展当前主体系，但保留模块级逻辑根

做法：

- 保留一个统一的持久化与 vault 主路径；
- 扩展当前 asset schema，使其可描述 snippets 域；
- 在逻辑上为 `console` 和 `snippets` 维护各自的隐藏根节点或等价域根；
- 视图层根据 `active_sidebar_destination` 过滤投影当前域的可见节点。

优点：

- 满足“进入当前主体系”的已确认方向；
- 仍然保留 `console` 与 `snippets` 的视图隔离；
- 后续同步、导入导出、vault snapshot 仍是一条主路径。

缺点：

- 需要扩展现有 schema、mapper、store 与 view model；
- 设计时必须非常清楚地定义“共享基础设施”和“模块私有语义”的边界。

#### 最终决策

选择方案 B。

收敛原则：

- 不新开第二套持久化/同步系统；
- 但也不把 `Snippets` 直接混成“console 资产的一种普通节点”；
- 使用统一主 schema + 模块级逻辑根的方式落地。

### 设计要点 2：`Package` 与 `Snippet` 的树形信息架构

#### 方案 A：`Package` 作为单层容器，`Snippet` 作为叶子；未分组 snippet 允许直接挂根

做法：

- `Package` 在 UI 上表现为 folder-like 容器；
- `Snippet` 为 leaf；
- `Package` 只允许一层，不支持 package 套 package；
- 未分组 snippet 直接显示在 `Snippets` 根层。

优点：

- 与用户给出的 `name / script / package` 表单语义一致；
- 与 Termius 的 package 概念接近；
- 树形足够表达结构，但不会过深。

缺点：

- 需要用类型约束阻止 package 多层嵌套；
- package 与普通 folder 不能完全等价。

#### 方案 B：把 `Package` 直接泛化为任意层 `Folder`

做法：

- snippets 直接复用 `Folder`；
- 所有层级规则都交给 tree 自由组织。

优点：

- 对现有 tree 基础设施最友好；
- 实现思路最直接。

缺点：

- 会偏离“package”这一业务语义；
- 用户可以构造过深层级，不符合本轮大众化目标；
- 后续很难保证“package 只是一层分类容器”。

#### 最终决策

选择方案 A。

明确规则：

- Package 只允许一层；
- `Package` 不能包含 `Package`；
- 根层允许同时存在 `Package` 与未分组 `Snippet`；
- `Package` 内允许存在多个 `Snippet`。

### 设计要点 3：树节点类型与类型约束

#### 方案 A：继续只保留 `Folder / SshConnection`，snippet package 也当作 `Folder`

优点：

- 最少的 schema 扩展；
- tree row 逻辑几乎不需要新分支。

缺点：

- 无法从领域层约束 `Package` 单层；
- snippets 模块会继承 console folder 的自由度；
- 后续 context menu、表单、校验都会被迫通过位置推断，而不是类型推断。

#### 方案 B：在现有主 schema 中显式新增 `SnippetPackage / Snippet`

优点：

- 领域边界清晰；
- context menu、表单、校验、投影都能按类型直接判定；
- package 单层约束可以落在模型层和 view model 层。

缺点：

- 要扩展 `PersistedAssetKind`、`VaultAssetKind` 与对应 payload；
- mapper、store、投影代码都会变长。

#### 最终决策

选择方案 B。

领域收敛建议：

- `Folder` 继续只服务 console 域；
- `SnippetPackage` 只服务 snippets 域；
- `Snippet` 作为独立叶子类型，payload 至少包含 `script` 与 `package_ref` 的等价信息；
- 不把 `SnippetPackage` 和 `Folder` 伪装成同一种业务类型。

### 设计要点 4：创建、编辑与激活动作

#### 方案 A：沿用当前 modal workflow，树项保守激活

做法：

- `New Snippet` 使用 modal，字段为 `name`、`package`、`script`；
- `New Package` 使用轻量 modal，只输入名称；
- 编辑也沿用 modal；
- 树项单击只选中；
- 双击默认 `Paste`；
- `Run` 作为显式操作保留给右键菜单或次级按钮。

优点：

- 与当前 `New Folder`、`New SSH Connection`、`Rename`、`Delete` 的 modal 体系一致；
- `Paste` 比 `Run` 更保守，误触成本更低；
- 符合 tree item 不应被设计成“直接触发命令按钮”的常见桌面习惯。

缺点：

- script 较长时，modal 的编辑体验不如大面板；
- 需要在后续实现里处理 multiline 文本框布局。

#### 方案 B：左侧只做列表，右侧或主区做 snippet 详情编辑

优点：

- 大脚本编辑体验更好；
- 未来可扩展预览、变量与执行历史。

缺点：

- 当前右侧面板已经承担 `Appearance / Sync & Vault`；
- 会改变现有壳层职责，超出本轮收敛边界；
- 首版复杂度明显偏高。

#### 最终决策

选择方案 A。

明确交互：

- 单击：选中；
- 双击：`Paste` 到当前活动终端；
- 显式动作：`Run`；
- 不允许把树项单击做成直接运行。

### 设计要点 5：toolbar 与创建入口

#### 方案 A：保持 snippets 只有 `New Snippet` 单一按钮

优点：

- 改动最小；
- 贴近当前占位实现。

缺点：

- `New Package` 入口发现性不足；
- 与 console 使用 create popover 的交互语言不统一。

#### 方案 B：snippets 也使用 create popover，包含 `New Snippet` 与 `New Package`

优点：

- 与 `Window Console` 的资产创建语言一致；
- package 和 snippet 的入口对称；
- 后续右键空白区菜单也更容易保持一致。

缺点：

- 需要把当前 snippets 的直连 create descriptor 改成 popover 语义。

#### 最终决策

选择方案 B。

收敛规则：

- snippets toolbar 也使用 create popover；
- 主按钮 tooltip 不再是单一 `New Snippet`，而是 snippets 资产创建入口；
- 空白区右键菜单与 toolbar create menu 的 IA 保持一致。

## 方案对比

本轮最终没有采用的方向有两类：

- 不采用“完全独立的 snippets 持久化 / vault 系统”
  - 原因：与已确认的主体系方向不一致，且会复制基础设施。
- 不采用“任意层 folder 泛化 snippets”
  - 原因：会把 package 语义做虚，偏离当前需求，也不利于保持大众习惯。
- 不采用“右侧详情面板作为首版编辑器”
  - 原因：会提前改动壳层职责，超出本轮边界。
- 不采用“树项双击默认 Run”
  - 原因：风险比 `Paste` 更高，不够保守。

## 最终决策

本轮确认后的最终方案如下：

- `Snippets` 进入当前持久化 / vault 主体系，不新开第二套同步系统；
- 主体系采用共享 schema，但在逻辑上保持 `console` 与 `snippets` 的域隔离；
- snippets 域显式新增 `SnippetPackage` 与 `Snippet` 类型；
- Package 只允许一层；
- package 不支持嵌套 package；
- 未分组 `Snippet` 允许直接挂在根层；
- snippets 继续复用现有 `Assets Sidebar`、tree row、toolbar、context menu 与 modal 视觉语言；
- snippets 的创建入口改为 create popover，包含 `New Snippet` 与 `New Package`；
- `Snippet` 表单字段为：
  - `name`
  - `script`
  - `package`
- `Package` 表单字段为：
  - `name`
- 树项默认行为为：
  - 单击选中
  - 双击 `Paste`
  - `Run` 为显式动作

## 实施步骤

这里只记录高层实施顺序，不展开实现计划细节。

1. 扩展领域模型与 schema
   - 为本地持久化和 vault snapshot 引入 snippets 域的类型与 payload。
2. 扩展运行时 view model
   - 让 `Snippets` 拥有真实的树投影、选择态、create/edit/delete 状态入口。
3. 统一左侧 explorer 投影
   - 在不破坏 console 资产流的前提下，为 snippets 面板接入 tree rows。
4. 新增 snippets modal 与 context menu IA
   - 包含 `New Snippet`、`New Package`、`Edit`、`Delete`、`Paste`、`Run`。
5. 接入本地存储与 vault 映射
   - 让 snippets 纳入当前 catalog load/save 与 vault snapshot 映射路径。
6. 补齐验证
   - 覆盖 schema、view model、Slint UI contract 与 smoke 行为。

## 风险与回滚策略

### 主要风险

- 把 snippets 拉进当前主 schema，会放大 `assets_catalog` 与 `vault` 迁移风险；
- 若类型边界定义不清，容易把 `console` 与 `snippets` 的 view model 逻辑缠在一起；
- 若直接把 `Package` 当 `Folder`，后续会失去单层约束；
- 若过早把 `Run` 设为默认激活动作，误执行风险会偏高。

### 风险缓解

- 领域层显式引入 `SnippetPackage / Snippet`，不要用位置或字符串推断类型；
- 保持 `console` 与 `snippets` 各自的逻辑根与投影路径；
- 首版坚持 `Paste` 为默认双击行为；
- 所有 snippets 交互都先通过现有 modal / context menu 模式落地，不提前扩展右侧编辑器。

### 回滚策略

- 若主 schema 扩展评估后发现风险过高，可回退到“共享 sidebar UI，但 snippets 先只做本地未同步 catalog”的过渡方案；
- 若 snippets tree 与 console tree 抽象耦合度过高，可在实现前改为“共享 tree engine，分离 catalog 根与 repository adapter”；
- 若 multiline modal 编辑体验明显不达标，可在后续增量迭代中再引入详情面板，而不是在首版设计中一次性切换架构。

## 验证清单

- [ ] `Snippets` 不再只是占位导航，而有真实数据域定义
- [ ] snippets 已纳入当前本地持久化与 vault 主路径设计
- [ ] Package 只允许一层
- [ ] 根层允许未分组 `Snippet`
- [ ] schema 能显式区分 `Folder`、`SshConnection`、`SnippetPackage`、`Snippet`
- [ ] snippets toolbar 使用 create popover，而不是单一直连按钮
- [ ] `New Snippet` 字段包含 `name / script / package`
- [ ] `New Package` 只要求名称
- [ ] 树项单击/双击/显式动作边界清晰
- [ ] 双击默认行为是 `Paste`，不是 `Run`
- [ ] 方案不会破坏现有 `Window Console` explorer 与 SSH runtime 主路径

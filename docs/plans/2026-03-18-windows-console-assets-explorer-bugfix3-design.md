# Windows Console Assets Explorer Bugfix3 Design

日期: 2026-03-18
执行者: Codex
状态: 方案已确认，待按需进入 implementation plan

## 背景

当前 `Windows Console` 资产区已经具备基础的 sidebar、toolbar、context menu 与 inline rename 壳层，但最近两轮 bugfix 后出现了新的交互回归，同时也暴露出更底层的模型问题。

本轮用户明确确认了以下目标：

1. 新建目录或 SSH 连接后，点击其他区域不应残留异常加深选中；
2. 顶部 `+` 按钮必须恢复为双入口创建，而不是只剩 `New SSH Connection`；
3. 资产项行本身必须显示图标，而不是纯文字；
4. 资产区需要向 `VS Code Explorer` 风格靠拢：空白处可创建根节点，文件夹内部也可创建目录或 SSH 连接，底层应是真正的树形结构，而不是继续堆补丁。

本轮不是 terminal runtime、SSH/SFTP actor、wezterm-term 集成或 renderer 重构，而是一次面向 `Assets Explorer` 的交互模型与数据模型校正。

## 调研结论

### 1. 当前 create 回归来自最近一次 toolbar 重构

最近相关提交链路如下：

- `07692e3 feat: add windows console assets context menu`
- `24a2aa8 fix: complete console assets context menu bugfix`
- `a9becfc feat: finalize windows console assets context menu bugfix2`

其中 `24a2aa8` 仍然保留顶部 `Create Popover`，而 `a9becfc` 删除了 `ui/components/assets-create-menu.slint`，并把 `Console` 面板的主创建动作改为固定 `new-ssh-connection`。

当前写死位置：

- `src/shell/sidebar.rs`
- `tests/assets_sidebar_toolbar_spec.rs`
- `tests/assets_sidebar_toolbar_smoke.rs`

结论：问题 2 不是视觉小问题，而是一次明确的行为回归。

### 2. 当前资产列表仍是扁平列表，不是真树

当前 `AssetsSidebar` 直接对 `console-asset-items` 做线性渲染：

- `ui/shell/assets-sidebar.slint`

Rust 侧当前真源也是简单的：

- `Vec<MockConsoleAssetItem>`
- `selected_asset_ids`
- `renaming_asset_id`
- `renaming_asset_text`

对应位置：

- `src/shell/view_model.rs`
- `src/shell/assets.rs`

当前并不存在：

- `parent_id`
- `depth`
- `expanded`
- `children`
- `visible_rows`

这意味着“文件夹内部创建”“展开/折叠”“像文件系统一样的树视图”目前都没有真正的数据基础。

### 3. 当前行模板只有文本，没有 Explorer 所需的结构

`AssetNodeRow` 当前只渲染：

- hover / selected 背景
- 文本 label
- rename `TextInput`

但没有：

- disclosure chevron
- kind icon
- indentation
- folder expanded / collapsed 状态

对应位置：

- `ui/components/asset-node-row.slint`

结论：问题 3 不能只补一个 icon，还需要把整行模板升级成 Explorer row contract。

### 4. 当前“点击空白后仍有选中高亮”是状态职责混乱导致的

当前新建资产后会立即：

- push 新项
- 设置 `selected_asset_ids`
- 进入 rename session

而空白点击只会触发：

- rename 提交
- 空搜索关闭

不会显式清掉 selection。

对应位置：

- `src/shell/view_model.rs`
- `src/app/bootstrap.rs`

结论：问题 1 的根因不是样式，而是 `selection / focus / editing / context target` 被混在一起管理。

### 5. 调研后的架构判断

当前项目的 terminal 主区域仍是 shell 占位与 welcome 内容，资产区仍然是独立壳层能力，因此本轮最合理的动作是先把 `Assets Explorer` 的状态模型与视图模型拉正，不牵扯 terminal core。

相关位置：

- `ui/welcome/welcome-view.slint`
- `Cargo.toml`

补充参考：

- Slint `ListView` 文档：<https://docs.slint.dev/latest/docs/slint/reference/std-widgets/views/listview/>
- Slint 通用 reference：<https://docs.slint.dev/latest/docs/slint/reference/common/>

本轮采用 `Rust tree as source of truth + Slint visible-row projection` 路线，而不是依赖不存在的内建 `TreeView`。

## 目标

### 本轮必须达成

- 修复空白点击后的残留选中问题；
- 恢复顶部 `+` 的双入口 create popover；
- 让资产行始终显示 kind icon；
- 把 `Window Console` 资产区升级为真实树形数据模型；
- 支持：
  - 空白区创建 root-level folder / ssh
  - folder 内创建 child folder / ssh
  - tree / flat 两种投影视图
  - inline rename、right-click menu、selection、focus 的稳定协作

### 体验目标

- 视觉风格接近 `VS Code Explorer`，但保持当前项目的 Fluent / Windows 11 语气；
- 保持英文菜单项与英文资产创建命名；
- 不再出现“功能上像树，数据上不是树”的临时补丁结构。

## 边界

### 本轮覆盖

- `Window Console` 资产区的 explorer 数据模型
- create popover
- asset row 模板
- selection / focus / rename / context target 状态拆分
- tree / flat 投影策略
- folder 内创建与 blank-area 创建

### 本轮不覆盖

- 真实 SSH 连接配置表单
- 真实 terminal session lifecycle
- SSH/SFTP actor / Tokio channel
- 持久化 schema 设计
- 拖拽排序 / 拖拽移动节点
- 多选、框选、批量操作
- Snippets / Keychain 的真实树模型

## 设计要点与方案对比

## 设计点 A：选中态与焦点态的状态拆分

### 方案 A1：最小修复版

保留现有 `selected_asset_ids` 主导一切，只在空白点击时补一个 `clear selection`。

优点：

- 改动面小；
- 能快速止住“残留选中”现象。

缺点：

- `selection / focus / editing / context target` 仍然耦合；
- 后续树导航、键盘导航、拖拽时会继续返工；
- 只能修表象，不能修状态职责边界。

### 方案 A2：Explorer 状态机版

把以下状态拆开：

- `focused_asset_id`
- `selected_asset_ids`
- `editing_asset_id`
- `editing_text`
- `context_target`

空白点击时：

- commit 或 cancel 当前 rename session
- clear selection
- clear focus

右键时：

- 更新 `context_target`
- 必要时同步 selection

优点：

- 状态职责清晰；
- 后续 tree、keyboard、drag/drop 都能承接；
- 与桌面端 Explorer 心智模型一致。

缺点：

- 设计与测试工作量更大；
- 需要同步更新 bootstrap 桥接与 UI contract。

**最终选择：A2**

## 设计点 B：顶部 `+` 的创建入口形态

### 方案 B1：恢复专用 Create Popover

重新引入 `AssetsCreateMenu`，点击顶部 `+` 弹出两项：

- `New Folder`
- `New SSH Connection`

两项都带图标，定位于 toolbar anchor。

优点：

- 完全符合用户对“回滚之前内容”的要求；
- 顶部 create 与右键 context menu 在语义上保持区分；
- 两项 create action 简单清晰，不需要复用复杂 submenu 逻辑。

缺点：

- create menu 与 context menu 有两套容器外观；
- 需要恢复之前删除的 overlay 状态链路。

### 方案 B2：复用现有 ContextMenuOverlay

顶部 `+` 直接使用当前 context menu overlay，只是锚定到 toolbar 按钮。

优点：

- 菜单容器可复用；
- 视觉资产数量更少。

缺点：

- 顶部 create menu 会显得过重；
- hover corridor / 多列语义对两项 create action 并不必要；
- 用户已经明确表达更偏好旧 create menu。

**最终选择：B1**

## 设计点 C：资产行模板

### 方案 C1：最小图标版

只在当前文本前补一个 kind icon。

优点：

- 快速修复“没有图标”的现象；
- 改动量最小。

缺点：

- 仍然不像 Explorer；
- 后续加树缩进与展开箭头时需要重写整行模板。

### 方案 C2：Explorer Row Contract

统一行结构为：

- disclosure chevron
- kind icon
- label / rename input
- optional right affordance area

并显式支持：

- `depth`
- `has_children`
- `expanded`
- `selected`
- `focused`
- `renaming`

优点：

- 一次性建立后续树形资产区的统一行契约；
- 视觉上更接近 VS Code Explorer；
- 便于 tree / flat 共用同一套 row 模板。

缺点：

- 需要配合树模型与投影层一起调整。

**最终选择：C2**

## 设计点 D：底层数据模型是否升级为真树

### 方案 D1：扁平列表补丁版

继续使用扁平 `Vec`，额外塞入 `parent_id / depth / expanded`。

优点：

- 初期迁移成本较低；
- 可以快速做出“看起来像树”的效果。

缺点：

- 数据语义别扭；
- child create、递归删除、搜索投影、拖拽移动都容易变脆；
- 后续必然再次重构。

### 方案 D2：Rust Canonical Tree + Visible Row Projection

Rust 侧维护 canonical tree，Slint 侧只接收 `visible_rows`：

- `tree` 模式按展开态投影
- `flat` 模式按扁平遍历投影
- `search` 对投影层过滤

优点：

- 结构稳定；
- 是 folder child create、tree expand/collapse、未来持久化的正确基础；
- 仍可保持 Slint UI 简洁。

缺点：

- 本轮设计与实现成本最高；
- 需要新增 tree mutation 与 projection 测试。

### 方案 D3：纯 Slint 递归树

让 Slint 组件自己递归 children。

优点：

- 声明式表达更直接。

缺点：

- 状态会分散在 Rust 与 Slint；
- 可测试性、可维护性、性能边界都更弱；
- 不适合当前项目把业务状态保留在 Rust 的方向。

**最终选择：D2**

## 最终决策

用户已确认本轮采用组合方案：

- `A2` Explorer 状态机版
- `B1` 恢复专用 Create Popover
- `C2` Explorer Row Contract
- `D2` Rust Canonical Tree + Visible Row Projection

这意味着本轮不再把问题当作单纯的“菜单 bugfix”，而是一次 `Assets Explorer shell` 的结构性校正。

## 最终设计

### 1. Rust 侧状态真源

新增 canonical tree 状态，替代当前扁平 `Vec<MockConsoleAssetItem>` 作为主要真源。

建议核心结构：

```text
AssetId = String

AssetNode {
  id: AssetId
  kind: Folder | SshConnection
  title: String
  parent_id: Option<AssetId>
  children: Vec<AssetId>
  expanded: bool
  mutable: bool
}

AssetExplorerState {
  nodes: BTreeMap<AssetId, AssetNode>
  root_ids: Vec<AssetId>
  focused_asset_id: Option<AssetId>
  selected_asset_ids: Vec<AssetId>
  editing_asset_id: Option<AssetId>
  editing_text: String
  context_target: ExplorerContextTarget
  asset_view_mode: Tree | Flat
  asset_search_query: String
}
```

说明：

- `nodes + root_ids` 比递归嵌套 struct 更适合 mutation 与投影；
- `selected_asset_ids` 当前仍保留数组形态，为未来多选留接口，但本轮默认单选；
- `focused_asset_id` 与 `selected_asset_ids` 分离，避免空白点击和右键菜单互相污染；
- `editing_asset_id` 与 `context_target` 独立，避免 rename 与 context menu 相互覆盖。

### 2. Slint 消费的可见行模型

Slint 不直接消费树，而是消费 Rust 投影后的 `VisibleAssetRow`：

```text
VisibleAssetRow {
  id: string
  kind: string
  label: string
  depth: int
  has_children: bool
  expanded: bool
  selected: bool
  focused: bool
  renaming: bool
  rename_text: string
}
```

这样可以保证：

- `tree` 模式与 `flat` 模式共用一个 row component；
- 搜索不需要改动底层树，只过滤 projection；
- UI 不承担业务树 mutation 责任。

### 3. Create Popover

恢复专用 `AssetsCreateMenu`：

- toolbar `+` 点击 -> open popover
- popover 项：
  - `New Folder`
  - `New SSH Connection`

行为定义：

- toolbar create 始终创建 root-level node；
- 如果当前有 rename session，先提交再打开 create popover；
- create popover 与右键 context menu 互斥；
- 点击外部关闭 popover。

### 4. Explorer Row 行模板

每个资产行统一结构：

- 左侧固定缩进区，宽度取决于 `depth`
- `chevron`
  - 仅 folder 且 `has_children == true` 时可见
- `kind icon`
  - folder -> folder / folder-open
  - ssh -> window-console
- `label / rename input`

交互定义：

- left click row
  - 设置 focus
  - 单选该项
- left click chevron
  - 只切换 folder expanded
- right click row
  - 如果未选中，则先单选
  - 打开 item context menu
- double click folder row
  - 可选：切换 expanded
- rename active
  - `Enter` -> commit
  - `Esc` -> cancel
  - click-away -> commit

### 5. Tree / Flat 模式语义

#### `Tree`

- 使用 canonical tree + expanded state 生成 visible rows；
- folder 可展开/折叠；
- 空白区创建 root node；
- folder 右键创建 child node。

#### `Flat`

- 不改变底层树；
- 仅改变 projection，把所有节点按稳定顺序扁平列出；
- 不显示 indentation 与 chevron；
- 仍保留 kind icon；
- child create 仍以选中的 folder 为上下文目标。

### 6. Context Menu 语义

#### Blank Area

- `New Folder`
- `New SSH Connection`

#### Folder

- `New Folder`
- `New SSH Connection`
- `Rename`
- `Delete`
- `Copy`
- `Cut`
- `Paste`（如未来需要）
- `Refresh`

关键语义：

- folder 场景下的 create action 默认写入当前 folder 下；
- blank-area 场景下的 create action 默认写入 root。

### 7. 命名与新建策略

命名策略继续保持英文，并扩展到树场景。

建议规则：

- root 与 child 节点均使用同类型最小缺失正整数补号；
- 默认名：
  - `Folder 1`
  - `SSH Connection 1`
- 作用域建议使用“同 parent 下唯一”，而不是全局唯一。

原因：

- 更接近文件系统目录语义；
- folder 内允许出现 `Folder 1`，root 也允许有自己的 `Folder 1`；
- 后续持久化与移动节点更自然。

## 实施步骤

1. `数据模型重构`
   - 引入 canonical tree 状态
   - 保留现有 mock 数据入口作为初始化桥接
2. `visible row projection`
   - 生成 `tree` / `flat` 可见行
   - 补充 depth / expanded / has_children
3. `状态拆分`
   - 拆出 `focused_asset_id`、`editing_asset_id`、`context_target`
   - 修复空白点击清理策略
4. `Create Popover 回归`
   - 恢复顶部 `+` 双入口
   - 与 context menu 互斥
5. `Explorer Row Contract`
   - 增加 chevron、kind icon、indentation
   - 保留 inline rename
6. `folder child create`
   - context menu create action 按 target folder 写入 child
7. `测试补齐`
   - view model
   - projection
   - bootstrap bridge
   - Slint UI contract

## 风险与回滚

### 主要风险

- 树模型与当前扁平桥接并存期间，容易出现投影与状态不同步；
- create / rename / context menu 三者互斥关系如果没定义清楚，容易再出现焦点残留；
- `flat` 模式若与 `tree` 模式共享状态不当，可能出现 selection 丢失或 row jump。

### 风险控制

- 以 Rust tree 为唯一真源，不允许 UI 自己维护 expanded / selection 真相；
- create popover、context menu、rename session 明确互斥；
- 先做 projection 测试，再接 UI。

### 回滚策略

如果 implementation 阶段发现树模型改动面超出预期，可临时回滚到以下止血组合：

- 恢复 `Create Popover`
- 增加 row icon
- 空白点击 clear selection

但该回滚只作为短期保底，不作为最终目标。

## 验证清单

### 状态层

- 新建 root folder 后自动进入 rename，点击空白区后提交并清掉 selection / focus；
- 新建 root ssh 后自动进入 rename；
- folder 右键创建 child folder / ssh 时，child 写入正确 parent；
- tree / flat 切换后 selection 不丢失；
- rename `Enter` / `Esc` / click-away` 行为一致。

### 投影层

- tree 模式下 depth、expanded、has_children 正确；
- flat 模式下不丢节点、不重复节点；
- 搜索仅过滤 visible rows，不破坏树结构。

### UI 层

- 顶部 `+` 弹出两个英文 create action，并带图标；
- asset row 始终显示 kind icon；
- folder row 在有 children 时显示 chevron；
- 空白区右键与 folder 右键的 create 目标不同；
- 点击空白区不再残留异常选中底色。

### 回归层

- 不影响现有 context menu overlay 布局与 tooltip；
- 不影响 `Snippets` / `Keychain` 当前占位壳层；
- 不影响主工作区 welcome / right panel / titlebar 现有行为。

## 后续文档建议

如进入实现阶段，建议补一份对应 implementation plan：

- `docs/plans/2026-03-18-windows-console-assets-explorer-bugfix3-implementation-plan.md`

该文档应继续细化：

- Rust data structure 迁移顺序
- UI contract 变更点
- 测试拆分策略
- 验证命令与回滚步骤

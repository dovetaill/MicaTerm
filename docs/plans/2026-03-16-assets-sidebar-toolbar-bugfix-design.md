# Assets Sidebar Toolbar Bugfix Design

日期: 2026-03-16
执行者: Codex
状态: 方案已确认，待实现

## 背景

`AssetsSidebar` 的顶部工具区已在 `2026-03-16` 首轮接入，但当前实现仍存在明显 UI 契约缺口：

- 顶部三个工具按钮未显示图标
- 左上标题仍为中文 `资产列表`
- `Create` 按钮没有采用 Fluent icon 体系
- `Create` 下拉菜单没有稳定地出现在按钮正下方
- 下拉菜单项只有文字，没有图标

本轮目标不是重做整套侧栏，而是在不触碰 terminal runtime、renderer 主路径和业务逻辑的前提下，完成一次聚焦于 shell UI 层的 bugfix 设计收敛。

## 调研结论

### 相关 Git 历史

- `53c4b1d feat: implement assets sidebar toolbar shell`
- `d7135b5 docs: add assets sidebar toolbar planning docs`
- `9266180 Stabilize Windows femtovg-wgpu mainline on DX12`

结论：

- 当前问题直接来自 `53c4b1d` 引入的首轮工具区壳层实现
- 当前主线运行时仍是 `winit + femtovg-wgpu + wgpu-28`
- Windows 路径已显式锁定 `DX12`，本轮不涉及 renderer 切换或 terminal widget 接入

关键代码位置：

- [ui/shell/assets-sidebar.slint](/home/wwwroot/mica-term/ui/shell/assets-sidebar.slint)
- [ui/components/assets-create-menu.slint](/home/wwwroot/mica-term/ui/components/assets-create-menu.slint)
- [ui/components/sidebar-toolbar-icon-button.slint](/home/wwwroot/mica-term/ui/components/sidebar-toolbar-icon-button.slint)
- [ui/app-window.slint](/home/wwwroot/mica-term/ui/app-window.slint)
- [src/app/bootstrap.rs](/home/wwwroot/mica-term/src/app/bootstrap.rs)
- [src/main.rs](/home/wwwroot/mica-term/src/main.rs)

### 现状证据

1. 顶部图标按钮组件本身已经支持 `icon-source` / `active-icon-source`，见 [ui/components/sidebar-toolbar-icon-button.slint](/home/wwwroot/mica-term/ui/components/sidebar-toolbar-icon-button.slint)。
2. 但 [ui/shell/assets-sidebar.slint](/home/wwwroot/mica-term/ui/shell/assets-sidebar.slint) 中的 `search-button`、`tree-expansion-button`、`view-mode-button` 目前均未绑定任何图标资源，因此按钮“完全不显示图标”是接线缺失，不是图标库不可用。
3. `Create` 菜单当前是嵌套在 `AssetsSidebar` 内部的 `PopupWindow`，直接使用 `create-button.absolute-position` 作为 `x/y` 锚点。
4. Slint 官方文档对 `absolute-position` 的定义指出它是相对 enclosing `Window` 或 `PopupWindow` 的绝对坐标，但参考系描述并不适合作为跨层级 popup 锚点的长期契约；结合当前截图，嵌套 popup 已出现偏移现象。

外部参考：

- Slint `PopupWindow`: <https://docs.slint.dev/latest/docs/slint/reference/window/popupwindow/>
- Slint `absolute-position`: <https://docs.slint.dev/latest/docs/slint/reference/common/>
- Windows Iconography: <https://learn.microsoft.com/en-us/windows/apps/design/iconography/>
- Fluent UI System Icons: <https://github.com/microsoft/fluentui-system-icons>

## 目标

- 修复 `AssetsSidebar` 顶部工具区按钮图标缺失
- 将左上标题切换为英文产品文案
- 让 `Create` 按钮与下拉项统一采用 Fluent icon 视觉语言
- 让 `Create` 菜单稳定锚定在按钮正下方
- 保持现有 `AppWindow -> Sidebar -> AssetsSidebar` 架构不变
- 为后续更多 sidebar overlay / menu 交互保留可扩展路径

## 边界

### 本文档覆盖

- 资产列表顶部工具区图标资源接入方式
- 左上标题英文命名
- `Create` 按钮视觉表达
- `Create` 下拉菜单锚点定位策略
- 菜单项图标与文字的布局约定
- 风险、回滚、验证标准

### 本文档不覆盖

- `wezterm-term` / `termwiz` / `russh` / `russh-sftp`
- 真实 terminal widget 或 SSH/SFTP 业务逻辑
- 资产树真实数据源、持久化、创建表单
- 去圆角重构
- 主题色板和全局动效重构

注：本轮明确选择 `F0`，即只修复工具区与菜单问题，不顺带处理圆角收敛。

## 设计点与方案对比

### 设计点 1：Toolbar 图标资源接入

#### 方案 A：继续使用 Slint 本地 `@image-url(...)` 绑定 Fluent SVG

优点：

- 与 titlebar、activity bar 现有图标接入方式完全一致
- 无新增运行时解析成本
- 改动面最小，便于快速验证

缺点：

- 图标声明主要位于 `.slint` 层，后续若要做运行时可配置图标，需要再扩展

#### 方案 B：由 Rust/view-model 提供 icon id 或 image handle

优点：

- 未来更容易做动态图标策略与平台差异化

缺点：

- 对当前 bugfix 来说过重
- 会显著放大状态与绑定复杂度

最终选择：`D1A`

### 设计点 2：左上标题英文文案

#### 方案 A：`Assets`

优点：

- 与 `AssetsSidebar` 命名一致
- 语义中性，能覆盖 hosts、recent sessions、favorites 等内容

缺点：

- 产品感偏工程术语

#### 方案 B：`Workspace`

优点：

- 更具桌面终端产品语气

缺点：

- 容易与主工作区语义冲突

#### 方案 C：`Explorer`

优点：

- 浏览区语义直接

缺点：

- 过于偏文件管理器，不完全贴合 SSH host / session 资产语义

最终选择：`D2A`

### 设计点 3：`Create` 按钮表达方式

#### 方案 A：单个复合按钮，布局为 `Add icon + Create + ChevronDown`

优点：

- 最符合当前需求
- 点击热区大，交互简单
- 视觉上接近 Fluent command button

缺点：

- 宽度会略大于当前纯文字按钮

#### 方案 B：split button

优点：

- 专业感更强
- 未来可挂默认动作

缺点：

- 当前没有明确默认动作
- 交互与实现复杂度明显上升

#### 方案 C：纯图标按钮

优点：

- 最省空间

缺点：

- 可发现性差
- 不适合作为当前首发版本的主创建入口

最终选择：`D3A`

### 设计点 4：`Create` 下拉菜单定位机制

#### 方案 A：继续在 `AssetsSidebar` 内部使用嵌套 `PopupWindow`，手动修正坐标

优点：

- 表面上实现最快

缺点：

- 依赖当前层级的坐标语义
- 对缩放、布局调整和未来 overlay 复用不稳
- 当前截图已经证明这一思路会产生偏移风险

#### 方案 B：将菜单提升到 `AppWindow` overlay / popup host，由根窗口统一锚点定位

优点：

- 锚点参考系最清晰
- 与现有 `tooltip-overlay` 的根窗口宿主模式一致
- 后续 titlebar menu、asset context menu 等可复用同一策略
- 跨平台可维护性最好

缺点：

- 需要额外穿透 anchor rect 或 popup state

#### 方案 C：放弃 `PopupWindow`，改为 `AssetsSidebar` 内部 absolute overlay

优点：

- 可以规避 `PopupWindow` 坐标歧义

缺点：

- click-away、层级、裁剪要自己承担
- 复用性不如根窗口 overlay

最终选择：`D4B`

### 设计点 5：菜单项图标 + 文本样式

#### 方案 A：静态自绘两项菜单，直接绑定 leading Fluent icon

建议项：

- `New Folder` -> `folder-20-regular.svg`
- `New SSH Connection` -> `window-console-20-regular.svg`

优点：

- 与当前两项固定动作完全匹配
- 视觉控制简单
- 改动范围最小

缺点：

- 后续动作数增加时还需要扩展结构

#### 方案 B：将菜单项抽象为 Rust 数据模型数组，Slint `for item in ...` 渲染

优点：

- 扩展性最好

缺点：

- 对当前两项菜单偏重

最终选择：`D5A`

## 最终决策

本轮确认的设计组合为：

- `D1A`: Toolbar 图标继续使用 Slint 本地 `@image-url(...)` 绑定 Fluent SVG
- `D2A`: 左上标题改为 `Assets`
- `D3A`: `Create` 使用单个复合按钮，布局为 `Add icon + Create + ChevronDown`
- `D4B`: `Create` 菜单提升到 `AppWindow` overlay / popup host
- `D5A`: 菜单项使用静态自绘 `leading icon + label`
- `F0`: 本轮不处理圆角收敛

## 视觉与交互约定

### Toolbar

- 左侧标题显示为 `Assets`
- 右侧保留 3 个 icon button：
  - Search
  - Tree expand / collapse
  - View mode toggle
- `Create` 为单个复合按钮，不拆分默认动作区

### `Create` 按钮

- 左侧使用 `Add` Fluent icon
- 右侧保留 `ChevronDown`
- 中间为 `Create` 文本
- 点击整个按钮均打开同一个下拉菜单

### `Create` 菜单

- 菜单锚点定义为按钮外框的底边
- 菜单默认出现在按钮正下方，保留小间距
- 菜单项统一为 `leading icon + label`
- 首轮仅保留两项：
  - `New Folder`
  - `New SSH Connection`

## 实施步骤

1. 在 `AssetsSidebar` 中补齐 toolbar 图标资源声明与按钮绑定。
2. 将 header 文案从 `资产列表` 切换为 `Assets`。
3. 将 `Create` 按钮重构为复合按钮视觉，但保留现有单击打开菜单的交互语义。
4. 将 `Create` 菜单从 `AssetsSidebar` 内部 popup 提升到 `AppWindow` 层级，改由根窗口管理锚点与开闭。
5. 为 `AssetsCreateMenu` 的菜单项增加 Fluent icon 槽位与统一布局。
6. 更新现有 UI contract smoke test 与必要的 Slint/Rust 状态同步测试。

## 风险与回滚

### 风险 1：Popup 提升到根窗口后，状态穿透链路变长

影响：

- `AssetsSidebar -> Sidebar -> AppWindow -> bootstrap` 的属性链需要新增 popup anchor 信息

缓解：

- 只提升菜单宿主，不改变现有 `asset_create_menu_open` 状态语义
- 保持 `assets-create-action-selected` 回调链不变

回滚：

- 若根窗口 overlay 在实现中出现意外阻塞，可临时退回 `AssetsSidebar` 内 absolute overlay，而不是退回当前嵌套 `PopupWindow + absolute-position` 写法

### 风险 2：图标选择与现有 Fluent 资源库存不完全对齐

影响：

- 若现有仓库缺少目标 SVG，需要补充资源文件

缓解：

- 优先复用仓库中已存在的 Fluent SVG
- 仅在缺失时再补新增资源

回滚：

- 若个别图标缺失，可暂时使用语义最接近的现有 Fluent icon，不引入新的图标技术栈

### 风险 3：复合按钮宽度增加后压缩 toolbar

影响：

- 在 `256px` 固定宽度下，header 水平空间会更紧

缓解：

- 保持 3 个 icon button 为紧凑尺寸
- 让标题区优先占用剩余空间，必要时压缩 `Create` 的左右内边距，而不是删除图标

回滚：

- 若实际验证表明宽度过紧，可在不改变 `D3A` 语义的前提下减小按钮 padding，而不是切换到 split button 或纯图标按钮

## 验证清单

- 顶部 3 个工具按钮均显示 Fluent icon
- 左上标题已改为 `Assets`
- `Create` 按钮显示 `Add icon + Create + ChevronDown`
- 点击 `Create` 后，菜单锚定在按钮正下方，而不是偏移到 sidebar 左侧
- 菜单项同时显示图标和文字
- `New Folder` 与 `New SSH Connection` 的动作回调保持原有语义不变
- 现有 `AppWindow -> Sidebar -> AssetsSidebar` 主结构不被改写
- 不引入新的图标技术栈，不切换 renderer，不触碰 terminal runtime

## 备注

本设计文档是对更宽范围 [2026-03-16-assets-sidebar-toolbar-design.md](/home/wwwroot/mica-term/docs/plans/2026-03-16-assets-sidebar-toolbar-design.md) 的 bugfix 收敛补充，后续实现阶段应以本文档的最终决策为准。

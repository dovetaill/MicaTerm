# Windows Console 资产列表右键菜单 Design

日期: 2026-03-17
执行者: Codex
状态: 方案已确认，待按需进入实现规划

## 背景

当前仓库已经完成自定义桌面壳层、标题栏、左侧 `Activity Bar + AssetsSidebar` 双层结构，以及 assets toolbar 的搜索与 create menu 原型，但 `Window Console` 面板中的资产列表仍处于占位阶段，尚未接入真实 `AssetNode`、SSH 连接树、文件夹树与终端会话模型。

本轮目标不是直接接入 `wezterm-term`、`termwiz`、`russh` 或 `russh-sftp`，而是在现有 `Rust + Slint + Tokio` 架构上，为 `Windows Console` 资产列表建立一套可持续扩展的右键菜单系统。首发重点是 Windows 11 Fluent 质感、方角、统一亮暗模式、自定义级联菜单与稳定的桌面交互语义，同时保证后续迁移到 macOS、Linux、Android 和 iOS 时，业务模型与菜单能力计算不需要推翻重做。

## 目标

- 为 `Window Console` 资产列表建立统一右键菜单架构，覆盖：
  - 空白区域
  - SSH 连接项
  - 文件夹项
- 保持项目当前的壳层架构：Rust 负责业务状态与能力裁决，Slint 负责根窗口 overlay 与视觉渲染。
- 菜单支持：
  - 固定标题 `操作`
  - 逻辑分组与分割线
  - `Icon + Text + Extra` 三段式行布局
  - 动态禁用态
  - 二级/三级级联菜单
  - 鼠标点定位与边界碰撞翻转
  - 第一版基础键盘闭环
- 第一版先定型完整菜单 IA，不要求所有动作都已经接通真实业务。

## 边界

### 本文档覆盖

- 右键菜单的宿主架构、状态模型与数据流
- 三类右击目标的菜单组合规则
- 菜单定位、碰撞处理、级联子菜单展开策略
- 禁用态、未接线动作与轻量反馈策略
- 第一版实施步骤、风险与验证边界

### 本文档不覆盖

- 真实 terminal widget 接入
- `wezterm-term` / `termwiz` / `russh` / `russh-sftp` 运行时逻辑
- 真实 SSH/SFTP 会话生命周期
- 数据库 schema 与持久化细节
- 完整桌面菜单键盘体验
- 真实资产树虚拟滚动与大数据量性能优化

## 调研摘要

### 代码现状

- `ui/app-window.slint` 已经具备根窗口级 overlay 宿主能力，现有 `assets-create-menu-overlay` 与 dismiss layer 说明项目已经验证过“根窗口统一浮层”的做法。
- `ui/shell/assets-sidebar.slint` 当前仍以占位文本表达 `Console Tree — Expanded` / `Console Flat List`，说明真实资产节点还未接入。
- `src/shell/view_model.rs` 已经沉淀出 `asset_search_expanded`、`asset_create_menu_open`、`asset_view_mode` 等壳层状态，符合“Rust 持有业务状态，Slint 消费渲染”的既有模式。
- `src/app/bootstrap.rs` 已经形成 `callback -> mutate ShellViewModel -> sync back into Slint` 的统一状态回写链路。

### 近期 Git 历史

- `53c4b1d feat: implement assets sidebar toolbar shell`
- `2470516 fix: finalize assets sidebar toolbar overlays`
- `9252165 fix: finalize assets sidebar toolbar bugfix2`
- `e4a97f3 fix: finalize assets sidebar toolbar bugfix3`
- `9266180 Stabilize Windows femtovg-wgpu mainline on DX12`

这些提交说明：

- 当前主线已经稳定到 `winit + femtovg-wgpu`，Windows 优先 DX12。
- assets toolbar 的搜索与 create menu 已经把“根窗口 overlay + Rust 状态同步”验证过一轮。
- 右键菜单应沿用现有模式，而不是在 Slint 内部再复制一套纯本地状态机。

### 终端控件边界

当前主工作区仍是占位壳层，真实 terminal surface 尚未接入。因此本轮的正确切入点是：

- 先补最小可右击资产节点壳层
- 先定型右键菜单能力体系
- 之后再把真实 SSH / 文件夹 / 会话数据逐步接回

### 官方能力确认

已核对 Slint 官方资料，确认以下能力可用于本轮方案：

- `ContextMenuArea` 可以在右键位置显示菜单，并支持 `show(Point)`。
- `Menu` 支持 submenu 与 separator。
- `PopupWindow` 支持自动关闭策略。
- `TouchArea` 可暴露鼠标坐标与 hover / pressed 信息。

参考：

- <https://docs.slint.dev/latest/docs/slint/reference/window/contextmenuarea/>
- <https://docs.slint.dev/latest/docs/slint/reference/window/popupwindow/>
- <https://docs.slint.dev/latest/docs/slint/reference/gestures/toucharea/>

结论：

- Slint 原生菜单在“能做”层面没有问题；
- 但如果目标是完全贴合当前项目的 Fluent 自绘视觉、标题栏、分组、extra slot、长文本撑宽与统一亮暗主题，继续采用自绘 root overlay 更稳。

## 方案对比与最终决策

| 设计点 | 备选方案 | 最终选择 | 选择理由 |
| --- | --- | --- | --- |
| 1. 首轮落地范围 | `1A` 只做菜单 infra / `1B` 补最小可右击节点壳层 | `1B` | 当前还没有真实资产节点，只做 infra 无法验证 SSH / 文件夹 / 空白区三套上下文。 |
| 2. 触发与宿主 | `2A` Slint 原生 `ContextMenuArea + Menu` / `2B` 自绘根层 overlay | `2B` | 与现有 `AppWindow` overlay 路线一致，视觉与交互可控性更高。 |
| 3. 状态归属 | `3A` Slint 本地状态 / `3B` Rust 统一状态 | `3B` | 与 `ShellViewModel` 既有模式一致，便于测试和后续扩展。 |
| 4. 子菜单形式 | `4A` Windows 级联菜单 / `4B` 同列替换或折叠展开 | `4A` | 更符合目标截图与桌面端直觉。 |
| 5. 菜单组合来源 | `5A` 按场景写死模板 / `5B` capability resolver | `5B` | 后续多选、批量、只读目录、连接态变化时返工最少。 |
| 6. 定位与碰撞 | `6A` 根窗口内手动 clamp / flip / `6B` 依赖原生菜单行为 | `6A` | 视觉与坐标体系统一，便于控制翻转与多级列定位。 |
| 7. 数据投影 | `7A` 直接把完整树交给 Slint / `7B` Rust 只投影当前可见列 / `7C` Slint 写死结构 | `7B` | 最适合根层多列级联与 Rust 统一状态机。 |
| 8. 右键选中语义 | `8A` 简化单项右击 / `8B` Explorer 风格 | `8B` | 更贴近桌面端资产管理器，利于后续多选与批量操作。 |
| 9. 级联宿主结构 | `9A` 每一级独立 overlay / `9B` 单一 overlay 内部多列 / `9C` 单列替换 | `9B` | `outside click`、`Esc`、z-order、hover path 更容易在一套坐标系里收口。 |
| 10. 子菜单展开时机 | `10A` 悬停立即展开 / `10B` 悬停延迟，点击立即展开 / `10C` 只点开 | `10B` | 降低误触发，保留桌面级联菜单效率。 |
| 11. 父子菜单容错 | `11A` 严格离开即关 / `11B` corridor 容错 / `11C` 简单延迟关闭 | `11B` | 追求更接近原生 Windows 的稳定体验。 |
| 12. 第一版键盘范围 | `12A` 基础闭环 / `12B` 完整桌面菜单键盘体验 / `12C` 仅 `Esc` | `12A` | 先完成核心菜单系统，完整版键盘体验放入 `TODO`。 |
| 13. 动作落地深度 | `13A` 完整 IA 先定型 / `13B` 只展示已实现动作 / `13C` 全部显示但多数灰掉 | `13A` | 先把信息架构和分组稳定下来，便于后续逐项接线。 |
| 14. 未接线动作反馈 | `14A` 一律灰掉 / `14B` 一律可点并提示 / `14C` 混合策略 | `14C` | 保留真正的上下文禁用态，同时允许已确认 IA 的未接线动作提供明确反馈。 |
| 15. 未接线动作在 UI 中是否显式标记 | `15A` 无标记 / `15B` 显示 `Planned` / `15C` UI 无标记，文档单独清单 | `15C` | 保持菜单纯净，项目层面通过独立 Markdown 文件追踪。 |

## 最终架构

### 1. 分层

#### `AssetNodeShell` / mock asset row 层

- 为 `Window Console` 面板补最小可右击节点壳层；
- 首轮只表达三类目标：
  - `blank-area`
  - `ssh-connection`
  - `folder`

#### Rust 侧状态层

新增独立上下文菜单状态，不直接塞进现有 create menu 或 search 状态中。建议按以下职责组织：

- `ContextTargetKind`
- `SelectionContext`
- `ContextMenuActionId`
- `ContextMenuActionNode`
- `ContextMenuColumnView`
- `ContextMenuOpenPath`
- `ContextMenuState`

Rust 保留完整动作树，Slint 只消费当前要显示的列视图。

#### capability resolver

根据以下输入生成完整动作树：

- `target kind`
- `selection context`
- `clipboard state`
- `mutability`
- `connection state`
- 后续可扩展字段，例如：
  - `has-active-tab`
  - `supports-bulk-edit`
  - `supports-proxy-tools`

resolver 需要输出三类状态：

- `enabled`
- `disabled because impossible now`
- `planned but not wired yet`

#### `ContextMenuOverlay`

- 根窗口只保留一个 overlay 宿主；
- overlay 内部按列渲染主菜单 / 二级菜单 / 三级菜单；
- 列数据完全来自 Rust 投影的 `visible columns`。

### 2. 数据流

1. 用户在空白区、SSH 节点或文件夹节点上右键。
2. 右键命中层先更新 `SelectionContext`，遵循 Explorer 风格语义。
3. Rust 记录触发点坐标与 `ContextTargetKind`。
4. capability resolver 生成完整动作树。
5. Rust 计算当前打开路径与第一列可见菜单数据。
6. `ContextMenuOverlay` 在根窗口中按鼠标点定位并做碰撞翻转。
7. 用户 hover / click 父项后，Rust 更新 submenu path，再投影新的可见列。
8. 用户执行动作时：
   - 已接线动作：走真实业务回调；
   - disabled 动作：不可点击；
   - 未接线动作：触发轻量反馈，并记录到日志/反馈通道。

## 视觉与交互契约

### 定位与展开

- 主菜单默认从右键点右下展开；
- 碰到右边界则向左翻转；
- 碰到底边则向上翻转；
- 子菜单默认向右展开，若右侧空间不足则翻转到左侧；
- 所有定位以根窗口可视区域为基准，而不是以单个 sidebar 为基准。

### 顶部结构

- 每个菜单顶部固定静态标题 `操作`；
- 标题下方有一条全局分割线；
- 子菜单顶部仍保留独立标题区，标题文字可按需求显示，例如：
  - `新建连接`

### 行布局

每一行遵循统一结构：

- 左：单色线性图标
- 中：左对齐文字
- 右：`extra slot`
  - submenu 箭头
  - 后续快捷键占位
  - 或留白

### 宽度

- 设定最小宽度；
- 若文案更长，则以内容撑宽；
- 文案保持单行，不折行；
- 典型长文本，例如 `上传SSH公钥(ssh-copy-id)`，必须直接撑宽菜单。

### 分组

按用户心智分组，不按实现状态分组：

- 新建组
- 窗口/标签组
- 编辑组
- 剪贴板组
- 高级工具组
- 刷新组
- 数据迁移组

### 状态语义

- 真正不可执行的动作走 disabled；
- 已确认 IA 但业务未接线的动作保持可点击，点击后给出明确反馈；
- 菜单 UI 本身不展示 `Planned` 文案或标记；
- 未接线动作统一在独立文档中追踪。

### 级联菜单

- hover 延迟展开；
- click 立即展开；
- 父项与子菜单之间使用 corridor 容错；
- 菜单关闭时优先保持当前打开路径稳定，避免闪烁。

### 键盘第一版

- `Esc`：关闭整套菜单
- `Enter`：触发当前项
- `Right`：进入子菜单
- `Left`：返回上一级

完整桌面菜单键盘体验纳入后续 `TODO`。

## 场景菜单定义

### 场景 1：空白区域

- 新建目录
- 新建连接
  - SSH
  - 本地终端
  - 串口
  - Telnet
  - SSH 隧道
- 批量打开
- 删除
- 重命名
- 复制
- 剪切
- 粘贴
- 刷新
- 导入
- 导出

### 场景 2：SSH 连接

- 关闭
- 新标签打开
- 新建目录
- 新建连接
  - SSH
  - 本地终端
  - 串口
  - Telnet
  - SSH 隧道
- 编辑
- 批量编辑
- 克隆
- 复制 Host
- 通过服务器代理 Chrome
- 上传SSH公钥(ssh-copy-id)
- 删除
- 重命名
- 刷新
- 导入
- 导出

### 场景 3：文件夹

- 新建目录
- 新建连接
  - SSH
  - 本地终端
  - 串口
  - Telnet
  - SSH 隧道
- 批量打开
- 删除
- 重命名
- 复制
- 剪切
- 粘贴
- 刷新
- 导入
- 导出

## 未接线动作策略

### 运行时策略

- `disabled because impossible now`
  - 例如：剪贴板为空时 `粘贴`
  - 例如：系统保护目录不允许 `删除`

- `planned but not wired yet`
  - 例如：`通过服务器代理 Chrome`
  - 例如：`上传SSH公钥(ssh-copy-id)`

这类动作允许点击，但只给出明确轻量反馈，不伪装成真实执行成功。

### 文档策略

单独维护：

- `docs/plans/2026-03-17-windows-console-assets-context-menu-unimplemented-actions.md`

用于记录：

- 未接线动作
- 所属场景
- 预期行为
- 后续依赖
- 接线优先级

## 实施步骤

1. 为 `Window Console` 面板补最小 `AssetNodeShell` / mock list，至少可区分空白区、SSH 节点、文件夹节点。
2. 在 Rust 中新增上下文菜单数据结构、动作树与 capability resolver。
3. 在 `AppWindow` 根层新增统一 `ContextMenuOverlay` 宿主，并支持多列渲染。
4. 接入右键命中、Explorer 风格选中语义、鼠标点定位与边界碰撞。
5. 接入 submenu path、hover 延迟、corridor 容错与基础键盘闭环。
6. 按三类场景完成菜单 IA、分组、disabled / planned 状态与轻量反馈。
7. 补齐 smoke / contract / state 验证，并将未接线动作同步到独立 Markdown 清单。

## 风险与回滚

### 风险

- corridor 容错与 submenu hover 状态机复杂，容易出现闪烁或错误关闭；
- 若把业务规则下沉到 Slint 组件，会导致后续难以维护；
- 当前资产树还是 mock 阶段，如果过早把菜单结构绑定到具体 UI 树形实现，后续真实数据接入时会返工；
- 第一版只做基础键盘闭环，若忘记记录完整键盘体验 `TODO`，后续容易被遗漏。

### 回滚策略

- 若 `11B corridor` 首轮稳定性不足，可临时降级为 `11C 延迟关闭`，但不改整体架构；
- 若某些 planned 动作在产品演示时造成误解，可临时改为 disabled，并保留独立文档追踪；
- 若多列 overlay 命中复杂度超预期，可先限制为最多二级菜单，保持 `9B` 根架构不变。

## 验证清单

### 交互验证

- 空白区、SSH 节点、文件夹节点三类目标的菜单组合正确；
- 右键选中语义符合 Explorer 风格；
- 主菜单和子菜单在边缘位置能正确翻转；
- submenu hover 延迟与 corridor 容错稳定，无明显闪烁；
- `Esc`、`Enter`、`Left`、`Right` 工作正常；
- outside click 能关闭整套菜单。

### 视觉验证

- 菜单标题、分割线、分组位置正确；
- 行布局保持 `Icon + Text + Extra`；
- 长文案单行撑宽，不折行；
- submenu 箭头位置与样式统一；
- 亮色/暗色主题下文字、图标、分割线与 disabled 态对比度正常。

### 状态验证

- disabled 动作不可点击；
- planned but not wired yet 动作可点击并返回明确反馈；
- 未接线动作与独立 Markdown 清单一致；
- capability resolver 输出与当前上下文一致。

## TODO

- 完整桌面菜单键盘体验（原设计点 `12B`），包括：
  - `Up / Down`
  - 首字符搜索跳转
  - 助记键
  - 快捷键提示位

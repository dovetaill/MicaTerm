# Assets Sidebar Toolbar Bugfix TDD Spec

日期: 2026-03-16
来源实现分支: `feature/assets-sidebar-toolbar-bugfix`
目的: 作为下一阶段 `test-driven-development` 的直接输入，覆盖 `AssetsSidebar` toolbar bugfix 的状态契约、UI 结构契约与边界情况。

## 1. 本轮实现摘要

本轮已完成以下 UI bugfix：

- `AssetsSidebar` 顶部标题从 `资产列表` 调整为 `Assets`
- Search / Tree / View 三个 toolbar button 均已绑定本地 Fluent SVG
- `Create` 按钮已调整为 `Add icon + Create + ChevronDown` 的单一点击目标
- `AssetsCreateMenu` 已从 `AssetsSidebar` 内部提升到 `AppWindow` 根层唯一 host
- `Create` 菜单项已改为 `leading icon + label`
- `AppWindow` 已导出 create-menu anchor 的根层 layout outputs

当前验证结果：

- `bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh` 通过
- `cargo test --test assets_sidebar_toolbar_smoke -- --nocapture` 通过
- `cargo test --test assets_sidebar_toolbar_spec -- --nocapture` 通过
- `cargo check --workspace` 通过
- `cargo clippy --workspace -- -D warnings` 通过

## 2. 关键结构与接口

### 2.1 Slint 组件

- `AppWindow`
  - 根窗口，当前持有唯一 `assets-create-menu-overlay := AssetsCreateMenu`
  - 导出根层 layout 输出：
    - `layout-assets-create-menu-anchor-x`
    - `layout-assets-create-menu-anchor-y`
    - `layout-assets-create-menu-anchor-width`
    - `layout-assets-create-menu-anchor-height`
- `Sidebar`
  - 作为 `AssetsSidebar` 与 `AppWindow` 之间的中继层
  - 透传 `create-menu-anchor-x/y/width/height`
- `AssetsSidebar`
  - 持有 toolbar 标题、Search / Tree / View 三个 icon button、复合 `Create` button
  - 负责暴露 `create-button.absolute-position` 对应的 anchor 输出
- `AssetsCreateMenu`
  - `PopupWindow`
  - 当前只保留两项动作：
    - `New Folder`
    - `New SSH Connection`
- `MenuActionItem`
  - 现在拥有 `in property <image> icon-source`
  - 每行布局为 `Image + Text`

### 2.2 Rust 结构与函数

- `AppWindow`
  - 由 Slint 生成 getter / setter / callback invoke 接口
- `ShellViewModel`
  - 仍是 toolbar 状态真源，关键字段：
    - `asset_view_mode`
    - `asset_search_expanded`
    - `assets_search_query`
    - `asset_create_menu_open`
    - `asset_tree_fully_expanded`
- `bind_top_status_bar_with_store`
  - 负责将 Slint callback 与 `ShellViewModel` 行为连接起来
- `sync_assets_toolbar_state`
  - 负责把 Rust 侧 toolbar 状态同步回窗口
- `AssetViewMode`
  - 保持 `Tree` / `Flat` 双态
- `AssetCreateAction`
  - 当前约定的动作 id：
    - `new-folder`
    - `new-ssh-connection`

### 2.3 Trait 状态

本轮没有新增 trait，也没有修改现有 trait 契约。下一阶段若要引入异步创建动作，不要为当前简单 UI 状态过早抽象 trait。

## 3. Slint Callback 契约

以下 callback 在下一阶段必须继续保持语义稳定：

- `toggle-assets-search-requested()`
- `assets-search-query-changed(string)`
- `collapse-assets-search-requested()`
- `toggle-assets-view-mode-requested()`
- `toggle-assets-tree-expansion-requested()`
- `toggle-assets-create-menu-requested()`
- `close-assets-create-menu-requested()`
- `assets-create-action-selected(string)`

关键约束：

- `toggle-assets-create-menu-requested()` 只负责开关 `asset_create_menu_open`
- `close-assets-create-menu-requested()` 必须能把根层 popup 状态收回
- `assets-create-action-selected(string)` 必须继续传递稳定 action id，不能把显示文案当作业务 id
- `Sidebar` 只做透传，不承担额外业务状态

## 4. 下一阶段优先测试点

### 4.1 UI 合约测试

- 验证 `AssetsSidebar` 标题固定为 `Assets`
- 验证 Search / Tree / View toolbar button 的 Fluent SVG 绑定路径
- 验证 `Create` 按钮必须包含：
  - `create-icon`
  - `create-label`
  - `create-chevron`
- 验证 `AssetsCreateMenu` 只能在 `AppWindow` 根层存在一个 host
- 验证 `MenuActionItem` 行结构包含 `icon-source`

### 4.2 Rust smoke / behavior tests

- 验证 `layout-assets-create-menu-anchor-width/height` 大于 `0`
- 验证 `toggle_assets_create_menu_requested` 后 `asset_create_menu_open == true`
- 验证 `close_assets_create_menu_requested` 后 `asset_create_menu_open == false`
- 验证 `AssetCreateAction` 的 id 仍为：
  - `new-folder`
  - `new-ssh-connection`
- 验证 `flat` 模式下 tree expansion 不被错误切换

### 4.3 建议新增的下一轮测试

- 增加针对根窗口 anchor 全量 getter 的断言：
  - `x >= 0`
  - `y >= 0`
  - `width > 0`
  - `height > 0`
- 增加 create-menu 行图标存在性的 UI contract
- 增加 popup close 行为测试：
  - 菜单项触发后，窗口状态应同步关闭
- 若后续引入 UI testing backend，可补充按钮点击后菜单锚点位置的可视化测试

## 5. 边界情况与风险

### 5.1 当前路径的真实风险

- 根层 popup 依赖 `create-button.absolute-position`
  - 若后续调整 sidebar 布局或把 `Create` 按钮包进新的容器，必须重新确认 anchor 输出仍指向按钮外框
- `asset_create_menu_open` 与 popup show/close 现在分布在 `AppWindow`
  - 若未来在其他层重复创建 `AssetsCreateMenu`，会重新引入双宿主状态错乱
- `flat` 模式下 tree expansion button 虽然禁用，但 view-model 保护仍不可删除
- 搜索框 collapse 逻辑依赖“query 是否为空”
  - 后续若引入 trim 或 debounce，需要重新验证空白字符串行为

### 5.2 Tokio / 并发注意点

当前 toolbar bugfix 路径没有引入新的 Tokio task、channel 或 actor mailbox，状态流仍是同步 UI callback -> Rust closure -> `ShellViewModel`。

但下一阶段若把 `assets-create-action-selected` 接到异步创建流程，必须遵守以下约束：

- 不要在异步任务里直接持有 Slint UI handle 做跨线程更新
- 回到 UI 线程时使用 `slint::invoke_from_event_loop`
- 不要跨 `await` 持有 `RefCell` / `Rc` 的活跃 borrow
- 若引入 `tokio::mpsc` 或 actor channel，必须定义 backpressure 行为，避免 UI 连点导致消息堆积
- 若动作执行失败，必须明确区分：
  - 关闭菜单
  - 执行业务动作失败
  这两个状态不能互相覆盖

### 5.3 数据竞争与一致性

- 当前实现没有新增共享可变跨线程状态，因此没有新的数据竞争面
- 未来若把 create action 接入后台任务，必须避免：
  - UI 状态已关闭但后台回调再次打开旧菜单
  - 旧 anchor 几何在窗口 resize 后被异步任务复用

## 6. 建议的 TDD 执行顺序

1. 从现有 smoke test 扩展根层 anchor getter 断言
2. 增加 `AssetsCreateMenu` action 触发后状态关闭的行为测试
3. 若引入异步动作，再单独新增 `invoke_from_event_loop` 回 UI 的线程安全测试
4. 最后才补充更细粒度的 UI 结构快照或 contract test

## 7. 参考文件

- `ui/shell/assets-sidebar.slint`
- `ui/shell/sidebar.slint`
- `ui/app-window.slint`
- `ui/components/assets-create-menu.slint`
- `src/app/bootstrap.rs`
- `tests/assets_sidebar_toolbar_ui_contract_smoke.sh`
- `tests/assets_sidebar_toolbar_smoke.rs`
- `tests/assets_sidebar_toolbar_spec.rs`

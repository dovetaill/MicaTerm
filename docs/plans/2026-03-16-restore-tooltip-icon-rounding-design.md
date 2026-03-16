# Restore Tooltip And Icon Button Rounding Design

日期: 2026-03-16
执行者: Codex

## 背景

上一轮 `remove-rounded-corners` 已把窗口外壳、共享组件与原生 Windows 顶层窗口统一收敛为方角，但用户确认有一个例外应保留：

- tooltip 提示可以继续使用圆角
- 纯图标按钮的轮廓可以继续使用圆角

这里的“纯图标按钮”指当前没有文字标签、仅由图标承载交互语义的按钮，尤其包括顶部状态栏中的：

- menu
- theme toggle
- right panel toggle
- always-on-top pin

以及 Assets 侧栏顶部工具栏中的纯图标按钮。

## 目标

- 恢复 tooltip 容器的圆角视觉
- 恢复纯图标按钮的圆角轮廓
- 保持窗口外层、面板、标签、输入框、菜单容器等其余区域继续为方角
- 收窄 smoke contract，使其允许这几个明确例外，但仍禁止其它非预期圆角回流

## 范围

### 恢复圆角

- `ui/components/titlebar-tooltip.slint`
- `ui/components/titlebar-icon-button.slint`
- `ui/components/sidebar-toolbar-icon-button.slint`

### 继续保持方角

- `ui/app-window.slint`
- `ui/components/sidebar-nav-button.slint`
- `ui/components/window-control-button.slint`
- 其它菜单、面板、标签、输入框与 shell 外层几何

## 方案对比

### 方案 A：只恢复 tooltip 圆角

优点：

- 改动最小

缺点：

- 顶部状态栏和 Assets 工具栏的纯图标按钮仍过硬
- 不符合用户对“图标按钮轮廓可圆角”的反馈

### 方案 B：恢复 tooltip + 纯图标按钮圆角

优点：

- 只恢复明确被用户点名的视觉例外
- 继续维持窗口壳与大部分组件的方角基线
- smoke contract 可以保持清晰边界

缺点：

- 需要把当前“全局全方角” contract 改成“有限例外” contract

### 方案 C：恢复所有按钮圆角

优点：

- 视觉回退更明显

缺点：

- 会重新放宽本轮刚建立的方角基线
- 容易把 `sidebar-nav-button`、window controls、菜单项等一起带回旧风格

最终选择：`方案 B`

## 设计结论

- `TitlebarTooltip` 恢复为 `8px` 圆角
- `TitlebarIconButton` 恢复为 `8px` 圆角
- `SidebarToolbarIconButton` 恢复为 `6px` 圆角
- `SidebarNavButton` 继续保持 `0px`
- `WindowControlButton` 继续保持 `0px`
- 更新 smoke contract：
  - 明确要求上述 3 个例外文件使用指定圆角值
  - 明确要求 `ui/` 其它文件不得再出现非零 `border-radius`

## 验证思路

- 先让 smoke contract 失败：
  - `tests/square_component_contract_smoke.sh`
  - `tests/top_status_bar_ui_contract_smoke.sh`
- 再做最小实现
- 最后执行：
  - `bash tests/square_component_contract_smoke.sh`
  - `bash tests/top_status_bar_ui_contract_smoke.sh`
  - `cargo check --workspace`
  - `cargo clippy --workspace -- -D warnings`

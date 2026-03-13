# Flatten Shell Chrome — TDD 交接规格

> 日期: 2026-03-13
> 分支: `feat/flatten-titlebar-right-panel-chrome`
> 前置计划: `docs/plans/2026-03-13-titlebar-right-panel-corner-implementation-plan.md`
> 前置设计: `docs/plans/2026-03-13-titlebar-right-panel-corner-design.md`

## 变更概述

本轮将 `shell-frame` 确立为窗口唯一外轮廓 owner，把 `Titlebar` 和 `RightPanel` 从独立 rounded card 收敛为 flat internal chrome / square docked pane，消除双边框与错误圆角。

## 核心 Struct / Trait / 组件接口变更

### Slint 组件

#### `Titlebar` (`ui/shell/titlebar.slint`)

| 属性 | 变更前 | 变更后 |
|------|--------|--------|
| `in property <bool> use-flat-window-chrome` | 存在，驱动 border-radius ternary | **已删除** |
| `border-radius` | `root.use-flat-window-chrome ? 0px : 12px` | `0px`（硬编码 flat） |
| `border-width` | `1px` | `0px` |
| `border-color` | `ThemeTokens.shell-stroke` | `transparent` |
| `out property <length> layout-border-width` | 不存在 | **新增**，绑定 `root.border-width` |

- 所有 callback、tooltip 逻辑、drag zone、window controls、maximize button geometry export **未变更**。

#### `RightPanel` (`ui/shell/right-panel.slint`)

| 属性 | 变更前 | 变更后 |
|------|--------|--------|
| `border-radius` | `14px` | `0px` |
| `border-width` | `1px` | `0px` |
| `border-color` | `ThemeTokens.shell-stroke` | `transparent` |
| `out property <length> layout-radius` | 不存在 | **新增**，绑定 `root.border-radius` |
| `out property <length> layout-border-width` | 不存在 | **新增**，绑定 `root.border-width` |
| `left-divider` | 不存在 | **新增** `Rectangle { width: 1px; background: ThemeTokens.shell-stroke; }` |

- `expanded` / width contract、`SegmentedControl` 布局 **未变更**。

#### `AppWindow` (`ui/app-window.slint`)

| 属性 | 变更前 | 变更后 |
|------|--------|--------|
| `content-column := Rectangle` | 无 clip/radius | **重命名为** `chrome-mask := Rectangle`，增加 `border-radius: parent.border-radius; clip: true;` |
| `titlebar` binding `use-flat-window-chrome` | 存在 | **已删除** |
| `out property layout-titlebar-border-width` | 不存在 | **新增** |
| `out property layout-right-panel-radius` | 不存在 | **新增** |
| `out property layout-right-panel-border-width` | 不存在 | **新增** |

- `shell-frame` 的 `border-radius: root.use-flat-window-chrome ? 0px : 14px` **未变更**（窗口级 outer geometry 保留）。
- `use-flat-window-chrome` 仅在 `AppWindow` 声明和 `shell-frame` 使用，不再传递给 `Titlebar`。

### Rust 侧

**无 Rust 源码变更**。以下保持不变：
- `src/app/bootstrap.rs` — `window.set_use_flat_window_chrome(...)` 调用链不变
- `src/shell/view_model.rs` — `WindowPlacementKind → WindowChromeMode` 映射不变
- `src/app/window_state.rs` — 窗口状态管理不变
- `src/app/windows_frame.rs` — Native frame adapter 不变

## 新增/修改的测试清单

### Rust 测试

| 测试文件 | 测试函数 | 断言要点 |
|----------|----------|----------|
| `window_geometry_spec.rs` | `shell_exports_internal_chrome_geometry_contracts` | titlebar border-width=0, right-panel radius=0, border-width=0 |
| `window_geometry_spec.rs` | `restored_window_keeps_rounded_shell_frame_but_flattens_titlebar` | shell-frame radius=14, titlebar radius=0, border-width=0 |
| `window_geometry_spec.rs` | `flat_window_chrome_flattens_shell_frame_without_reintroducing_titlebar_card` | shell-frame radius=0, titlebar radius=0, border-width=0 |
| `window_geometry_spec.rs` | `expanded_right_panel_is_flat_and_owns_no_full_card_border` | right-panel radius=0, border-width=0 |
| `top_status_bar_smoke.rs` | `maximize_toggle_only_changes_outer_shell_chrome` | flat/restored 切换时 titlebar 始终 flat |

### Shell Smoke 测试

| 脚本 | 新增断言 |
|------|----------|
| `top_status_bar_ui_contract_smoke.sh` | `chrome-mask` 存在、titlebar `0px` 几何、无旧 ternary binding、无 `use-flat-window-chrome` 输入 |
| `shell_layout_ui_contract_smoke.sh` | `left-divider` 存在、right-panel `0px` 几何、无旧 `14px` 圆角 |
| `windows_frame_contract_smoke.sh` | frame adapter 不引用 `layout-titlebar-radius` |

## 潜在边缘情况 (Edge Cases)

### 1. chrome-mask 裁剪精度

- `chrome-mask` 使用 `clip: true` + `border-radius: parent.border-radius` 约束内部元素
- **风险**: 不同 renderer（software vs femtovg-wgpu）在 clip boundary 的亚像素处理可能不同
- **验证建议**: 在 restored 态下检查窗口四角是否有标题栏背景色溢出到圆角区域外

### 2. 最大化/贴靠态过渡

- `shell-frame` 从 `14px` → `0px` 过渡时，`chrome-mask` 会同步跟随
- **风险**: 快速连续 maximize/restore 切换时，如果 Slint 的 property binding 存在帧延迟，可能出现一帧的 clip 不匹配
- **验证建议**: 测试快速双击标题栏（restore ↔ maximize）时是否有视觉闪烁

### 3. RightPanel 展开/收起时的 left-divider

- `left-divider` 固定 `width: 1px`，当 panel `visible: false` 时整个组件不可见
- **风险**: 展开动画（如果未来引入）时 divider 可能先于内容出现
- **验证建议**: 当前无动画，但未来如加入 width 过渡需确认 divider 时序

### 4. 窗口极窄时右侧面板强制收起

- 现有 `collapse_order_matches_design_under_narrow_widths` 测试覆盖 `width < 1080px` 时 right-panel collapse
- **风险**: 收起后 `layout-right-panel-radius` 和 `layout-right-panel-border-width` 仍为 `0`，但 `width` 为 `0px`
- **当前状态**: 已通过测试验证，无问题

### 5. 主题切换时的描边一致性

- `left-divider` 和 `shell-frame` 都使用 `ThemeTokens.shell-stroke`
- **风险**: light/dark 切换时如果 token 更新有延迟，可能出现短暂颜色不一致
- **验证建议**: 快速切换主题时检查 divider 与外框描边颜色是否始终同步

## 文件变更清单

```
ui/shell/titlebar.slint          — 删除 use-flat-window-chrome、flat 化几何、新增 layout-border-width
ui/shell/right-panel.slint       — flat 化几何、新增 layout-radius/layout-border-width、新增 left-divider
ui/app-window.slint              — content-column→chrome-mask(clip)、删除 titlebar binding、新增 3 个诊断属性
tests/window_geometry_spec.rs    — 替换 2 个旧测试、新增 2 个测试
tests/top_status_bar_smoke.rs    — 新增 1 个回归测试
tests/top_status_bar_ui_contract_smoke.sh   — 新增 7 条断言
tests/shell_layout_ui_contract_smoke.sh     — 新增 4 条断言
tests/windows_frame_contract_smoke.sh       — 新增 1 条断言
```

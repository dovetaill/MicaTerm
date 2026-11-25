# Windows 窗口激活态左/右/下系统主题色边框去除方案

## 1. 问题现象与截图核验

根据实测反馈与截图（`1432 × 859` 原图像素级采样）：
- 当系统开启 Windows「在标题栏和窗口边框上显示强调色（Show accent color on title bars and window borders）」且 **Mica Term GPUI 主窗口处于激活/选中态** 时：
  - **左边缘**（`x = 85, y = 43..=838`）、**右边缘**（`x = 1406, y = 43..=838`）、**下边缘**（`y = 838, x = 85..=1406`）各出现 **1px 系统主题强调色实线**（截图中为 `RGB(197, 75, 183)` 偏红品红色）；
  - **上边缘**（`y = 42`）无该强调色线，直接从 `y = 43` 进入自绘深色 `TitleBar`（`RGB(40, 40, 43)`），形成突兀的「左、右、下三边亮色包边」。
- 排查 `src/ui/*` 与 `gpui-component 0.10.0`（`window_border.rs` 在 Windows `Decorations::Server` 下为无边框透明容器）确认：**应用 UI 层未绘制任何外框或读取系统强调色**，该线条 100% 来自 `patches/gpui-pre-windows` 的 Win32 / DWM 非客户区边框渲染。

---

## 2. 精确根因定位

问题由 `patches/gpui-pre-windows` 中两处底层窗口机制叠加导致：

1. **未抑制 Windows 11+ DWM 窗口外框色（`DWMWA_BORDER_COLOR`）**
   - 位置：[window.rs](file:///home/wwwroot/mica-term-gpui/patches/gpui-pre-windows/src/window.rs#L639)、[util.rs](file:///home/wwwroot/mica-term-gpui/patches/gpui-pre-windows/src/util.rs#L134-L148)、[events.rs](file:///home/wwwroot/mica-term-gpui/patches/gpui-pre-windows/src/events.rs#L1277-L1290)
   - 现状：主窗口通过 `TitleBar::window_options()` 开启 `hide_title_bar = true`（`appears_transparent = true`），样式为 `WS_SYSMENU | WS_THICKFRAME | WS_MAXIMIZEBOX | WS_MINIMIZEBOX`。初始化与系统主题变更时仅设置了 `DWMWA_USE_IMMERSIVE_DARK_MODE`，未对自定义标题栏窗口设置 `DWMWA_BORDER_COLOR = DWMWA_COLOR_NONE`（`0xFFFFFFFE`），导致 Windows 11 DWM 在激活态按系统默认策略绘制强调色边框。

2. **`WM_NCCALCSIZE` 仅消除了顶部非客户区，左/右/下仍残留 1px 可见非客户区边框（Windows 10 / RDP / 无 `WS_CAPTION` 路径）**
   - 位置：[events.rs](file:///home/wwwroot/mica-term-gpui/patches/gpui-pre-windows/src/events.rs#L776-L797)
   - 现状：`handle_calc_client_size` 先调用 `DefWindowProcW(handle, WM_NCCALCSIZE, wparam, lparam)`，使 `rgrc[0]` 四周向内收缩 `SM_CXSIZEFRAME + SM_CXPADDEDBORDER`（96 DPI 下为 `8px` = 外侧 `7px` 透明缩放热区 + 内侧 `1px` `SM_CXBORDER`/`SM_CYBORDER` 可见非客户区边框），随后**仅将 `rgrc[0].top` 还原为 `saved_top`**。
   - 结果：顶部可见非客户区厚度变为 `0px`（无顶边线），但左、右、下仍保留那 `1px` 可见非客户区边框环；在 Windows 10（不支持 `DWMWA_BORDER_COLOR`）以及无 `WS_CAPTION` / 远程桌面（RDP）合成路径下，系统会在该 `1px` 环内绘制激活态系统强调色。

---

## 3. 修改方案设计（最小完整改动）

仅修改平台补丁层 `patches/gpui-pre-windows/src/` 下 3 个文件，零新增配置项、零 UI 层侵入，同时完整保留外侧透明缩放热区、最大化工作区对齐、Aero Snap 与终端右侧滚动条点击体验。

### 3.1 [util.rs](file:///home/wwwroot/mica-term-gpui/patches/gpui-pre-windows/src/util.rs)：统一系统构建号查询与 DWM 边框抑制入口

1. 提取单一事实源 `pub(crate) fn windows_build_number() -> Option<u32>`：
   - 统一封装 `windows::Wdk::System::SystemServices::RtlGetVersion`（正确初始化 `dwOSVersionInfoSize`），供 `util.rs` 与 `window.rs` 复用，消除多处手写版本检测重复代码。
2. 新增 `pub(crate) fn suppress_dwm_window_border(hwnd: HWND)`：
   - 仅当 `windows_build_number().is_some_and(|build| build >= 22000)`（Windows 11 Build 22000+）时调用：
     `DwmSetWindowAttribute(hwnd, DWMWA_BORDER_COLOR, &DWMWA_COLOR_NONE, size_of::<u32>())`，并使用 `.log_err()` 记录真实异常（不在 Windows 10 上盲调产生 `E_INVALIDARG` 噪音，也不静默吞掉 Windows 11+ 的意外失败）。

### 3.2 [window.rs](file:///home/wwwroot/mica-term-gpui/patches/gpui-pre-windows/src/window.rs)：窗口创建、退出全屏同步抑制边框并复用版本检测

1. 在 `WindowsWindow::new`（紧随 `configure_dwm_dark_mode(hwnd, appearance)` 之后、`border_offset.update(hwnd)` 之前）：
   - 若 `hide_title_bar` 为 `true`，调用 `suppress_dwm_window_border(hwnd)`。
2. 在 `WindowsWindowInner::toggle_fullscreen` 退出全屏恢复 `GWL_STYLE` 后：
   - 若 `!this.state.is_fullscreen() && this.hide_title_bar`，调用 `suppress_dwm_window_border(this.hwnd)`，确保样式重置后边框不会回退。
3. 将 `dwm_set_window_composition_attribute`（`>= 22621`）与 `set_window_composition_attribute`（`>= 17763`）改为复用 `util::windows_build_number()`。

### 3.3 [events.rs](file:///home/wwwroot/mica-term-gpui/patches/gpui-pre-windows/src/events.rs)：`WM_NCCALCSIZE` 吞并左/右/下可见 1px 边框 + 顶角命中对称修正 + 主题消息同步

1. 提取纯函数 `adjust_custom_titlebar_client_rect` 并在 `handle_calc_client_size` 中调用：
   - 保存 `DefWindowProcW` 调用前的原始窗口矩形 `window_rect = (*params).rgrc[0]`；
   - 调用 `DefWindowProcW(handle, WM_NCCALCSIZE, wparam, lparam)` 获取默认客户区矩形 `client_rect`；
   - 按窗口当前 DPI（`GetDpiForWindow(handle)`）读取系统物理边框宽度 `border_x = GetSystemMetricsForDpi(SM_CXBORDER, dpi)`、`border_y = GetSystemMetricsForDpi(SM_CYBORDER, dpi)`；
   - **最大化态（`is_maximized == true`）**：保持左、右、下等于 `DefWindowProcW` 计算出的显示器工作区边界，仅将 `top` 设为 `window_rect.top + get_frame_thicknessx(dpi)`，防止跨显示器溢出；
   - **常规窗口态（`is_maximized == false`）**：
     - `top = window_rect.top`
     - `left = (client_rect.left - border_x).max(window_rect.left)`
     - `right = (client_rect.right + border_x).min(window_rect.right)`
     - `bottom = (client_rect.bottom + border_y).min(window_rect.bottom)`
     - 几何效果：常规可缩放窗口的客户区向外精确扩展 `1` 逻辑像素（96 DPI 为 `1px`、200% DPI 为 `2px`），与 `DWMWA_EXTENDED_FRAME_BOUNDS` 完全齐平，彻底覆盖左/右/下可见边框环，同时保留外侧 `frame - border`（96 DPI 为 `7px`）原生透明缩放热区；不可缩放窗口因 `max/min` 夹紧不会越过 `window_rect`。
   - `WindowBorderOffset::update` 本身基于实时 `GetWindowRect - GetClientRect` 差值计算，自动适配新偏移无需硬编码改动。
2. 修正 `handle_hit_test_msg` 右上角缩放热区计算（[events.rs](file:///home/wwwroot/mica-term-gpui/patches/gpui-pre-windows/src/events.rs#L1021-L1030)）：
   - 原逻辑 `right - 2 * frame_x <= cursor_point.x` 隐含假设左右非客户区总宽恒等于 `2 * frame_x`；当左右各缩减 `border_x` 后，改为基于 `GetClientRect(handle, &mut client_rect)` 判定 `client_rect.right - 1 <= cursor_point.x`，与左上角 `cursor_point.x <= 0` 完全对称（均为客户区内 `1px` + 客户区外 `frame_x - border_x`，总宽恰为 `frame_x`）。
3. 在 `handle_system_theme_changed` 处理 `"ImmersiveColorSet"` 时：
   - 若 `self.hide_title_bar` 为 `true`，无论深浅色枚举是否改变（用户在保持深色模式时单独开关或修改系统强调色也会广播 `"ImmersiveColorSet"`），均调用 `suppress_dwm_window_border(handle)`。

---

## 4. 质量约束与验证清单

### 4.1 反过度防御编码约束
- **最小完整改动**：仅改动上述 3 个文件，禁止新增配置开关、包装类型、定时器或 `WM_NCPAINT` / `WM_NCACTIVATE` 投机钩子。
- **单一事实源**：系统版本号读取统一走 `util::windows_build_number()`，DWM 边框抑制统一走 `util::suppress_dwm_window_border()`。
- **严禁静默吞错与冗余防御**：禁止写无意义的 `let _ = DwmSetWindowAttribute(...)` 或多处重复状态缓存；通过版本前置判断区分平台能力边界，调用结果统一走 `.log_err()`。

### 4.2 自动化测试与交叉编译验收
1. **纯逻辑单元测试**：为 `adjust_custom_titlebar_client_rect` 编写单元测试（支持在 Linux 主机与 Windows 目标下运行或独立校验），覆盖：
   - 96 DPI / 192 DPI（200% 缩放）常规可缩放窗口左/右/下精确外扩 `border_x`/`border_y` 且保留外侧透明缩放边；
   - 最大化窗口保持工作区边界不外扩；
   - 无边框/不可缩放窗口（`client_rect == window_rect`）不越界。
2. **双平台编译与测试检查**：
   - Linux 本机：`cargo test --offline` 全量测试通过；
   - Windows 交叉目标：`cargo xwin check --target x86_64-pc-windows-msvc --offline`（含 `gpui-pre-windows` 平台补丁与测试目标）零错误、零警告通过。

# Windows 主窗口的强调色细线与自绘窗口边框

## `e5d0858` 为什么仍有三条线

开启「在标题栏和窗口边框上显示强调色」后，`e5d0858` 发布包的激活主窗在左、右、下各有一条 1px 线。1432 × 859 截图中，三条线都是 `RGB(197, 75, 183)`。用户在 Windows 实机确认，改用当前工作区的四边客户区实现后，这三条线已消失。

主入口使用 `TitleBar::window_options()` 创建普通顶层窗口。工作区铺满客户区并绘制不透明背景。终端、SFTP 和标签页各自绘制内部控件。Windows 后端默认使用 `WS_EX_NOREDIRECTIONBITMAP` 和 DirectComposition，交换链按客户区大小绘制，根 visual 直接承载交换链。上述路径没有按系统强调色描画主窗口外框。

| `e5d0858` 的路径 | 对实机现象的解释 |
| --- | --- |
| 普通窗口的 `WM_NCCALCSIZE(TRUE)` | `DefWindowProcW` 先按 `WS_THICKFRAME` 收缩客户区。补丁恢复整个顶边，却只按 `SM_CXBORDER/SM_CYBORDER` 向左、右、下扩一小段。以默认 96 DPI 的 8px 框架和 1px 基本边框为例，三边各剩 7px 非客户区。旧单测还明确断言应剩 7px。 |
| `WM_NCCALCSIZE(FALSE)` | 创建时可能收到这个形式，参数直接指向 `RECT`。旧处理交给默认窗口过程，同一窗口有两套客户区算法。 |
| 右上角缩放 | 上游用 `2 * frame_x` 推算横向偏移，`e5d0858` 改用客户区的 `right - 1`。两种写法都依赖旧的非客户区布局，现由统一的八向缩放命中处理。 |

`e5d0858` 还修正了构建号查询。共享的 `util::windows_build_number()` 在调用 `RtlGetVersion` 前将 `dwOSVersionInfoSize` 设为结构体大小。DWM 边框色（22000）、系统背景材质（22621）和窗口合成属性（17763）各自用该查询判断支持的构建号。 [微软 RtlGetVersion 说明](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/wdm/nf-wdm-rtlgetversion)

## 现在的窗口行为

- 自绘标题栏窗口在普通态以及所有 `WM_NCCALCSIZE(FALSE)` 消息中保留建议矩形并返回 0，得到 `rcClient == rcWindow`。最大化时，`WM_NCCALCSIZE(TRUE)` 用 `MonitorFromRect` 从建议矩形确定目标屏，将客户区设为该屏的 `rcWork`。工作区查询失败会记录错误并交回默认窗口过程。 [微软 WM_NCCALCSIZE 说明](https://learn.microsoft.com/en-us/windows/win32/winmsg/wm-nccalcsize)
- 普通自绘主窗保留完整的 `WS_OVERLAPPEDWINDOW`（含 `WS_CAPTION`），以支持原生最小化／还原动画和 Aero Snap。自绘标题栏窗口都在首次显示前用 `SetWindowPos(..., SWP_FRAMECHANGED | ...)` 应用框架。 [微软自定义框架说明](https://learn.microsoft.com/en-us/windows/win32/dwm/customframe)
- 创建自绘标题栏窗口时只判断一次 `build >= 22000`。Windows 11+ 的普通主窗用 `DwmExtendFrameIntoClientArea(cyTopHeight = 1)` 保留阴影与圆角，同一版本分支下的所有自绘标题栏窗口均设置 `DWMWA_BORDER_COLOR = DWMWA_COLOR_NONE`。Windows 10 跳过这两项 DWM 调用，由客户区覆盖四边。 [微软 DWM 属性定义](https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/ne-dwmapi-dwmwindowattribute)
- 边框色按 `HWND` 设置，`DWMWA_COLOR_DEFAULT` 显式恢复系统默认色。退出全屏在原句柄上恢复 `GWL_STYLE`，`ImmersiveColorSet` 广播通知应用重新读取外观。微软没有记载其中任一操作会重置边框色，因此 Windows 11+ 仅在创建自绘窗口时写入该属性。深浅色变化时仍更新 `DWMWA_USE_IMMERSIVE_DARK_MODE`。 [微软 DwmSetWindowAttribute 说明](https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/nf-dwmapi-dwmsetwindowattribute) · [窗口样式](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwindowlongptrw) · [设置变更通知](https://learn.microsoft.com/en-us/windows/win32/winmsg/wm-settingchange)
- 普通可缩放的自绘窗口用带符号的屏幕坐标做 `WM_NCHITTEST`。上、下、左和四角使用当前 DPI 的缩放框架及附加边距。右边中段只保留最外侧 2 个逻辑像素（换算后向下取整），给滚动条留出点击区域。四角优先，窗口内其余位置走 GPUI 标题栏拖动／按钮命中或 `HTCLIENT`，矩形外返回 `HTNOWHERE`。缩放区的光标和左键消息由系统处理。最大化、全屏和固定大小窗口跳过自绘缩放命中。
- `WM_GETMINMAXINFO` 只写 `ptMinTrackSize`，系统管理 `ptMaxSize` 和 `ptMaxPosition`。`WM_DPICHANGED` 对普通窗口应用建议矩形。最大化与全屏时用建议矩形选取目标屏，分别使用 `rcWork` 和 `rcMonitor`。设置位置成功后，最大化的渲染尺寸取自实际客户区。 [微软 WM_DPICHANGED 说明](https://learn.microsoft.com/en-us/windows/win32/hidpi/wm-dpichanged)
- **保存与还原**。自绘框架的 `WindowBorderOffset` 恒为零，最小跟踪尺寸以及客户区与外窗的换算都使用这个值。`calculate_window_bounds` 和 `retrieve_window_placement` 在屏幕与工作区坐标之间反向换算，处理任务栏在上方或左侧产生的原点差。工具窗口遵循 Win32 的屏幕坐标约定。 [微软 WINDOWPLACEMENT 定义](https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-windowplacement)

## 验证

Linux 的 `cargo test --locked --offline` 运行应用测试。Windows 补丁由目标平台条件编译，在 Linux 上使用以下两条命令分别检查主程序和补丁测试目标。

```sh
cargo xwin check --target x86_64-pc-windows-msvc --locked --offline
cargo xwin check --target x86_64-pc-windows-msvc -p gpui-pre-windows --lib --tests --locked --offline
```

本次 Linux 应用测试通过 55 项，上述两个 Windows 目标均通过离线类型检查。补丁单元测试覆盖 96、120、144、192 DPI 下的八向缩放命中、右侧滚动条与缩放热区的相邻像素边界、矩形排他上界及负坐标副屏场景。

Windows 桌面测试 `custom_titlebar_hwnd_has_no_nonclient_insets` 通过正常 GPUI 入口创建隐藏主窗。它用 `GetWindowInfo` 在同一屏幕坐标系比较真实 `rcClient` 和 `rcWindow`，核对保存与还原位置的双向换算，并向 HWND 发送普通 `RECT` 形式的消息。本机仅交叉检查了这个测试。带交互桌面和 Direct3D 的 Windows 机器可执行以下命令。

```sh
cargo test -p gpui-pre-windows --lib --locked --offline custom_titlebar_hwnd_has_no_nonclient_insets
```

以下是 Windows 10 和 Windows 11 的桌面回归用例。已有实机反馈覆盖清理前的三边细线，删除两处重复设置后的全屏与主题切换尚未在 Windows 桌面复测。

| 桌面操作 | 验收点 |
| --- | --- |
| 选择与应用背景反差明显的强调色，启用标题栏／边框强调色及系统阴影。在同一窗口激活、失焦、重新激活，切换强调色和浅／深色，再进入并退出全屏 | 每次切换后检查四条直边和顶部第一行，均应无强调色实线。Windows 11 另验普通窗口的阴影与圆角。 |
| 在 100%、150%、200% 缩放下拖动四边和四角，再实际点击、拖动终端与 SFTP 右侧滚动条 | 八个方向使用相应的缩放光标并改变窗口尺寸，右边中段越过最外侧热区即由控件接收点击，最大化后最右列也可点。 |
| 从自绘标题栏拖到屏幕顶端和侧边，双击还原，点击标题栏按钮及任务栏图标最小化／还原 | Snap 分屏、最大化、还原正常。系统开启动画时有原生过渡，Windows 11 上另验最大化按钮的 Snap 布局。 |
| 在分辨率、DPI 不同且有负坐标的双屏间移动，分别最大化、跨屏、还原、全屏再退出。在系统支持时把任务栏放到顶边或左边 | 最大化四边对齐目标屏工作区，相邻屏无溢出，任务栏可用，全屏占目标显示器。还原位置和尺寸无逐次偏移。 |

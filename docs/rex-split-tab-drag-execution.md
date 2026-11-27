# Mica Term GPUI：Rex 分屏、标签栏、本地 Shell 标题与窗口拖拽分步实施手册（执行文档）

> [!NOTE]
> **前置依据**：本执行手册与 [rex-split-tab-drag-plan.md](file:///home/wwwroot/mica-term-gpui/docs/rex-split-tab-drag-plan.md) 配套，逐文件细化数据结构、方法签名、边界条件与自动化测试断言，供审阅通过后按序执行。

---

## Task 1：修复顶栏窗口拖拽 + 标签栏 `.w_auto()` 紧随 `+` + 多标签高亮与分隔线

### 1.1 下移根级 `.track_focus` 并统一跨平台空白拖拽区（[src/ui/workspace.rs](file:///home/wwwroot/mica-term-gpui/src/ui/workspace.rs)）
1. **调整 `MicaWorkspace::render` 焦点追踪层级**：
   - `#mica-workspace-root` 保留 `.on_modifiers_changed(...)` 与 `.capture_key_down(cx.listener(Self::handle_capture_key_down))`，**移除** `.track_focus(&self.focus_handle)`；
   - 将 `.track_focus(&self.focus_handle)` 移至下方 `#mica-workspace-stage` 容器上，确保点击顶栏拖拽区不会调用 `window.prevent_default()` 吞掉 Windows `WM_NCLBUTTONDOWN(HTCAPTION)`，也不会抢夺当前终端面板焦点。
2. **提取 `bind_window_drag_area` 辅助函数并同时覆盖两处顶栏空白**：
   - 应用位置：
     1. 标签新建按钮组右侧的弹性主空白 `drag_space`（`div().id("titlebar-drag-space").flex_1().min_w(px(32.0)).h_full()`）；
     2. Windows / Linux 客户端装饰模式下窗口控制三键（`3 × 46×32`）底部的呼吸条（`div().id("titlebar-controls-understrip").w_full().h(px(8.0))`，配合 `40px` 顶栏总高：`32 + 8 = 40`）。
   - 平台行为：
     - `cfg!(target_os = "windows")`：仅设置 `.window_control_area(WindowControlArea::Drag)`（绝不在包裹标签或窗口三键的父级挂 `Drag`，保留最大化键原生 `HTMAXBUTTON` 与 Windows 11 Snap Layouts）；
     - `linux_client`：挂载共享拖拽状态（左键按下调 `gpui_kit::component::GlobalState::suppress_text_selection(cx)`，首次移动调 `window.start_window_move()`，原地 `click_count == 2` 调 `window.zoom_window()`，右键调 `window.show_window_menu(event.position)`，并在 `on_mouse_up`、`on_mouse_up_out`、`on_mouse_down_out` 及窗口失活时清理状态）。

### 1.2 标签条 `.w_auto()` 紧凑滚动与 `+` 紧跟末标签（[src/ui/workspace.rs](file:///home/wwwroot/mica-term-gpui/src/ui/workspace.rs)）
1. 在 `MicaWorkspace` 增加 `tab_scroll_handle: gpui_kit::ScrollHandle`（用于新建标签或快捷键切标签后自动 `scroll_to_item` / 滚至末尾可见）。
2. 将顶栏标签列表容器改为：
   ```rust
   h_flex()
       .id("workspace-tab-strip")
       .w_auto()
       .min_w_0()
       .h_full()
       .items_center()
       .overflow_x_scrollbar()
       .children(tab_elements_with_dividers)
   ```
   - 1~3 个标签时，`.w_auto()` 使滚动条外层紧包标签实际宽度，不可压缩的新建按钮组（`+` 新建本地终端 + `▾` 下拉新建菜单，左边距 `ml(px(6.0))`）紧贴最后一个标签右侧；
   - 标签溢出时，`min_w_0()` 自动收缩容器并支持鼠标滚轮与触控板横向滚动。

### 1.3 终端活动标签高亮表面与未选中竖向分隔线（[src/ui/theme.rs](file:///home/wwwroot/mica-term-gpui/src/ui/theme.rs) & [src/ui/workspace.rs](file:///home/wwwroot/mica-term-gpui/src/ui/workspace.rs)）
1. 在 [src/ui/theme.rs](file:///home/wwwroot/mica-term-gpui/src/ui/theme.rs) 新增纯函数 `terminal_tab_chrome(cx: &App) -> TerminalTabChrome`（含 `active_bg`、`active_border`、`active_fg`、`inactive_fg`、`divider`、`badge_bg`）：
   - 浅色模式：`active_bg` 采用明亮高净度表面（如 `#FFFFFF` / `cx.theme().popover`），配 `1px` 清晰描边与 `shadow_2xs()`；
   - 深色模式：`active_bg` 基于 `cx.theme().title_bar` 向白色提亮（确保亮度显著高于 `title_bar`，告别凹陷黑底），配微高光描边，并对 `active_fg` 保证 `>= 4.5:1` 可读对比度。
2. 精修 `render_single_tab_pill`：
   - 尺寸与间距：`h(px(30.0))`、`max_w(px(220.0))`、`rounded(px(8.0))`、`pl(px(10.0))`、`pr(px(8.0))`、`gap(px(7.0))`；
   - 左侧图标：采用 `18×18px`、`rounded(px(5.0))` 精致终端/服务器徽章（本地终端为深色/主题色小框配亮色 `>_`，SSH 为带实时连接状态指示的服务器徽章，对齐 `xjjovf.png` / `xjjlzf.png`）；
   - 相邻标签分隔线：遍历 `self.tabs` 时，在 `idx > 0` 处插入固定 `w(px(1.0)).h(px(14.0)).mx(px(2.0))` 节点；**仅当 `!prev_active && !cur_active` 时**填充 `chrome.divider`，否则透明，确保切换选中标签零位移。

---

## Task 2：本地 Shell / PowerShell 实时路径与 `powershell ~` / `12字符+...` 标题

### 2.1 启动期返回精确 Shell 身份与初始目录（[src/terminal/pty_backend.rs](file:///home/wwwroot/mica-term-gpui/src/terminal/pty_backend.rs)）
1. 定义返回结构：
   ```rust
   pub struct SpawnedLocalPty {
       pub backend: LocalPtyBackend,
       pub shell_name: String,
       pub initial_cwd: Option<PathBuf>,
       pub home_dir: Option<PathBuf>,
   }
   ```
2. 新增单一归一函数 `normalize_shell_program_name(shell: &Path) -> String`：
   - 文件名去 `.exe` 后大小写不敏感匹配：`"pwsh" | "powershell" => "powershell"`，`"cmd" => "cmd"`，其余取小写文件名（兜底 `"shell"`）。
3. 记录 `home_dir = directories::UserDirs::new().map(|u| u.home_dir().to_path_buf())`；**仅当候选启动目录存在且成功调用 `cmd.cwd(&dir)` 时**，才令 `initial_cwd = Some(dir)`。

### 2.2 字节安全的 OSC 目录解码与 Windows 提示符路径提取（[src/terminal/engine.rs](file:///home/wwwroot/mica-term-gpui/src/terminal/engine.rs) & [src/terminal/semantic.rs](file:///home/wwwroot/mica-term-gpui/src/terminal/semantic.rs)）
1. **修复跨包 UTF-8 截断**：
   - 将 `AlacrittyTerminalEngine.osc_tail` 由 `String` 改为 `Vec<u8>`（上限保护 `4096` 字节防异常未闭合序列堆积）；扫描原始字节中的 `\x1b]` 与最先到达的终止符 `\x07` 或 `\x1b\\`，拿到完整负载字节切片后再执行 `std::str::from_utf8` 解码。
2. **扩充本地与远端路径协议支持**：
   - 在 `AlacrittyTerminalEngine` 增加路径风格标记 `windows_paths: bool`（Windows 本地会话为 `true`，Unix 本地与所有 SSH 会话为 `false`）；
   - `scan_osc_sequences` 新增支持 `OSC 9;9;<path>`（剥离首尾可选 `"`）；
   - `parse_osc7_file_uri(uri, windows_paths)`：当 `windows_paths == true` 时，`file:///C:/Users/Admin` 或 `file://localhost/C:/Users/Admin` 解码规范化为 `C:\Users\Admin`；非本机 host `file://server/share/dir` 解码为 `\\server\share\dir`；`windows_paths == false` 时完全保持现有 Unix `/path` 逻辑。
3. **支持 Windows 空闲提示符目录提取与防旧提示符回刷**：
   - 在 [src/terminal/semantic.rs](file:///home/wwwroot/mica-term-gpui/src/terminal/semantic.rs) 新增 `extract_windows_cwd_from_prompt_line(line: &str) -> Option<String>`（或为 `extract_cwd_from_prompt_line` 增加路径模式参数）：
     - 复用 `detect_prompt_command_byte_offset(line)` 确认处于空闲提示符态（`has_cmd == false`）；
     - 提取可选 `(venv) ` / `PS ` 之后、`>` 之前的本地文件系统盘符路径（`[A-Za-z]:\...`）或 `Microsoft.PowerShell.Core\FileSystem::` 路径，明确拒绝 `HKLM:`、`HKCU:`、`Env:`、`Variable:`、`Function:` 等非目录 Provider；
   - 在 `AlacrittyTerminalEngine` 记录 `last_prompt_cwd_anchor: Option<(usize, String)>`（绝对行号 + 空闲提示符文本）：仅当光标所在空闲提示符行本身新出现或文本变化、且本批未收到更高优先级的显式 OSC 目录时，才采纳提示符提取结果，杜绝后续光标/颜色控制包用旧提示符覆盖新 OSC 目录。

### 2.3 单一事实源标题格式化与本地/远端目录事件解耦（[src/terminal/session.rs](file:///home/wwwroot/mica-term-gpui/src/terminal/session.rs) & [src/ui/workspace.rs](file:///home/wwwroot/mica-term-gpui/src/ui/workspace.rs)）
1. 在 `TerminalSessionEntity` 明确拆分：
   - 本地会话：`local_shell_name: String`、`local_cwd: Option<String>`、`local_home_dir: Option<String>`；
   - 远端会话：保留 `remote_cwd`、`sftp_cwd` 等字段**仅在 `SessionKind::Ssh` 下更新并触发 `RemoteCwdChanged`**。
   - 本地会话收到 `EngineSideEffect::TitleChanged` 时不再盲目覆盖标题；收到 `EngineSideEffect::CurrentDirChanged(dir)` 时更新 `local_cwd` 并发出 `SessionEntityEvent::TitleChanged`。
2. 在 `TerminalSessionEntity` 实现唯一标题方法（移除 `WorkspaceTab.title` 冗余字段及 `workspace.rs` 中 5 处手工同步）：
   - `pub fn pane_title(&self) -> String`：
     - SSH：返回 `config.name.clone()`；
     - LocalShell：格式化 `<shell_name> <display_path>`（若等于 `local_home_dir` 则 `display_path = "~"`；若 `local_cwd` 暂未知则仅返回 `<shell_name>`；否则返回完整路径如 `powershell C:\Users\Administrator`）。
   - `pub fn tab_title(&self) -> String`：
     - SSH：返回 `config.name.clone()`；
     - LocalShell：若等于 `local_home_dir` 返回 `powershell ~`；否则对路径部分按 `.chars()` 计数——**超过 12 个字符时截取前 12 个 `char` 并拼接 `"..."`**（如 `powershell C:\Users\Adm...`），不超过 12 个字符原样保留（如 `powershell C:\Users`）。

---

## Task 3：分屏区块卡片化、`6px` 间距、内嵌 `28px` 顶栏与精确目标分屏

### 3.1 舞台底壳露出、单层卡片圆角外框与 `6px` 透明分隔条（[src/ui/workspace.rs](file:///home/wwwroot/mica-term-gpui/src/ui/workspace.rs)、[sidebar.rs](file:///home/wwwroot/mica-term-gpui/src/ui/sidebar.rs)、[sftp_panel.rs](file:///home/wwwroot/mica-term-gpui/src/ui/sftp_panel.rs)、[welcome_view.rs](file:///home/wwwroot/mica-term-gpui/src/ui/welcome_view.rs)）
1. 将 `#mica-workspace-stage` 改为 `mx(px(6.0)).mb(px(6.0)).bg(cx.theme().title_bar)`（去掉整块大外框，避免双层叠边）。
2. 为 `LeftSidebar`、`SftpPanelView`、`WelcomeView` 各自外层赋予独立的 `rounded(px(8.0)).border_1().border_color(cx.theme().border).overflow_hidden()`。
3. 实现共享 `workspace_gap_handle_appearance() -> ResizeHandleRenderer`：
   - `ResizeHandleState::Idle`：返回 `Some(div().size_full().into_any_element())`（完全透明，不画穿缝实线）；
   - `Hovered | Pressed | Dragging`：在 `6px` 底壳间距中央绘制平滑过渡的 `3px` 圆角指示胶囊。
4. 在 `h_resizable` / `v_resizable` 相邻子面板交界侧各加 `3px` 内缩（首项尾侧 `3px`、中间项双侧 `3px`、末项首侧 `3px`），保证任意平铺或嵌套分屏相邻两条卡片边框之间恒为精确 **`6px` 底壳间距**。

### 3.2 `TerminalPaneView` 内嵌 `28px` 顶栏与悬停/选中显隐规则（[src/ui/terminal_view.rs](file:///home/wwwroot/mica-term-gpui/src/ui/terminal_view.rs)）
1. 移除窗口主顶栏的 `title-split-h`、`title-split-v`，并删除 `TerminalPaneView` 旧的绝对定位压字按钮 `split_close_badge`。
2. 在 `TerminalPaneView` 维护外层卡片状态字段：
   - `pub pane_count_in_tab: usize`（等于 1 时隐藏关闭、浮动、最大化按钮）
   - `pub is_active_pane: bool`
   - `pub is_maximized_pane: bool`
   - `pub is_floating_pane: bool`
   - `pub is_floating_collapsed: bool`
   - `card_hovered: bool`（挂在整张 `TerminalPaneView` 根卡片 `.on_hover(...)` 上，窗口失活时同步归零）
3. 卡片布局结构：
   - 根容器：`v_flex().size_full().rounded(px(8.0)).border_1().border_color(card_border).overflow_hidden().bg(snapshot.background)`；
   - **顶部 `28px` 面板栏**（`h(px(28.0)).flex_shrink_0().px_2p5().gap_2().items_center().justify_between()`）：
     - 左侧：`Icon::new(Terminal / Server)` + `session.read(cx).pane_title()`（`truncate().text_xs().text_color(muted_foreground)`）；
     - 右侧按钮组（当 `is_active_pane || card_hovered || is_floating_pane || is_maximized_pane` 时渲染）：
       - 始终可用：`向右分屏 (Columns2)`、`向下分屏 (Rows2)`；
       - 仅当 `pane_count_in_tab > 1` 时追加：`浮动 / 停靠还原 (PictureInPicture2 / Dock)`、`最大化 / 还原 (Maximize2 / Minimize2)`、（`is_floating_pane && !is_maximized_pane` 时）`折叠 / 展开浮窗 (CircleMinus / CirclePlus)`、`关闭当前分屏 (X)`；
       - 窄宽度自适应：记录面板头宽度，若 `< 190px` 且 `pane_count_in_tab > 1`，保留 `最大化/还原` 与 `关闭`，其余收纳进紧凑 `MoreHorizontal` 下拉菜单。
   - **下方终端正文容器**（当 `!is_floating_collapsed` 时渲染）：`div().relative().flex_1().min_h_0().min_w_0()`，内部放置 `terminal_canvas`、`search_bar`（`top_1p5().right_3()`）与 `phase_overlay`。
4. **精确目标事件**：
   ```rust
   pub enum TerminalPaneEvent {
       Activate,
       SplitRequested(TabSplitAxis),
       ToggleFloatRequested,
       ToggleMaximizeRequested,
       ToggleCollapseFloatingRequested,
       CloseRequested,
   }
   ```
   `MicaWorkspace` 新增 `split_pane(target_pane, axis, window, cx)`（主顶栏快捷键/标签菜单分屏只是 `split_pane(active_pane, ...)` 的特例），先检查 `tab.total_pane_count() >= 6`，再读取 `target_pane` 会话类型创建新会话并紧邻 `target_pane` 插入。

---

## Task 4：分屏比例保留最大化/还原、终端区内浮动面板与粒子特效边框

### 4.1 `PaneLayoutNode::Split` 持久化 `Entity<ResizableState>` 与 `WorkspaceTab` 状态闭环（[src/ui/workspace.rs](file:///home/wwwroot/mica-term-gpui/src/ui/workspace.rs)）
1. 在 `PaneLayoutNode::Split` 中持有 `state: Entity<ResizableState>`，渲染时传入 `h_resizable(group_id).with_state(&state)` / `v_resizable(group_id).with_state(&state)`；同轴中间插入子项或移除子项时同步调用 `state.update(cx, |s, cx| ...)` 维护对应槽位尺寸，保证最大化或切标签后还原不丢比例。
2. 在 `WorkspaceTab` 补齐 `floating: Vec<FloatingPaneEntry>` 与 `maximized_pane: Option<Entity<TerminalPaneView>>`：
   - 统一 `all_panes(&self) -> Vec<Entity<TerminalPaneView>>`（合并平铺树叶子与 `floating` 列表），供 6 面板上限、字体/主题偏好同步、快捷键及 SFTP 绑定统一消费；
   - 严格实现计划文档第 `2.4-(2)` 节表格中的全部 6 条状态转换不变量（特别是关闭最后一片底层平铺面板时自动将顶层浮动面板提升为唯一平铺根，以及最大化态仅渲染目标面板单实例）。

### 4.2 终端区域内浮动容器与零后台开销微光粒子边框（新建 `src/ui/floating_pane.rs`）
1. **舞台内几何约束与交互**：
   - 测量中央终端舞台实时像素尺寸 `stage_size`；默认初始浮窗位于右下象限（宽 `(stage_w * 0.52).clamp(340.0, 760.0)`，高 `(stage_h * 0.54).clamp(220.0, 540.0)`，距右/下边缘 `20px`）；
   - 单一纯函数 `clamp_floating_rect(rect, stage_size, collapsed)` 保证在任意侧栏开合或主窗口缩小时，浮窗（含折叠态 `28px` 标题条）始终完整留在中央终端舞台内部可抓取；
   - 支持按住浮窗 `28px` 顶栏空白拖拽移动、按住四边/四角 `6px` 热区拖拽缩放（折叠态仅允许移动不允许纵拉），浮窗本体 `.occlude()` 拦截下层鼠标点击与滚轮，外围空白透传给底层平铺终端。
2. **`FloatingBorderParticles` 微光星尘边框视图**：
   - 外层包裹两层静态柔和 `BoxShadow`（贴边锐影 + 外扩柔光晕）；
   - 独立子视图 `FloatingBorderParticles` 仅包含一个外扩 `12px`、不接收鼠标命中的轻量 `canvas`：基于 `Instant` 相位沿圆角矩形外周长计算 26 颗微粒位置，交替使用 `cx.theme().primary` 与 `cx.theme().cyan` 低透明度双圈柔光 quad 绘制细腻粒子流光感；
   - **零后台帧率守卫**：仅在 `is_tab_active && window.is_window_active() && !cx.reduce_motion()` 时由 `FloatingBorderParticles` 自身调用 `window.request_animation_frame()` 推进相位（**绝不 `notify` 内层 `TerminalPaneView`**），切后台或失焦时帧请求立即归零。

---

## Task 5：自动化测试与双平台构建验收清单

1. **单元测试补充**：
   - `pty_backend::tests`：`pwsh.exe` / `powershell.exe` / `cmd.exe` / `/bin/zsh` 名称归一测试；
   - `engine::tests` & `semantic::tests`：
     - 中文目录名在 `OSC 7` / `OSC 9;9` 中被拆成两半字节切片喂入 `feed_bytes` 仍准确解码不乱码；
     - `file:///C:/Users/Administrator`、`file://server/share/doc`、`OSC 9;9;"C:\Users\L"` 与 `PS C:\Users\Administrator> `、`C:\Users> ` 正确产出目录，且 `PS HKLM:\> `、`ALT_SCREEN` 与残留旧提示符不误改目录；
   - `session::tests`：
     - `home = C:\Users\L`（含大小写 `c:\users\l\` 变体）时 `pane_title` 与 `tab_title` 均为 `powershell ~`；
     - `cwd = C:\Users`（8 字符）时双端均为 `powershell C:\Users`；
     - `cwd = C:\Users\Administrator`（22 字符）时 `pane_title == "powershell C:\\Users\\Administrator"`、`tab_title == "powershell C:\\Users\\Adm..."`；
     - 12 字符与 13 字符精确边界、中文路径按 `char` 截断不碎字、本地目录变更不触发 `RemoteCwdChanged`。
   - `workspace::tests`：
     - 平铺 `[A, B]` $\rightarrow$ `B` 浮动 $\rightarrow$ 关闭 `A` 时 `B` 自动升为唯一平铺根且 `pane_count == 1`；
     - 悬停非活动 SSH 面板 `B` 触发分屏时，克隆 `B` 并插在 `B` 右侧/下侧；
     - 不等比分屏最大化再还原后 `ResizableState` 尺寸保持不变。
2. **GPUI Headless 集成测试扩展（[tests/settings_ui.rs](file:///home/wwwroot/mica-term-gpui/tests/settings_ui.rs)）**：
   - 校验全部 54 款内置主题下终端活动标签背景与文字对比度 `>= 4.5:1`，且深色模式活动标签亮度高于 `title_bar`；
   - 校验 1 个标签时 `+` 按钮紧邻该标签右侧（间距 `< 20px`）且右侧 `drag_space` 宽度充裕，15 个标签时横向滚动容器正常收缩不挤出右侧工具栏；
   - 校验单分屏不渲染关闭按钮，双分屏渲染独立 `28px` 顶栏且画布 `grid_bounds.origin.y` 严格位于顶栏底边之下（零重叠）。
3. **编译与交叉构建验收**：
   - Linux 本机：`cargo test` 全量通过、零编译警告；
   - Windows 产物校验：运行 `./build-win-64.sh`（或 `cargo check --target x86_64-pc-windows-msvc`）确认 Windows 专属 `WindowControlArea`、ConPTY 与路径逻辑零错误通过。

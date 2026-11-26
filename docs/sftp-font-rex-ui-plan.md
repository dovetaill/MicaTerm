# Mica Term GPUI：SFTP 标签同步修复、字体下拉重构与 Rex 风格界面升级方案

## 一、联合根因诊断总结

结合源码逐链路排查与 Codex 深度交叉核验，当前三大问题的精确根因如下：

### 1. SFTP 没有跟随 SSH 标签变化
1. **跨标签异步 `list_dir` 竞态覆盖（核心根因）**：`SftpPanelView` 既没有记录当前绑定的会话身份 `(session_id, conn_epoch)`，也没有为异步 `navigate_to` 维护单调请求序号 `nav_seq`。当标签 A（慢速服务器或刚发起目录读取）尚未返回时切换到标签 B（或本地终端标签），A 晚到的异步结果会无条件写回 `this.entries`、`this.loading` 与 `this.error_message`，导致面板标题虽然显示 B，文件列表却被 A 覆盖；切换期间旧行也未清空，仍可被误点。
2. **后台会话连接事件干扰前台标签**：`MicaWorkspace::create_pane_for_session` 对 `RemoteConnected | PhaseChanged` 未判断 `is_active_session`，后台任意 SSH 标签握手步骤（`Step`）都会触发当前激活标签重跑 `sync_sftp_binding`，冲掉用户手动进入的远程目录并重复触发请求；且一次 SSH 连接成功同时发出 `RemoteConnected` 与 `PhaseChanged`，导致单次连接连续触发两次 `navigate_to`。
3. **缺少每会话 SFTP 浏览目录记忆与重连代数隔离**：`TerminalSessionEntity` 只记录了 Shell 的 `remote_cwd`，未记录该会话在 SFTP 面板中实际浏览的 `sftp_cwd`，切去其他标签再切回会丢失手动浏览位置（同名克隆标签尤为明显）；`reconnect()` 复用同一会话 UUID 却未隔离旧事件泵，也未在清空 `sftp` 时立即通知面板解绑。
4. **路径输入框残留、跟随开关割裂与文件操作目标漂移**：
   - 切到本地终端或未就绪会话（`service = None`）时，`current_path` 与 `path_input` 未清空，仍残留上一个 SSH 标签的路径；无 `Window` 的事件回调未刷新 `path_input`。
   - `toggle_sftp_panel` 展开面板时 `set_panel_visible` 与 `sync_sftp_binding` 会双重触发读取；顶栏 SFTP 按钮将 `sftp_busy` 传入 `.loading()`，导致有文件传输时按钮被禁用无法点击。
   - `mkdir` / `rename` / `delete_entry` 在报错后立即调用 `this.refresh(cx)`，被 `navigate_to` 开头的 `error_message = None` 静默清空错误；删除确认框等异步文件操作未固定发起时的 `(service, binding_key, dir)`。
   - 无 OSC 7 且无窗口标题的 Linux 服务器（仅提示符含 `user@host:/path#`）缺少光标提示符路径提取兜底。

### 2. 字体选择平铺杂乱与 Termius / Rex 字体对比去重
1. **交互问题**：当前「通用与终端」页同时放了裸文本输入框、失焦/回车/完成三处提交逻辑以及 13 个平铺按钮墙，不符合 Termius / Rex 的标准下拉选择交互。
2. **Termius 14 项与现有 12 款内置字体的去重映射**：
   - **直接重合或同源合并（6 项 -> 内置已覆盖）**：
     - `Source Code Pro` + `Source Code Pro Medium` → 合并为内置 `Source Code Pro`（已打包 Regular / Medium / Semibold，保留旧配置 `"Source Code Pro Medium"` 字重别名兼容）；
     - `JetBrains Mono` → 内置 `JetBrains Mono`；
     - `Fira Code` → 内置 `Fira Code`；
     - `Ubuntu Mono` → 内置 `Ubuntu Mono`；
     - `Meslo` → 内置 `Meslo LG M`（支持 `"Meslo"` 别名匹配）；
     - `Cascadia Code` → 内置无连字终端网格版 `Cascadia Mono`（若本机装有 `Cascadia Code` 则在「本机已安装」分组独立可选，绝不伪装或重复）。
   - **Mica / Rex 现代精选保留（6 项）**：`Sarasa Term SC Nerd`（默认推荐 + CJK/Nerd 兜底）、`JetBrains Maple Mono`、`Geist Mono`、`Commit Mono`、`IBM Plex Mono`、`Hack`。
   - **其余 Termius 经典字体（按本机实际安装动态发现去重）**：`Fira Mono`（归并 `Fira Mono Medium` 别名）、`Inconsolata-g`、`Anonymous Pro`、`Droid Sans Mono`、`DejaVu Sans Mono`、`PT Mono` 等，通过缓存一次的 `all_font_names()` 自动筛选等宽候选归入下拉框「本机已安装等宽」分组，无需增大安装包体积、也无需引入额外 `Cargo.toml` 依赖。
3. **实时预览升级**：保留共用 `TerminalFont::measure` 的真实网格 `canvas` 预览，升级为带度量信息头（字体名、字号、单元格像素尺寸）的 3 行真实终端预览（彩色提示符命令、易混符号对、中英文与制表线严格对齐）。

### 3. 整体界面“Zed 式脏浊感”与对标 Superlogical Rex 的视觉重构
对比 4 张 Rex 实机截图（`r732zs.png` 浅色主界面、`r732v2.png` 深色主界面、`r5q4d8.png` 浅色设置、`r5qfcw.png` 深色设置），当前界面显脏、不通透、普通的五大成因：
1. **全局 UI 字体误用等宽代码字体**：`src/ui/theme.rs` 将全局 `theme.font_family` 强行改写成了 `JetBrains Maple Mono`，导致标题栏、侧边栏、弹窗文案、按钮全部呈现宽大生硬的等宽代码字体感。恢复 `Theme::change` 原生解析的系统比例 UI 字体即可立竿见影改善精致度。
2. **缺少 Rex 标志性的「统一外层底壳 + 圆角内嵌画布（Inset Stage）」层次**：
   - Rex 浅色采用 `#ECEEF2` 干净冷灰底壳托起 `#FFFFFF` / `#FAFAFA` 纯白圆角主画布（左/右/下留 `6px` 呼吸边距 + `8px` 圆角 + `1px` 细边）；深色采用微亮石墨底壳（约 `#1E2025`）衬托深邃纯净的终端画布（`#12141A` / `term_bg`），不开启系统透明材质也具备极强的通透悬浮感。
   - 当前 Mica Term 顶栏拉通一条底边框，画布直抵窗口死角，且左栏 `.border_r_1()`、右栏 `.border_l_1()` 与 `ResizablePanel` 自带的 `1px` 分隔线叠成 **2px 双重灰边**。
3. **表面混色过多导致“脏灰感”**：`theme.rs` 在 `term_bg`、`sidebar_bg`、`surface_bg` 之上反复叠加多层黑白半透明混色，辅助文字对比度偏灰暗。
4. **侧边栏与顶栏视觉噪点过多**：侧边栏每一行都画 `26×26` 彩色半透明方块徽章（`macos_squircle_badge`），顶栏新建按钮外包了一层灰底胶囊框。
5. **偏好设置弹窗拥挤陈旧**：当前 `690px` 宽顶部 Tab + 局部嵌套滚动区 + 画满假窗口/假侧边栏的双列繁琐主题卡，远不如 Rex 的「左侧垂直图标导航栏 + 右侧极简分割行 + `Aa` 圆点色卡带」清爽通透。

---

## 二、最小完整实施计划

### 阶段 1：SFTP 会话绑定、目录记忆与异步操作闭环
- **`src/terminal/session.rs`、`src/terminal/engine.rs`、`src/terminal/semantic.rs`**
  - `TerminalSessionEntity` 增加 `conn_epoch: u64`（每次 `start_backend` 递增，后台事件泵校验代数防重连串扰；`reconnect` 清空 `sftp` 后立即发出 `PhaseChanged`）。
  - 增加 `sftp_cwd: Option<String>`、`remote_cwd_seq: u64`、`sftp_synced_cwd_seq: u64`：记录每会话最后浏览的 SFTP 目录与已同步的 Shell CWD 序号；连接就绪只发一次 `PhaseChanged`。
  - `semantic.rs` / `engine.rs` 补充严谨的 `extract_cwd_from_prompt_line`：仅在非备用屏幕、当前未收到 OSC 7/1337 与窗口标题目录、且光标所在提示行处于空闲提示态（`has_cmd == false`）时，提取明确的绝对路径或已知 `remote_home` 下的 `~` 路径。
- **`src/sftp/service.rs`**
  - 收窄 `canonicalize` / `list_dir` 的通道重开条件：对 `russh_sftp::client::error::Error::Status(_)`（如权限不足、目录不存在）直接返回明确错误，仅在底层通道/IO 异常时失效缓存并重开一次。
  - `TransferRecord` 增加 `session_label: String` 标明来源主机。
- **`src/ui/sftp_panel.rs`、`src/ui/workspace.rs`、`src/ui/terminal_view.rs`**
  - `SftpPanelView` 引入 `SftpBindingKey { session_id, conn_epoch }` 与单调递增 `nav_seq: u64`：
    - 切换绑定或路径时立即递增 `nav_seq` 并清空旧目录行，异步 `list_dir` 回调核对 `(binding, nav_seq)` 一致后才提交结果，并发出 `SftpPanelEvent::Navigated { session_id, path }` 写回对应会话的 `sftp_cwd`。
    - `service = None` 时清空 `current_path` 与 `path_input`（无 `window` 时释放旧 `path_input` 实体，下次渲染按最新路径重建，杜绝 `render` 期副作用与残留路径）。
    - 上传、下载、新建目录、重命名、删除、在线编辑在发起时固定 `(service, binding_key, target_dir, session_label)`；报错时直接写入 `error_message`（不再紧跟 `refresh` 冲掉错误），成功后仅当面板仍停留在该会话同一目录时刷新。
  - `MicaWorkspace` 改用 `cx.subscribe_in(&session, window, ...)`：仅当 `is_active_session` 为真时才由 `PhaseChanged` 驱动 `sync_sftp_binding(Some(window), cx)`；统一设置页与面板的 `follow_sftp_cwd` 入口（开启时立即对齐当前活动会话最新 `remote_cwd`）；移除顶栏 SFTP 按钮的 `.loading(sftp_busy)` 阻塞，改为可点击状态下的传输数角标提示；分屏激活同步更新 `tab.title` 与 SFTP。
  - 为 `SftpPanelView` 状态机与提示符路径提取补充单元测试（覆盖慢 A 快 B 跨标签竞态、A→B→A 手动目录保持、重连代数隔离、错误不丢失）。

### 阶段 2：字体去重、可搜索下拉选择器与预览卡片升级
- **`src/ui/fonts.rs`**
  - 保持零新增依赖，使用 `OnceLock<Vec<String>>` 缓存 `cx.text_system().all_font_names()`。
  - 在 `TerminalFont::resolve` 单一入口支持 Termius 名称/字重别名兼容（`Source Code Pro Medium`、`Fira Mono Medium`、`Meslo`、`Commit Mono`、`Cascadia Code`、`Fira Mono`）。
  - 提供分组字体项构建函数：第一组为 12 款内置校准等宽字体，第二组为去重后的本机已安装等宽字体（覆盖 Termius 其余本机字体及常见编程等宽字体，剔除内置同族与字重后缀重复项，保留用户当前有效自定义项）。
- **`src/ui/modals.rs`**
  - 移除裸字体 `Input`、13 个平铺按钮及关闭弹窗时的重复写回；接入 `gpui_kit::component::select::{Select, SelectState, SelectEvent}`（开启 `.searchable(true)`，选中即调 `ws.set_font_family` 实时生效）。
  - 升级 `terminal_font_preview` 为带字族/字号/单元格像素规格头的 3 行终端画布预览，自适应窄宽度裁剪。

### 阶段 3：对标 Superlogical Rex 的通透界面与双栏设置重构
- **`src/ui/theme.rs` & `src/ui/theme_catalog.rs`**
  - 移除 `theme.font_family = BUNDLED_MAPLE_FONT_FAMILY` 覆盖，恢复系统原生比例 UI 字体；仅 `mono_font_family` 与终端/代码区使用等宽字体。
  - 重整表面层级语义：清晰区分「窗口底壳（`title_bar` / `sidebar`）」与「高净度主工作面（`background` / `popover` / `group_box`）」；提升 `muted_foreground` 可读对比度（≥ 4.5:1），收敛半透明混色与边框透明度，消除灰蒙脏感。
- **`src/ui/workspace.rs`、`src/ui/sidebar.rs`、`src/ui/sftp_panel.rs`、`src/ui/welcome_view.rs`、`src/ui/terminal_view.rs`**
  - 顶栏高度调为 `40px` 并去掉拉通底边线；活动标签采用 `30px` 高清晰圆角胶囊，清理新建按钮外的灰底包裹框。
  - 主体工作区统一包裹在距左/右/下 `6px` 的 `8px` 圆角、`1px` 细边框内嵌画布容器中；移除 `LeftSidebar` 右边框与 `SftpPanelView` 左边框，消除与 `ResizablePanel` 分隔线的 2px 双线叠加。
  - 侧边栏列表移除每行 `26px` 彩色方块底托，改为清爽的 `15–16px` 线性图标与清晰主副字阶；分屏焦点提示改为不挤压终端网格高度的 `1.5px` 顶部内浮层细条；优化欢迎页留白与列表层次。
- **`src/ui/modals.rs`（偏好设置双栏化与 Rex 主题色卡）**
  - 偏好设置改为居中无冗余头尾的 Rex 双栏结构（`w = 880px`，左侧 `192px` 图标导航栏沿用现有 `SettingsTab`，右侧单一滚动区 + 极简分割线设置行）。
  - 主题预览卡改为紧凑的 Rex 风格色卡（`Aa` + `~/` 胶囊 + 光标块 + 7 颗圆形 ANSI 色点，外置主题名称与当前模式标记，选中态使用清晰主色外框）。

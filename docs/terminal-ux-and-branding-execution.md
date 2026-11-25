# Mica Term GPUI 终端交互体验、代码高亮补全与新版品牌图标执行手册（执行文档 · Codex 联合评审修订版）

> [!NOTE]
> **前置依据**：本执行手册严格对齐经 Codex 只读评审修订后的 [terminal-ux-and-branding-plan.md](file:///home/wwwroot/mica-term-gpui/docs/terminal-ux-and-branding-plan.md)，所有 API 签名、事件冒泡顺序、Win32 消息路径与依赖特性均已对照当前 `Cargo.lock` 锁定源码核验完毕。

---

## 阶段总览与实施拓扑

| 阶段 | 任务名称 | 核心改动文件 | 关键防坑与验收红线 |
| :--- | :--- | :--- | :--- |
| **Task 1** | **新版品牌图标生成、多色 `img()` 挂载与 Windows DPI/PE 集成** | `assets/icons/*`、`assets/linux/dev.micaterm.gpui.desktop`、`scripts/generate-icons.py`、[build.rs](file:///home/wwwroot/mica-term-gpui/build.rs)、[build-win-64.sh](file:///home/wwwroot/mica-term-gpui/build-win-64.sh)、[platform.rs](file:///home/wwwroot/mica-term-gpui/patches/gpui-pre-windows/src/platform.rs)、[window.rs](file:///home/wwwroot/mica-term-gpui/patches/gpui-pre-windows/src/window.rs)、[events.rs](file:///home/wwwroot/mica-term-gpui/patches/gpui-pre-windows/src/events.rs)、[main.rs](file:///home/wwwroot/mica-term-gpui/src/main.rs)、`src/ui/brand.rs`、[welcome_view.rs](file:///home/wwwroot/mica-term-gpui/src/ui/welcome_view.rs)、[workspace.rs](file:///home/wwwroot/mica-term-gpui/src/ui/workspace.rs) | 严禁对多色图标用 `svg()`（必走 `img(Arc<Image>)`）；独立探测 `llvm-rc`/`rc.exe`；每窗口 DPI 加载不带 `LR_SHARED` 并安全 `DestroyIcon`；保留原 `RT_MANIFEST(ID 1)` |
| **Task 2** | **`cat` 代码高亮断层修复、`PromptScanCache` 与内置 PowerShell 语法** | [semantic.rs](file:///home/wwwroot/mica-term-gpui/src/terminal/semantic.rs)、[engine.rs](file:///home/wwwroot/mica-term-gpui/src/terminal/engine.rs)、[session.rs](file:///home/wwwroot/mica-term-gpui/src/terminal/session.rs)、[syntax/mod.rs](file:///home/wwwroot/mica-term-gpui/src/syntax/mod.rs)、`src/syntax/powershell.sublime-syntax` | 正负区间缓存防 30,000 行无 Prompt 日志卡顿；`OSC 133` 仅作校验候选；ConPTY `White` 放宽仅限 Windows 本地 Shell 纯读取命令且遇彩格即撤销 |
| **Task 3** | **超链接 `HTTP(S)` 安全网关、渐进下划线与 Windows/分屏「小手」光标** | [colors.rs](file:///home/wwwroot/mica-term-gpui/src/terminal/colors.rs)、[semantic.rs](file:///home/wwwroot/mica-term-gpui/src/terminal/semantic.rs)、[engine.rs](file:///home/wwwroot/mica-term-gpui/src/terminal/engine.rs)、[session.rs](file:///home/wwwroot/mica-term-gpui/src/terminal/session.rs)、[events.rs](file:///home/wwwroot/mica-term-gpui/patches/gpui-pre-windows/src/events.rs)、[workspace.rs](file:///home/wwwroot/mica-term-gpui/src/ui/workspace.rs)、[terminal_view.rs](file:///home/wwwroot/mica-term-gpui/src/ui/terminal_view.rs) | **P0 安全红线**：普通 URL 与 `OSC 8` 共用 `validate_openable_http_url`，杜绝 `ShellExecuteW` 打开非 HTTP(S) 目标；修复 Windows `handle_cursor_changed` 句柄比较 Bug；处理双分屏非焦点悬停按 `Ctrl` 及离窗/失活清理 |
| **Task 4** | **鼠标多击/右键归属状态机、多行粘贴确认、触控板余量与边缘自动滚屏** | [domain/mod.rs](file:///home/wwwroot/mica-term-gpui/src/domain/mod.rs)、[shortcuts.rs](file:///home/wwwroot/mica-term-gpui/src/domain/shortcuts.rs)、[models.rs](file:///home/wwwroot/mica-term-gpui/src/domain/models.rs)、[store.rs](file:///home/wwwroot/mica-term-gpui/src/storage/store.rs)、[modals.rs](file:///home/wwwroot/mica-term-gpui/src/ui/modals.rs)、[workspace.rs](file:///home/wwwroot/mica-term-gpui/src/ui/workspace.rs)、[terminal_view.rs](file:///home/wwwroot/mica-term-gpui/src/ui/terminal_view.rs) | `MouseDown` 锁定手势归属并配对 `on_mouse_up_out`；传统鼠标协议 `>223` 列消费不回落粘贴；旧配置迁移严格区分“缺失新字段”与“显式选 `ContextMenu`”；5 个粘贴入口统一走多行保护 |
| **Task 5** | **全量单测矩阵、PE 资源读取断言与隔离产物构建验证** | 各模块 `#[cfg(test)]`、命令行构建验证 | `cargo test` 全绿；`llvm-readobj --coff-resources` 实锤校验 `RT_GROUP_ICON(1)` + 9 帧 `RT_ICON` + `RT_VERSION` + `RT_MANIFEST(1)` |

---

## Task 1：新版品牌图标生成、多色 `img()` 挂载与 Windows DPI/PE 集成

### 1.1 矢量母版与确定性导出脚本
1. **新建 3 份 SVG 源文件**：
   - `assets/icons/mica-term-gpui-app.svg`（`256×256`）：深曜石渐变底板（`#1A202E -> #0E1118`，`x=18, y=18, w=220, h=220, rx=50`）+ 顶部 `1px` 云母微高光描边 + 右上冰青半透明云母叠片 + 极光青翠右向折翼 `>`（`#5EEAD4 -> #0891B2`）+ 暖琥珀金胶囊光标 `_`（`#FDE047 -> #F59E0B`）；
   - `assets/icons/mica-term-gpui-taskbar.svg`（`256×256` 小尺寸光学特化母版）：移除细描边与次级半透明叠片，外框扩展至 `12..244`（`rx=52`），`>` 折翼与 `_` 琥珀光标笔画加粗 18%，保证缩至 `16/20/24/32px` 时对比鲜明；
   - `assets/icons/mica-term-gpui-mark.svg`（`256×256` 无底板透明背景徽标）：仅保留加粗版青翠折翼 `>` 与暖琥珀金光标 `_`，供顶栏小尺寸品牌位使用。
2. **新建 `scripts/generate-icons.py` 并生成入库光栅资产**：
   - 调用 `rsvg-convert`：
     - 从 `mica-term-gpui-taskbar.svg` 生成 `assets/icons/png/mica-term-gpui-{16,20,24,32}.png`；
     - 从 `mica-term-gpui-app.svg` 生成 `assets/icons/png/mica-term-gpui-{40,48,64,128,256}.png`；
     - 从 `mica-term-gpui-mark.svg` 生成 `assets/icons/png/mica-term-gpui-mark-64.png`；
   - 将 `16, 20, 24, 32, 40, 48, 64, 128, 256` 共 9 张 PNG 封装为标准多帧 `assets/icons/windows/mica-term-gpui.ico`；
   - 附带创建 `assets/linux/dev.micaterm.gpui.desktop` 标准桌面入口模板。

### 1.2 Windows PE 图标与版本资源可靠编译（[build.rs](file:///home/wwwroot/mica-term-gpui/build.rs) & [build-win-64.sh](file:///home/wwwroot/mica-term-gpui/build-win-64.sh)）
1. **修复 [build-win-64.sh](file:///home/wwwroot/mica-term-gpui/build-win-64.sh#L88-L116) 工具探测遗漏**：
   - 将 `command -v llvm-rc` 纳入独立必查条件（此前仅当 `clang-cl` 或 `llvm-lib` 缺失时才挂载 `/usr/lib/llvm-{20..16}/bin`，若机器已有 `/usr/bin/clang-cl` 但 `llvm-rc` 位于 `/usr/lib/llvm-19/bin` 或仅有 `llvm-rc-19` 会漏挂）；同时在打包阶段将 `assets/icons/windows/mica-term-gpui.ico` 拷入分发目录。
2. **扩展 [build.rs](file:///home/wwwroot/mica-term-gpui/build.rs)**（保留全部既有 `/STACK:16777216`、`aws_lc` 符号保留与 `xwin-link-shims`）：
   - 增加 `println!("cargo:rerun-if-changed=assets/icons/windows/mica-term-gpui.ico");`；
   - 当 `target_os == "windows"` 时：
     - 解析 `CARGO_PKG_VERSION` 拆出四段式版本号 ` major,minor,patch,0 `；
     - 将 `assets/icons/windows/mica-term-gpui.ico` 拷贝至 `OUT_DIR/mica-term-gpui.ico`（**消除工作目录特殊字符、空格或 Windows 反斜杠转义问题**）；
     - 在 `OUT_DIR/mica-term-gpui.rc` 写入：
       ```rc
       1 ICON "mica-term-gpui.ico"
       1 VERSIONINFO
       FILEVERSION ...
       PRODUCTVERSION ...
       BEGIN
         BLOCK "StringFileInfo"
         BEGIN
           BLOCK "040904b0"
           BEGIN
             VALUE "CompanyName", "Mica Term\0"
             VALUE "FileDescription", "Mica Term GPUI — Terminal, SSH & SFTP Workspace\0"
             VALUE "FileVersion", "...\0"
             VALUE "InternalName", "mica-term-gpui\0"
             VALUE "OriginalFilename", "mica-term-gpui.exe\0"
             VALUE "ProductName", "Mica Term GPUI\0"
             VALUE "ProductVersion", "...\0"
           END
         END
         BLOCK "VarFileInfo"
         BEGIN
           VALUE "Translation", 0x0409, 1200
         END
       END
       ```
     - 独立探测资源编译器：按顺序查找环境变量 `RC`、`PATH` 中的 `llvm-rc` / `llvm-rc-{20..16}`、`/usr/lib/llvm-{20..16}/bin/llvm-rc` 以及原生 Windows 下的 `rc.exe`；使用 `Command::new(rc_bin).current_dir(&out_dir)` 独立传参（`llvm-rc` 传 `["/fo", "mica-term-gpui.res", "mica-term-gpui.rc"]`），编译失败直接 `panic!` 暴露明确错误（不写半残的手工 `.res` 兜底），最后通过 `println!("cargo:rustc-link-arg-bins={}", out_dir.join("mica-term-gpui.res").display())` 链入（注：`lld-link` 会自动合并 `.res` 与 GPUI 自身的 `RT_MANIFEST(1)`，Task 5 必须用 `llvm-readobj --coff-resources` 校验两者共存）。

### 1.3 Windows 进程 `AppUserModelID` 与每窗口 DPI 图标管理
1. **设置进程身份与 `app_id`（[main.rs](file:///home/wwwroot/mica-term-gpui/src/main.rs#L49-L65)）**：
   - 在 `application().run(move |cx| { ... })` 内、`open_window` 之前调用：
     ```rust
     cx.set_app_identity("dev.micaterm.gpui", "Mica Term GPUI");
     ```
   - 在 `WindowOptions` 中补充 `app_id: Some("dev.micaterm.gpui".into())`。
2. **升级窗口类与每窗口 DPI 图标生命周期（`patches/gpui-pre-windows`）**：
   - 在 [window.rs](file:///home/wwwroot/mica-term-gpui/patches/gpui-pre-windows/src/window.rs#L1448-L1462) 将 `WNDCLASSW` 升级为 `WNDCLASSEXW`（设好 `cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32`），`hIcon` 与 `hIconSm` 填入平台级共享默认图标兜底；
   - 在 `WindowsWindowState` 新增 `owned_icons: Cell<(Option<HICON>, Option<HICON>)>`（分别保存当前窗口按 DPI 独立创建的 `(big_icon, small_icon)` 句柄），并封装辅助函数 `refresh_window_icons_for_dpi(&self, hwnd: HWND, dpi: u32)`：
     - 使用 `GetSystemMetricsForDpi(SM_CXICON, dpi)` / `SM_CYICON` 与 `GetSystemMetricsForDpi(SM_CXSMICON, dpi)` / `SM_CYSMICON` 计算目标像素尺寸；
     - 调用 `LoadImageW(Some(get_module_handle().into()), PCWSTR(1 as _), IMAGE_ICON, cx, cy, IMAGE_FLAGS(0))`（**不传 `LR_SHARED`，避免 Win32 缓存首次尺寸导致大小图标或跨屏 DPI 复用同一帧！**）；
     - 若新句柄加载成功，通过 `SendMessageW(hwnd, WM_SETICON, WPARAM(ICON_BIG as usize), LPARAM(big.0 as isize))` 与 `ICON_SMALL` 应用到当前 `HWND`，再对替换下来的旧句柄调用 `DestroyIcon`（若加载失败则保留旧图标不动）；
     - 在窗口创建完成（`WindowsWindow::new`）时按 `GetDpiForWindow(hwnd)` 调用一次，在 [events.rs](file:///home/wwwroot/mica-term-gpui/patches/gpui-pre-windows/src/events.rs#L873) `handle_dpi_changed_msg` 中按 `new_dpi` 再调用一次，并在窗口销毁（`handle_destroy_msg` / `Drop`）时释放持有的两个 `HICON` 句柄。

### 1.4 应用内多色品牌图标渲染（新建 `src/ui/brand.rs`）
1. 在 `src/ui/brand.rs` 中利用 `LazyLock<Arc<gpui_kit::Image>>` 一次性解码内嵌 PNG（保留 `gpui_kit::assets::AllAssets` 原封不动）：
   ```rust
   use std::sync::{Arc, LazyLock};
   use gpui_kit::{Image, ImageFormat, IntoElement, Styled as _, img, px};

   static APP_ICON_IMAGE: LazyLock<Arc<Image>> = LazyLock::new(|| {
       Arc::new(Image::from_bytes(
           ImageFormat::Png,
           include_bytes!("../../assets/icons/png/mica-term-gpui-128.png").to_vec(),
       ))
   });

   static MARK_ICON_IMAGE: LazyLock<Arc<Image>> = LazyLock::new(|| {
       Arc::new(Image::from_bytes(
           ImageFormat::Png,
           include_bytes!("../../assets/icons/png/mica-term-gpui-mark-64.png").to_vec(),
       ))
   });
   ```
2. 修改 [welcome_view.rs](file:///home/wwwroot/mica-term-gpui/src/ui/welcome_view.rs#L267-L272)：将原 `macos_squircle_badge(LucideIcon::Terminal, ...)` 替换为 `img(brand_app_icon()).size(px(46.0))`（自带圆角曜石底板与双色棱镜渐变）；
3. 修改 [workspace.rs](file:///home/wwwroot/mica-term-gpui/src/ui/workspace.rs#L1674-L1678)：将顶栏空标签态的 `Icon::new(LucideIcon::Terminal)` 替换为 `img(brand_mark_icon()).size(px(15.0))`，其余新建终端按钮、Tab 内终端类型小图标继续保留语义清晰的 `LucideIcon::Terminal` / `Server`。

---

## Task 2：`cat` 终端代码高亮断层修复与缓存优化

### 2.1 扩充多生态 Prompt 与 PowerShell/CMD 文件读取命令（[semantic.rs](file:///home/wwwroot/mica-term-gpui/src/terminal/semantic.rs)）
1. **升级 `detect_prompt_command_byte_offset(line: &str)`**：
   - **行首无前缀符号分支**：
     - `trimmed_end.starts_with("❯ ")`：直接返回 `( "❯ ".len(), has_cmd )`；
     - `trimmed_end.starts_with("➜ ")`（Oh-My-Zsh `robbyrussell`）：解析 `➜` 后的第 1 个非空 Token（当前目录名），若后续紧跟 `git:(` 开头的 Token 则继续跳过该 Token 及可选的单个状态符号 Token（`✗` / `✔` / `⚡` / `*`），将剩余部分起点作为 `cmd_start`！（若仅有 `➜  dir` 无后续命令则返回 `(trimmed_end.len(), false)`）。
   - **扩展常规前缀 `looks_like_shell_prompt_prefix(prefix, marker)`**：
     - 先循环剥离行首可选虚拟环境标记 `(<env>)`（如 `(base) `、`(.venv) `）与双行框线前导符（`└─`、`╰─`、`┗━` 及其紧随连字符）；
     - 剥离后若 `p` 为空且原串带有双行框线前导符（如 `└─# `、`└─$ `、`╰─❯ `），判定为合法 Prompt；
     - 新增合法形态：
       - Unix 路径型：`p.starts_with('/') && !p.contains(' ')`（如 `/etc/nginx# `）或 `user:/path`（含 `:` 且 `:` 右侧以 `/` 或 `~` 开头且不含空格）；
       - 标准 Shell / 短主机名型：当 `marker` 为 `"$ "`、`"% "` 或 `"# "` 时，`p` 匹配简短标识符（如 `bash-5.2`、`sh-5.1`、`zsh` 或 `[a-zA-Z0-9._-]{1,32}` 且仅对 `"% "` / `"$ "` 放行纯主机名，防止 `# word` 注释误判！）。
   - **新增 Windows CMD 盘符无空格提示符识别**：
     - 匹配行首（可选 `(<env>) ` 或 `PS ` 后）形如 `[A-Za-z]:\` 开头、不含 `<` / `|` / `"`、以单个 `>` 结束路径前缀（且 `>` 前一字符不是 `-` / `=`、后一字符不是 `=` / `>`，排除 `->`、`=>`、`>=`、`>>`），支持 `C:\Users\Admin>type main.rs` 与 `PS C:\Users\Admin> gc main.rs`。
2. **扩充 `evaluate_pipeline_stage` 文件读取命令**：
   - 对命令名做大小写不敏感匹配 `"type" | "gc" | "get-content"`：
     - 针对 PowerShell `gc` / `Get-Content`，正确处理带参选项：`-Path` / `-LiteralPath` 的下一个 Token 作为候选文件路径；`-TotalCount` / `-Head` / `-Tail` / `-Last` / `-First` / `-Encoding` / `-Filter` / `-Include` / `-Exclude` 跳过其紧随的参数值 Token；忽略 `-Raw` / `-ReadCount` 等开关；遇到 `-Wait` 视同 `tail -f` 流式读取；最终仅当提取到唯一单个目标文件时返回 `CommandSyntaxTarget::Hint(file)`。
   - **将 `bat` / `batcat` 与纯无色读取命令区分标记**：在 `CommandSyntaxTarget::Hint` 中携带 `allow_conpty_white: bool`（仅 `cat` / `tac` / `head` / `tail` / `more` / `less` / `type` / `gc` / `get-content` 设为 `true`，`bat` / `batcat` / `diff` / `git` / `kubectl` 均为 `false`）。

### 2.2 `PromptScanCache` 正负双向缓存、`OSC 133` 修正与受限 ConPTY 白字兼容（[engine.rs](file:///home/wwwroot/mica-term-gpui/src/terminal/engine.rs) & [session.rs](file:///home/wwwroot/mica-term-gpui/src/terminal/session.rs)）
1. **修复 `scan_osc_sequences` 终止符优先级**：
   - 将 [engine.rs](file:///home/wwwroot/mica-term-gpui/src/terminal/engine.rs#L1278-L1281) 的 `.or_else` 改为同时查找 `\x07` 与 `\x1b\\` 并取字节偏移**更小（先到达）**的那一个，防止同一批次紧邻多条混合终止符 OSC 序列吞并中间内容。
2. **引入 `PromptScanCache` 消除无 Prompt 长日志重复回溯**：
   - 在 `AlacrittyTerminalEngine` 新增轻量锚点缓存，记录在当前窗口列宽 `cols` 下已知的最近 Prompt 逻辑行锚点列表及“已扫描确认无 Prompt 的历史区间 `[checked_start_abs, checked_end_abs]`”；
   - 当 `history_size` 增长（普通换行滚屏）时，按真实网格行移量或行首摘要校验平移有效区间；`resize`、清屏或历史回退时清空缓存；
   - `find_enclosing_prompt_start` 查找当前视口所属命令起点时：
     - 先在已缓存锚点 / 已检查负区间中二分查询：若视口顶部落在“已知无 Prompt 区间”内，**`O(1)` 立即返回 `view_top`，零行字符串拼接**；
     - 若 `osc133_records` 存在候选起始行，先校验该行处于 `[min_history_gl, view_top]` 内且实际逻辑行匹配 `detect_prompt_command_byte_offset(&text)` 的 `has_cmd == true` 才采纳（绝不盲目夹逼越界记录）；
     - 否则仅对尚未扫描过的历史缺口区间向上扫描（上界为 `min_history_gl`），将扫描结果（命中锚点或无命中区间）写回 `PromptScanCache`。
3. **调优单块行数预算与超长行恢复**：
   - 将 `MAX_SYNTAX_BLOCK_LINES` 提升至 `8_000` 行；
   - 保留现有“单逻辑行 `> 2048` 字节中止后续有状态 checkpoint，且当活动尾行重排/回退缩短后自动恢复解析”的既有契约，不做可能染错多行注释/heredoc 状态的假 32 行预热。
4. **Windows 本地 ConPTY `NamedColor::White` 精确受限放宽**：
   - 在 `AlacrittyTerminalEngine` 增加字段 `is_windows_local_pty: bool`（默认 `false`，仅在 `TerminalSessionEntity::new_local` 且 `cfg!(target_os = "windows")` 时设为 `true`）；
   - 在 `highlight_single_output_block` 中：仅当 `self.is_windows_local_pty && target.allows_conpty_white()` 时，先快速扫描本块可见与已缓存行的单元格——若块内存在任何非默认背景或除 `Foreground` / `White` 之外的显式 ANSI 颜色（如红/绿/黄/蓝/256色/TrueColor），则判定该命令自带彩色输出，**整块禁用白色放宽**；仅在整块纯单色白字/默认字且单元格满足 `bg_opt.is_none() && !flags.intersects(Flags::BOLD | Flags::INVERSE | Flags::HIDDEN | Flags::DIM)` 时，代码语法色才允许覆盖 `NamedColor::White`（URL 与诊断层仍严守原守卫）。

### 2.3 内置 `PowerShell.sublime-syntax` 与语法映射实测（[src/syntax/mod.rs](file:///home/wwwroot/mica-term-gpui/src/syntax/mod.rs)）
1. 新增 `src/syntax/powershell.sublime-syntax`（纯无反向引用正则编写，覆盖 `#` 与 `<# ... #>` 注释、单双引号字符串与 `` ` `` 转义、`$env:VAR` / `$var` 变量、`function/param/if/elseif/else/foreach/switch/try/catch/return` 关键字、`Get-*/Set-*/Invoke-*` Cmdlet、`-Parameter` 参数与数字常量）；
2. 在 `SyntaxRegistry` 初始化与 `reload_from_dir` 构建基础 `SyntaxSet` 时，在用户拓展之前加载这份内置 `powershell.sublime-syntax`（用户拓展目录若有同名自定义语法仍可覆盖）；
3. 在 `devops_basename_alias` 补充 `"hosts" | "fstab" => Some("sh")`、`".npmrc" | ".yarnrc" => Some("ini")`；单测对 `.rs`, `.py`, `.sh`, `.ps1`, `.psm1`, `.bat`, `.vue`, `.svelte`, `.tf`, `.proto`, `.graphql`, `.fish`, `Cargo.toml`, `nginx.conf` 逐一断言 `find_syntax` 返回预期的具体 `syntax.name`。

---

## Task 3：超链接 `HTTP(S)` 安全网关、渐进下划线与动态「小手」光标

### 3.1 静态去自增下划线与保留原生 `SGR 4`（[colors.rs](file:///home/wwwroot/mica-term-gpui/src/terminal/colors.rs)）
1. 修改 `TerminalPalette::semantic_style` 中 `OutputUrl` 与 `OutputFileLine` 的第 3 元组项 `add_underline` 为 **`false`**（Dark `OutputUrl` 设为柔和 `#6CB6FF`，`OutputFileLine` 设为 `#56C2C6`；Light 保持 `#0969DA` 与 `#1B7C83`）。
2. [engine.rs](file:///home/wwwroot/mica-term-gpui/src/terminal/engine.rs#L574) 中 `let mut underline = flags.intersects(Flags::ALL_UNDERLINES);` 保持不变，确保远端程序显式输出的 `SGR 4` 原生下划线不受影响。

### 3.2 统一 `HTTP(S)` 安全校验、括号平衡 URL 提取与双向 `WRAPLINE` 命中（[semantic.rs](file:///home/wwwroot/mica-term-gpui/src/terminal/semantic.rs) & [engine.rs](file:///home/wwwroot/mica-term-gpui/src/terminal/engine.rs)）
1. **实现唯一可打开链接校验器 `pub fn validate_openable_http_url(raw: &str) -> Option<String>`**：
   - 去除首尾空白无残留后，拒绝含任何空白字符或 ASCII 控制字符（`<= 0x1F` 或 `0x7F`）的字符串；
   - 大小写不敏感匹配 `http://` 或 `https://` 前缀；
   - 提取 authority 段（`scheme://` 后至首个 `/`、`?`、`#` 之前），去掉可选 `user@` 后校验主机非空：
     - 若以 `[` 开头（IPv6），必须包含配对 `]` 且内部仅含十六进制与 `:` / `.`，可选 `:port` 为 `1..=65535`；
     - 否则主机段非空、不含非法分隔符（如 `\`、`:` 多重未括号包裹），可选 `:port` 合法，且主机为 `localhost`、合法 IPv4 或含至少一个字母/点号的域名（支持短内网地址如 `http://git:3000`！修正原 `tok.len() > 10` 误拒短内网 URL 的问题）。
2. **升级普通文本 URL 扫描器 `extract_http_url_byte_ranges(line: &str) -> Vec<(usize, usize)>`**：
   - 不再依赖先按空白切词，而是在行内大小写不敏感扫描 `http://` 与 `https://` 起点（要求起点位于行首或前一字符非 ASCII 字母数字，避免截断 `myhttp://`）；
   - 向右扫描直至遇到空白字符或分隔符 `<`、`>`、`"`、`'`、`` ` ``；
   - 尾部标点智能修剪：剥离尾随 `,`、`;`、`.`（句末句号）、`!`、`?`（仅当问号后无查询内容时）及冒号（非端口）；对 `)`、`]`、`}` 采用**配对计数平衡法**——若 URL 内部开括号数 `<` 闭括号数，仅剥离多出来的外层闭括号，完整保留 `https://en.wikipedia.org/wiki/Rust_(programming_language)` 与 `http://[::1]:8080/`，并天然支持 `[链接](https://example.com/a?x=1&y=2#top)` 与 `{"url":"https://example.com/api/"}`；
   - 每个候选子串均通过 `validate_openable_http_url` 校验后才输出字节范围，**静态语义着色、鼠标悬停下划线、左键打开完全共享此函数结果**。
3. **引擎双向拼行与 `OSC 8` 精确身份归并（`engine.rs`）**：
   - 定义返回结构：
     ```rust
     #[derive(Debug, Clone, PartialEq, Eq)]
     pub struct HoveredLinkMatch {
         pub url: String,
         /// 当 OSC 8 显示文字非该 URL 本身且域名不同时，提供目标主机提示文案
         pub osc8_host_hint: Option<String>,
         /// 当前视口内的左闭右开列区间 `(vp_row, start_col, end_col)`
         pub viewport_spans: Vec<(usize, usize, usize)>,
     }
     ```
   - 实现 `AlacrittyTerminalEngine::link_at(&self, vp_row: usize, col: usize) -> Option<HoveredLinkMatch>`：
     1. **先查 `OSC 8`**：读取网格 `(grid_line, Column(col))`（若落在双宽字符占位格则回退一列）的 `cell.hyperlink()`。若存在 `hyperlink`，先调用 `validate_openable_http_url(hyperlink.uri())`——**校验不通过（如 `file://`、自定义协议）直接忽略**；校验通过后，沿前后相邻行按 **`cell.hyperlink().as_ref() == Some(&hyperlink)`（同时比对 Alacritty 的内部 `id` 与 `uri` 相等！）** 向左右扩展连续范围，将其在当前视口内的片段收集为 `viewport_spans`（同时在 `snapshot()` 静态渲染时，仅对通过 `validate_openable_http_url` 的无色 `OSC 8` 单元格赋予柔和 `OutputUrl` 前景色）。
     2. **再查普通文本 URL**：从 `grid_line` 向上沿前一行的 `row_has_wrapline` 回退至真实逻辑行起点 `logical_start_gl`（允许跨出视口顶部！），再调用 `stitch_logical_line_at` 向后拼到完整逻辑行末尾（允许跨出视口底部！）；在完整逻辑行上运行 `extract_http_url_byte_ranges`，定位包含当前 `(grid_line, col)` 对应逻辑字节偏移的那条 URL，再将该 URL 的字节范围回投到当前视口可见物理行，生成 `viewport_spans`！

### 3.3 修复 Windows 动态光标更新 Bug 与跨分屏悬停状态机
1. **修复 [patches/gpui-pre-windows/src/events.rs](file:///home/wwwroot/mica-term-gpui/patches/gpui-pre-windows/src/events.rs#L1177-L1191) `handle_cursor_changed`**：
   ```rust
   fn handle_cursor_changed(&self, handle: HWND, lparam: LPARAM) -> Option<isize> {
       let prev_cursor = self.state.current_cursor.get();
       let new_cursor = if lparam.0 == 0 {
           None
       } else {
           Some(HCURSOR(lparam.0 as _))
       };
       self.state.current_cursor.set(new_cursor);

       if prev_cursor.map(|c| c.0) != new_cursor.map(|c| c.0)
           && self.state.hovered.get()
           && unsafe { IsWindowEnabled(handle).as_bool() }
           && self.is_cursor_in_client_area(handle)
       {
           let effective = if self.state.cursor_visible.load(Ordering::Relaxed) {
               new_cursor
           } else {
               None
           };
           unsafe { SetCursor(effective) };
       }
       Some(0)
   }
   ```
   *(其中 `is_cursor_in_client_area` 通过 `GetCursorPos` + `SendMessageW(handle, WM_NCHITTEST, ...)` 确认结果为 `HTCLIENT`，确保多窗口广播时不干扰非悬停窗口、禁用模态父窗口、窗口缩放边框及打字隐藏光标状态！)*
2. **构建无残留的分屏悬停状态与根级修饰键同步（[workspace.rs](file:///home/wwwroot/mica-term-gpui/src/ui/workspace.rs) & [terminal_view.rs](file:///home/wwwroot/mica-term-gpui/src/ui/terminal_view.rs)）**：
   - **真实网格几何命中 `try_point_to_grid_cell(&self, pos: Point<Pixels>, rows: usize, cols: usize) -> Option<(usize, usize)>`**：区别于拖选夹逼用的 `point_to_grid_cell`，严格要求 `pos` 落在 `inner_bounds` 实际网格像素宽度 `cell_w * cols` 与高度 `cell_h * rows` 之内，且不在右侧滚动条区、搜索浮层或连接状态遮罩层（`!matches!(phase, SessionPhase::Connected)`）上，越界直接返回 `None`；
   - `TerminalPaneView` 只保存几何指针状态 `hover_pointer: Option<Point<Pixels>>` 与 `hover_link_mod: bool`（**不持久化缓存静态 `spans`，而是在每次 `render()` 拿到最新 `snapshot` 且确认 `window.is_window_hovered()` 时，实时用 `hover_pointer` 调用 `session.read(cx).link_at(row, col)` 计算本次帧的 `active_hover_link`**——这样滚屏、远端输出覆盖、窗口 `resize`、离窗后天然 100% 零旧跨度残留！）；
   - 在 `terminal_canvas` 挂载 `.on_hover(cx.listener(|this, hovered: &bool, _, cx| { if !*hovered && this.hover_pointer.take().is_some() { cx.notify(); } }))` 处理移出分屏，并通过 `cx.observe_window_activation(window, ...)` 在窗口失活（如 `Alt+Tab`）时清零 `hover_link_mod`；
   - 在 [workspace.rs](file:///home/wwwroot/mica-term-gpui/src/ui/workspace.rs) 根容器挂载 `.on_modifiers_changed`，将最新 `modifiers` 同步给当前活动标签下的所有可见分屏 `pane.update(cx, |p, cx| p.sync_modifiers(ev.modifiers, cx))`（内部仅当该分屏 `hover_pointer.is_some()` 且 `link_mod` 状态变化时才 `cx.notify()`），彻底打通**焦点在左分屏、鼠标悬停右分屏原地按/松 `Ctrl` 即时变小手**！
   - **光标样式映射**：
     - 右侧 12px 滚动条区 $\rightarrow$ `CursorStyle::Arrow`；
     - `active_hover_link.is_some() && effective_link_mod` $\rightarrow$ **`CursorStyle::PointingHand`（小手）**；
     - `snapshot.mode.intersects(TermMode::MOUSE_MODE)`（且未按 `Shift`）$\rightarrow$ `CursorStyle::Arrow`；
     - 其余终端网格区域 $\rightarrow$ **`CursorStyle::IBeam`（文本光标）**。

---

## Task 4：鼠标手势归属、多行粘贴保护、触控板余量与边缘自动滚屏

### 4.1 领域配置平滑演进与精准一次迁移（[domain/mod.rs](file:///home/wwwroot/mica-term-gpui/src/domain/mod.rs)、[shortcuts.rs](file:///home/wwwroot/mica-term-gpui/src/domain/shortcuts.rs)、[models.rs](file:///home/wwwroot/mica-term-gpui/src/domain/models.rs)、[store.rs](file:///home/wwwroot/mica-term-gpui/src/storage/store.rs)）
1. 在 [shortcuts.rs](file:///home/wwwroot/mica-term-gpui/src/domain/shortcuts.rs) 新增并从 [domain/mod.rs](file:///home/wwwroot/mica-term-gpui/src/domain/mod.rs) 导出：
   ```rust
   #[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
   pub enum RightClickAction {
       #[default]
       ContextMenu,
       SmartCopyPaste,
   }
   ```
2. 在 [models.rs](file:///home/wwwroot/mica-term-gpui/src/domain/models.rs) `AppSettings` 中：
   - 新增 `#[serde(default)] pub right_click_action: RightClickAction`；
   - 新增 `#[serde(default)] pub copy_on_select: bool`；
   - 新增迁移标记 `#[serde(default)] pub mouse_v2_migrated: bool`（`AppSettings::default()` 中设为 `true`！）。
3. **无歧义存储迁移逻辑（[store.rs](file:///home/wwwroot/mica-term-gpui/src/storage/store.rs#L127-L147)）**：
   - 在 `load_snapshot` 中先以 `serde_json::Value` 读取原始 `KEY_SETTINGS`（或在 `!settings.mouse_v2_migrated` 时判断）：
     - **仅当 `!settings.mouse_v2_migrated` 且原始 JSON 不含 `"right_click_action"` 字段且 `settings.double_click_action == DoubleClickAction::Paste` 时**，将 `settings.right_click_action` 迁移为 `RightClickAction::SmartCopyPaste`，同时将 `settings.double_click_action` 归一为 `DoubleClickAction::SelectWord`；
     - 若原始 JSON 已显式包含 `"right_click_action"`（哪怕值为 `"ContextMenu"`），尊重已有新字段值绝不覆盖；
     - 随后置 `settings.mouse_v2_migrated = true` 并复用 `load_snapshot` 既有的一次写回机制持久化，后续重启永不重复触发！
4. 同步更新 `PanePreferences`（[terminal_view.rs](file:///home/wwwroot/mica-term-gpui/src/ui/terminal_view.rs#L50)）、`MicaWorkspace::current_pane_preferences`（[workspace.rs](file:///home/wwwroot/mica-term-gpui/src/ui/workspace.rs#L316)）、`sync_preferences` 热更新广播以及 [modals.rs](file:///home/wwwroot/mica-term-gpui/src/ui/modals.rs#L1995) 偏好设置 UI（提供「终端右键动作：`弹出菜单` | `智能复制 / 粘贴`」与「选中文本自动复制」Switch）。

### 4.2 左右中键手势归属状态机与 `on_mouse_up_out` 配对（[terminal_view.rs](file:///home/wwwroot/mica-term-gpui/src/ui/terminal_view.rs)）
1. 在 `TerminalPaneView` 定义明确的手势归属状态：
   ```rust
   #[derive(Clone, Copy, PartialEq, Eq)]
   enum LeftGesture {
       None,
       Scrollbar,
       LinkOpen,
       Tui { sent_press: bool },
       Selecting { multi_click: bool, dragged: bool },
   }

   #[derive(Clone, Copy, PartialEq, Eq)]
   enum RightGesture {
       None,
       Menu,
       Tui { sent_press: bool },
       SmartAction,
   }
   ```
2. **左键完整闭环（`on_mouse_down` + `on_mouse_move` + `on_mouse_up` + `on_mouse_up_out`）**：
   - `Down`：
     - 右侧滚动条 $\rightarrow$ `LeftGesture::Scrollbar`；
     - `link_mod` 且 `try_point_to_grid_cell` 命中 `link_at` $\rightarrow$ `LeftGesture::LinkOpen`，调用 `cx.open_url(&link.url)`；
     - 未按 `Shift` 且 `mode.intersects(TermMode::MOUSE_MODE)` $\rightarrow$ 尝试发送 Press，记录 `LeftGesture::Tui { sent_press }`；
     - 其余进入本地选区 $\rightarrow$ `session.begin_selection(row, col, ev.click_count.max(1), cx)`，记录 `LeftGesture::Selecting { multi_click: ev.click_count >= 2, dragged: false }`。
   - `Move`（按住左键）：
     - `Scrollbar` $\rightarrow$ 更新滚动比例；
     - `Tui { .. }` $\rightarrow$ 按需上报 Drag；
     - `Selecting { .. }` $\rightarrow$ 置 `dragged = true`，更新选区端点，并联动 4.4 节边缘自动滚屏！
   - `Up` 与 `UpOut`（共用同一收尾函数 `finish_left_gesture`）：
     - 先 `std::mem::replace(&mut self.left_gesture, LeftGesture::None)` 停止边缘自动滚屏任务；
     - 若为 `Tui { sent_press: true }`：调用不检查 `mods.shift` 的底层释放方法 `session.send_mouse_release_unconditional(MouseButton::Left, col, row, ev.modifiers)` 配对发送一次 Release；
     - 若为 `Selecting { multi_click, dragged }` 且 `self.copy_on_select && (multi_click || dragged)`：读取一次 `session.read(cx).selected_text()`（非空即写入剪贴板，不做 `.trim()`；普通单击清选 `multi_click == false && dragged == false` 绝不触碰剪贴板！）。
3. **右键完整闭环（严格执行计划文档 §2.1 归属表）**：
   - `Down`：
     - `ev.modifiers.shift` $\rightarrow$ 聚焦激活，设 `RightGesture::Menu`，不调 `stop_propagation()`，交由外层 `ContextMenuExt` 弹出完整菜单；
     - 无 `Shift` 且 `mode.intersects(TermMode::MOUSE_MODE)` $\rightarrow$ 聚焦激活，尝试编码发送右键 Press，设 `RightGesture::Tui { sent_press }`，调用 `cx.stop_propagation()`（即使非 SGR 坐标 `>223` 未发出字节，也必须 `stop_propagation()` 消费手势，严禁回落成粘贴！）；
     - 无 `Shift` 且 `self.right_click_action == RightClickAction::SmartCopyPaste` $\rightarrow$ 聚焦激活，设 `RightGesture::SmartAction`，调用 `cx.stop_propagation()`：若 `session.read(cx).selected_text()` 为 `Some(text)`，写入剪贴板并调用 `clear_selection`；否则读取剪贴板文本调用 `this.request_paste(text, window, cx)`；
     - 其余（`ContextMenu`）$\rightarrow$ 聚焦激活，设 `RightGesture::Menu`，冒泡弹菜单。
   - `Up` 与 `UpOut`：仅当 `std::mem::replace(&mut self.right_gesture, RightGesture::None)` 为 `RightGesture::Tui { sent_press: true }` 时，配对发送一次右键 Release。

### 4.3 统一粘贴入口与非安全模式多行粘贴确认（[modals.rs](file:///home/wwwroot/mica-term-gpui/src/ui/modals.rs) & [terminal_view.rs](file:///home/wwwroot/mica-term-gpui/src/ui/terminal_view.rs)）
1. 将全部 5 个粘贴入口——① `ShortcutAction::Paste`、② 右键菜单「粘贴」项、③ `RightClickAction::SmartCopyPaste` 无选区右键、④ `MouseButton::Middle` 中键粘贴、⑤ `EntityInputHandler::paste`——统一改为调用 `TerminalPaneView::request_paste(&mut self, text: String, window: &mut Window, cx: &mut Context<Self>)`。
2. **多行粘贴安全判定**：
   - 检查 `!text.is_empty()` 且会话处于 `SessionPhase::Connected`；
   - 若 `!self.session.read(cx).mode().contains(TermMode::BRACKETED_PASTE)` **且** `text.contains(['\n', '\r'])`（含三击选行复制自带的末尾换行！）：
     - 调用 `modals::open_multiline_paste_confirm_dialog(line_count, preview_snippet, window, cx, on_confirm)`（展示总行数、最多前 6 行等宽代码框预览及“当前终端未开启括号粘贴保护，粘贴换行将立即执行命令”提示；点击「确认粘贴」时向绑定的原 `session` 写入一次当时快照文本，点击「取消」或按 `Esc` 零字节写入！）；
   - 其余情况（单行无回车文本，或远端 Shell 已开启 `BRACKETED_PASTE` 安全包裹模式）：直接调用 `session.update(cx, |s, cx| s.send_paste(&text, cx))`。

### 4.4 触控板像素滚动累积余量与边缘拖选自动滚屏（[terminal_view.rs](file:///home/wwwroot/mica-term-gpui/src/ui/terminal_view.rs)）
1. **触控板像素滚动余量（`scroll_line_remainder: f32`）**：
   - 在 `.on_scroll` 中：
     - `Ctrl + 滚轮` 缩放与 TUI `MOUSE_MODE` 滚轮上报维持原即时响应；
     - 对常规滚屏：当 `ev.delta` 为 `ScrollDelta::Pixels(pt)` 时，计算增量 `delta_lines = f32::from(pt.y) / ch`；若 `self.scroll_line_remainder * delta_lines < 0.0`（方向反转）先清零余量；累加 `self.scroll_line_remainder += delta_lines`，取整数部分 `whole_lines = self.scroll_line_remainder.trunc() as i32`——若 `whole_lines == 0` 直接返回等待后续帧累积，否则 `self.scroll_line_remainder -= whole_lines as f32` 并执行 `scroll_lines(whole_lines, cx)`！（`ScrollDelta::Lines` 传统鼠标滚轮刻度仍直接按整数行滚动并清零像素余量）。
2. **边缘拖选自动滚屏（`auto_scroll_task: Option<Task<()>>`）**：
   - 在本地选区拖拽（`LeftGesture::Selecting`）的 `on_mouse_move` 中记录最新指针位置 `drag_pointer_pos: Point<Pixels>`；
   - 若 `drag_pointer_pos.y < inner_bounds.origin.y`（向上越界）或 `> inner_bounds.origin.y + inner_bounds.size.height`（向下越界）：
     - 按越界像素距离折算每拍步进 `step_lines = ((overshoot_px / cell_h).ceil() as i32).clamp(1, 5)`（向上为正 `scroll_lines(+step)`，向下为负 `scroll_lines(-step)`）；
     - 若 `auto_scroll_task` 未运行，通过 `cx.spawn` 启动 `32ms` 周期任务：每拍在仍处于 `LeftGesture::Selecting` 且指针越界时滚动 `step_lines` 并用当前夹逼后的边缘网格坐标调用 `update_selection`，一旦指针移回视口内、松手、失焦或 `display_offset` 到达边界无变化则自动退出释放任务。

---

## Task 5：全量自动化测试与隔离构建验收清单

### 5.1 必须覆盖的单测与回归矩阵
1. **超链接与安全边界测试（`semantic.rs` + `engine.rs`）**：
   - 拒绝非 HTTP(S) 目标：`file:///etc/passwd`、`\\10.0.0.1\share\a.exe`、`javascript:alert(1)`、`custom://open`、`http://`（空主机）在 `validate_openable_http_url` 与 `OSC 8` `link_at` 下均返回 `None`；
   - 复杂合法 URL 保留：Markdown `[文档](https://example.com/a(b)?x=1&y=2#frag)`、紧凑 JSON `{"u":"https://example.com/v1/"}`、IPv6 `http://[::1]:8080/health`、内网短地址 `http://git:3000` 均完整命中；
   - 双向软折行命中：在 20 列窄屏下灌入跨 3 行的长 URL，分别对第 0、1、2 行的列坐标调用 `link_at`，均返回同一完整 URL 及全部可见折行 `viewport_spans`；
   - 相邻不同 ID 的同 URI `OSC 8` 标签（`\x1b]8;id=1;https://a.com\x07一\x1b]8;id=2;https://a.com\x07二\x1b]8;;\x07`）分别独立点亮各自跨度；
   - 更新既有 `highlights_cat_shell_script_with_cjk_urls_and_ansi_priority` 测试：保留 CJK 双宽对齐、代码注释/关键字/字符串色与显式 ANSI 红字优先断言，将静态 URL 断言更新为 `underline == false` 且 `link_at` 命中完整 URL，同时新增断言远端显式 `\x1b[4m` 下划线依然保持 `underline == true`。
2. **`cat` 代码高亮与缓存测试（`semantic.rs` + `engine.rs` + `syntax/mod.rs`）**：
   - Prompt 与命令矩阵：覆盖 `❯ cat a.rs`、Oh-My-Zsh `➜  mica-term-gpui git:(refactor/gpui) ✗ cat Cargo.toml`、`└─# cat deploy.sh`、`/etc/nginx# cat nginx.conf`、`host% cat a.py`、`C:\work>type main.rs`、`PS C:\work> Get-Content -Tail 20 -Encoding utf8 app.ps1`；同时断言 `# 注释`、`> 引用`、`a -> b`、`x >= y` 返回 `None`；
   - 长文件与大历史性能边界：
     - `1,500` 行 `cat large.rs` 停在底部（`display_offset == 0`）仍完整着色；
     - 在 `5,000` 与 `30,000` 行无 Prompt 纯日志填满历史后连续多次调用 `snapshot()`，通过统计计数器断言第 2 次起 `PromptScanCache` 扫描逻辑行数为 `0`；
     - 验证同包投喂 `OSC 133 A+C+多行输出+D`、满历史软折行溢出、超长单行（`>2048B`）后重排缩短恢复、以及相同 `cat a.rs` 连续执行两次的缓存正确性；
   - ConPTY 白色放宽与混色撤销：断言 `is_windows_local_pty == false`（SSH）时 `\x1b[37m` 保持原白字不被染色；`is_windows_local_pty == true` 时纯 `\x1b[37m` 的 `type a.rs` 获得代码着色，而一旦块内夹杂 `\x1b[32m` 绿字则旁边的 `\x1b[37m` 白字与绿字均保持原色；
   - 语法名精确断言：断言 `.ps1` 命中内置 `"PowerShell"`，`.vue` / `.svelte` / `.tf` / `.proto` / `.graphql` / `.fish` 命中各自真实内置语法。
3. **设置迁移与鼠标/粘贴状态测试（`store.rs` + `terminal_view.rs`）**：
   - 覆盖 4 组持久化场景：① 旧版 `double_click_action: "Paste"`（无新字段）$\rightarrow$ 迁移为 `SmartCopyPaste` 且写回后修改回 `ContextMenu` 重启不再覆盖；② 旧版 `"SelectWord"` $\rightarrow$ 保持 `ContextMenu`；③ 新旧字段并存时以新字段为准；④ 更新 `store_persists_and_reloads_snapshot` 覆盖 `right_click_action` 与 `copy_on_select`；
   - 验证双击/三击在连续中文、路径、宽字符第 2 格上的选区行为，以及 `copy_on_select` 在“双击选词写剪贴板 vs 单击清选不改剪贴板 vs 点链接不改剪贴板”下的精确性。

### 5.2 构建与 PE 资源实证命令序列
实施完成后依次执行以下验证命令（使用隔离构建目录避免污染无关发布区）：
```bash
# 1. 生成图标资产并校验 9 张 PNG 与多帧 ICO 头结构
python3 scripts/generate-icons.py

# 2. 运行全量单元测试与编译检查
cargo test

# 3. 交叉编译 Windows x64 Release 产物（仅本地构建，跳过外部发布）
OUTPUT_DIR=target/verify-publish ./build-win-64.sh --skip-publish

# 4. 使用 llvm-readobj 实证检查生成的 EXE 同时包含 RT_GROUP_ICON(ID 1)、9 帧 RT_ICON、RT_VERSION 与原有 RT_MANIFEST(ID 1)
llvm-readobj --coff-resources target/x86_64-pc-windows-msvc/release/mica-term-gpui.exe
```

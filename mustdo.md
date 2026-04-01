# Windows Native Terminal Surface Must-Do

日期: 2026-04-01
状态: 未完成，当前必须以“修复/恢复”而不是“已完成”视角继续推进
适用范围: `mica-term` Windows native terminal surface, terminal font stack, packaging, Windows 真机验证

## 为什么需要这份清单

当前仓库里已经有一批 `WindowsNativeSurfaceBackend` / `NativeTerminalSurface` / `DirectWriteFontSystem` 相关代码，
也已经有多份 2026-04-01 文档把工作描述成“已完成实现”。
但 Windows 真机构建后的真实表现仍然是“终端区域空白、只有光标闪烁”，因此这些“完成态”结论不能再被继续引用。

这份文件的目的只有一个：把已经证实的 blocker、还没做完的功能、必须继续移植的代码来源、
以及接下来的真实执行顺序全部写清楚，避免再次把“能编译”“有 source-level test”“有 D2D 结构体”误当成“Windows 真机可用”。

## 当前已确认事实

1. `build-win-x64-software.sh` 仍然走 `slint-renderer-software` + `terminal-native-renderer` 组合。
   - 证据: `build-win-x64-software.sh:40`
2. `NativeTerminalSurface` 目前通过 `window.window().set_rendering_notifier(...)` 来触发 native backend 的 `present()`。
   - 证据: `src/app/terminal_renderer/native_surface.rs:52-69`
3. `NativeTerminalSurface` 里真正调用 backend 绘制的地方只有 `draw_retained_frame()` -> `state.backend.present()`。
   - 证据: `src/app/terminal_renderer/native_surface.rs:138-142`
4. `i-slint-core` 的默认 `set_rendering_notifier()` 实现返回 `Unsupported`。
   - 证据: `/root/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/i-slint-core-1.15.1/renderer.rs:115-119`
5. 当前 vendored 的 `winit-software` 路径没有看到针对 notifier 的支持实现；Skia renderer 明确实现了 notifier。
   - 证据: `vendor/i-slint-backend-winit/renderer/sw.rs`
   - 证据: `/root/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/i-slint-renderer-skia-1.15.1/lib.rs:896-902`
6. Windows backend 已经存在真实的 Direct2D 绘制函数，但它们是否被稳定触发是另外一个问题。
   - 证据: `src/app/terminal_renderer/platform/windows.rs:499-754` 已有 `draw_background_runs` / `draw_monochrome_glyphs` / `draw_color_glyphs` / `draw_selection_overlay` / `draw_underline_overlay` / `draw_cursor_overlay` / `draw_ime_preview_overlay`
7. 当前 `DirectWriteFontSystem` 名字像 DirectWrite，但实际仍是 `ab_glyph + swash + rustybuzz` 的过渡实现，并不是真正的 Windows system font stack。
   - 证据: `src/app/terminal_font/windows_dwrite.rs:1-272`

## 当前代码里“看起来像完成，实际上还不能算完成”的部分

### 1. Windows D2D backend 已经能画，但绘制触发链路不可靠

- `src/app/terminal_renderer/platform/windows.rs` 已经具备 D2D factory、`ID2D1HwndRenderTarget`、brush/bitmap cache、overlay draw 函数。
- 但在当前 `winit-software` 路径下，`present()` 很可能根本没有被稳定调用。
- 这意味着“backend 代码存在”不等于“Windows 真机看得到文本”。

### 2. `DirectWriteFontSystem` 只是“Windows 风格接口包装”，不是真正的 DirectWrite font backend

- 当前实现仍然只加载 bunded `Fusion-JetBrainsMapleMono` 主字体。
- fallback family 只是字符串列表，不是真实的 fallback font locate + shape。
- `shape_text_runs()` 最终仍只拿第一个 face 结果。

### 3. 彩色 emoji 仍是假数据而不是真字形

- `rasterize_color_glyph()` 目前只是根据 `glyph_id` 生成假 RGBA 方块。
- 这能让 contract test 过，但不能代表真正的 Windows emoji/彩色字形渲染完成。

### 4. ligature / OpenType feature 合同已经建了，但并未真正喂给 shaping

- `OpenTypeFeatureSet` 已有结构。
- `shape_text_with_rustybuzz()` 仍然用 `shape(&face, &[], buffer)`，feature 数组是空的。
- 这意味着 `liga` / `calt` / 其他 feature 当前并没有真正生效。

### 5. damage tracking / partial redraw / frame scheduling 还没达到成熟终端级别

- 当前 backend 有 retained frame 和若干 draw counter。
- 但还没有像 Alacritty 那样完整的 `DamageTracker` / frame swap / resize / device-loss hardening 体系。

## 已确认 blocker

### Blocker A: software renderer 路径下的 notifier / present hook 不可信

这是当前“终端区空白”的头号怀疑点，也是优先级最高的 blocker。

如果 `present()` 没有被调用：
- 即使 D2D 绘制代码本身正确，窗口里也不会出现文本
- 只能看到宿主 UI 或系统/光标层的变化
- 继续盲改 `windows.rs` 很可能只是把代码越堆越多，但症状不变

### Blocker B: Windows 字体栈并未真正 native 化

当前实现缺少：
- system font enumerate / locate
- DirectWrite fallback mapping
- 多 face shaping
- fallback chain 递进尝试
- 真实 color emoji / symbol font

所以“Windows native font backend 已完成”这个说法不成立。

### Blocker C: source-level test 覆盖了很多合同，但没有证明 Windows 真机渲染成功

现在不少测试验证的是：
- 某个字段存在
- 某段函数源码出现
- 某个 contract 数据结构有对应 payload

这些都很有价值，但它们证明不了：
- 真机进程里 `present()` 被调用了
- D2D target 成功 `BeginDraw/EndDraw`
- 文本真的显示到了 Windows 窗口上

## 必须完成的事项

### 1. 先修复“绘制调用链路”而不是继续堆叠 backend 细节

必须把 native draw 的触发链从“单点依赖 `set_rendering_notifier`”改成可验证、可诊断、可在 Windows 软件路径下稳定运行的机制。

最少需要做到：
- 引入独立的 present driver / render trigger seam
- 能记录 `present()` 实际调用次数、最近一次 frame token、最近一次 D2D `EndDraw()` 结果
- 能区分“frame 准备成功”和“frame 真正提交到了 native target”

优先涉及文件：
- `src/app/terminal_renderer/native_surface.rs`
- `src/app/terminal_renderer/platform/mod.rs`
- `src/app/terminal_renderer/platform/windows.rs`
- `src/app/windows_frame.rs`
- `src/app/bootstrap.rs`
- `src/app/runtime_profile.rs`

### 2. 增加可用于 Windows 真机排障的 runtime diagnostics

至少要能在日志或调试状态里看到：
- `attach()` 是否拿到 `HWND`
- render target 是否创建成功
- 每次 `present()` 是否被调用
- D2D `BeginDraw/EndDraw` 是否报错
- 当前 frame 内绘制了多少 background / mono glyph / color glyph / selection / underline / cursor / ime preview

这一步不只是为了调试，也为了防止下次再把“看起来应该能画”当成“已经画出来了”。

### 3. 把 Windows 字体定位与 fallback 真正做成 system-backed 实现

当前必须补的能力：
- system font enumerate / locate
- 按 codepoint/cluster 选择 fallback font
- fallback font handle 到实际 font bytes 或 font face
- primary face 与 fallback face 的 metrics 对齐策略

优先涉及文件：
- `src/app/terminal_font/windows_dwrite.rs`
- 建议新增 `src/app/terminal_font/windows_locator.rs`
- 建议新增 `src/app/terminal_font/windows_fallback.rs`
- `tests/terminal_renderer_dwrite_spec.rs`
- `tests/terminal_layout_harfbuzz_spec.rs`

### 4. 把多 face shaping / OpenType features / ligature 真正接进 shaping 流程

必须补上：
- feature tag -> HarfBuzz feature array 映射
- 单 cluster 失败时递归 fallback 尝试
- cluster / cell width / ligature 映射稳定性
- emoji / symbol / CJK 宽度边界不被 fallback 破坏

优先涉及文件：
- `src/app/terminal_font/backend.rs`
- `src/app/terminal_font/windows_dwrite.rs`
- `src/app/terminal_layout/shaper.rs`
- `tests/terminal_layout_harfbuzz_spec.rs`
- `tests/terminal_renderer_dwrite_spec.rs`

### 5. 把 color glyph / emoji 从“假图”升级成真实 Windows 路径

必须补上：
- 真实 COLR/CPAL 或等价彩色字形解析/绘制路径
- color glyph cache 与 mono atlas 分离
- D2D 侧 color bitmap 上传/复用策略
- selection 覆盖 emoji 后仍可见

优先涉及文件：
- `src/app/terminal_font/windows_dwrite.rs`
- `src/app/terminal_renderer/platform/windows.rs`
- `src/app/terminal_renderer/wgpu_renderer.rs`
- `tests/terminal_color_emoji_spec.rs`

### 6. 把 damage tracking / partial redraw / resize / device-loss / shutdown 顺序补齐

必须补上：
- frame damage 合并
- resize 后 bitmap/brush/render-target 资源刷新
- device loss / `D2DERR_RECREATE_TARGET` 恢复
- window close / session dispose 后禁止悬挂回调继续写 surface

优先涉及文件：
- `src/app/terminal_renderer/platform/windows.rs`
- `src/app/terminal_renderer/native_surface.rs`
- `src/app/windows_frame.rs`
- 必要时新增 `src/app/terminal_renderer/damage.rs`

### 7. 重新定义“完成”的验收标准，并执行 Windows 真机验证

以下条件缺一不可：
- `cargo check --workspace` 通过
- `cargo clippy --workspace -- -D warnings` 通过
- `./build-win-x64-software.sh` 通过
- `./build-win-x64.sh` 或 Windows 主线构建通过
- Windows 真机上终端文本、selection、underline、cursor、emoji 都可见
- resize、滚动、焦点切换、IME、关闭窗口不崩溃

## 可以直接参考 / 部分移植的 GitHub 仓库与文件

### Slint 官方 / 相关资料

1. `slint` notifier 能力边界
   - `https://docs.rs/slint/latest/slint/enum.SetRenderingNotifierError.html`
2. backends/renderers 文档
   - `https://docs.slint.dev/latest/docs/slint/guide/backends-and-renderers/backends_and_renderers/`
3. maintainer 关于自定义渲染/平台的建议
   - `https://github.com/slint-ui/slint/discussions/6754`

建议用途：
- 明确什么时候应该继续使用 notifier
- 明确什么时候应该自建 `Platform` / `WindowAdapter`
- 明确 `winit-software` 与 Skia 路径的能力差异

### WezTerm（字体、fallback、彩色字形）

1. `wezterm/wezterm` -> `wezterm-font/src/shaper/harfbuzz.rs`
   - 可参考内容：
     - fallback font 递归尝试
     - cluster 解析与 cell width 修正
     - HarfBuzz feature 管理
     - ligature / grapheme / presentation width 处理
2. `wezterm/wezterm` -> `wezterm-font/src/locator/gdi.rs`
   - 可参考内容：
     - Windows 下 system font locate
     - GDI / DirectWrite fallback
     - `FontFallback::get_system_fallback()` 调用方式
3. `wezterm/wezterm` -> `wezterm-font/src/rasterizer/colr.rs`
   - 可参考内容：
     - COLR/CPAL 彩色字形 paint op
     - gradient / path paint 抽象

建议策略：
- 优先移植算法与状态机逻辑，不要盲目整包复制整个 crate 架构。
- 重点抄“fallback / shaping / color glyph 处理方法”，不要把 WezTerm 的完整 renderer 结构直接硬搬进来。

### Alacritty（damage / frame scheduling / overlay 组织）

1. `alacritty/alacritty` -> `alacritty/src/display/damage.rs`
   - 可参考内容：
     - `DamageTracker`
     - line damage 合并
     - viewport rect damage
     - partial present 区域合并
2. `alacritty/alacritty` -> `alacritty/src/display/mod.rs`
   - 可参考内容：
     - render loop 组织
     - IME preview / cursor / overlay 的绘制阶段顺序
     - swap / request frame / resize / context recovery

建议策略：
- 参考其 damage / frame lifecycle 思路
- 不要照抄 OpenGL / glutin / crossfont 结构；我们只需要“成熟终端如何组织 frame 生命周期”的经验

## 推荐执行顺序

1. 先修 notifier / present trigger / runtime diagnostics
2. 再做“真实可见文本”的最小 Windows 验证闭环
3. 再做 Windows font locate + fallback
4. 再做 feature / ligature / cluster 稳定性
5. 再做 color glyph / emoji
6. 再做 damage / resize / device-loss / shutdown hardening
7. 最后做 packaging、Windows 真机回归和新的 TDD 交接文档

## 明确禁止的自欺结论

以下结论在新的 Windows 真机验证之前一律禁止再说：

- “Windows native terminal surface 已完成”
- “DirectWrite backend 已完成”
- “emoji/color glyph 已完成”
- “build-win-x64-software.sh 可构建，所以运行时没问题”
- “source-level test 都过了，所以功能完成”

只有同时满足“真机可见文本 + 真机可见 overlay + 真机可见 emoji + resize/IME/关闭稳定”之后，
才能重新讨论“完成”二字。

## 完成标准

当且仅当满足以下全部条件时，才能把这个主题标记为完成：

1. Windows 真机首次打开终端标签时即可看到背景和文字，而不是空白面板
2. 光标、selection、underline、IME preview 都能随输入变化正确刷新
3. emoji / symbol / Nerd Font / CJK 混排可见，且不会退化为假色块
4. ligature / OpenType feature 的行为与设计一致，可通过测试和真机观察确认
5. resize、滚动、字体缩放、窗口关闭、session 销毁后没有悬挂回调或资源泄漏
6. `cargo check --workspace`、`cargo clippy --workspace -- -D warnings`、Windows 打包脚本全部通过
7. 新的 TDD / verification 文档如实记录剩余风险，而不是继续写“已完成实现”口径

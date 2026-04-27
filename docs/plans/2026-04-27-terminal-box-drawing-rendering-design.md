# Terminal Box-Drawing Rendering Design

**Date:** 2026-04-27
**Branch:** `master`

## Context

这轮需求的目标非常明确：不是改 terminal theme，也不是继续微调普通正文文本，而是专门修复 terminal 中 Unicode box-drawing / block element 的渲染质量问题，让 TUI 中由字符拼出来的边框、分隔线、盒子看起来更像“连续、完整、锐利的一体化边框”，而不是“很多单独 glyph 拼起来的线”。

用户给出的核心观察点包括：

- `│` 的连续性不足，视觉上有“一节一节”的感觉。
- `─`、`╭╮╰╯` 与相邻格子的连接不够紧，不像一条整体边框。
- 边缘存在一点发虚、发亮、发光、晕边感。
- 希望优先通过 special-case rendering 修正 box-drawing / block elements，而不是只动颜色。
- 普通英文、中文、数字、符号的现有渲染观感尽量不受影响。

## Current State

### 1. 当前默认终端策略

当前 runtime profile 采用 `Native` 优先，而不是 bitmap atlas 优先：

- `src/app/runtime_profile.rs`
- `src/app/bootstrap.rs`

但从实现完整度看，真正有实装的 native text path 主要是 Windows retained path：

- `src/app/terminal_renderer/platform/windows.rs`

Linux 下的：

- `src/app/terminal_renderer/platform/x11.rs`
- `src/app/terminal_renderer/platform/wayland.rs`

目前仍是 surface backend scaffold，不承担真正的文本几何绘制质量主责。

### 2. 当前普通文本链路

当前 native terminal 的主链路是：

1. runtime surface -> `TerminalModelFrame`
   - `src/app/terminal_model.rs`
2. row segmentation / cluster mapping
   - `src/app/terminal_layout/run_segmentation.rs`
3. shaping
   - `src/app/terminal_layout/shaper.rs`
4. font fallback + glyph raster request
   - `src/app/terminal_font/windows_dwrite.rs`
5. prepared draw list
   - `src/app/terminal_renderer/wgpu_renderer.rs`
6. Windows native present
   - `src/app/terminal_renderer/platform/windows.rs`

当前 Windows native 显示单色 glyph 时：

- 优先走 `DrawGlyphRun`
- 失败才回落到 atlas mask / `FillOpacityMask`

这意味着 box-drawing / block elements 目前实质上仍被当成“普通文本 glyph”来处理。

### 3. 当前 box/block 的识别状态

代码已经能识别这类字符属于“grid-like symbol”：

- `src/app/terminal_renderer/wgpu_renderer.rs`

其中 `is_grid_fitted_symbol(...)` 已覆盖：

- `U+2500–U+257F` Box Drawing
- `U+2580–U+259F` Block Elements

但这个识别目前只会产出 `PreparedMonochromeGlyphVisualFit::GridSymbol` 这样的视觉标签；它并没有真正改写渲染策略。也就是说：

- 这些字符当前没有专门 painter
- 没有按 cell geometry 生成 mask
- 没有 full-bleed 边缘契约
- 没有 box/block 专用 pixel snapping 规则

## Diagnosis

### 1. 为什么当前 box 看起来像“很多竖线拼起来”

根因不是 terminal model 存错字符，而是 box/block 仍在走“普通正文 glyph”链路：

- 普通正文的 baseline 和 line box 契约也套在了 box 上
- 普通正文的字距策略也作用到了 box 上
- 普通正文的 glyph raster 和 visible-bounds 逻辑并不知道“这个字符应该贴满 cell 边缘”

结果就是：

- `│` 不保证真正触达 cell 的上下边缘
- `─` 不保证真正触达 cell 的左右边缘
- `╭╮╰╯` 的连接点仍受字体 glyph 自身形状和 hinting 影响
- 相邻 cell 之间并没有“共享边界”的绘制契约

所以视觉上自然更像“很多单独 glyph 排在一起”，而不是同一条边框。

### 2. 为什么当前会有一点“发光 / 发虚 / 发亮感”

当前 halo / blur 感主要来自几类因素叠加：

- native path 对单色正文优先使用 `DrawGlyphRun`
- 对细线字符，这条链路的 AA / ClearType 倾向更适合正文，不适合 box 线条
- box 仍按普通 glyph 处理，边缘 coverage 更像文字边缘，而不是边框硬边
- atlas / mask fallback 仍延续了普通 glyph coverage 映射思路

它不是某一个单点 bug，更像：

- glyph rendering
- cell metrics
- pixel snapping
- AA/compositing

四者叠加后，box line 的视觉结果不够“实”和“净”。

### 3. 当前最大根因判断

这次问题的最大根因不是 theme，不是颜色，也不是单一 clipping 问题。

最大的结构性根因是：

1. box/block 仍依赖字体 glyph 渲染
2. 这些 glyph 仍吃正文 cell metrics / baseline / spacing
3. 没有独立的 device-pixel snapped geometry/mask contract
4. Windows native 主路径仍优先把它们当普通字去 `DrawGlyphRun`

因此这次修复如果只调颜色、只减亮度、只关一点 AA，都只能缓解，不能根治。

## External References

这轮设计参考了几类成熟终端的公开方法论和源码/讨论：

- WezTerm `custom_block_glyphs`
  - https://wezterm.org/config/lua/config/custom_block_glyphs.html
- WezTerm `anti_alias_custom_block_glyphs`
  - https://wezterm.org/config/lua/config/anti_alias_custom_block_glyphs.html
- Kitty 作者关于 box drawing 程序化渲染的讨论
  - https://github.com/kovidgoyal/kitty/discussions/7680
- Ghostty 内部 sprite/draw 模块
  - `src/font/sprite/draw/box.zig`
  - `src/font/sprite/draw/block.zig`
- Windows Terminal 关于 box drawing gaps 与 custom glyphs 的 issue / PR
  - https://github.com/microsoft/terminal/issues/14654
  - https://github.com/microsoft/terminal/pull/16729

这些资料给出的共同方向是：

- 对 box/block 不完全信任字体 glyph
- 通过程序化生成或内部 sprite/mask 获得连续、贴边、pixel-snapped 的结果
- 将是否抗锯齿作为 special glyph 自己的策略，而不是沿用正文链路

## Goals

### 1. 视觉目标

- 让 box-drawing 字符在视觉上更连续、更一体化
- 让 `│`、`─`、`╭╮╰╯` 更贴边、更像整体边框
- 明显减少 halo / blur / glow 感
- block elements 看起来更像几何填充，而不是文字轮廓

### 2. 架构目标

- 不影响普通文本主渲染
- 让 `Native` 与 `Bitmap` 两条路径共享 special-case 结果
- 避免把所有逻辑硬塞进平台后端
- 最小侵入地引入可维护的 special glyph rendering 层

### 3. 产品目标

优先提升这些典型 TUI 的观感：

- Codex
- lazygit
- htop / btop
- vim / nvim
- less

## Non-Goals

- 不做新的 terminal theme redesign
- 不为了解决 box/block 而整体重写普通文本渲染
- 不在 v1 扩展到 Braille / Legacy Computing / 全量 Nerd Font 图标
- 不引入新的沉重依赖
- 不把平台后端变成 special glyph 的唯一真相来源

## Chosen Approach

## A. 核心原则

本轮采用：

- **共享 special glyph mask 生成层**
- **双路径复用**
- **白名单 special-case**

而不采用：

- 只在 `windows.rs` 中临时直接画几何线
- 只调颜色或只调 AA
- 复用现有 `GridSymbol` 作为最终 routing key

一句话概括：

> box/block 的真正修复入口应当是“共享的单色 mask 生成与 prepared draw contract”，而不是“平台层再开一套临时 painter”。

## B. 新增层次

建议新增一个共享模块，例如：

- `src/app/terminal_renderer/custom_grid_glyphs.rs`

职责：

1. 检测一个 cluster 是否属于 v1 special-case 白名单
2. 按 cell geometry 生成 device-pixel snapped monochrome mask
3. 输出独立于字体 glyph 的 prepared payload

建议新增的核心结构：

```rust
enum CellRenderKind {
    NormalText,
    BoxDrawing,
    BlockElement,
}

enum CustomGridGlyphKind {
    BoxDrawing(BoxDrawingGlyph),
    BlockElement(BlockElementGlyph),
}

enum MonochromeGlyphSourceKind {
    FontOutline,
    GeneratedMask,
}

struct GeneratedMaskGlyph {
    kind: CustomGridGlyphKind,
    width_px: u32,
    height_px: u32,
    alpha: Vec<u8>,
}
```

命名可调整，但职责边界不应变化：

- 识别
- 生成
- 路由

必须清楚分开。

额外约束：

- `GeneratedMaskGlyph` 只描述“生成出来的 mask 长什么样”，必须与具体落点解耦
- native prepare 与 bitmap blit 侧各自根据 snapped cell rect 计算 `dest_x_px / dest_y_px`
- 不要把“mask 生成”和“最终放置”揉成一个共享缓存对象，否则会和现有 atlas/sprite cache 的职责边界冲突

## C. 为什么不能直接复用 `GridSymbol`

当前 `GridSymbol` 只是一个“视觉拟合提示”，范围过大，除了 box/block 还覆盖：

- Braille
- 一些 Powerline / PUA 范围
- 其他 grid-like symbol

因此它不适合作为 v1 geometry/mask 的直接白名单。

本轮必须新增一个更窄的 classifier，只服务：

- Box Drawing
- Block Elements

并且由显式白名单控制，而不是靠“看起来像 grid”来偷懒。

## D. v1 白名单范围

### v1 必做

第一版只覆盖用户最关心、同时风险最低的字符：

- `─`
- `│`
- `┌┐└┘`
- `├┤┬┴┼`
- `╭╮╰╯`
- `█`
- `▀`
- `▄`
- `▌`
- `▐`

这些字符足以显著改善：

- Codex 样式边框
- vim / nvim floating border
- lazygit / htop 常见分隔线
- block-element 进度/填充

### v1 明确不做

第一版暂不纳入：

- Braille
- Legacy Computing
- emoji
- 全量 Nerd Font / PUA 图标
- 全量 Powerline separators
- 双线 / 重线 / 虚线完整家族
- 复杂对角线与特殊装饰线

这样做的原因是：

- 先锁定主收益区
- 先把最常见 TUI 边框做对
- 减少误伤普通文本与 fallback glyph 的概率

如后续人工评审认为确有必要，可在 v1.1 独立扩充 Powerline separators。

## E. 触发条件

special-case 只在以下条件同时满足时触发：

- 单 codepoint
- 单 grapheme
- 单 monochrome cluster
- `cell.width == 1`
- 无 ZWJ / variation selector
- 无 color glyph
- 字符属于 v1 白名单

任何条件不满足，都必须回退到原有字体 glyph 路径。

实现上的补充约束：

- classifier 的输入应以 `text + cell_span` 这类“cluster 级事实”为主，而不是假设调用方已经拿到了像素宽度
- `run.has_color_glyphs == true` 这类外部上下文，可由调用方先行短路，不必强塞进共享 mask 结构
- 必须显式拒绝 variation selector、ZWJ、combining mark 和多 codepoint cluster，而不是只做“字符白名单 contains”式判断

这条边界是为了保证：

- 不误伤正文
- 不误伤 fallback
- 不误伤 emoji / complex cluster

## F. 几何与像素策略

### F1. box drawing

box drawing 采用 cell-box anchored 规则，而不是正文 baseline 规则：

- `─`：full-bleed 到 cell 左右边缘
- `│`：full-bleed 到 cell 上下边缘
- `┌┐└┘╭╮╰╯`：按统一线宽和连接点生成
- `├┤┬┴┼`：按中心连接点与边缘延伸生成

这些字符的目标是：

- 相邻 cell 之间视觉连续
- 不依赖字体 overhang
- 不依赖 glyph 自带边缘形状

### F2. block elements

block elements 按填充几何生成：

- `█`：整格填满
- `▀`：上半填充
- `▄`：下半填充
- `▌`：左半填充
- `▐`：右半填充

目标是：

- 填充边界稳定
- 不出现像文字轮廓一样的灰边与内缩

### F3. pixel snapping

special glyph 必须在 device-pixel 空间内生成：

- 不允许 half-pixel placement
- 不复用普通正文的 fractional x phase
- 先得到最终 device-space cell box，再在其内生成 mask

这点对 DPI 缩放下的 crisp 稳定性至关重要。

### F4. AA 策略

special glyph 的 AA 策略应独立于正文：

- horizontal / vertical / fill 优先硬边、少 AA
- 圆角连接可允许极轻的边缘过渡
- 不延续正文 `DrawGlyphRun` 的视觉风格

目标不是“更亮”，而是“更实、更净、更稳定”。

## G. 路由与消费

### G1. prepare 层

在：

- `src/app/terminal_renderer/wgpu_renderer.rs`

中，prepare 阶段优先尝试 custom grid glyph classifier。

若命中白名单：

- 直接生成 `GeneratedMask`
- 不再请求普通字体 glyph raster
- 不再把 placement 建立在正文 baseline 上
- 但在 Windows native live path 中，只有 source-kind 分流准备好后，才能把这类 draw 真正放进 mixed frame

若未命中：

- 保持原有 `FontOutline` 路径

### G2. native 路径

在：

- `src/app/terminal_renderer/platform/windows.rs`

中，消费层只做分流：

- `FontOutline` -> `DrawGlyphRun`
- `GeneratedMask` -> 复用现有 monochrome mask 绘制链

本轮不建议在 Windows 平台层新增“第三条长期几何绘制主路径”。

同时必须补齐两条约束：

- mixed frame 中的 diagnostics / draw counters 不能再默认“所有 monochrome draw 都是字体 glyph”
- `active_font_chain`、`active_baseline_px`、`active_glyph_bounds_trace` 这类诊断要么只看 `FontOutline`，要么显式携带 `source_kind`

### G3. bitmap 路径

在：

- `src/app/terminal_atlas.rs`

中，同样优先尝试 custom grid glyph mask 生成。

这样 `Bitmap` 和 `Native` 两条路径共享的是：

- 相同 special-case 白名单
- 相同几何规则
- 相同 mask 结果

差异只应体现在最终显示链的 AA 纹理，而不应体现在几何连续性本身。

### G4. atlas / cache contract

generated mask 虽然和 font glyph 共用“单色位图缓存”大方向，但 contract 必须分清：

- key space 必须与字体 glyph 完全隔离
- generated entry 必须按 `glyph kind + cell width + cell height + scale bucket (+ bold if any)` 复用
- generated entry 默认应使用 zero horizontal padding / full-mask size，不继承字体 glyph 的 overhang padding 习惯
- body text 继续保留原 `GlyphRasterRequest` 与 fractional x phase 逻辑；special glyph 不得吃正文 fractional phase

## H. fallback / escape hatch

v1 需要保留内部 fallback 兜底，但不应把它做成“机会主义混用”：

- 默认 special-case 打开
- generator 不支持或失败 -> 回退字体 glyph
- 可保留内部 kill switch，便于排障

但不建议在 v1 同时做复杂用户可见设置面板。

## Testing Strategy

本轮只产出设计文档，不进入实现；但设计必须明确测试落点，方便人工评审。

### 1. prepare / classify

建议扩展：

- `tests/terminal_glyph_fit_spec.rs`
- `tests/terminal_glyph_origin_snap_spec.rs`
- `tests/terminal_renderer_prepare_cache_spec.rs`

重点覆盖：

- v1 白名单分类正确
- `┌┐└┘├┤┬┴┼╭╮╰╯` 的连接 topology 正确，而不只是面积大致正确
- variation selector / ZWJ / combining mark / 多 codepoint cluster 明确拒绝 special-case
- box/block 与普通正文混排时不会误标普通文本
- special glyph 不再吃正文 fractional phase
- repeated box/block 仍能稳定复用 cache

### 2. bitmap / raster

建议扩展：

- `tests/terminal_atlas_renderer_spec.rs`

重点覆盖：

- `──────` 横向连续
- `││││││` 纵向连续
- `┌──┬──┐ / │  │  │ / └──┴──┘` 连接关系正确
- `█▀▄▌▐` 填充比例正确
- 至少一个 fractional DPI 比例（如 `1.25x` 或 `1.5x`）下 seam 与半格比例仍稳定
- 混排 `A╭─╮中` 时，正文 cluster 与 generated mask cluster 的来源可被测试区分

### 3. native routing contract

建议扩展：

- `tests/windows_native_text_renderer_contract_spec.rs`
- `tests/native_terminal_surface_contract_spec.rs`

重点覆盖：

- special glyph 不再走正文 `DrawGlyphRun`
- special glyph 能与正文 `DrawGlyphRun` 同帧并存
- 不会把正常 special-case 分流误报成“native fallback”
- `Native` 模式不会因为 special glyph 而整体退回 `Bitmap`
- mixed frame 下的 monochrome draw counter 不丢正文计数
- font-chain / baseline / glyph-bounds diagnostics 不被 generated draw 污染

### 4. 手工 smoke

建议补充：

- `docs/terminal-tui-smoke-checklist.md`
- `tests/terminal_tui_smoke_fixture_spec.rs`

新增观察点：

- `──────`
- `││││││`
- `┌──┬──┐ / │  │  │ / └──┴──┘`
- `█▀▄▌▐`
- 混排：正文 + box/block 共存
- resize / scale 后 continuity 是否稳定
- `glyphs` smoke fixture 的自动化 contract 不只检查源码字符串，还要直接执行脚本并断言样例输出

## Risks

### 1. 白名单过宽

如果第一版把 Braille、PUA、Powerline、正文标点都一起并进来，回归风险会显著放大。

缓解方式：

- v1 只做小白名单
- 明确触发条件
- 先把最常见 TUI case 做稳定

### 2. 双路径分叉

如果 special-case 只进 Windows native，不进 bitmap atlas，则 native/bitmap 观感会分裂。

缓解方式：

- 共享 mask 生成层
- 两条路径共用同一份 special glyph 结果

### 3. 平台层职责膨胀

如果把几何生成塞到 `windows.rs`，平台后端会变成绘制规则真相来源，后续难以维护。

缓解方式：

- 平台层只消费 prepared payload
- 几何规则集中在共享模块

### 4. 误伤普通文本

如果触发条件不严，正文可能被误路由到 special-case，导致排版退化。

缓解方式：

- 只允许单 codepoint / 单 cluster / 单 cell / 白名单字符
- 其他情况全部保留原链路

## Final Decision

本轮设计最终决定：

1. 不做纯颜色应付式修补。
2. 不只改 Windows 平台后端。
3. 不直接复用现有 `GridSymbol` 作为最终 routing key。
4. 新增共享的 custom grid glyph mask 生成层。
5. v1 只覆盖最常见 box drawing / block elements 白名单。
6. `Native` 与 `Bitmap` 两条路径共享 special-case 结果。
7. 普通文本继续走现有主渲染链路，不做无关重构。

如果后续人工评审确认该方向成立，再进入 implementation plan 阶段。

# Terminal Box-Drawing Rendering Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 为 terminal 的 v1 白名单 box-drawing / block element 字符引入共享的 special-case geometry/mask 渲染层，让 Native 与 Bitmap 路径都能画出更连续、更贴边、更 crisp 的边框与填充，同时不回退普通正文文本渲染观感。

**Architecture:** 新增 `custom_grid_glyphs` 共享层，在 prepare 阶段先对单 codepoint、单 cell、单 monochrome cluster 的 box/block 白名单做分类，再按 cell geometry 和 device pixels 生成 `GeneratedMask`。`WgpuTerminalRenderer` 与 `TerminalAtlasRenderer` 复用同一 classifier/painter；Windows native backend 只负责把 `FontOutline` 继续交给 `DrawGlyphRun`，把 `GeneratedMask` 留在 opacity-mask/bitmap 链路，避免正文和 special glyph 相互污染。

**Tech Stack:** Rust, existing terminal renderer/atlas pipeline, Windows DirectWrite/Direct2D retained surface path, Unicode box-drawing/block-elements, `cargo test`, `cargo check`, Bash smoke fixtures

---

## Input Design

- 设计基线固定为 `docs/plans/2026-04-27-terminal-box-drawing-rendering-design.md`
- 实施不得偏离以下已确认决策：
  - 不靠 theme/颜色微调硬撑，要做真正的 special-case rendering
  - v1 只覆盖高价值小白名单：`─ │ ┌┐└┘ ├┤┬┴┼ ╭╮╰╯ █ ▀ ▄ ▌ ▐`
  - v1 明确不做：Braille、Legacy Computing、emoji、Powerline/PUA 全量、双线/重线/虚线完整家族
  - 只对“单 codepoint + 单 grapheme + 单 cell + 非 color glyph + 非 ZWJ/VS”的 cluster 触发 special-case
  - Native 与 Bitmap 必须共享同一份几何/像素对齐规则，不能各画各的
  - 普通英文、中文、数字、标点继续沿用现有字体 glyph 主链路
  - 不引入新的重依赖，不做无关重构，不整体重写 `terminal_model` / `terminal_presenter`

## Execution Notes

- 每个 task 都先用 `@superpowers:test-driven-development`：先写失败测试，再做最小实现，再验证通过。
- 如果连续性、pixel snapping、AA 或 atlas/cache 行为与预期不符，立刻切到 `@superpowers:systematic-debugging`；不要凭感觉堆 magic number。
- 所有 special glyph 的几何都必须在 device-pixel 空间生成；不要把 fractional pen phase 继续带进 box/block 路径。
- 除非测试证明 `terminal_model` 或 `terminal_presenter` 缺少必要数据，否则不要动这两个模块。
- 普通文本必须继续保留 `DrawGlyphRun` / swash glyph 路线；special glyph 只是旁路，不是替换整个正文管线。
- 任何“更亮、更白、更发光”的补救都不属于本计划；如需视觉收口，只允许在 alpha/AA 规则内做保守调整。
- 现有正文 contract 测试只能追加新 case，不能用 special glyph 断言替换或放宽已有正文断言。
- shared generator 产物必须是“与最终落点解耦的 mask 数据”；`dest_x_px / dest_y_px` 由 native prepare 或 bitmap blit 侧基于 snapped cell rect 另算。
- generated atlas entry 必须使用独立 key space，并明确锁定 zero horizontal padding / full-mask size contract，避免继承字体 glyph padding。
- Windows mixed frame 必须同时维护好 `source_kind`、fallback 语义、draw counter 语义与 diagnostics 语义；不要只让画面看起来对。

## Task Sequence Overview

1. 先锁定 smoke/checklist 与 v1 scope contract，避免实现过程中目标漂移。
2. 新增共享 `custom_grid_glyphs` 模块，独立承载分类、像素对齐和 mask 生成。
3. 先补齐 `GeneratedMask` 的 source-kind / atlas key / padding bookkeeping，确保不会提前破坏现有 Windows live routing。
4. 让 bitmap atlas renderer 先复用同一份 generator，并暴露足够的测试可观测性来验证“正文没被误伤”。
5. 再一次性启用 native prepare 的 `GeneratedMask` routing 和 Windows mixed rendering / diagnostics，避免中间态把 special glyph 吞进正文路径。
6. 跑完整回归矩阵和手工 smoke，确认 Codex / lazygit / htop / vim / less 等 TUI 都受益且普通文本不退化。

### Task 1: 冻结 v1 smoke 观察点与人工验收样例

**Files:**
- Modify: `docs/terminal-tui-smoke-checklist.md`
- Modify: `scripts/dev/terminal-tui-smoke.sh`
- Modify: `tests/terminal_tui_smoke_fixture_spec.rs`

**Step 1: Write the failing test**

在 `tests/terminal_tui_smoke_fixture_spec.rs` 里扩展 source contract，要求 smoke 资产明确覆盖 box/block 观察点，例如：

```rust
for observation in ["box drawing", "block elements", "╭╮╰╯", "█▀▄▌▐", "DPI", "continuity"] {
    assert!(checklist.contains(observation));
}
```

脚本 contract 还要直接执行 `glyphs` 场景，并要求 stdout 明确输出以下样例：

```text
╭────╮
│Codex│
╰────╯
█▀▄▌▐
```

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test --test terminal_tui_smoke_fixture_spec --quiet
```

Expected: FAIL，因为现有 smoke 资产还没有把 box-drawing continuity / block-element fill / DPI 稳定性写成明确观察项，`glyphs` 脚本输出里也还没有这些固定样例。

**Step 3: Write minimal implementation**

- 在 `docs/terminal-tui-smoke-checklist.md` 中新增专门的 `box drawing` / `block elements` 观察区块
- 在 `scripts/dev/terminal-tui-smoke.sh` 的 `glyphs` 场景里补充 `╭╮╰╯`、`─│`、`█▀▄▌▐` 和 resize/DPI 提示
- 在测试里保留现有“场景入口存在”检查，但新增对脚本真实 stdout 的断言，避免注释/死分支误报通过
- 保持脚本现有 `all / codex / vim / less / htop / links / glyphs / progress` 入口不变，只扩充内容，不重写结构

**Step 4: Run test to verify it passes**

Run:

```bash
cargo test --test terminal_tui_smoke_fixture_spec --quiet
bash scripts/dev/terminal-tui-smoke.sh glyphs
```

Expected: PASS，且脚本能直接打印新的 box/block 手工验收样例。

**Step 5: Commit**

```bash
git add docs/terminal-tui-smoke-checklist.md scripts/dev/terminal-tui-smoke.sh tests/terminal_tui_smoke_fixture_spec.rs
git commit -m "test: expand terminal box drawing smoke fixtures"
```

### Task 2: 新增共享 custom grid glyph classifier 与 geometry/mask generator

**Files:**
- Create: `src/app/terminal_renderer/custom_grid_glyphs.rs`
- Modify: `src/app/terminal_renderer/mod.rs`
- Create: `tests/terminal_custom_grid_glyphs_spec.rs`

**Step 1: Write the failing test**

在 `tests/terminal_custom_grid_glyphs_spec.rs` 中新增纯逻辑 contract 测试，至少覆盖：

```rust
assert!(classify_custom_grid_glyph("│", 1).is_some());
assert!(classify_custom_grid_glyph("╭", 1).is_some());
assert!(classify_custom_grid_glyph("█", 1).is_some());
assert!(classify_custom_grid_glyph("┼", 1).is_some());
assert!(classify_custom_grid_glyph("⣿", 1).is_none());
assert!(classify_custom_grid_glyph("", 1).is_none());
assert!(classify_custom_grid_glyph("─\u{FE0F}", 1).is_none());
assert!(classify_custom_grid_glyph("─\u{200D}", 1).is_none());
assert!(classify_custom_grid_glyph("─\u{0301}", 1).is_none());
assert!(classify_custom_grid_glyph("ab", 2).is_none());
```

再加几何 contract，要求：

- `│` 的 alpha mask 贴满上下边界
- `─` 的 alpha mask 贴满左右边界
- `┌┐└┘├┤┬┴┼╭╮╰╯` 的 arm/topology 正确，不允许少臂或把圆角误画成方角
- `█` 为整格填充
- `▀ / ▄ / ▌ / ▐` 各自只填对应半格
- 常见 scale（`1.0 / 1.25 / 1.5 / 2.0`）下返回的 snapped rect 不出现 half-pixel origin

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test --test terminal_custom_grid_glyphs_spec --quiet
```

Expected: FAIL，因为模块和 API 还不存在。

**Step 3: Write minimal implementation**

在 `src/app/terminal_renderer/custom_grid_glyphs.rs` 中先落下最小可用的共享层：

```rust
pub enum CellRenderKind {
    NormalText,
    BoxDrawing,
    BlockElement,
}

pub enum CustomGridGlyphKind {
    BoxDrawing(BoxDrawingGlyph),
    BlockElement(BlockElementGlyph),
}

pub enum BoxDrawingGlyph {
    Horizontal,
    Vertical,
    CornerTopLeft,
    CornerTopRight,
    CornerBottomLeft,
    CornerBottomRight,
    TeeLeft,
    TeeRight,
    TeeTop,
    TeeBottom,
    Cross,
    RoundCornerTopLeft,
    RoundCornerTopRight,
    RoundCornerBottomLeft,
    RoundCornerBottomRight,
}

pub enum BlockElementGlyph {
    Full,
    UpperHalf,
    LowerHalf,
    LeftHalf,
    RightHalf,
}

pub struct GeneratedMaskGlyph {
    pub width_px: u32,
    pub height_px: u32,
    pub alpha: Vec<u8>,
}
```

并实现：

- `classify_custom_grid_glyph(text: &str, cell_span: u32) -> Option<CustomGridGlyphKind>`
- `generate_custom_grid_mask(kind, cell_width_px, cell_height_px, scale) -> GeneratedMaskGlyph`
- `DevicePixelSnapper` 或等价 helper，统一把逻辑 cell 映射到 snapped device rect

注意：

- v1 classifier 只认显式白名单，不复用现有“大而宽”的 `GridSymbol`
- module 必须对 bitmap atlas 路径可见，不要把共享逻辑锁死在 native-only feature gate 里
- 先把正交线、直角/圆角拐点、block fills 做出来；不要在第一步引入 heavy/double/dashed 复杂分支
- 除圆角外，alpha 先采用 0/255 硬边策略，优先保证 crisp，而不是追求花哨 AA
- `GeneratedMaskGlyph` 只描述 mask，不携带最终 `dest_x_px / dest_y_px`

**Step 4: Run test to verify it passes**

Run:

```bash
cargo test --test terminal_custom_grid_glyphs_spec --quiet
cargo check --workspace
```

Expected: PASS，且模块已经能独立给出 v1 classifier 与 geometry/mask 结果。

**Step 5: Commit**

```bash
git add src/app/terminal_renderer/custom_grid_glyphs.rs src/app/terminal_renderer/mod.rs tests/terminal_custom_grid_glyphs_spec.rs
git commit -m "feat: add shared terminal custom grid glyph generator"
```

### Task 3: 先补 GeneratedMask bookkeeping / key space / padding contract，不提前改变 Windows live routing

**Files:**
- Modify: `src/app/terminal_renderer/atlas.rs`
- Modify: `src/app/terminal_renderer/wgpu_renderer.rs`
- Modify: `tests/terminal_glyph_fit_spec.rs`
- Modify: `tests/terminal_renderer_prepare_cache_spec.rs`

**Step 1: Write the failing tests**

先补 bookkeeping/source contract，至少覆盖：

```rust
assert_eq!(draw.source_kind, PreparedMonochromeGlyphSourceKind::FontOutline);
assert_eq!(generated_entry.padding_left_px, 0);
assert_eq!(generated_entry.padding_right_px, 0);
assert_ne!(font_key, generated_key);
```

重点用例：

- `tests/terminal_glyph_fit_spec.rs` 中现有正文 visual-fit 断言保持原样，只追加 `source_kind` 的存在性/默认值守护
- `tests/terminal_renderer_prepare_cache_spec.rs` 明确 generated atlas key 与 `GlyphRasterRequest` key 空间隔离
- generated atlas entry 默认使用 zero horizontal padding / full-mask size
- 这一步不允许为了让新测试通过而删除、替换或放宽现有正文断言

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test terminal_glyph_fit_spec --test terminal_renderer_prepare_cache_spec -q
```

Expected: FAIL，因为当前 prepare 还只知道 `FontOutline + GlyphRasterRequest + GlyphAtlasKey::from(request)` 这条字体链路。

**Step 3: Write minimal implementation**

先把 shared/native 侧的 plumbing 补齐，但保持 live routing 仍然安全：

```rust
pub enum PreparedMonochromeGlyphSourceKind {
    FontOutline,
    GeneratedMask,
}
```

并在 `PreparedMonochromeGlyphDraw` 中补足必要元数据，例如：

```rust
pub struct PreparedMonochromeGlyphDraw {
    // existing fields...
    pub source_kind: PreparedMonochromeGlyphSourceKind,
}
```

实现要点：

- 先给 `PreparedMonochromeGlyphDraw`、`GlyphAtlasKey`、atlas entry bookkeeping 补上 generated 所需的类型与 contract
- 这一 task 里保留现有 body-text draw 默认 `source_kind = FontOutline`
- 不要在这一 task 里把 `GeneratedMask` draw 直接放进会被 Windows live path 消费的混合帧里
- `GlyphAtlas` 新增 generated-key 路由，例如：

```rust
pub enum GlyphAtlasKey {
    Font(GlyphRasterRequestKey),
    Generated(GeneratedGlyphAtlasKey),
}
```

或等价设计，只要满足：

- 字体 glyph 与 generated mask 不共享 key 空间
- generated mask 可按 `glyph kind + cell width + cell height + scale bucket + bold(if any)` 稳定复用
- generated atlas entry 默认 `padding_left_px = 0`、`padding_right_px = 0`
- body text 仍复用原 `GlyphRasterRequest` 路线

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test terminal_glyph_fit_spec --test terminal_renderer_prepare_cache_spec -q
cargo check --workspace
```

Expected: PASS，且 generated bookkeeping contract 已就位，同时现有正文 prepare contract 保持不变。

**Step 5: Commit**

```bash
git add src/app/terminal_renderer/atlas.rs src/app/terminal_renderer/wgpu_renderer.rs tests/terminal_glyph_fit_spec.rs tests/terminal_renderer_prepare_cache_spec.rs
git commit -m "refactor: prepare generated mask bookkeeping for terminal box glyphs"
```

### Task 4: 让 bitmap atlas renderer 复用 shared generator，并暴露“正文未误伤”的可观测性

**Files:**
- Modify: `src/app/terminal_atlas.rs`
- Modify: `tests/terminal_atlas_renderer_spec.rs`

**Step 1: Write the failing tests**

在 `tests/terminal_atlas_renderer_spec.rs` 中新增像素级 contract，至少覆盖：

```rust
assert!(vertical_seam_gap_count(&frame.image) == 0);
assert!(horizontal_seam_gap_count(&frame.image) == 0);
assert_eq!(filled_pixel_ratio("█"), 1.0);
assert_eq!(upper_half_ratio("▀"), 0.5);
```

具体场景：

- `╭────╮ / │    │ / ╰────╯` 的边框在相邻 cell seam 上没有背景漏缝
- `││││` 的竖线在多行上连续
- `────` 的横线在多列上连续
- `█ ▀ ▄ ▌ ▐` 的填充几何比例在 `1x`、`1.25x`、`2x` raster scale 下都稳定
- 混排 `A╭─╮中` 不会让普通正文 cluster 被错误改成 generated mask
- 需要新增最小可测试的来源可观测性，例如 `RenderedClusterSourceKind` 或等价 test-only contract，让测试能区分 `FontRaster` 与 `GeneratedMask`

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test terminal_atlas_renderer_spec -q
```

Expected: FAIL，因为当前 atlas renderer 仍通过 `rasterize_mono_cluster_sprite(...)` 走 swash/font glyph raster。

**Step 3: Write minimal implementation**

在 `src/app/terminal_atlas.rs` 中把 `rasterize_cluster_sprite(...)` 改为优先尝试 shared generator：

```rust
if let Some(kind) = classify_custom_grid_glyph(text, span) {
    return CachedClusterSprite::MonoAlpha {
        width: mask.width_px,
        height: mask.height_px,
        alpha: mask.alpha,
    };
}
```

并保持：

- emoji 仍优先走现有 `TerminalEmojiRenderer`
- 非命中字符仍走 `rasterize_mono_cluster_sprite(...)`
- sprite cache key 继续使用现有 `text + span + bold`，依赖 `set_raster_scale()` 清 cache；不要在这一步引入新的缓存系统
- generated mask 的 blit 必须 full-bleed 到 cell edges，不再依赖字体可见边界的“居中”校正
- atlas 侧的“正文 vs generated”来源信息只需做到测试可观测，不要顺手扩散成新的产品功能

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test terminal_atlas_renderer_spec -q
cargo check --workspace
```

Expected: PASS，且 bitmap path 在 `1x / 1.25x / 2x` 下都能得到连续、贴边、无明显 halo 的 box/block 结果，并能证明正文 cluster 没被误切到 generated path。

**Step 5: Commit**

```bash
git add src/app/terminal_atlas.rs tests/terminal_atlas_renderer_spec.rs
git commit -m "feat: reuse generated box glyph masks in bitmap atlas renderer"
```

### Task 5: 一次性启用 native prepare 的 GeneratedMask routing，并修正 Windows mixed rendering / diagnostics

**Files:**
- Modify: `src/app/terminal_renderer/wgpu_renderer.rs`
- Modify: `src/app/terminal_renderer/platform/windows.rs`
- Modify: `tests/terminal_glyph_fit_spec.rs`
- Modify: `tests/terminal_glyph_origin_snap_spec.rs`
- Modify: `tests/terminal_glyph_grid_anchor_spec.rs`
- Modify: `tests/terminal_renderer_prepare_cache_spec.rs`
- Modify: `tests/windows_native_text_renderer_contract_spec.rs`
- Modify: `tests/native_terminal_surface_contract_spec.rs`

**Step 1: Write the failing tests**

先补 native routing + Windows mixed-frame contract，明确要求：

```rust
assert!(windows_backend_source.contains("PreparedMonochromeGlyphSourceKind::GeneratedMask"));
assert!(windows_backend_source.contains("PreparedMonochromeGlyphSourceKind::FontOutline"));
assert!(windows_backend_source.contains("if draw.source_kind == PreparedMonochromeGlyphSourceKind::FontOutline"));
```

要锁定的行为：

- `││││`、`────`、`╭─╮`、`╰─╯` 在 native prepare 中不再触发字体 raster
- `a│b`、`中文│`、`co-op` 混排时，只有 v1 白名单字符走 `GeneratedMask`
- `terminal_glyph_origin_snap_spec.rs` 要求 box/block draw 不吃 fractional x phase
- `terminal_glyph_grid_anchor_spec.rs` 要求 generated draw 仍然贴住 cell/grid anchor，而不是回到正文 bearing/baseline 契约
- `terminal_renderer_prepare_cache_spec.rs` 要求重复 box/block mask 复用 atlas/cache，不因每格重建而反复 upload
- `draw_directwrite_text(...)` 只消费 `FontOutline`
- `GeneratedMask` 不进入 `DrawGlyphRun`
- `draw_monochrome_glyphs(...)` 在 directwrite body text 成功时，仍然会继续画 special glyph mask
- diagnostics/fallback state 不应把“正文走 DWrite + special glyph 走 opacity mask”的混合帧误报成全面 fallback
- mixed frame 下 `last_drawn_monochrome_glyphs` / draw counters 不能丢掉正文计数
- `active_font_chain`、`active_baseline_px`、`active_glyph_bounds_trace` 这类 diagnostics 要么只看 `FontOutline`，要么显式带 `source_kind`

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test terminal_glyph_fit_spec --test terminal_glyph_origin_snap_spec --test terminal_glyph_grid_anchor_spec --test terminal_renderer_prepare_cache_spec --test windows_native_text_renderer_contract_spec --test native_terminal_surface_contract_spec -q
```

Expected: FAIL，因为当前 native prepare 还没有真正产出 `GeneratedMask` draw，而 Windows 侧 `last_directwrite_text_drawn` 也仍会让整段 monochrome bitmap 绘制直接返回，special glyph 没有单独分流入口。

**Step 3: Write minimal implementation**

在 `src/app/terminal_renderer/wgpu_renderer.rs` 与 `src/app/terminal_renderer/platform/windows.rs` 中一起完成最小可运行分流：

```rust
let directwrite_draws = frame
    .frame
    .presentable_frame
    .monochrome_glyph_draws
    .iter()
    .filter(|draw| draw.source_kind == PreparedMonochromeGlyphSourceKind::FontOutline);
```

并把 bitmap/mask stage 改成：

- directwrite 成功时：继续绘制 `GeneratedMask`
- directwrite 失败时：保留当前全文 `FillOpacityMask` fallback

换言之，`draw_monochrome_glyphs(...)` 不能再用“只要正文 DWrite 成功就整段 return”的粗粒度逻辑，而要改成“按 draw.source_kind 逐项决定”。

同时注意：

- native prepare 必须在 cluster 层先做 special-case 分流，命中白名单时不再申请字体 raster
- generated path 的 `dest_x_px / dest_y_px` 以 snapped cell rect 为准，不再基于正文 baseline/bearing 计算
- 现有正文 draw 继续保持 `FontOutline` 路线和既有断言，不得回退
- `ensure_monochrome_glyph_bitmap(...)` 应继续复用 upload payload，不需要为 generated mask 再引入 font-face 解析
- `SetAntialiasMode(D2D1_ANTIALIAS_MODE_ALIASED)` 可以继续用于 generated mask 路径，正文 `DrawGlyphRun` 的 grayscale/ClearType 策略不变
- mixed frame 下的 draw counter / diagnostics 语义要同步修正；不要只修绘制分流
- 不要改动 color glyph、selection、cursor、IME overlay 责任边界

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test terminal_glyph_fit_spec --test terminal_glyph_origin_snap_spec --test terminal_glyph_grid_anchor_spec --test terminal_renderer_prepare_cache_spec --test windows_native_text_renderer_contract_spec --test native_terminal_surface_contract_spec -q
cargo check --workspace
```

Expected: PASS，且 native prepare 已把 v1 白名单正确路由到 `GeneratedMask`，Windows native backend 也能在同一帧里同时承载 `DrawGlyphRun` 正文与 `GeneratedMask` box/block，而 diagnostics/counter 语义不退化。

**Step 5: Commit**

```bash
git add src/app/terminal_renderer/wgpu_renderer.rs src/app/terminal_renderer/platform/windows.rs tests/terminal_glyph_fit_spec.rs tests/terminal_glyph_origin_snap_spec.rs tests/terminal_glyph_grid_anchor_spec.rs tests/terminal_renderer_prepare_cache_spec.rs tests/windows_native_text_renderer_contract_spec.rs tests/native_terminal_surface_contract_spec.rs
git commit -m "fix: route terminal box drawing through generated native masks on windows"
```

### Task 6: 跑回归矩阵并完成人工 smoke

**Files:**
- Test only: `tests/terminal_custom_grid_glyphs_spec.rs`
- Test only: `tests/terminal_glyph_fit_spec.rs`
- Test only: `tests/terminal_glyph_origin_snap_spec.rs`
- Test only: `tests/terminal_glyph_grid_anchor_spec.rs`
- Test only: `tests/terminal_renderer_prepare_cache_spec.rs`
- Test only: `tests/terminal_atlas_renderer_spec.rs`
- Test only: `tests/windows_native_text_renderer_contract_spec.rs`
- Test only: `tests/native_terminal_surface_contract_spec.rs`
- Test only: `tests/terminal_tui_smoke_fixture_spec.rs`
- Manual: `docs/terminal-tui-smoke-checklist.md`
- Manual: `scripts/dev/terminal-tui-smoke.sh`

**Step 1: Run the targeted automated matrix**

Run:

```bash
cargo test \
  --test terminal_tui_smoke_fixture_spec \
  --test terminal_custom_grid_glyphs_spec \
  --test terminal_glyph_fit_spec \
  --test terminal_glyph_origin_snap_spec \
  --test terminal_glyph_grid_anchor_spec \
  --test terminal_renderer_prepare_cache_spec \
  --test terminal_atlas_renderer_spec \
  --test windows_native_text_renderer_contract_spec \
  --test native_terminal_surface_contract_spec \
  -q
cargo check --workspace
```

Expected: PASS。

**Step 2: Run smoke fixtures**

Run:

```bash
bash scripts/dev/terminal-tui-smoke.sh glyphs
bash scripts/dev/terminal-tui-smoke.sh all
```

Expected: PASS，并输出人工观察样例。

**Step 3: Run manual TUI verification**

按 `docs/terminal-tui-smoke-checklist.md` 至少人工检查：

- Codex：边框、分隔线、状态框 continuity
- lazygit：面板边框与表格分隔线
- htop / btop：高频刷新下竖线是否仍连续
- vim / nvim：floating border、split separator
- less：框线/分页后 resize 稳定性

重点观察：

- `│` 是否消除“分节感”
- `─` 是否 full-bleed 到左右 cell 边界
- `╭╮╰╯` 是否与 `─│` 自然连接
- `█▀▄▌▐` 是否无灰边、无裁切、无错位
- 正文英文字母、中文、数字是否保持原有观感
- `1.25x` / `1.5x` 这类 fractional DPI 下是否仍无明显 seam / half-pixel 错位

**Step 4: If any check fails, debug before broadening scope**

如果失败，优先回到对应 task：

- 分类错误 -> Task 2
- prepare/caching/snap 错误 -> Task 3
- bitmap seam/gap -> Task 4
- Windows mixed routing 错误 -> Task 5

不要顺手把 Braille、Powerline、双线、斜线一起并进修；先把 v1 白名单稳定住。

---

## Out of Scope Guardrails

以下内容即便实现过程中“顺手能做”，本计划也不应纳入：

- 把整个 `GridSymbol` 范围全部切到 geometry renderer
- 改默认 terminal theme、前景色、背景色或全局字重
- 调整普通正文 letter spacing / font size / line height 作为 box 问题的替代修复
- 新增用户可见设置面板（例如“启用自定义 box glyph”开关）
- 扩展到 Braille、PUA、Powerline 全量支持

## Expected End State

完成后，代码库应满足以下状态：

- `src/app/terminal_renderer/custom_grid_glyphs.rs` 成为 box/block special-case 的唯一共享几何来源
- `src/app/terminal_renderer/wgpu_renderer.rs` 能把 v1 box/block cluster 路由成 `GeneratedMask`
- `src/app/terminal_atlas.rs` 与 Windows native backend 共享同一份 generated mask 结果/规则
- `src/app/terminal_renderer/platform/windows.rs` 能在同一帧里共存 `DrawGlyphRun` 正文和 generated box glyph mask
- 典型 TUI 中的 Unicode 边框看起来更像连续整体边框，而不是零散 glyph 拼接
- 普通正文文本外观与现有 terminal contract 保持兼容

# Startup UI Font Fallback Design

## Problem

`startup private/commit` 仍然没有明显下降。前一轮证据说明：

- 终端 idle shrink / trim 只能让 working set 看起来下降，不能真正降低 private/commit。
- D3D / Skia Ganesh cache budget 收紧后，用户实测只有几 MB 级改善，说明它不是主因。
- Slint 文本布局在启动阶段会通过 `sharedparley` 直接触发 `sharedfontique::COLLECTION` 初始化；当前共享字体集合默认带 `system_fonts = true`，Windows 下会在启动期枚举系统字体 family/fallback 元数据。

因此，本轮目标不是继续做 trim 或 cache cosmetic，而是让首帧 UI 尽量不触发 system font catalog。

## Chosen Approach: Conservative Enhanced Path (Option B)

采用一个保守的两阶段字体查询方案：

1. 把 Slint 共享字体主集合改成“轻量 primary collection”，启动时不枚举系统字体。
2. 在主集合里预置一个 bundled 的比例字体种子（复用 Slint 自带的 `DejaVuSans.ttf`），并把它绑定到 `SansSerif` / `SystemUi` / `UiSansSerif` generic family。
3. 运行时文本查询先查 primary collection；只有 primary miss，才懒加载并查询 secondary system collection。
4. `sharedparley` 的常驻 `FontContext` 只绑定 primary collection，这样首帧常见 Latin UI 文本不需要触发系统字体扫描。

## Why This Is Conservative

这版不做激进改动：

- 不碰 terminal 字体链。
- 不改 trim / purge / memory diagnostics 默认行为。
- 不强行移除 system font fallback，只是把它改成 miss 后再启用。
- 不要求应用层先引入新的 UI 字体资源，先复用 Slint crate 已经随源码提供的 `DejaVuSans.ttf`。

预期风险主要是：

- 首帧 UI 的默认 Latin 字体可能从系统字体轻微偏向 DejaVu Sans，导致少量文字宽度和观感变化。
- 非 Latin / emoji / 特殊 family 仍会回落到 system collection，因此功能兼容性应该可保留。

## Architecture

### 1. Primary Shared Collection

在 vendored `i-slint-common` 中把 `sharedfontique::COLLECTION` 改成：

- `system_fonts: false`
- 启动时注册内置 `DejaVuSans.ttf`
- 把该字体映射到常见 generic family

这个 primary collection 负责：

- 首帧 UI 文本布局
- 常见 Latin 文本查询
- 作为 sharedparley 的常驻 `FontContext`

### 2. Secondary System Collection

新增一个 lazy `SYSTEM_COLLECTION`：

- 只有发生字体 miss 时才初始化
- 保持 `system_fonts: true`
- 用于 explicit family、CJK、emoji、脚本 fallback 等场景

### 3. Two-Phase Query

在 vendored `i-slint-core` 中把 `FontRequest::query_fontique()` 改成：

- Phase 1: primary query
- Phase 2: if none => system query

这样保留原有 fallback 能力，但避免默认把启动路径绑定到 system font enumeration。

## Testing Strategy

本轮只做 source contract + build verification，不做 Linux 下伪造字体运行时测试：

- 契约测试验证 primary collection 不再默认携带 system fonts。
- 契约测试验证 primary collection 注册了 bundled DejaVu Sans generic family。
- 契约测试验证 `query_fontique()` 采用 primary-then-system 的两阶段查询。
- `cargo check`
- Windows 交叉打包 `./build-win-x64.sh`
- 最后交给用户在 Windows 上复测 startup private/commit。

## Success Criteria

满足下面几点就算这刀成立：

1. Windows 启动后 `startup private/commit` 比当前基线再下降一个可感知量级，而不是仅几 MB 浮动。
2. 常规 Latin UI 文本不出现缺字。
3. CJK / emoji / explicit family 仍能正常显示，必要时通过 lazy system fallback 获得系统字体。
4. 不依赖 trim 才能“看起来下降”。

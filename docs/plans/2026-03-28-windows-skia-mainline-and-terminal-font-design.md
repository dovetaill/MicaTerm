# Windows Skia Mainline and Terminal Font Design

日期: 2026-03-28  
执行者: Codex  
状态: 已确认，停止于文档阶段

## 背景

当前仓库在两个方向上同时承压：

- Windows 包装入口 `build-win-x64.sh` 仍沿用软件渲染主线语义，无法直接产出 `Skia` 包
- `SarasaTermSCNerd-Regular.ttf` 一旦通过 `.slint` 顶层 `import` 进入启动路径，会显著推高空载内存

同时，用户已经明确了这次调整的边界：

- Windows 默认包装入口改为 `Skia`
- 额外保留一个 `winit-software` 的兼容包装入口
- 保留本地 vendored `Slint winit` patch，不回退 Windows partial visibility 修复
- `SarasaTermSCNerd` 继续随程序分发，并保留终端观感
- 只接受“启动不预加载大字体”，不接受“终端字体完全退回 Iosevka”

这意味着这次设计要同时解决三件事：

1. Windows 包装脚本需要形成 `Skia` 主线包和 `software` 兼容包
2. 渲染选择要能在构建产物层面切换，而不是只能靠源码手改
3. `SarasaTermSCNerd` 必须保留在 exe 内，但只能在真正进入终端路径时再注册

## 目标

- 让 `build-win-x64.sh` 默认打出 `winit-skia-software` Windows 包
- 新增 `build-win-x64-software.sh`，打出 `winit-software` Windows 包
- 两个脚本共享同一套 `build-desktop.sh` 打包逻辑，只通过参数/环境变量切换 renderer 与产物后缀
- 保留 [Cargo.toml](/home/wwwroot/mica-term/Cargo.toml) 中 vendored `i-slint-backend-winit` patch，继续带上当前 Windows partial visibility 修复
- 让终端继续优先使用 `Sarasa Term SC Nerd`
- 避免 `SarasaTermSCNerd-Regular.ttf` 在应用启动阶段通过 `.slint` 全局导入进入内存

## 非目标

- 本轮不重写 terminal UI 结构
- 本轮不移除软件渲染路径
- 本轮不把字体资源改为包外文件
- 本轮不承诺“打开终端后仍严格维持 50MB 内存”；目标是先压低空载启动和非终端场景的内存
- 本轮不移除或重做现有 vendored `Slint` patch

## 已确认约束

### 1. Windows partial visibility 修复必须保留

当前仓库通过 [Cargo.toml](/home/wwwroot/mica-term/Cargo.toml#L54) 的 `[patch.crates-io]` 强制使用
vendored [vendor/i-slint-backend-winit](/home/wwwroot/mica-term/vendor/i-slint-backend-winit)。

并且 [tests/slint_backend_patch_contract_smoke.sh](/home/wwwroot/mica-term/tests/slint_backend_patch_contract_smoke.sh)
已把以下契约写死：

- `mod partial_visibility;`
- `WindowEvent::Moved(_)`
- `handle_partial_visibility_change`
- `renderer/sw.rs` 中的 `present_existing_buffer`

这说明此次方案不能回退到上游 crates.io 原版 backend。

### 2. `Skia` 在当前 vendored backend 中已可用

当前 vendored backend 的 renderer 选择路径已经支持 `skia-software`：

- [vendor/i-slint-backend-winit/lib.rs](/home/wwwroot/mica-term/vendor/i-slint-backend-winit/lib.rs#L467)

因此不需要为了切 `Skia` 再 vendor 一份新的 backend；应优先复用现有 patch。

### 3. 启动内存问题的根因更偏向字体导入时机

最近的修复提交 `879e10b fix: trim startup font memory regression` 已经把问题聚焦到“字体是否在启动期全局导入”。
当前回归测试 [tests/startup_font_memory_regression.rs](/home/wwwroot/mica-term/tests/startup_font_memory_regression.rs)
明确禁止把 `SarasaTermSCNerd-Regular.ttf` 放回 `.slint` 启动路径。

因此本轮正确方向不是“去掉 Sarasa”，而是“保持 Sarasa，但改成首次终端显示时再注册”。

## 方案对比

### 方案 A：Windows 包装层切 `Skia`，软件兼容包保留，Sarasa 改为内嵌延迟注册

做法：

- 保持 generic cargo/dev 默认行为尽量稳定
- Windows 包装脚本通过环境变量显式选择 `skia-software` 或 `software`
- 运行时 profile 改为读取打包时注入的 renderer/flavor
- `SarasaTermSCNerd-Regular.ttf` 保留在 exe 内，但通过 Rust 代码首次终端激活时注册
- `.slint` 中继续只保留轻量字体或 fallback，不做 Sarasa 的全局 `import`

优点：

- 最符合用户要求
- 对现有 vendor patch 侵入最小
- 不需要把字体放到包外
- 可以把“空载低内存”和“终端 Sarasa 观感”同时兼顾

缺点：

- 需要新增 build-time flavor/render 约束
- 要同步更新多组脚本 smoke、runtime profile 测试、日志文本测试
- 终端首次进入时仍会产生一次可见的字体注册开销

### 方案 B：全仓库默认 feature 直接切到 `Skia`，软件兼容包通过 wrapper 单独回切

优点：

- 逻辑上更直观，主线就是 `Skia`

缺点：

- 会让 Linux / 通用 `cargo build` 路径也一起切到 `Skia`
- 与用户当前明确点名的入口 `build-win-x64.sh` 相比，影响范围过大
- 会额外放大现有 runtime profile 与测试变更面

### 方案 C：继续保持软件主线，只新增一个 `Skia` 实验 wrapper

优点：

- 改动最小

缺点：

- 与用户要求“默认 `build-win-x64.sh` 变成 Skia”冲突
- 不解决主入口的目标语义

## 最终决策

选择 `方案 A`。

本次方案以“Windows 包装层的双产物策略”为核心：

- `build-win-x64.sh` 成为 Windows `Skia` 主线包装入口
- `build-win-x64-software.sh` 成为 Windows `software` 兼容包装入口
- 两者都复用 [build-desktop.sh](/home/wwwroot/mica-term/build-desktop.sh)
- `SarasaTermSCNerd-Regular.ttf` 继续嵌入二进制，但只在终端真正进入 `terminal` host mode 时做一次进程级字体注册

## 架构设计

### 1. 渲染选择改为“构建时注入”，而不是“源码唯一真相”

当前 [src/main.rs](/home/wwwroot/mica-term/src/main.rs) 与
[src/app/runtime_profile.rs](/home/wwwroot/mica-term/src/app/runtime_profile.rs) 把 renderer 写死为
`software`。这会让包装脚本即使设置了不同的 Cargo feature，运行时仍然回落到软件渲染。

新的设计应把这层改成“由打包入口注入构建时变量”：

- `MICA_TERM_PACKAGE_RENDERER=skia-software|software`
- `MICA_TERM_BUILD_FLAVOR=windows-mainline|windows-software-compat|development`

源码通过 `option_env!()` 或等价的 compile-time 环境读取上述值，生成 `AppRuntimeProfile`，
再由 `BackendSelector` 选择对应 renderer。

这样做的结果是：

- generic `cargo build` 仍可保持当前开发默认
- Windows 打包入口可以显式选 `Skia`
- 软件兼容包不需要复制一套源码

### 2. `build-desktop.sh` 继续做唯一打包骨架

[build-desktop.sh](/home/wwwroot/mica-term/build-desktop.sh) 已经承担了：

- 目标三元组校验
- 构建
- staging
- zip/tar.gz 打包

它应该继续做唯一骨架，但新增几个可由 wrapper 注入的变量：

- `CARGO_NO_DEFAULT_FEATURES`
- `CARGO_FEATURES`
- `MICA_TERM_PACKAGE_RENDERER`
- `MICA_TERM_BUILD_FLAVOR`
- `PACKAGE_FLAVOR_SUFFIX`

其中：

- `build-win-x64.sh` 注入 `skia-software` 与 `-skia`
- `build-win-x64-software.sh` 注入 `software` 与 `-software`

产物需要显式区分，例如：

- `dist/mica-term-x86_64-pc-windows-gnu-release-skia.zip`
- `dist/mica-term-x86_64-pc-windows-gnu-release-software.zip`

### 3. `SarasaTermSCNerd` 保持内嵌，但脱离 `.slint` 启动导入路径

当前仓库里字体文件位于：

- [ui/fonts/SarasaTermSCNerd-Regular.ttf](/home/wwwroot/mica-term/ui/fonts/SarasaTermSCNerd-Regular.ttf)
- [ui/fonts/IosevkaTerm-Regular.ttf](/home/wwwroot/mica-term/ui/fonts/IosevkaTerm-Regular.ttf)

本次方案不把 Sarasa 放到包外，而是把它改成：

- Rust 侧 `include_bytes!()` 内嵌
- 首次进入终端 host mode 时，通过 Slint 的 shared Fontique collection 注册
- 进程内只注册一次

这里优先使用 Slint 的 runtime font API，而不是继续依赖 `.slint` 顶层 `import`，原因是：

- `.slint` 顶层 `import` 会把大字体拉进启动路径
- shared Fontique collection 是 renderer-agnostic 的，`Skia` 和 `software` 都能共用
- 可以避免为字体加载再去碰 vendor backend 内部接口

### 4. UI 侧字体策略改为“终端优先 Sarasa，启动期仍轻量”

建议保持 [ui/app-window.slint](/home/wwwroot/mica-term/ui/app-window.slint) 继续只导入轻量字体
`IosevkaTerm-Regular.ttf`，不要恢复 Sarasa 的全局导入。

[ui/shell/terminal-session-host.slint](/home/wwwroot/mica-term/ui/shell/terminal-session-host.slint)
中的 `terminal-font-family` 改为：

`"Sarasa Term SC Nerd, Iosevka Term, Cascadia Mono, Consolas, monospace"`

这样做的行为是：

- 启动时：仍然是轻量路径，不预载 Sarasa
- 第一次进入终端后：Sarasa 被注册，终端文本优先命中 Sarasa
- 注册失败时：仍有 Iosevka/Cascadia/Consolas 作为 fallback

### 5. 字体注册触发点挂到 `terminal` host mode 同步路径

[src/app/bootstrap.rs](/home/wwwroot/mica-term/src/app/bootstrap.rs#L2345) 的
`sync_workspace_session_state()` 已经集中负责：

- `workspace_session_host_mode`
- terminal visible lines
- terminal surface cells/cursor/rows/cols

而 [src/shell/view_model.rs](/home/wwwroot/mica-term/src/shell/view_model.rs#L486) 已明确把
`workspace_session_host_mode()` 归类为：

- `welcome`
- `terminal`
- `session-error`

因此最稳妥的触发点就是：

- 在 `sync_workspace_session_state()` 中，当 `state.workspace_session_host_mode() == "terminal"` 时，
  调用 `ensure_terminal_font_registered()`
- 该调用由 `OnceLock` 或等价机制保护，只执行一次

这能保证字体注册只在真正出现终端场景时发生，不依赖具体是“打开新 SSH tab”还是“恢复已有 tab”。

## 测试策略

### 必须保留

- [tests/slint_backend_patch_contract_smoke.sh](/home/wwwroot/mica-term/tests/slint_backend_patch_contract_smoke.sh)
  继续保证 vendored backend patch 不被回退

### 必须改写

- [tests/build_win_x64_script_smoke.sh](/home/wwwroot/mica-term/tests/build_win_x64_script_smoke.sh)
  从“主线 software”改为“主线 Windows Skia wrapper”
- [tests/build_win_x64_skia_script_smoke.sh](/home/wwwroot/mica-term/tests/build_win_x64_skia_script_smoke.sh)
  不再适合作为“禁止 Skia 出现”的契约，应删除或改造
- [tests/runtime_profile.rs](/home/wwwroot/mica-term/tests/runtime_profile.rs)
- [tests/bootstrap_profile_smoke.rs](/home/wwwroot/mica-term/tests/bootstrap_profile_smoke.rs)
- [tests/panic_logging.rs](/home/wwwroot/mica-term/tests/panic_logging.rs)
- [tests/logging_runtime.rs](/home/wwwroot/mica-term/tests/logging_runtime.rs)
- [tests/window_theme_contract_smoke.sh](/home/wwwroot/mica-term/tests/window_theme_contract_smoke.sh)
  中对 `winit-skia-software` 的否定契约需要调整
- [tests/startup_font_memory_regression.rs](/home/wwwroot/mica-term/tests/startup_font_memory_regression.rs)
  要改成“禁止全局 Sarasa import，但允许终端 runtime 注册”

### 建议新增

- `tests/build_win_x64_software_script_smoke.sh`
- `tests/terminal_font_registration_smoke.rs`

后者不必做真正的 renderer 集成测试，也可以是 source contract：

- `ui/app-window.slint` 不能全局导入 Sarasa
- `terminal-session-host.slint` 的 family 必须把 Sarasa 放第一位
- `bootstrap.rs` 必须出现 `ensure_terminal_font_registered`

## 风险

### 风险 1：`Skia` 主线路径与现有 source-based 测试冲突较多

应对：

- 先改测试，再改代码
- 先更新 runtime/profile 契约，再动包装脚本和文档

### 风险 2：runtime font registration 依赖 Slint 的 `unstable-fontique-07`

应对：

- 明确在设计中接受这项不稳定 feature
- 把 runtime font loading 封装到单独模块，避免扩散到业务层

### 风险 3：`Skia` 下仍可能触发新的 Windows partial visibility 边角问题

应对：

- 继续保留 vendored backend patch
- 如果 `Skia` 下仍复现相同问题，再在后续迭代中为 `Skia` 增补专属恢复路径

### 风险 4：终端首次打开时会有一次字体注册延迟

应对：

- 只注册一次
- 优先在 `terminal` host mode 的状态同步阶段尽早触发

## 验证标准

- `build-win-x64.sh` 默认打出 `Skia` Windows 包
- 新增 `build-win-x64-software.sh`，并能打出 `software` Windows 包
- 两个 Windows 包装脚本都只复用 [build-desktop.sh](/home/wwwroot/mica-term/build-desktop.sh) 的公共打包逻辑
- [Cargo.toml](/home/wwwroot/mica-term/Cargo.toml) 中的 vendored `i-slint-backend-winit` patch 保持不变
- `SarasaTermSCNerd-Regular.ttf` 不再通过 `.slint` 顶层导入进入启动路径
- 终端 host mode 的默认字体链以 `Sarasa Term SC Nerd` 开头
- 新日志和 startup failure 文案能够区分 `winit-skia-software` 与 `winit-software`
- 与 vendor patch 相关的 smoke 契约继续通过

# OxideTerm 借鉴笔记

来源项目：
- 仓库地址: https://github.com/AnalyseDeCircuit/oxideterm/
- 主要参考: `README.zh-CN.md`、插件运行时、终端注册表、状态分层设计

本文目标：
- 记录 `mica-term` 可以借鉴的设计思路
- 明确哪些是“可迁移的架构思想”，哪些只是 Tauri/React 生态下的具体实现
- 给出后续实现优先级，避免一开始就把复杂度拉高

## 总判断

OxideTerm 值得借鉴的核心，不是某个前端库或者 Tauri 技巧，而是它对几个边界处理得很清楚：
- 终端运行时状态与 UI 渲染分离
- 稳定 ID 与易变连接对象分离
- 插件能力通过受限上下文暴露，而不是让扩展直接碰宿主内部状态
- 产品功能虽然多，但底层围绕少数几个核心抽象展开：`node`、`connection`、`session`、`pane`

`mica-term` 当前仍处于较早阶段，现状更接近“有设计壳层的桌面终端 UI 原型”，因此适合先借架构骨架，再逐步长产品功能。

## 三条可落地路径

### 1. PTY / runtime-first

这是最建议优先做的路径。

目标：
- 先建立 `pane_id / session_id / connection_id` 三层标识
- 建立独立于 UI 的 runtime store
- 让 Slint 界面只绑定只读状态和命令入口

适合 `mica-term` 的原因：
- 当前项目还没有真正复杂的运行时层
- UI 目前主要是静态结构，先把状态模型定下来，后续本地 PTY、SSH、SFTP、分屏都能往里接
- 这是后续插件、命令面板、AI 上下文、恢复机制的基础

建议的状态分层：
- `connection_id`: 物理连接，适合 SSH 连接池、保活、重连
- `session_id`: 逻辑会话，适合 PTY/远端 shell 生命周期
- `pane_id`: UI 上的展示单元，适合分屏、聚焦、布局

建议先落的能力：
- pane registry
- session registry
- connection registry
- active pane tracking
- terminal buffer / selection / write capability registry

### 2. plugin-abi-first

这条路值得做，但不应早于 runtime-first。

目标：
- 先定义插件 manifest
- 定义受限 `PluginContext`
- 定义 activate / deactivate / cleanup 生命周期
- 为未来扩展预留稳定 ABI

适合借鉴的点：
- 冻结上下文，插件只能拿到 capability，而不是内部 store 引用
- 所有注册行为返回可释放句柄
- 插件卸载时统一 cleanup
- 对可调用后端能力做白名单或 capability 声明

为什么现在不建议优先：
- `mica-term` 当前连核心 runtime 都还没有稳定下来
- 太早做插件层，容易把未来会变的内部实现过早固化成 ABI

### 3. ui-kit-first

这条路最快见效，但架构收益最弱。

目标：
- 把现有 Slint 组件提炼成统一 Shell UI Kit
- 统一指标、动效、状态视觉语义、交互层级

适合先整理的组件：
- titlebar
- tabbar
- sidebar
- status pill
- command palette
- right panel
- welcome surface

适用价值：
- 能快速提升一致性
- 能为后续功能扩张减少 UI 重复劳动

限制：
- 如果 runtime 模型不稳，UI Kit 很快会被新需求反复冲击

## 推荐顺序

推荐实际顺序：
1. PTY / runtime-first
2. UI Kit
3. plugin ABI

原因：
- 先把状态和运行时边界定稳
- 再让 UI 组件绑定稳定模型
- 最后再对外暴露扩展接口

## 值得借鉴的架构点

### 1. 稳定对象寻址

OxideTerm 的一个好思路是：前端尽量不直接依赖易变资源句柄，而是依赖稳定抽象。

对 `mica-term` 的翻译：
- UI 层尽量围绕 `pane_id` 和 `workspace item id`
- runtime 层内部再解析到具体 PTY 或 SSH 资源
- 未来 SSH 重连、分屏拆分、会话恢复时，UI 不必跟着大改

### 2. 运行时注册表

OxideTerm 的 terminal registry 很值得借。

对 `mica-term` 的启发：
- 终端缓冲区读取不要散落在组件里
- 统一注册 `buffer getter / selection getter / writer`
- 未来 AI、命令面板、搜索、录制、广播输入都能直接复用

### 3. 多层状态，而不是一个大状态对象

OxideTerm 的 Zustand 多 store 不必照搬，但“按领域拆状态”值得借。

对 `mica-term` 建议：
- `runtime`: pane/session/connection 生命周期
- `workspace`: 标签、右侧面板、欢迎页、聚焦状态
- `theme`: 色板、动效、可访问性开关
- `command`: palette、actions、recent actions

Rust 里不一定非要做多个 store 类型，但语义边界要清晰。

### 4. 功能开关与可裁剪构建

OxideTerm 对本地 PTY 做 feature gate，这点非常好。

对 `mica-term` 建议：
- 本地 PTY
- SSH
- SFTP
- AI
- 插件

这些都适合做成可裁剪模块，而不是一开始全部绑死。

### 5. 生命周期清理机制

OxideTerm 在插件卸载、资源释放、资产清理上做得比较系统。

对 `mica-term` 的借鉴：
- 每个 runtime 注册动作都应该有对称的 unregister / dispose
- 每个 pane/session 关闭时，要明确释放 buffer、writer、focus、selection、background task
- 后续如果有插件或 AI 会话，也要遵守同一个 teardown 模型

## 值得借鉴的产品特性点子

下面这些不是让 `mica-term` 现在全部做，而是记录为候选方向。

### 第一优先级候选

- 混合终端体验：本地 PTY 与远端 SSH 放在同一工作区模型里
- 分屏与活动 pane 跟踪：为 AI、命令面板、后续工作流打基础
- 命令面板：统一入口比单独堆按钮更适合终端产品
- 会话恢复思路：即便一开始不做完整恢复，也应先把恢复所需状态留在 runtime 模型里
- 连接池思路：一个连接服务多个终端/SFTP/附属能力，避免重复建立 SSH

### 第二优先级候选

- Grace Period 重连：尽量避免重连时杀掉正在运行的 TUI 程序
- 背景任务和状态门禁：未 ready 的资源不能被 UI 或命令误用
- 统一的资源监控面板：连接、会话、缓冲区、任务状态可视化
- 深历史搜索：如果后续有后端缓冲区，搜索应在 runtime 层做，而不是纯 UI 扫描

### 有特色、但应晚做的点子

- IDE 模式
- 双面板 SFTP
- 插件市场或运行时插件系统
- AI 侧栏聊天与跨 pane 上下文捕获
- 自定义主题引擎
- 背景图片画廊
- WSL 图形应用集成

这些都很吸引人，但在 `mica-term` 当前阶段，优先级应低于“运行时状态正确、UI 绑定清晰、资源释放可靠”。

## 不要直接照搬的部分

### 1. Tauri IPC / React / Zustand 具体实现

这些属于 OxideTerm 的技术选型，不是它成功的根因。

`mica-term` 更适合借鉴：
- 能力边界
- 资源生命周期
- 标识体系
- 模块分层

而不是照抄：
- React store 写法
- Tauri command 调用方式
- ESM 插件装载细节

### 2. ESM Runtime 现在不适合优先做

当前 `mica-term` 是 `Rust + Slint`，不是浏览器式前端应用。

如果未来真的要扩展运行时，优先考虑：
- Rust trait + manifest 风格扩展
- WASM 插件
- Lua / QuickJS 这类更可控的嵌入式脚本环境

而不是先复刻浏览器侧 ESM 插件系统。

## 对 mica-term 的具体建议

### 短期建议

- 先建立 runtime 模块，定义 pane/session/connection 三层模型
- 让 `AppWindow` 绑定运行时快照，而不是静态演示数据
- 给 titlebar、tabbar、right panel 加明确的状态输入接口
- 建立统一 registry，为未来终端缓冲区、焦点和命令执行做准备

### 中期建议

- 做 Slint Shell UI Kit
- 接入本地 PTY
- 建立命令面板和 action registry
- 预留 SSH 连接池结构，但可以先不真正接 SSH

### 长期建议

- 设计稳定的扩展 ABI
- 选择插件运行时方案
- 引入 AI、SFTP、IDE 等高阶能力

## 一句话结论

OxideTerm 最值得 `mica-term` 学的，不是“Tauri + React + Zustand + ESM”，而是：

“先把运行时对象模型、状态边界、资源释放和能力暴露做清楚，再往上长复杂功能。”

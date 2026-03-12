# Mica Term System Logging Design

日期: 2026-03-11  
执行者: Codex  
状态: 已确认方案，未进入实现

## 背景

当前仓库仍处于桌面壳层原型阶段，最近提交主要集中在 `titlebar`、`tooltip` 和 UI 偏好持久化：

- `52c8569 feat: stabilize top status bar tooltip overlay`
- `5c5b95e feat: implement top status bar style bugfix2`
- `b832f42 feat: polish top status bar style`

基于当前源码，可以确认以下事实：

- 主窗口中心区域仍是 `WelcomeView` 占位，真实 terminal 控件尚未接入。
- `Cargo.toml` 尚未接入 `wezterm-term`、`termwiz`、`russh`，说明本轮日志设计不需要迁就既有 terminal runtime。
- 现有唯一落盘日志能力是 `TooltipDebugLog`，默认写当前工作目录 `logs/titlebar-tooltip.log`，属于临时调试产物，不是系统日志基建。
- 现有偏好持久化已使用 `ProjectDirs` 标准目录，说明项目已经接受“按平台标准应用目录存储数据”的方向。

因此，本轮目标不是“给现有 terminal 内核补日志”，而是先为 `Rust + Slint + Tokio` 壳层建立一套可长期扩展、能承接后续 SSH/SFTP/terminal 子系统的系统日志基础设施。

## 目标

- 为 `Mica Term` 建立统一的系统日志框架，而不是继续扩散零散文件日志。
- 默认仅持久化严重问题，避免普通运行时产生噪音日志。
- 在 Windows 11 首发阶段优先满足本地排障需求，同时保持后续跨平台可移植性。
- 让启动失败、未捕获 panic、关键初始化失败都能形成稳定的 `fatal/crash` 证据。
- 为后续接入 `wezterm-term`、`termwiz`、`russh` 保留稳定的日志 target 与模块边界。

## 边界

### 本文档覆盖

- 日志框架选型
- 日志目录与 portable/override 策略
- 日志文件结构、默认级别、保留策略
- panic/fatal 处理
- 模块拆分、数据流、失败降级原则
- 验证清单

### 本文档不覆盖

- 真实 terminal 渲染接入
- SSH / SFTP 功能实现
- `Diagnostics` 页具体 UI 设计
- 远程崩溃上报平台接入
- `WER LocalDumps` / `MiniDumpWriteDump` 第二阶段实现细节
- 逐文件命令级 implementation plan

## 设计要点与方案对比

### 1. 日志框架

#### 方案 A1：继续扩展自研 `std::fs + OpenOptions + eprintln!`

优点：

- 依赖最少
- 改造成本最低

缺点：

- 需要自行维护 level、filter、rolling、non-blocking、多 sink
- 后续接入 SSH/SFTP/terminal 后容易失控
- 不适合作为长期诊断基建

#### 方案 A2：采用 `tracing + tracing-subscriber + tracing-appender`

优点：

- Rust 生态主流方案
- 与 Tokio/异步上下文天然契合
- 支持 rolling file、filter、non-blocking
- 后续可平滑扩展到 JSON、span、target 细分

缺点：

- 首轮接入复杂度略高于自研最小实现

#### 方案 A3：采用 `log + fern/env_logger/flexi_logger`

优点：

- 成熟稳定
- 传统桌面应用可用

缺点：

- 在结构化事件、异步上下文和未来 span 诊断上弱于 `tracing`

**最终决策：选择 `A2`**

### 2. 默认日志目录

#### 方案 B1：默认程序目录 `./logs`

优点：

- 直观
- 便于手工查找

缺点：

- 不符合 Windows 桌面应用常规
- 在 `Program Files`、MSIX 或标准用户权限场景下不稳
- 多用户环境隔离差

#### 方案 B2：默认标准应用目录

优点：

- 符合平台规范
- 权限更稳
- 用户隔离天然成立

缺点：

- 用户手工定位路径不如程序目录直接

#### 方案 B3：默认标准应用目录，同时支持 portable/override

做法：

- 安装版默认走标准本地应用数据目录
- 环境变量可覆盖日志目录
- 便携版通过 portable marker 自动切到程序目录相对路径

优点：

- 兼顾标准安装与便携分发
- 适合 Windows 首发，也利于后续跨平台

缺点：

- 路径优先级规则多一层

**最终决策：选择 `B3`**

日志路径优先级：

`env override > portable marker > standard local app data`

### 3. 默认记录范围

#### 方案 C1：只记录 `panic/fatal`

优点：

- 最安静

缺点：

- 大量“未崩溃但功能失效”的严重问题会丢失

#### 方案 C2：默认持久化 `ERROR` 与 `PANIC/FATAL`

优点：

- 符合“默认只记严重问题”的目标
- 仍保留关键故障证据

缺点：

- 需要设计调试提升入口

#### 方案 C3：默认持久化 `WARN` 及以上

优点：

- 线索更多

缺点：

- 日志噪音明显增加

**最终决策：选择 `C2`**

### 4. 崩溃与严重故障捕获

#### 方案 D1：`panic::set_hook` + 顶层 fatal 收口

优点：

- 跨平台
- 实现难度可控
- 足以覆盖大多数 Rust panic 和启动失败

缺点：

- 对底层 native crash 的诊断深度有限

#### 方案 D2：在 D1 基础上增加 Windows dump 方案

优点：

- 诊断能力更强

缺点：

- 复杂度高
- 超出首发范围

#### 方案 D3：直接接外部崩溃平台

优点：

- 长期运维体验更强

缺点：

- 当前阶段过重

**最终决策：首发选择 `D1`，`D2` 作为第二阶段高级诊断方向**

### 5. 文件结构

#### 方案 E1：按日期建目录

示例：

- `logs/2026-03-11/error.log`
- `crash/2026-03-11/panic-15-20-33.log`

优点：

- 目录结构直观

缺点：

- 与 `tracing_appender` 的日滚动模型不完全贴合
- 清理规则更麻烦

#### 方案 E2：扁平目录 + 按天滚动文件

示例：

- `logs/system-error.log.2026-03-11`
- `crash/panic-2026-03-11T15-20-33.log`

优点：

- 最贴合现成生态能力
- 清理规则简单

缺点：

- 不如日期目录直观

#### 方案 E3：按会话建目录

优点：

- 单次复现隔离最好

缺点：

- 目录增长最快
- 对默认只记严重问题来说过度设计

**最终决策：选择 `E2`**

### 6. 保留策略

#### 方案 F1：只按保留天数

优点：

- 简单

缺点：

- 某天错误爆量时占用不可控

#### 方案 F2：只按总容量

优点：

- 磁盘占用可控

缺点：

- 时间维度不稳定

#### 方案 F3：保留天数 + 总容量双阈值

优点：

- 兼顾时效性与磁盘上限
- 更符合成熟桌面客户端行为

缺点：

- 规则稍复杂

**最终决策：选择 `F3`**

默认策略：

- 保留最近 `14 天`
- 总容量上限 `64MB`

### 7. 调试模式提升

#### 方案 G1：仅环境变量

#### 方案 G2：直接做持久化配置

#### 方案 G3：首发只做临时提升，后续再做持久化入口

**最终决策：选择 `G3`**

补充说明：

- 首发阶段只支持临时提升日志级别
- 后续 `Diagnostics` 页中的持久化日志级别开关作为 TODO，不纳入本轮实现

### 8. 日志格式

#### 方案 H1：human-readable text

优点：

- 本地打开即可阅读
- 最适合当前桌面端排障

缺点：

- 自动分析不如 JSON 方便

#### 方案 H2：JSON Lines

优点：

- 结构化最强

缺点：

- 人工阅读较差

#### 方案 H3：双格式

优点：

- 同时兼顾人读与机读

缺点：

- 首发阶段偏重

**最终决策：选择 `H1`**

## 最终决策

### 总体策略

- 框架采用 `tracing + tracing-subscriber + tracing-appender`
- 默认目录采用平台标准本地应用数据目录
- 支持 portable marker 与环境变量 override
- 文件结构采用扁平目录 + 按天滚动文件
- 默认只持久化 `ERROR` 与 `PANIC/FATAL`
- `panic/fatal` 默认附带 backtrace
- 保留策略采用 `14 天 + 64MB`
- 首发仅支持临时调试提升
- `Diagnostics` 页中的持久化日志级别开关作为后续 TODO

### 模块拆分

建议新增以下长期模块：

- `app::logging::paths`
- `app::logging::config`
- `app::logging::runtime`
- `app::logging::cleanup`
- `app::logging::panic`

短期过渡点：

- 当前 `TooltipDebugLog` 只视为临时调试代码
- `ui.tooltip` 不是长期必保留日志域
- tooltip bug 修复完成后，该类 instrumentation 可以直接移除

### 初始化顺序

1. 解析日志路径
2. 构建 logging config
3. 初始化 `tracing runtime`
4. 注册 `panic::set_hook`
5. 执行轻量 cleanup
6. 进入现有 `AppWindow::new()` / `bind_top_status_bar()` / `run()`

### 数据流

普通严重错误流：

- 模块内产生 `ERROR`
- subscriber 根据默认 filter 判断可写
- 写入 `logs/` 的日滚动文件

panic / fatal 流：

- `panic::set_hook` 或顶层启动失败收口
- 生成包含 `message/location/thread/backtrace` 的 fatal record
- 写入 `crash/` 独立文件

调试模式流：

- 临时通过 env/CLI 提升 level
- `ui`、未来 `ssh`、`sftp`、`terminal` 的 debug 信息进入统一 tracing 输出

## 错误等级、事件分类与命名规范

### 等级约束

- `ERROR`
  - 表示功能失败但进程未崩
  - 默认落盘到 `logs/`
- `PANIC/FATAL`
  - 表示进程即将退出或已不可恢复
  - 默认写入 `crash/`
- `WARN`
  - 本轮默认不落盘
  - 仅在临时调试模式启用
- `INFO / DEBUG / TRACE`
  - 本轮默认不落盘
  - 仅在临时调试模式启用

### 事件分类建议

- `app.lifecycle`
- `app.logging`
- `app.window`
- `ui.*`
- `config.preferences`
- `ssh.connection`
- `sftp.transfer`
- `terminal.session`
- `platform.windows`
- `platform.macos`
- `platform.linux`

说明：

- `ui.tooltip` 仅作为当前问题排查的临时调试域，不作为长期设计要求。

### 命名规则

- 使用稳定 `target`，不要把具体模块路径当成长期契约
- `message` 只描述“发生了什么”
- 关键信息尽量记录为字段，例如：
  - `session_id`
  - `host`
  - `port`
  - `path`
  - `error_kind`
  - `phase`
- 不记录明文密码、私钥内容、完整敏感输出
- 同类错误保持字段集合稳定

## 实施步骤

1. 新增 logging 模块边界，独立出路径、配置、runtime、cleanup、panic 五部分。
2. 在启动最早阶段完成日志路径解析与 runtime 初始化。
3. 注册 `panic::set_hook`，确保 panic/fatal 进入 `crash/`。
4. 把顶层启动错误和关键初始化错误统一收口到 `fatal`。
5. 把现有零散 `eprintln!` 逐步迁移到统一 `tracing`。
6. 将临时 `TooltipDebugLog` 视为待移除调试实现，不纳入长期系统日志能力。
7. 为路径优先级、级别过滤、panic/fatal、cleanup、降级策略补测试。
8. 后续若用户确认，再单独产出 implementation plan。

## 风险与回滚

### 主要风险

- 日志初始化放得过晚，导致启动失败无法记录
- portable marker 与 override 规则实现不清，导致路径行为混乱
- panic hook 处理不当，出现二次失败
- cleanup 逻辑过重，拖慢启动
- 临时调试事件和长期日志边界不清，导致系统日志被 UI 细节污染

### 风险控制

- 把日志初始化前置到 UI 创建前
- 明确路径优先级为固定规则
- panic hook 只做最小、安全的输出工作
- cleanup 失败只降级，不阻断启动
- 将 `ui.tooltip` 标记为临时 instrumentation，而非正式日志域

### 回滚策略

- 若统一 logging runtime 接入出现问题，可暂时回退到最小 `stderr/eprintln!` 输出
- 若 cleanup 策略不稳定，可先关闭容量/天数清理，仅保留写入主链路
- 若 panic hook 集成不稳定，可保留 runtime 初始化，暂时只记录顶层 fatal

## 失败处理与 Fallback

- 日志系统是诊断基础设施，不应阻断应用启动。
- 路径解析失败时，尝试下一级候选路径；全部失败则退回 `stderr`。
- runtime 初始化失败时，应用继续启动，保留最小错误输出。
- 滚动文件创建失败时，本次不写文件，下次写入仍可重试。
- cleanup 失败时，只记一条降级错误。
- panic hook 写文件失败时，退回 `stderr`，避免在 hook 内再次制造崩溃。
- backtrace 获取失败时，仍保留 `message/location/thread`，并标记 `backtrace_unavailable`。

## 验证清单

- [ ] 安装版默认进入标准本地应用数据目录
- [ ] portable marker 命中后切换到程序目录相对路径
- [ ] 环境变量 override 具有最高优先级
- [ ] `logs/` 生成按天滚动文件
- [ ] `crash/` 生成独立 panic/fatal 文件
- [ ] 默认模式下 `WARN/INFO/DEBUG` 不落盘
- [ ] `ERROR` 默认可落盘
- [ ] 调试模式提升后，低级别事件出现
- [ ] panic 触发后生成包含 `message/location/thread/backtrace` 的 crash 文件
- [ ] 超过 `14 天` 的文件会被清理
- [ ] 超过 `64MB` 时按最旧文件淘汰
- [ ] 日志目录不可写时，应用仍可启动
- [ ] logger 初始化失败时，不影响主窗口运行
- [ ] `ui.tooltip` 这类临时调试事件不会成为长期正式日志要求

## 后续 TODO

- 新增 `Diagnostics` 页
- 在 `Diagnostics` 页提供持久化日志级别开关
- 评估第二阶段 Windows dump 能力：
  - `WER LocalDumps`
  - `MiniDumpWriteDump` / `minidumper`


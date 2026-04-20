# SFTP Resumable Transfer Design

日期: 2026-04-20
执行者: Codex
状态: 已确认

## 背景

当前项目已经具备：

- 右上角入口和独立 `Transfer Center`；
- 基于 `TransferQueue` / `TransferTask` 的上传、下载、删除、移动任务模型；
- `quick browser` 与独立 SFTP workspace 共用的远端文件浏览能力。

但现阶段的传输实现仍然停留在“整文件一次性搬运”阶段：

- 上传路径在 `src/app/sftp/session_binding.rs` 中通过 `fs::read(...)` 把整个本地文件读入内存，再调用 `runtime.upload_file(...)` 一次写入；
- 下载路径在 `src/app/sftp/session_binding.rs` 中通过 `runtime.download_file(...)` 一次读出整份远端内容，再用 `fs::write(...)` 落地到本地；
- `src/app/sftp/runtime.rs` 当前只暴露 `upload_file(remote_path, data)` 和 `download_file(remote_path) -> Vec<u8>` 这类整文件接口；
- `src/app/bootstrap/sftp.rs` 中的后台调度也是围绕“创建临时 `TransferQueue` -> 执行 -> 把快照投影回 UI”的一次性批处理，而不是长期持久化下载中心；
- 应用数据目录已经有统一入口 `src/app/app_paths.rs`，且资产、钥匙串都已使用 `redb` 做本地持久化，但传输任务尚未持久化。

因此，当前实现虽然有“传输中心”的 UI 形态，但还不符合成熟桌面 SFTP 客户端对“下载中心 / 上传中心”的预期：

- 断线后不能按已完成字节继续；
- 重启应用后不会保留未完成任务；
- 大文件传输会产生不必要的内存峰值；
- 上传/下载的恢复语义、错误语义、临时文件语义都不清晰。

## 目标

### 产品目标

本轮要把当前 SFTP 传输能力升级为真正的“可断点传送下载中心”：

- 支持 SFTP 单文件上传和下载的断点续传；
- 支持应用运行中断线后的自动恢复；
- 支持应用重启后的任务恢复与继续；
- 目录任务仍可由用户一次发起，但内部按“目录任务 = 多个文件/目录结构任务”执行，每个文件独立断点续传；
- 传输中心要明确区分 `Resume`、`Restart`、`Interrupted`、`Failed` 等状态，而不是把所有异常都压成同一种失败。

### 体验目标

- 用户把任务加入 `Transfer Center` 后，即使 SSH/SFTP 中途断线，也能在重连后继续；
- 用户关闭应用后重新打开，未完成任务仍然存在，并且能校验现场后继续；
- 下载不会直接污染正式目标文件，上传也不会在远端把半截内容冒充成正式文件；
- 大文件不再走“整文件进内存”的实现路径。

## 非目标 / 边界

本轮不包含：

- 多连接并发分片下载或上传；
- 全量 checksum 同步引擎；
- 跨协议统一断点续传抽象；
- 对所有 SFTP 服务端做“必然支持续传”的承诺；
- 在本设计文档中拆到逐步实现任务，implementation plan 另写。

边界定义：

- 首版只要求“单文件真断点续传”；
- 目录任务由多个文件任务组成，每个文件任务独立记录 checkpoint；
- 若服务端或现场状态不允许安全续传，产品必须显式回退到 `Restart required`，而不能静默重头传。

## 行业参考

本设计采用的产品方向，参考了成熟桌面客户端已经验证过的行为：

- WinSCP 官方文档明确说明 SFTP 支持 transfer resume，并建议在传输期间先写入临时文件名，再在完成后重命名，以便在中断后恢复；
- WinSCP 还支持会话在传输中断后自动 reconnect 并继续传输；
- Cyberduck 官方文档说明 `Transfers` 列表在应用关闭后会保留，重启后可以重新取回并继续；
- Cyberduck 对 SFTP 上传和下载都明确支持 `Resume`；
- `russh-sftp 2.1.1` 官方文档显示其 `client::fs::File` 支持 `AsyncRead`、`AsyncWrite` 与 `AsyncSeek`，并提供 `OpenFlags`，说明基于偏移量的续传在当前依赖层面是可行的。

参考链接：

- `https://winscp.net/eng/docs/resume`
- `https://winscp.net/eng/docs/ui_pref_resume`
- `https://docs.duck.sh/cyberduck/transfer/`
- `https://docs.rs/russh-sftp/latest/russh_sftp/client/fs/struct.File.html`
- `https://docs.rs/russh-sftp/latest/russh_sftp/protocol/struct.OpenFlags.html`

## 方案比较

### 方案 A：在现有队列上补“失败后重试即续传”

做法：保留当前 `TransferQueue` 与整文件上传/下载接口，只在 `Retry` 时检查本地或远端已有文件大小，然后尝试从该偏移继续。

优点：

- 代码改动最少；
- 适合临时热修；
- 能较快做出“失败后接着传”的表层行为。

缺点：

- 无法解决当前整文件读写的内存问题；
- 很难自然支持应用重启后的恢复；
- 行为会被绑定到 UI 按钮语义，而不是稳定的持久化下载中心；
- 容易出现“看起来是续传，实际上是重传”的伪恢复逻辑。

### 方案 B：持久化任务 + 流式偏移传输

做法：

- 把传输任务和 checkpoint 持久化到本地数据库；
- 下载写本地 `*.part` 临时文件，上传写远端 `*.part` 临时文件；
- 实际传输改成分块读取 / 分块写入 / 基于 offset seek 的流式执行；
- 完成后再 rename 成正式目标文件；
- 启动时恢复未完成任务，校验现场后继续。

优点：

- 最符合成熟桌面客户端的用户预期；
- 运行中断线和应用重启后的恢复都自然成立；
- 顺手解决大文件内存峰值；
- 能把 `Transfer Center` 真正变成“长期存在的任务中心”。

缺点：

- 涉及 runtime、queue、bootstrap、UI 状态与持久化的中等规模重构；
- 需要补充恢复校验逻辑和临时文件清理策略。

### 方案 C：分片清单 + 校验驱动的高级恢复引擎

做法：把每个文件切成多个 chunk，记录 chunk 清单、校验、并发状态，失败后逐块恢复。

优点：

- 恢复能力最强；
- 后续容易扩展并发分片、多连接下载。

缺点：

- 明显超出当前项目边界；
- 会把当前 SFTP 传输演进成独立下载引擎；
- 与当前代码结构和需求规模不匹配。

### 最终决策

采用方案 B：`持久化任务 + 流式偏移传输`。

## 当前实现现状

### 1. 传输 API 仍是整文件读写

`src/app/sftp/runtime.rs` 的接口仍然是：

- `upload_file(remote_path, data: Vec<u8>)`
- `download_file(remote_path) -> Vec<u8>`

这意味着 runtime 边界本身没有暴露“打开远端文件并基于 offset 继续读写”的能力。

### 2. 执行路径会把整文件读入内存

`src/app/sftp/session_binding.rs` 当前上传逻辑：

- 先 `fs::read(&local_path)`；
- 再调用 `runtime.upload_file(...)`。

当前下载逻辑：

- 先 `runtime.download_file(...)`；
- 再 `fs::write(&local_path, &bytes)`。

这对大文件不友好，也让真正的断点续传没有落脚点。

### 3. 传输任务是内存态，不是持久态

- 任务列表当前保存在 `ShellViewModel.sftp_transfer_tasks`；
- `TransferQueueSummary` 也只是从当前内存任务计算而来；
- `src/app/bootstrap/sftp.rs` 背景线程执行完成后，才把队列快照投影回 UI。

这意味着应用退出后，未完成任务无法恢复。

### 4. 已存在适合接入持久化的本地基础设施

- `src/app/app_paths.rs` 已定义统一 `data_dir`；
- `src/app/assets_catalog/redb_store.rs` 与 `src/app/keychain/redb_store.rs` 已经证明：本项目接受 `redb` 作为桌面本地状态库。

因此，传输任务持久化沿用同一模式是自然且一致的。

## 核心设计

## 设计要点 1：任务模型从“瞬时队列项”升级为“可恢复任务”

现有 `TransferTask` 继续作为 UI 与队列的主投影结构，但需要补足 resume 语义：

- `resume_mode`：是否允许断点续传、是否只能重传；
- `temp_target_path`：下载时的本地 `.part` 路径，上传时的远端 `.part` 路径；
- `bytes_confirmed`：已确认写入目标临时文件的字节数；
- `bytes_total`：源文件总大小；
- `source_fingerprint`：首版至少存源文件大小与 `mtime`；
- `persisted_state`：区分 `Paused`、`Interrupted`、`VerifyingResume`、`Failed` 等恢复相关语义；
- `last_checkpoint_at`：最近一次落库时间；
- `retry_policy`：恢复失败时是提示用户、自动重传还是停在 attention 状态。

目录任务仍然通过多个文件任务表达：

- 目录创建任务只负责 `mkdir`；
- 每个文件单独记录 checkpoint；
- “恢复目录任务”本质是恢复剩余未完成子任务。

## 设计要点 2：引入持久化传输存储

在 `src/app/sftp` 下新增本地存储模块，例如：

- `src/app/sftp/transfer_store.rs`

职责：

- 保存未完成 / 已暂停 / 已中断任务；
- 保存每个任务的 checkpoint；
- 保存必要的 UI 恢复字段（方向、源路径、目标路径、宿主 session、当前状态、错误摘要）；
- 启动时加载任务并投影回 `ShellViewModel`；
- 提供清理已完成、移除记录、升级 schema 的能力。

数据库建议：

- 新建 `transfers.redb`；
- 沿用现有 `redb` 封装风格，使用 metadata table + task records table；
- 只持久化下载中心真正需要恢复的数据，不把瞬时 UI 缓存一起塞进去。

## 设计要点 3：runtime 从整文件 API 升级为流式文件 API

要实现真正的断点续传，`src/app/sftp/runtime.rs` 与 `src/app/ssh/runtime/sftp_backend.rs` 需要从“整文件搬运”升级为“可 seek 的流式远端文件句柄”。

建议新增一层抽象：

- `open_remote_reader(path)`
- `open_remote_writer(path, flags)`
- `remote_metadata(path)`
- `remote_try_exists(path)`

句柄能力要求：

- 支持 `AsyncRead` / `AsyncWrite` / `AsyncSeek`；
- 支持按 `OpenFlags` 打开写入、追加或创建句柄；
- 支持读取远端文件 metadata 以校验大小和时间。

这样：

- 下载可以在本地 `.part` 已有 `N` 字节时，让远端 reader `seek(N)` 后继续读；
- 上传可以在远端 `.part` 已有 `N` 字节时，让本地 reader `seek(N)`、远端 writer `seek(N)` 后继续写。

## 设计要点 4：下载采用本地 `.part`，上传采用远端 `.part`

### 下载

- 正式目标：`report.zip`
- 传输中目标：`report.zip.part`
- 中断后保留 `.part`
- 完成后原子 rename 为正式文件

优点：

- 不污染最终文件名；
- 用户能明确知道该文件仍未完成；
- 恢复时只需读取 `.part` 的大小即可获得候选 offset。

### 上传

- 正式目标：`/srv/app/release.tar.gz`
- 传输中目标：`/srv/app/release.tar.gz.part`
- 完成后 rename 成正式路径

优点：

- 避免远端消费方把半截文件当作正式产物；
- 恢复时只需要检查 `.part` 是否存在以及当前大小。

冲突策略仍保留现有方向：

- 若正式目标已存在，沿用现有 overwrite / skip / conflict 语义；
- 续传优先围绕 `.part` 文件进行，不直接把正式目标当断点文件。

## 设计要点 5：恢复流程分成“校验现场”和“继续传输”两个阶段

### 运行中断线

- 运行中的任务先转成 `Interrupted`；
- 若 session 自动重连成功，任务进入 `VerifyingResume`；
- 校验通过后再切回 `Running`。

### 应用重启

启动时加载 `transfers.redb` 中的未完成任务，并逐个做轻量恢复校验：

- 下载：
  - `.part` 是否存在；
  - `.part` 大小是否小于等于远端源文件大小；
  - 远端源文件大小 / `mtime` 是否与建任务时记录一致；
- 上传：
  - 本地源文件是否还存在；
  - 本地源文件大小 / `mtime` 是否未变化；
  - 远端 `.part` 是否存在；
  - 远端 `.part` 大小是否不超过本地源文件大小。

恢复结果：

- 校验通过 -> `Resume available`，可自动继续；
- 校验不通过但仍可重传 -> `Restart required`；
- 现场已损坏或路径不可达 -> `Failed`，等待用户处理。

## 设计要点 6：传输中心需要更精确的状态语义

当前 `Queued / Running / Paused / Completed / Failed / Conflict` 不足以表达恢复流程。

建议状态扩展为：

- `Queued`
- `Running`
- `Paused`
- `Interrupted`
- `VerifyingResume`
- `Failed`
- `Completed`
- `Conflict`

对应动作：

- `Running`：`Pause`、`Details`
- `Paused`：`Resume`、`Restart`、`Remove`
- `Interrupted`：`Resume`、`Restart`、`Details`
- `Failed`：根据能力显示 `Resume` 或 `Restart`
- `Completed`：保留 `Open File`、`Open Folder`、`Remove`

文案应显式表达恢复语义，例如：

- `Resuming — 1.4 GB / 5.0 GB`
- `Verifying partial file…`
- `Restart required — source changed`

## 设计要点 7：暂停语义采用“分块边界暂停”

首版 `Pause` 不需要引入复杂的底层抢占式 IO 取消。

建议：

- 传输引擎按固定 chunk 大小执行读写；
- 每写完一个 chunk 都检查暂停 / 取消信号；
- 命中暂停后立即 flush、写 checkpoint、更新任务状态为 `Paused`；
- 恢复时从最近一次已确认 offset 继续。

这样实现简单、行为稳定，也符合桌面客户端对 pause/resume 的常见预期。

## 详细数据流

### 下载

1. 创建下载任务，确定正式目标路径和 `.part` 路径；
2. 若 `.part` 已存在，读取其大小作为候选 offset；
3. 打开远端源文件并读取 metadata；
4. 校验 `.part` 大小是否合理；
5. 远端 reader `seek(offset)`，本地 writer 追加写入 `.part`；
6. 每完成一个 chunk：
   - 更新 `bytes_confirmed`；
   - 刷新内存态 UI；
   - 定期落库 checkpoint；
7. 读到 EOF 后 flush 并 rename `.part` -> 正式文件；
8. 任务标记为 `Completed`。

### 上传

1. 创建上传任务，确定正式远端路径和远端 `.part` 路径；
2. 若远端 `.part` 已存在，读取其大小作为候选 offset；
3. 校验本地源文件大小 / `mtime` 是否仍匹配；
4. 本地 reader `seek(offset)`，远端 writer `seek(offset)`；
5. 每完成一个 chunk：
   - 更新 `bytes_confirmed`；
   - 刷新 UI；
   - 定期落库 checkpoint；
6. 写完后关闭 writer，并 rename 远端 `.part` -> 正式路径；
7. 任务标记为 `Completed`。

## 错误处理策略

### 连接中断

- 不直接记成普通失败；
- 切成 `Interrupted`；
- 若能重连则优先自动恢复；
- 若长时间无法恢复，再进入 attention 状态。

### 源文件变化

- 上传：本地源文件大小或 `mtime` 变化 -> 不再续传，改为 `Restart required`；
- 下载：远端源文件大小缩小，或 `mtime` 明显变化 -> 不再续传，改为 `Restart required`。

### 临时文件异常

- `.part` 无法打开、seek、rename、flush 失败时保留现场；
- 不静默清除临时文件，避免把可恢复现场直接破坏。

### 服务端能力异常

若某些 SFTP 服务端对 append/seek 表现不可靠：

- 任务需显式标记为 `FallbackRestart`；
- UI 文案明确提示“该任务无法继续断点续传，将从头开始重传”；
- 不能伪装成 resume 成功。

### 本地磁盘/权限错误

- 直接进入 `Failed`；
- 只保留明确可恢复的现场，不做无意义自动重试。

## 测试策略

优先扩展现有测试带：

- `tests/sftp_transfer_flow_spec.rs`
- `tests/sftp_runtime_spec.rs`
- `tests/shell_view_model.rs`
- `tests/bootstrap_smoke.rs`
- `tests/transfer_center_smoke.rs`

必测场景：

1. 下载从本地 `.part` 继续；
2. 上传从远端 `.part` 继续；
3. `Pause -> Resume` 在 chunk 边界稳定生效；
4. 应用重启后从持久化任务恢复；
5. 本地源文件变化导致上传续传失效；
6. 远端源文件变化导致下载续传失效；
7. 目录任务恢复时只继续未完成子文件；
8. rename 成功后才标记 `Completed`；
9. 服务端不支持稳定续传时，明确回退到 `Restart required`；
10. 现有 completed/open/remove 等传输中心行为不回归。

## 风险与取舍

### 风险 1：`russh-sftp` 句柄级行为需要适配

虽然官方文档表明 `File` 支持 `AsyncSeek`，但实际项目集成时仍需验证：

- 打开 writer 时的 flags 组合；
- `seek` + `write_all` 在目标服务端上的兼容性；
- rename 时的覆盖语义。

### 风险 2：启动恢复会引入新的 bootstrap 顺序

启动恢复既要读取本地持久化任务，也要等相关 session 或 runtime 可用；需要避免把恢复流程塞进 UI 主线程，导致开机阶段卡顿。

### 风险 3：下载中心语义会更“产品化”

一旦引入持久化任务，就不能再把传输中心当成纯临时浮层；后续文案、清理策略、完成记录保留时长都要更稳定。

## 成功标准

- 单文件上传、下载都能真正按字节偏移续传；
- 断线重连后任务可恢复，不需要整文件重传；
- 应用重启后，未完成任务仍保留并能恢复；
- 下载使用本地 `.part`，上传使用远端 `.part`，完成后才 rename；
- 目录任务以单文件粒度恢复；
- 大文件传输不再走整文件内存搬运；
- 传输中心能清晰表达 `Interrupted / Resume / Restart required` 等状态；
- 当前已存在的冲突、完成态、本地打开/删除等能力不回归。

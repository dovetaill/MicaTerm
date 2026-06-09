# 第 1 步：Windows packaged 内存 baseline 验证手册

日期：2026-06-09  
工作目录：`/home/wwwroot/mica-term/.worktrees/feature-memory-footprint-reduction`  
分支：`feature/memory-footprint-reduction`

## 1. 目的与边界

- 本手册只建立可复现 baseline、日志口径、renderer/path 识别方式与测量矩阵。
- 本手册不宣称任何内存已经下降，更不把 `working set` 回落当成真实优化结论。
- 所有 packaged Windows 结果都必须同时记录 `private/commit` 与 `working set`，不能只凭 working set 回落宣称成功。

## 2. 打包矩阵

必须分开记录以下两条 packaged Windows 路径：

1. Windows mainline package：`./build-win-x64.sh`
2. Windows software compatibility package：`./build-win-x64-software.sh`

两条路径都必须记录：

- `requested_build_flavor`
- `requested_terminal_render_mode`
- `requested_native_present_path`
- `selected_backend`
- `selected_renderer`
- `selected_graphics_api`
- `fallback_level`
- `profile_selector`

判断规则：

- `requested_build_flavor=windows-mainline` 代表 mainline package。
- `requested_build_flavor=windows-software-compat` 代表 software compatibility package。
- `fallback_level=0` 代表命中首选 host renderer。
- `fallback_level>0` 代表发生 runtime fallback，必须单独记录，不能与首选路径混在同一组结论里。

## 3. 日志抓取口径

PowerShell 启动步骤：

```powershell
ni .mica-term-portable -ItemType File -Force
$env:MICA_TERM_LOG = "debug"
$env:MICA_TERM_MEMORY_DIAGNOSTICS = "1"
.\mica-term.exe
```

推荐日志筛选：

```powershell
Select-String -Path .\logs\system-error.log* -Pattern "app.renderer","requested_build_flavor","requested_terminal_render_mode","requested_native_present_path","fallback_level","app.memory"
```

解释：

- `app.renderer` 用来确认 packaged mainline / software compatibility / runtime fallback 的真实落点。
- `app.memory` 用来补充后续 step 2 及之后的内存事件观测。
- 若日志里看不到 `requested_build_flavor`、`requested_terminal_render_mode`、`requested_native_present_path` 或 `fallback_level`，则 baseline 记录不完整。

## 4. 计量指标

优先记录以下 Windows 指标：

- `Private Bytes`
- `Page File Bytes / Commit Size`
- `Working Set - Private`
- `Working Set`
- `Handle Count`
- `Thread Count`

推荐外部工具：

- Process Explorer
- VMMap
- PerfMon
- `typeperf`

`typeperf` 示例：

```powershell
typeperf "\Process(mica-term)\Private Bytes" "\Process(mica-term)\Page File Bytes" "\Process(mica-term)\Working Set - Private" "\Process(mica-term)\Working Set" "\Process(mica-term)\Handle Count" "\Process(mica-term)\Thread Count"
```

解释规则：

- `Private Bytes` / `Page File Bytes / Commit Size` 是判断真实私有提交是否回落的主轴。
- `Working Set` 只说明当前驻留页，不足以证明真实对象已经释放。
- `Handle Count` / `Thread Count` 作为辅助指标，用于识别 runtime/service retained state。

## 5. 场景矩阵

每个 package 都必须完整覆盖以下场景：

1. 冷启动空载
2. 欢迎页空载 30 秒 / 60 秒
3. 首次打开终端
4. 大量输出
5. 重滚动
6. 3 到 5 个 session
7. 关闭全部 session 后立即 / 30 秒 / 60 秒
8. 重启应用

建议记录模板：

| package | 场景 | renderer/path 身份 | Private Bytes | Page File Bytes / Commit Size | Working Set - Private | Working Set | Handle Count | Thread Count | 备注 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| mainline | 冷启动空载 | `requested_build_flavor=windows-mainline`, `fallback_level=0/1...` | 待填 | 待填 | 待填 | 待填 | 待填 | 待填 | 记录欢迎页是否稳定 |
| software compatibility | 冷启动空载 | `requested_build_flavor=windows-software-compat`, `fallback_level=0/1...` | 待填 | 待填 | 待填 | 待填 | 待填 | 待填 | 记录 software host 与 native terminal 组合 |

## 6. 本地可验证部分

当前 Linux worktree 内，本步只验证以下 contract 与脚本口径：

- runtime profile 仍区分 mainline / software compatibility。
- `app.renderer` 启动日志现在会暴露 `requested_build_flavor`、`requested_terminal_render_mode`、`requested_native_present_path` 与 `fallback_level`。
- `MICA_TERM_MEMORY_DIAGNOSTICS=1` 现在会额外输出 `startup-snapshot`、`session-close`、`close-shrink`、`idle-shrink`、`trim-request`、`trim-executed`，并带上 `private_usage_bytes` / `pagefile_usage_bytes` 相关字段。
- `session-close` 现在会额外暴露 `before_session_count` / `after_session_count`、`before_runtime_control_count` / `after_runtime_control_count`、`terminal_memory_release_succeeded`、`runtime_disconnect_succeeded`，用于区分 session runtime 真实释放与后续 surface clear / idle shrink 的视觉清理。
- `readme.md` 指向本手册，避免 packaged baseline 复现流程孤立。
- Windows 打包 wrapper smoke 继续锁住 `./build-win-x64.sh` 与 `./build-win-x64-software.sh` 的 packaged 合同。

## 7. 当前缺口

- 本 worktree 不能在当前 Linux 环境直接完成真正的 Windows packaged 运行测量。
- 本步尚未证明任何 `private/commit` 已下降；这里只是把 baseline、记录字段、脚本口径与测量矩阵固定下来。
- 本 worktree 仍不能替代真实 Windows packaged 运行；即便这些事件已经接线，本地也只能验证日志 contract，不能直接证明现场数值改善。

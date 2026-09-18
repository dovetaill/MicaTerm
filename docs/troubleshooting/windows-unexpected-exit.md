# Windows 无提示退出排查

当前 crash/panic 日志捕获 Rust panic，不覆盖全部原生异常、abort、强制终止。
默认 tracing 只输出 error，且使用异步队列，因此空日志不能说明没有异常。

## 新版诊断记录

应用数据目录的 `logs/lifecycle-时间戳-PID.log` 同步写入关键阶段，不受
`MICA_TERM_LOG` 过滤影响；与普通日志一同执行已有的 14 天/64 MiB 清理策略。
目录遵循 `MICA_TERM_APP_DIR`、便携模式和标准数据目录的现有规则。

- `ui-window-close-requested`：自定义关闭按钮触发。
- `os-window-close-requested`：窗口系统发出关闭请求（不能据此断言是用户操作）。
- `event-loop-returned-ok/error`：事件循环返回。
- `async-runtime-shutdown-begin/complete`：异步运行时销毁进度。
- `shutdown-complete`：应用工作及运行时销毁完成。
- `application-error`：返回错误；详细错误链写入 crash/fatal 文件。

缺少 shutdown-complete 只表示未记录完整退出，不能区分崩溃、任务管理器终止、
断电或日志写入失败。每次启动单独建文件，支持多实例，不把仍在运行的实例误报为崩溃。

## 抓取真正的 Windows dump

从微软官方下载 ProcDump：
https://learn.microsoft.com/en-us/sysinternals/downloads/procdump

先关闭应用，在 PowerShell 中执行（按实际位置修改路径）：

```powershell
New-Item -ItemType Directory -Force C:\MicaDumps
$env:MICA_TERM_LOG = 'debug'
$env:MICA_TERM_MEMORY_DIAGNOSTICS = '1'
.\procdump64.exe -ma -e -t -x C:\MicaDumps 'C:\Apps\mica-term.exe'
```

`-e` 捕获未处理异常，`-t` 捕获进程终止（正常关闭也会产生 dump），`-ma` 保存完整内存。
保持 ProcDump 运行直到问题出现。dump 放在独立目录，避免应用日志清理删除。
若进程已经运行，可用具体 PID 附加：`procdump64.exe -ma -e -t 1234 C:\MicaDumps`。

保存当次 lifecycle、普通日志、crash 文件、ProcDump 输出、dump、对应版本 exe/PDB，
以及退出时间和最后操作。Windows 事件查看器 → Windows 日志 → 应用程序中的
Application Error / Windows Error Reporting 记录也能提供异常码和故障模块。
完整 dump 可能包含会话内容和凭据，应通过私密渠道传递。

用 WinDbg 打开 dump 后执行 `!analyze -v`，结合异常线程栈和匹配的 PDB 定位。
只有拿到这些证据，才能区分内存访问错误、栈溢出、原生渲染组件异常、显式退出等原因。
当前代码检查确认的是日志覆盖缺口，还没有证据证明某个具体模块导致本次闪退。

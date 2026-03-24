# SSH Shell Modal Runtime Tabs Verification

日期: 2026-03-24
阶段: Task 10 verification
状态: 自动化验证通过；Win11 人工验证与内存基线记录待补

## 自动化验证

### 聚焦测试集

已执行：

```bash
cargo test --test async_runtime_spec --test bootstrap_profile_smoke --test assets_modal_smoke --test shell_view_model --test assets_catalog_domain --test assets_catalog_store --test credential_store_spec --test ssh_profile_spec --test ssh_session_manager_spec --test terminal_session_spec --test workspace_tabs_spec --test bootstrap_smoke --test known_hosts_spec
```

结果：

- PASS

### UI Contract Smoke

已执行：

```bash
bash tests/assets_modal_ui_contract_smoke.sh
bash tests/shell_layout_ui_contract_smoke.sh
bash tests/ssh_connect_tabs_ui_contract_smoke.sh
```

结果：

- PASS

### 全量回归

已执行：

```bash
cargo test
```

结果：

- PASS

说明：

- `tests/panic_logging.rs` 中的 `panic_hook_writes_crash_file_for_child_process` 会显式拉起子进程并触发一次受控 panic；日志中会出现子进程 panic 输出，但最终测试结果为 PASS，属于预期行为。

## 关键结果记录

### 1. 启动后无白色竖条

- 状态：待人工验证
- 自动化依据：
  - `tests/shell_layout_policy.rs`
  - `tests/workspace_tabs_spec.rs`
  - `bash tests/shell_layout_ui_contract_smoke.sh`
- 备注：布局契约已由 `WorkspacePane` 收口，但仍需 Win11 真机观察首屏。

### 2. tab 数量增加时 workspace 不再被撑裂

- 状态：待人工验证
- 自动化依据：
  - `tests/workspace_tabs_spec.rs`
  - `tests/assets_explorer_smoke.rs`
- 备注：默认复用与 `Open in New Tab` 已通过自动化覆盖。

### 3. `New SSH / New Folder / Rename / Edit` modal 可拖动、不可 click-away dismiss

- 状态：部分自动化覆盖，待人工验证拖动手感
- 自动化依据：
  - `tests/assets_modal_smoke.rs`
  - `tests/shell_view_model.rs`
  - `bash tests/assets_modal_ui_contract_smoke.sh`
- 备注：callback/backdrop/focus restore 已接线；需要人工确认拖动与阻断体验。

### 4. `Test / Connect / Save / Save and Connect` 语义符合设计

- 状态：自动化通过
- 自动化依据：
  - `tests/bootstrap_smoke.rs`
  - `tests/shell_view_model.rs`
  - `tests/ssh_session_manager_spec.rs`
- 结果摘要：
  - `TestConnection` 走真实 probe，不创建 tab
  - `Connect` 打开临时 session，不持久化资产
  - `Save` 只持久化
  - `SaveAndConnect` 先持久化再开 tab

### 5. `Connect` 打开的不是假连接 tab

- 状态：自动化通过
- 自动化依据：
  - `tests/ssh_session_manager_spec.rs`
  - `tests/terminal_session_spec.rs`
  - `tests/workspace_tabs_spec.rs`
- 结果摘要：
  - `Connected` 只在 handshake/auth/channel/pty/shell 成功后发出
  - terminal host 已消费 `TerminalSurfaceState`，不再展示 placeholder 文案

### 6. close tab 真正关闭 session

- 状态：自动化通过
- 自动化依据：
  - `tests/ssh_session_manager_spec.rs`
  - `tests/assets_explorer_smoke.rs`
  - `tests/workspace_tabs_spec.rs`
- 结果摘要：
  - `close_session` 会移除 registry 项与 surface snapshot
  - `Disconnected / Error` tab 保留，显式 close 才移除

### 7. 编辑 SSH 资产时字段正确回填，secret 仍由 keyring 管理

- 状态：自动化通过
- 自动化依据：
  - `tests/assets_modal_smoke.rs`
  - `tests/credential_store_spec.rs`
  - `tests/ssh_profile_spec.rs`
- 结果摘要：
  - 非敏感字段回填
  - 保存资产时通过 `credential_ref` 管理 secret
  - password / inline private key / passphrase 不直接落 catalog 明文

### 8. Windows 任务管理器或 Process Explorer 对比前后常驻内存

- 状态：待人工验证
- 自动化依据：
  - `tests/async_runtime_spec.rs`
  - `tests/bootstrap_profile_smoke.rs`
- 备注：
  - 结构上已合并为单一 app async runtime 接入 session bridge
  - 本环境无法替代 Win11 任务管理器或 Process Explorer 的常驻内存测量

## 建议的人工补充步骤

1. 在 Win11 真机启动应用，观察首屏 workspace 是否仍出现白色竖条。
2. 连续打开多个 SSH tab，确认 workspace 宽度稳定、tab close 焦点回退符合设计。
3. 拖动 `New SSH / Rename / Delete / Host Key` modal，确认拖动对象是 modal 本身，不是主窗口。
4. 用真实 SSH 主机验证 host key 首次确认、再次连接、认证失败和断线后的 tab 状态。
5. 用 Windows 任务管理器或 Process Explorer 记录空闲启动内存与打开多个 session 后的内存变化。

# SSH Status Reference Alignment Design

**Reference:** `dist/tmp/1.png`

**Goal:** 让 SSH connection status 页面在 `verifying_host_key_with_jump_host` 预览态下明显接近参考图，同时继续嵌入 MCA TERM 现有 app shell，而不是做成独立弹窗或工程调试页。

## 设计结论

- 页面结构改成单列产品页：`title + status badge -> horizontal hop chain -> main card -> connection details -> bottom action bar`。
- 不再使用当前的左侧窄 `workflow-rail` + 右侧 `current-task-panel` 双栏结构。
- 连接链路与页面内容拆开：hop chain 用于表达 `Local / Jump Host / Target` 的链路；main card 用于表达当前状态内容（verify / connecting / warning / failed）。
- `Connection details` 作为真正的可折叠产品级详情区，始终位于主卡下方，不再作为 diagnostics-only 区块，更不能固定在窗口底部。
- `Cancel connection` 作为底部左侧低强调 tertiary action，和右侧主动作共享同一 action bar，不再单独做成一块说明区。

## 与参考图对齐的关键点

- 标题区开放排版，不包进摘要卡。
- hop chain 为横向整条路径卡，支持无 jump host、单 jump host、多 jump host 扩展。
- verify 卡左侧使用 shield + key 主视觉，右侧使用字段式信息排版，并为关键字段提供 copy affordance。
- `verifying_host_key_with_jump_host` 作为主预览态，布局、层级、按钮位置、details 位置均以参考图为准。
- failed/warning/connecting 复用相同页面骨架，避免回退到旧 debug layout。

## 工程边界

- 保留现有 MCA TERM shell、tab、assets sidebar。
- 不重写真实 SSH runtime；通过最小侵入的 preview/demo 注入来稳定展示 6 个状态。
- 继续使用现有 `connection_progress` 投影主链路，但增加 hop-first 的页面数据组织与 preview fixtures。
- 新增一套轻量 SVG 图标，统一放入 `assets/icons/ssh-flow/`。

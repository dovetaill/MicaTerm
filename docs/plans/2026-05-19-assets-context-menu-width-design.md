# Assets Context Menu Width Design

日期: 2026-05-19
执行者: Codex
状态: 已确认方向，待在独立 worktree 中按 implementation plan 执行

## 目标

修复左侧 assets sidebar 右键菜单中多项文案显示不全的问题，确保当前英文菜单项在默认桌面窗口宽度下完整可见，不再出现 `New SSH Connection`、`Proxy ...`、`Upload ...` 这类被截断的表现。

本轮是共享右键菜单宽度契约修复，不是单独修 SSH 一种 target，也不是调整 workspace tab context menu。

## 当前现状结论

当前左侧 assets 右键菜单的宽度预算明显偏小，而且这个偏小是共享链路级问题：

- `ui/components/assets-context-menu-column.slint` 把 menu column 固定写成了 `224px`
- `src/shell/context_menu.rs` 同步把 column geometry 常量写成了 `224.0`
- `ui/components/assets-context-menu-row.slint` 里 label 区域还要继续扣掉 icon、padding、右侧留白，所以真实 label 可用宽度约只有 `146px`
- `ui/theme/typography.slint` 当前 UI 正文字体是 `JetBrains Maple Mono` `14px`，对英文长文案的宽度需求比比例字体更高

结合 `src/shell/context_menu.rs` 当前 label，可确认最容易被截断的项并不是偶发：

- `New SSH Connection`
- `Proxy Chrome via Server`
- `Upload SSH Public Key (ssh-copy-id)`
- `Upload Files...`
- `Upload Folder...`

因此，用户看到 SSH 资产右键时很多项显示不全，是当前宽度契约本身不足，不是 hover、clip 或单条 copy 的局部 bug。

## 范围

### 本轮覆盖

- 左侧 assets sidebar 共享右键菜单宽度契约
- SSH / Folder / SFTP / Keychain / Snippets 这些共用 assets context menu 的 target
- Rust placement / corridor rect / Slint overlay 的统一宽度传递
- 单元测试、bridge smoke、UI contract smoke 的回归保护

### 本轮不覆盖

- `ui/components/workspace-tab-context-menu.slint` 那条独立 tab 菜单
- 菜单文案重命名
- 资产树宽度、字体、row 高度、主题 token
- SFTP workspace 正文布局
- 菜单多行换行显示

## 方案对比

### 方案 A：统一固定加宽到一个大宽度

做法：

- 直接把共享 column width 提升到 `360px-368px`
- placement、corridor、overlay 全部跟着改成新的固定宽度

优点：

- 实现最简单
- 能直接覆盖当前用户提到的几个长文案

缺点：

- `New Folder`、`Open`、`Edit` 这类短菜单也会显得过宽
- 后续如果字体或 copy 再变长，还要继续人工抬宽度
- 视觉上不够克制

### 方案 B：共享分档自适应宽度（推荐）

做法：

- 根据当前可见菜单项中最长 label 的长度，选择统一 column width 档位
- 推荐三档：
  - `256px`：短菜单
  - `312px`：中菜单
  - `368px`：长菜单
- 当前一次菜单会话内，primary / secondary / tertiary column 统一使用同一宽度
- Rust placement 和 Slint column 都消费这个运行时宽度，而不再写死 `224`

优点：

- 既能覆盖当前超长项，又不会把所有菜单一律撑大
- 几何关系仍然稳定，corridor / clamp 不需要做复杂重构
- 容易写测试，便于后续维护

缺点：

- 比固定宽度多一层运行时宽度传递
- 需要更新 bridge 和 placement 测试

### 方案 C：缩短文案，保留 `224px`

做法：

- 把 `Proxy Chrome via Server`、`Upload SSH Public Key (ssh-copy-id)` 等改短
- 保持当前 menu width 基本不变

优点：

- 改动最少

缺点：

- 改用户可见 copy，不符合这次问题本质
- 后续再出现更长 label 还会继续坏
- 不是系统性修复

## 最终决策

采用方案 B：共享分档自适应宽度。

核心决策：

1. 修共享 assets context menu 宽度契约，而不是只修 SSH 一处。
2. 菜单仍保持单行、不换行；`overflow: elide` 继续保留，但只作为极端兜底，不应成为正常显示。
3. 使用三档宽度：
   - `256px`：最长 label `<= 18` 字符
   - `312px`：最长 label `19-26` 字符
   - `368px`：最长 label `>= 27` 字符
4. 一次菜单会话里，所有可见 column 共用同一宽度，避免多列 submenu 出现主列和子列宽度跳变。
5. 右键菜单 placement、column rect、corridor 计算全部改成消费运行时 width，不再依赖 `224.0` 魔法数。
6. 不在本轮改菜单文案，也不扩大到 workspace tab context menu。

## 详细行为

### 1. 宽度决策时机

- 菜单打开时，根据当前可见 column 中最长 label 计算本次 menu width
- 当 `open_path` 变化、可见 column 集合发生变化时，重新计算 width
- 如果重新计算后 width 变化，placement 需要同步重算，保证不出屏

### 2. 宽度档位规则

建议按字符长度分档，而不是精确像素测量：

- `0..=18` -> `256px`
- `19..=26` -> `312px`
- `27+` -> `368px`

原因：

- 当前字体是 mono，字符长度和视觉宽度基本同向
- 档位规则比精确文本测量更稳、更好测
- 对本轮现有 label 足够覆盖：
  - `New SSH Connection` -> `256px`
  - `Proxy Chrome via Server` -> `312px`
  - `Upload SSH Public Key (ssh-copy-id)` -> `368px`

### 3. 几何与 overlay 规则

- `AssetsContextMenuColumn` 不再固定 `224px`，改为消费外部传入的 `column-width`
- `AssetsContextMenuOverlay` 的总宽度继续按 column width + gap 计算，只是 width 改为运行时值
- `src/app/bootstrap/assets_keychain.rs` 的 placement / rect helper 也改为消费运行时 width
- `src/shell/context_menu.rs` 中 column offset 的 stride 改成 `column_width + CONTEXT_MENU_COLUMN_GAP`

### 4. 视觉规则

- 保持当前 icon、padding、row 高度、divider 语义不变
- 不改 hover / open surface token
- 不引入多行 wrap
- 目标是在现有 Fluent / Mica 风格下，仅解决展示预算不够的问题

## 代码边界

预计涉及：

- `src/shell/context_menu.rs`
- `src/app/bootstrap/assets_keychain.rs`
- `ui/app-window.slint`
- `ui/components/assets-context-menu-overlay.slint`
- `ui/components/assets-context-menu-column.slint`
- `tests/assets_context_menu_spec.rs`
- `tests/assets_context_menu_smoke.rs`
- `tests/assets_context_menu_ui_contract_smoke.sh`

明确不涉及：

- `ui/components/workspace-tab-context-menu.slint`
- `ui/shell/assets-sidebar.slint`
- `ui/theme/tokens.slint`

## 测试与验收

### 自动化回归

- `tests/assets_context_menu_spec.rs`：宽度分档、placement、column rect
- `tests/assets_context_menu_smoke.rs`：AppWindow bridge 与 overlay width projection
- `tests/assets_context_menu_ui_contract_smoke.sh`：Slint contract 与 runtime width 传递

### 手动验收

- 左侧 blank area 右键中的 `New SSH Connection` 完整显示
- SSH 资产右键中的 `Proxy Chrome via Server`、`Upload SSH Public Key (ssh-copy-id)` 完整显示
- SFTP blank-area 右键中的 `Upload Files...`、`Upload Folder...` 完整显示
- 菜单靠近窗口右边缘时仍正确翻转 / clamp
- 现有 corridor / dismiss / hover 交互不退化
- 不改任何菜单 copy

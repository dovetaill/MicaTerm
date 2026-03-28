# SSH Key Import Design

日期: 2026-03-28
方案名: `ssh-key-import`
状态: 已确认，可进入 implementation planning

## 背景

当前 SSH connection modal 已经具备以下能力：

- `Password` 登录；
- `Private Key` 登录；
- `Private Key` 支持两种底层来源：
  - `private_key_source = "content"`：直接保存私钥内容；
  - `private_key_source = "path"`：保存文件路径，连接时再从本地磁盘读取。

但从真实用户视角看，这套交互仍然不够直接：

- 用户需要自己理解“内容模式”和“路径模式”的差异；
- modal 里虽然可以粘贴私钥文本，也可以手填路径，但没有真正的“导入文件”能力；
- 用户容易误以为 SSH key 登录需要同时填写公钥和私钥。

本次需求非常明确：

- SSH 连接增加 `SSH Key` 登录方式；
- 用户想确认 SSH key 登录是不是需要“公钥私钥都要有”；
- UI 需要支持：
  - 导入私钥文件；
  - 粘贴私钥文本。

## 用户问题澄清

SSH key 登录时，客户端只需要提供私钥。

服务端需要提前安装与之配对的公钥，一般放在目标用户的 `~/.ssh/authorized_keys` 中。换句话说：

- 本客户端 UI 只需要收集 `private key`；
- 不需要额外要求用户在连接表单里手动录入公钥；
- 但需要在交互文案中明确说明这一点，避免误解。

## 目标

- 在 SSH connection modal 中，为 `SSH Key` 登录提供明确、可直接使用的交互；
- 支持两种等价输入方式：
  - 直接粘贴私钥文本；
  - 通过系统文件选择器导入私钥文件；
- 导入文件后，最终保存的是 `private_key_content`，而不是文件路径；
- 私钥内容继续走现有 secret store，不写入资产元数据；
- 保持对已有 `private_key_source = "path"` 资产的兼容。

## 非目标

- 本轮不改变 SSH runtime 的公钥认证协议实现；
- 本轮不要求用户上传或保存公钥；
- 本轮不移除底层 `PrivateKeyPath` runtime 分支；
- 本轮不扩展到 SFTP、jump host、proxy、environment 等额外配置重构；
- 本轮不做私钥格式预解析器或复杂密钥管理系统。

## 方案比较

### 方案 A：完全移除路径模式，所有私钥都保存为内容

新建和编辑 SSH key 连接时，统一只保留 `private_key_content`，底层不再暴露路径模式。

优点：

- 交互最简单；
- 新旧逻辑统一；
- 保存后与本地文件路径解耦。

缺点：

- 当前仓库已经存在 `path` 模式数据、runtime 分支和测试；
- 直接移除会引入老资产兼容风险；
- 改动范围不必要地扩大。

### 方案 B：兼容模式

对新建连接提供新的推荐交互：

- `Password`
- `SSH Key`
  - 粘贴私钥文本
  - 导入私钥文件

两种输入最终都落到 `private_key_content`。底层继续保留 `path` 模式，仅用于兼容旧资产。

优点：

- 完整满足需求；
- 新用户不需要理解底层 `path/content` 差异；
- 已保存的旧 SSH 资产不会被破坏；
- 风险最低。

缺点：

- 需要保留一层 legacy UI/数据兼容；
- 比纯内容模式多一点桥接代码。

### 方案 C：最小改动，在现有内容模式里只增加 Import 按钮

继续保留 `content/path` 二选一 UI，只在 `content` 输入框旁边新增 `Import`。

优点：

- 改动最少；
- 可以快速实现。

缺点：

- 交互语义依然不够干净；
- 用户仍然会面对“路径模式”与“内容模式”的选择负担；
- 不利于后续把 SSH key 登录塑造成推荐流程。

## 最终决策

采用方案 B：兼容模式。

也就是：

- 新建 SSH 连接时，`SSH Key` 登录只暴露推荐交互：
  - 粘贴文本；
  - 导入文件。
- 两者最终都保存为 `private_key_content`；
- 旧的 `private_key_source = "path"` 连接继续可编辑、可连接；
- 只有当旧连接被用户改成粘贴/导入私钥并保存时，才迁移为 `content` 模式。

## 交互设计

### Authentication 区域

继续保留两种一级认证方式：

- `Password`
- `SSH Key`

当选择 `SSH Key` 时：

- 新建连接默认进入 `content` 路径；
- 显示多行 `Private Key` 输入框；
- 输入框支持直接粘贴私钥内容；
- 输入框提供 `Import` 按钮，触发系统文件选择器；
- 保留单独的 `Passphrase` 字段。

### 私钥说明文案

在 `SSH Key` 区域显示一行简短说明：

`Only the private key is needed here. The public key must already be installed on the server.`

目的：

- 明确回答“是不是公钥私钥都要有”的问题；
- 减少用户把公钥误粘进私钥输入框的概率。

### Legacy File Path 兼容

如果编辑的是已有的 `private_key_source = "path"` 资产：

- modal 继续显示旧字段；
- 字段名改成 `Legacy File Path`，明确它是兼容路径，不是推荐新路径；
- 同时仍允许用户导入文件或直接粘贴私钥；
- 一旦用户保存了新的 `private_key_content`，该资产就迁移到 `content` 模式。

## 数据流设计

### 新建连接

1. 用户打开 `New SSH Connection`；
2. 选择 `SSH Key`；
3. 通过以下任一方式提供私钥：
   - 粘贴文本；
   - 点击 `Import`，从系统文件选择器读取文件内容；
4. modal draft 更新：
   - `auth_method = "private-key"`
   - `private_key_source = "content"`
   - `private_key_content = <imported or pasted content>`
5. 保存时：
   - `AssetSshConnectionSpec` 仍只保存元数据和 `credential_ref`；
   - `private_key_content` / `passphrase` 存入 credential store。

### 编辑旧路径资产

1. 打开已有 `path` 模式 SSH 资产；
2. modal 显示 `Legacy File Path`；
3. 如果用户不导入新私钥，也不粘贴内容：
   - 继续按旧路径模式保存和连接；
4. 如果用户导入私钥文件或粘贴私钥文本：
   - draft 自动切到 `private_key_source = "content"`；
   - 保存时迁移为内容模式。

## 文件导入行为

导入动作的实现原则：

- 通过 Rust 侧打开系统文件选择器；
- 只选择单个文件；
- 允许常见私钥文件名，但不强制文件扩展名；
- 成功后直接读取文本内容回填到 `private_key_content`；
- 导入成功不立即写盘，只更新当前 draft；
- 真正持久化仍发生在用户点击 `Save` / `Save and Connect` 时。

## 错误处理

### 用户取消文件选择

- 不报错；
- 不修改当前 draft；
- 不打断 modal 交互。

### 文件读取失败

- 在现有 modal feedback 区展示错误；
- 示例文案：`Failed to read private key file.`

### 导入空文件或非私钥文本

- 导入阶段只负责读文件，不做复杂密钥格式验证；
- 统一交给现有 `profile` / `runtime` 鉴权链路在 `Test` / `Connect` 时校验；
- 这样可以保证“粘贴文本”和“导入文件”走同一套错误语义。

## 架构边界

本次只修改以下范围：

- `ui/components/assets-ssh-connection-modal.slint`
- `ui/app-window.slint`
- `src/shell/view_model.rs`
- `src/app/bootstrap.rs`
- 必要的测试文件

本次不修改以下范围：

- `russh` 公钥认证流程本身；
- 已存在的 `PrivateKeyPath` runtime 分支；
- 资产树、workspace tab、session manager 的无关逻辑。

## 测试策略

需要覆盖的核心场景：

- 新建 SSH 连接时，`SSH Key` 模式可直接粘贴私钥文本；
- 点击 `Import` 后能通过系统文件选择器导入私钥内容；
- 导入成功后 draft 切换为 `content` 模式；
- 保存后私钥内容进入 secret store，而不是资产元数据；
- 旧的 `path` 模式资产编辑时仍可正常显示与保存；
- 旧资产导入新私钥并保存后，会迁移到 `content` 模式；
- 导入取消不报错；
- 导入失败会显示 feedback。

## 验收标准

- 新建 SSH 连接时，`SSH Key` 登录交互可直接用于粘贴或导入私钥；
- modal 明确说明“这里只需要私钥，公钥需要已安装在服务器端”；
- 导入文件后，表单中出现私钥内容；
- 保存后私钥明文不写入资产元数据；
- 已保存的 `private_key_source = "path"` 连接不回归；
- 用户可通过重新导入/粘贴私钥把旧连接迁移到 `content` 模式。

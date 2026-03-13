# APT Dependency Inventory Design

日期: 2026-03-13

## 目标

在仓库根目录补一份可读的 apt 依赖清单，以及一份可执行的交互式安装脚本，覆盖：

- 这次 Windows 构建工作里实际安装过的 apt 包
- 当前 Windows 构建链相关的 apt 前置依赖
- 当前项目代码直接使用的 Cargo 依赖概览

## 方案

### 1. 根目录清单文件

新增 `apt-packages.md`，作为单一事实源，分成三个部分：

- `APT Packages Installed During This Windows Build Work`
- `Current APT Prerequisites For The Build Chain`
- `Current Cargo-Managed Project Dependencies`

这样可以明确区分“已经装过的包”和“推荐保留的当前前置依赖”，避免混在一起。

### 2. 根目录安装脚本

新增 `install-apt-packages.sh`，行为固定为：

- 默认只处理 apt 层面的系统依赖
- 先列出待安装包与用途
- 要求用户输入 `y`
- 执行 `apt-get update`
- 执行 `apt-get install -y ...`
- 最后输出每个包的安装状态，并探测关键命令是否可用

脚本不负责 `rustup target add ...`，但会在输出中提醒这些非 apt 前置步骤。

### 3. 验证方式

新增一个 shell smoke test，锁定以下契约：

- 两个根目录文件存在
- 安装脚本可通过 `bash -n`
- `--help` 会列出关键包
- 输入 `n` 时，脚本会先展示包列表，再中止安装
- 清单文件包含三组信息与关键依赖名

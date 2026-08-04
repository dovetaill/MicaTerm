<div align="center">
  <p>
    <a href="readme.md">English</a> |
    <strong>简体中文</strong> |
    <a href="readme.ja.md">日本語</a>
  </p>

  <img src="assets/icons/mica-term-app.svg" width="112" alt="Mica Term 标志">

  <h1>Mica Term</h1>

  <p><strong>专注于终端、远程服务器及其配套工具的桌面工作空间。</strong></p>

  <p>
    <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/Rust-2024-000000?style=flat-square&logo=rust" alt="Rust 2024"></a>
    <a href="https://slint.dev/"><img src="https://img.shields.io/badge/UI-Slint%201.15-2379F4?style=flat-square" alt="Slint 1.15"></a>
    <img src="https://img.shields.io/badge/platform-Windows%20%7C%20Linux%20%7C%20macOS-4F5D75?style=flat-square" alt="Windows、Linux 和 macOS">
    <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-E6B450?style=flat-square" alt="MIT 协议"></a>
  </p>
</div>

Mica Term 将现代终端、SSH 连接、SFTP 文件管理、命令片段、凭据管理和加密配置同步整合到一个原生桌面应用中。它面向需要频繁切换本地 Shell 与远程服务器的用户，提供安静、快速且统一的工作环境。

> [!IMPORTANT]
> Mica Term 正在积极开发中。当前版本为 `0.1.0`，在首个稳定版本发布前，行为、存储格式和界面都可能发生变化。

## 核心功能

| 领域 | Mica Term 提供的能力 |
| --- | --- |
| 终端 | 基于 WezTerm 的终端内核、标签页会话、Unicode 文本塑形、彩色 Emoji、主题、选区、URL 打开、命令块，以及 Windows 原生呈现路径。 |
| SSH | 保存连接、密码与 SSH 密钥认证、known-host 校验、连接过程展示、SOCKS5/HTTP 代理和 SSH 跳板机。 |
| SFTP | 感知终端工作目录的文件浏览器，支持历史导航、目录跟随、上传/下载队列、冲突处理、断点续传和独立工作区。 |
| 工作空间 | 快速启动、会话标签页、重连与克隆、命令面板、多行粘贴安全确认和统一传输中心。 |
| 资产管理 | 可搜索的 SSH 连接、命令片段、片段包、身份和 SSH 密钥树，并使用嵌入式本地数据库持久化。 |
| 安全同步 | 加密 Vault 快照、合并与恢复、冲突收件箱，并支持 Git 仓库、GitHub Gist、Gitee Gist 和兼容 S3 的存储；GitLab Snippet 连接器仍在开发中。 |

## 为什么选择 Mica Term

- **统一的远程工作界面。** 在同一套流程中管理终端会话、远程文件、连接配置、命令片段和凭据。
- **原生桌面性能。** 使用 Rust 和 Slint 构建，并采用平台感知的渲染路径，而不是浏览器运行时。
- **可靠的终端文字呈现。** 覆盖 Unicode 分段、文本塑形、字体回退、自定义网格字符和彩色 Emoji。
- **重视安全的存储。** 密钥接入操作系统钥匙串；同步 Vault 使用 Argon2 派生密钥并通过 ChaCha20-Poly1305 加密。
- **按需便携。** Windows 打包版本可通过 `.mica-term-portable` 标记，将数据与日志保存到可执行文件旁。

## 技术栈

| 层级 | 技术 |
| --- | --- |
| 编程语言 | Rust 2024 edition |
| 桌面 UI | Slint 1.15、Winit、软件与 Skia 渲染器 |
| 终端 | WezTerm terminal/surface forks、Termwiz |
| 文本与图形 | RustyBuzz、Swash、FontDB、Windows DirectWrite |
| 异步运行时 | Tokio |
| 远程访问 | Russh、Russh SFTP |
| 本地持久化 | Redb、Bincode、Serde |
| 密钥与加密 | 操作系统钥匙串、Argon2、ChaCha20-Poly1305、Zeroize |
| 同步与网络 | Git2、Gix、Reqwest、OAuth 2.0、AWS SDK for S3 |
| 诊断 | Tracing、滚动日志和崩溃记录 |

## 架构概览

```text
Slint 视图
    |
Shell 与 View Model 投影
    |
应用服务
    |-- 终端内核与渲染器
    |-- SSH 会话与连接路由
    |-- SFTP 浏览器与传输引擎
    |-- 资产、钥匙串与本地持久化
    `-- 加密 Vault 与远端 Provider
```

UI 保持声明式，Rust 负责状态、校验、异步任务、持久化和平台集成，从而将界面渲染与终端、远程会话行为清晰分离。

## 快速开始

### 环境要求

- 当前稳定版 Rust 工具链与 Cargo
- 对应目标平台的构建工具
- Debian/Ubuntu 上需要 `pkg-config` 和 `libwayland-dev`

仓库当前的 Linux 构建依赖与交叉编译说明见 [apt-packages.md](apt-packages.md)。

### 从源码运行

```bash
git clone https://github.com/dovetaill/MicaTerm.git
cd MicaTerm
cargo run
```

### 运行测试

```bash
cargo test
```

### 构建发行包

```bash
# Linux x64
./build-desktop.sh

# Windows x64 MSVC + Skia
./build-win-x64.sh

# Linux x64 与 Windows GNU/software 汇总构建
./build-release.sh
```

`build-desktop.sh` 还可通过 `TARGET` 环境变量构建 Linux ARM64、macOS Intel/Apple Silicon 以及 Windows GNU/MSVC 目标。执行 `./build-desktop.sh --help` 可查看完整矩阵。

## 项目结构

```text
src/app/        终端、SSH、SFTP、Vault、持久化与平台服务
src/shell/      工作区状态、投影、导航与 UI 编排
src/theme/      运行时主题定义
ui/             Slint 视图、组件、Shell 与设计 Token
assets/         品牌资源、应用图标与内置字体
tests/          行为、回归、集成与 UI 契约测试
scripts/        开发、验证与资源工具
```

## 参与贡献

欢迎提交 Issue 和 Pull Request。请保持改动聚焦，根据行为风险补充相应测试，并在提交前运行相关 Rust 测试与 Shell 契约测试。

## 开源协议

Mica Term 基于 [MIT License](LICENSE) 发布。

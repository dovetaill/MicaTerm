# Mica Term (GPUI Edition)

基于 **Rust 2024 + Zed GPUI (`longbridge/gpui-kit`) + `alacritty_terminal`** 全新重构的轻量、GPU 加速桌面终端、SSH 与 SFTP 一体化工作空间。

> 老版本（Slint + WezTerm fork）完整保留在同级归档目录 `../mica-term` 中，互不干扰。

---

## 重构亮点与架构演进

1. **渲染引擎全面升级为 GPUI + GPUI Kit (`0.10`)**
   - 彻底移除 Slint 与自维护软件/Skia 双栈，全面采用 Zed 同款 GPU 硬件加速框架 GPUI 与 Longbridge 的桌面组件体系 `gpui-kit`。
2. **终端内核切换为 `alacritty_terminal` + 自研 GPUI GPU 终端组件**
   - 移除旧版数千行 `vendor/tattoy-wezterm-term` 魔改依赖；
   - 基于 `alacritty_terminal` (`0.26`) 处理 ANSI/VT 状态机、回滚缓存、选区与正则搜索；
   - 自研 GPUI `canvas` + `TextSystem::shape_line(..., Some(cell_width))` 终端网格渲染器，严格等宽对齐 ASCII/CJK/符号，支持 16/256/TrueColor、粗斜体、下划线、多种光标形态、鼠标协议穿透（`htop` / `vim` / `tmux`）与平滑滚轮回滚。
3. **去芜存菁的极简界面布局**
   - **一体化顶栏**：去掉旧版左上角意义不明的汉堡菜单与右上角拥挤的 6 颗图标按钮；左侧仅保留侧栏折叠按钮，中间融合多会话标签栏、新建会话（本地 Shell / SSH）与水平/垂直分屏按钮，右上角仅保留 `SFTP` 开关与 `设置` 按钮。
   - **单层高密度左侧栏**：取消旧版双列（44px 图标条 + 320px 面板）割裂设计，顶部直接切换 `主机 / 片段 / 密钥`，内置实时搜索与右键菜单。
   - **右侧联动 SFTP 面板**：跟随当前激活 SSH 会话目录（支持 OSC 7 自动目录跟随），支持目录浏览、上传、下载、新建文件夹、重命名、删除、UTF-8 文本文件在线编辑，并在面板底部内嵌安静的轻量传输状态条（无任务时自动隐藏）。
   - **大幅精简依赖与体积**：砍掉旧版重度云同步 Vault（S3 / Git / Gist / OAuth）、ZModem 弹窗与双传输中心等低频臃肿模块。

---

## 核心功能

- **多标签与分屏终端工作区**：支持本机默认 Shell (`portable-pty`) 与远程 SSH (`russh 0.65`) 混排标签页、水平/垂直分屏、断线重连、会话克隆。
- **SSH 连接与安全校验**：支持密码、本机私钥文件、内联粘贴私钥、密钥库复用凭据；支持直连、SOCKS5 代理、HTTP CONNECT 代理、多级 SSH 跳板机转发；内置 `known_hosts` SHA-256 指纹首次信任交互与防篡改拦截。
- **SFTP 远程文件与在线编辑**：会话复用零额外握手，支持路径直达、OSC 7 目录跟随、文件上传/下载、在线文本编辑与底部内嵌进度显示。
- **快捷命令片段与凭据管理**：本地嵌入式数据库 `redb` 持久化，敏感凭据优先写入系统钥匙串（`keyring`）并提供 `XChaCha20-Poly1305` 加密本地回退存储；支持 `.mica-term-portable` 便携模式。

---

## 常用快捷键（终端焦点下）

| 快捷键 | 功能 |
| --- | --- |
| `Ctrl + Shift + C` (macOS `Cmd + C`) | 复制终端选区文本 |
| `Ctrl + Shift + V` (macOS `Cmd + V`) | 粘贴剪贴板内容（自动适配 Bracketed Paste） |
| `Ctrl + Shift + F` (macOS `Cmd + F`) | 打开终端内搜索栏 |
| `Ctrl + =` / `Ctrl + -` / `Ctrl + 0` | 放大 / 缩小 / 重置终端字号 |
| `Ctrl + 鼠标滚轮` | 缩放终端字号 |
| `Shift + PageUp / PageDown` | 翻阅终端历史回滚缓存 |
| `Shift + 鼠标拖拽` | 在开启鼠标捕获的 TUI 程序（如 `vim`/`tmux`）中强制本地选词 |

---

## 构建与运行

```bash
cd /home/wwwroot/mica-term-gpui

# 运行单元测试
cargo test

# 启动桌面应用
cargo run --release
```

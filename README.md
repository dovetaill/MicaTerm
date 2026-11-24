# Mica Term (GPUI Edition)

基于 **Rust 2024 + Zed GPUI (`longbridge/gpui-kit`) + `alacritty_terminal`** 全新重构的轻量、GPU 加速桌面终端、SSH 与 SFTP 一体化工作空间。

> 老版本（Slint + WezTerm fork）完整保留在同级归档目录 `../mica-term` 中，互不干扰。

---

## 重构亮点与架构演进

1. **渲染引擎全面升级为 GPUI + GPUI Kit (`0.10`)**
   - 彻底移除 Slint 与自维护软件/Skia 双栈，全面采用 Zed 同款 GPU 硬件加速框架 GPUI 与 Longbridge 的桌面组件体系 `gpui-kit`。
2. **内置 `Sarasa Term SC Nerd` 严格 2:1 中英等宽字体 + 自研 GPUI GPU 终端组件**
   - 开箱内置更纱黑体 `Sarasa Term SC Nerd`（Regular + SemiBold），完整覆盖简体中文与 Nerd Fonts 图标，严格保证 1 个汉字 = 2 个英文字符像素级对齐（自定义西文字体时亦自动回退至更纱黑体）；
   - 原生接入 GPUI `EntityInputHandler` 系统输入法协议，支持中文 IME 组词内联预编辑与光标跟随候选框；
   - 支持 **OSC 133 命令块分割装饰**（左侧运行/成功/失败状态色条与命令块跳转）、**输出高亮规则**（`file:line` 代码引用、路径、`Ctrl+点击` 打开 URL、诊断等级）与 **滚动条全局概览标记**。
3. **去芜存菁的极简界面布局**
   - **一体化顶栏**：左侧保留侧栏折叠按钮，中间融合多会话滚动标签栏、新建会话菜单与 2D 混合分屏按钮（支持独立关闭单个分屏），右上角仅保留 `SFTP` 开关与 `设置` 按钮。
   - **单层高密度左侧栏**：顶部直接切换 `主机 / 片段 / 密钥`，内置实时搜索、右键菜单与删除二次确认。
   - **右侧联动 SFTP 面板**：复用当前 SSH 会话并缓存 SFTP 子通道，自动跟随终端目录（兼容 `OSC 7` / `OSC 1337` / 标准 Linux `user@host:path` 窗口标题），支持目录软链接跳转、UTF-8 在线文本编辑与 `64 KiB` 流式分块上传/下载实时进度条。

---

## 常用快捷键（终端焦点下）

| 快捷键 | 功能 |
| --- | --- |
| `Ctrl + Shift + C` (macOS `Cmd + C`) | 复制终端选区文本 |
| `Ctrl + Shift + V` (macOS `Cmd + V`) | 粘贴剪贴板内容（自动适配 Bracketed Paste） |
| `Ctrl + Shift + F` (macOS `Cmd + F`) | 打开终端内搜索栏（不区分大小写，高亮当前跳转项） |
| `Ctrl + Shift + ↑ / ↓` (macOS `Cmd + ↑ / ↓`) | 在上一条 / 下一条历史命令块之间快速跳转 |
| `Ctrl + Shift + W` (macOS `Cmd + W`) | 关闭当前激活的分屏面板（单屏时关闭标签页） |
| `Ctrl + 鼠标左键` (macOS `Cmd + 左键`) | 在浏览器中打开终端内高亮的 URL 链接 |
| `Ctrl + =` / `Ctrl + -` / `Ctrl + 0` | 放大 / 缩小 / 重置为默认终端字号 |
| `Ctrl + 鼠标滚轮` | 缩放终端字号 |
| `Shift + PageUp / PageDown` | 翻阅终端历史回滚缓存 |
| `Shift + 鼠标拖拽` | 在开启鼠标捕获的 TUI 程序（如 `vim`/`tmux`）中强制本地选词 |

---

## 构建与运行

```bash
cd /home/wwwroot/mica-term-gpui

# 运行单元测试与静态检查
cargo test
cargo clippy --all-targets -- -D warnings

# 启动桌面应用
cargo run --release

# 交叉编译 Windows 64 位便携版压缩包并发布至 /var/www/html
./build-win-64.sh
```

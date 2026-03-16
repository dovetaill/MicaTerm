# Mica Term Top Status Bar Corner Background Design

日期: 2026-03-16
执行者: Codex

## 背景

当前窗口截图暴露的问题不是“窗口没有圆角”，而是顶部状态栏自己先画了一层 rounded 内层，而它外面还露出一层方角背景或容器，导致左上角和右上角出现明显的“里面圆、外面方”。

基于当前分支源码与提交历史，现状如下：

- [ui/app-window.slint](/home/wwwroot/mica-term/ui/app-window.slint#L84) 的 `shell-frame` 持有整窗背景、描边和恢复态 `14px` 外轮廓圆角。
- [ui/shell/titlebar.slint](/home/wwwroot/mica-term/ui/shell/titlebar.slint#L57) 的 `Titlebar` 又额外持有 `12px` 圆角和完整描边。
- [ui/shell/right-panel.slint](/home/wwwroot/mica-term/ui/shell/right-panel.slint#L7) 当前仍然是 `14px` 圆角 card 语义。
- `HEAD` 对应提交 `ce10e63`，它回退了 2026-03-13 当天一版“flat internal chrome”尝试，说明这个问题此前已经被识别并尝试过结构性收敛。

同时，当前主工作区仍是 [ui/welcome/welcome-view.slint](/home/wwwroot/mica-term/ui/welcome/welcome-view.slint#L3) 的占位界面，尚未接入真实 terminal surface。这意味着本轮的核心是 shell chrome 几何问题，而不是 terminal emulation 或字符栅格渲染问题。

## 目标

- 消除顶部状态栏左右上角“里面圆、外面方”的割裂观感。
- 明确窗口外轮廓、顶部状态栏、右侧面板三者的几何 ownership。
- 将整体壳层收敛为 `flat internal chrome`，对齐成熟终端与工具型桌面应用的结构语义。
- 保留现有窗口级规则：
  - `Restored => Rounded`
  - `Maximized / Snapped => Flat`
- 为后续接入真实 terminal viewport、tab strip、pane splitter 保留一致的跨平台壳层基础。

## 边界

### 本文档覆盖

- `shell-frame` / `Titlebar` / `RightPanel` 的圆角、背景、描边和裁剪职责
- 顶部状态栏的产品语义
- Windows 11 首发外观与行业终端风格取向
- 风险、回滚与验证方式

### 本文档不覆盖

- `wezterm-term` / `termwiz` 接入
- SSH / SFTP 业务逻辑
- 新增导航结构、全新交互组件
- 大规模主题 token 重构
- 逐文件实现细节与提交步骤

## 调研结论

### 源码与提交历史

- [ui/app-window.slint](/home/wwwroot/mica-term/ui/app-window.slint#L84) 当前定义了外层 `shell-frame`。
- [ui/shell/titlebar.slint](/home/wwwroot/mica-term/ui/shell/titlebar.slint#L57) 当前仍把 `Titlebar` 作为 rounded card 绘制。
- [tests/window_geometry_spec.rs](/home/wwwroot/mica-term/tests/window_geometry_spec.rs#L105) 当前测试契约明确要求恢复态下 `shell-frame = 14`、`titlebar = 12`。
- `git log` 显示最近相关链路为：
  - `4f9fc9e feat: flatten shell chrome — Titlebar/RightPanel 收敛为 flat internal chrome`
  - `ce10e63 Revert "Merge branch 'feat/flatten-titlebar-right-panel-chrome'"`

### 平台与行业判断

- Windows 11 官方几何语义更强调 top-level app window 的圆角，而不是每个贴边 pane 都有独立完整圆角。
- 最大化、贴靠等状态下，窗口应切换为 flat geometry。
- 成熟终端产品通常是“窗口外轮廓跟随 OS 几何，内部 titlebar/tabbar/viewport 多为直边 docked 结构”。
- Slint 的 `border-radius` 可以使用，但当前 `femtovg` 路线对非矩形 clip 需要额外 layer 路径，结构性 flat 方案比依赖 mask/clip 的补丁方案更稳。

参考：

- https://learn.microsoft.com/en-us/windows/apps/desktop/modernize/ui/apply-rounded-corners
- https://learn.microsoft.com/en-us/windows/apps/design/signature-experiences/geometry
- https://learn.microsoft.com/en-us/windows/apps/develop/title-bar?tabs=wasdk
- https://docs.slint.dev/latest/docs/slint/reference/elements/rectangle

## 方案对比

### 设计点 1：顶部角 ownership

#### 方案 1A：只让外层 `shell-frame/window` 持有圆角

说明：

- `Titlebar` 不再是 rounded card，而是贴在窗口内部的 flat 顶栏。
- 恢复态的圆角只属于窗口外轮廓。

优点：

- 最符合 Windows 11 与主流终端壳层语义
- 能根治“里面圆、外面方”
- 后续接入 terminal viewport 和 tab strip 更自然

缺点：

- 个性化程度低于 card 化 header

#### 方案 1B：继续保留 rounded `Titlebar`

优点：

- 视觉更柔和，更有品牌化 header 感

缺点：

- 天然更容易重复出现双层几何和双层描边
- 后续壳层会持续复杂化

最终选择：`1A`

### 设计点 2：外层方背景的消除方式

#### 方案 2A：新增统一 `chrome-mask` 裁剪容器

说明：

- 在 `shell-frame` 内新增统一裁剪层，让标题栏和内容区共同服从外轮廓。

优点：

- 结构语义最完整
- 适合保留 rounded internal chrome 的路线

缺点：

- 对当前 `femtovg` renderer 更依赖 layer clip 路径
- 成本高于纯 flat 内层

#### 方案 2B：通过背景色融合或透明角遮罩掩盖方角

优点：

- 实现最轻

缺点：

- 本质是视觉补丁，不是结构修复
- 主题、透明度、Mica 强度变化后容易再次露底

#### 方案 2C：不做遮罩补丁，直接转向 `flat internal chrome`

说明：

- 不让内部标题栏继续承担 rounded 语义。
- 不让 `RightPanel` 和其他内部 chrome 承担 rounded card 语义。
- 保留窗口最外层在 `Restored` 状态下顺应 Windows 11 的系统圆角。
- 通过移除内部 rounded card 和重复描边，从结构上消除外层方背景露出问题。

优点：

- 结构最干净
- 性能与渲染成本最稳
- 与 `1A / 3A / 4A` 完全一致

缺点：

- 牺牲独立 rounded header 的柔和感

最终选择：`2C`

### 设计点 3：顶部状态栏产品语义

#### 方案 3A：工具型 flat top bar

说明：

- 顶部状态栏是 terminal / IDE 风格的 flat bar。
- 用 surface 差异、divider、hover feedback 建立层次，而不是用完整圆角卡片表达层级。

优点：

- 最符合终端软件与专业工具观感
- 未来接 terminal 内容区最自然
- 与右侧 pane、tabbar、sidebar 更一致

缺点：

- 视觉语气更克制

#### 方案 3B：品牌化 rounded header card

优点：

- 更显眼，更容易形成产品识别

缺点：

- 与 terminal 工具属性冲突
- 后续 pane 和 tabbar 语义更容易失衡

最终选择：`3A`

### 设计点 4：行业取向与整体几何风格

#### 方案 4A：外层 rounded，内部全部 square/docked

优点：

- 最接近成熟终端与桌面工具软件
- 最适配未来多 pane、split、tab strip 结构
- 最利于跨平台统一

缺点：

- 少一些实验性视觉表现

#### 方案 4B：外层 rounded，顶部栏保留轻微圆润，其余 square

优点：

- 平衡柔和感与工具感

缺点：

- 规则会越来越细碎
- 后续维护容易继续出现例外分支

#### 方案 4C：顶部栏与侧边 pane 都 rounded card 化

优点：

- 风格感最强

缺点：

- 与终端行业观感偏差最大
- 最容易回到当前问题

最终选择：`4A`

## 最终决策

本轮确认方案为：`1A + 2C + 3A + 4A`

具体解释如下：

1. 只让最外层 `shell-frame/window` 在 `Restored` 状态下顺应系统圆角。
2. 不通过遮罩补丁去掩盖方角，而是把内部 `Titlebar / RightPanel / Workspace chrome` 结构性收敛为 `flat internal chrome`。
3. 顶部状态栏按工具型 terminal top bar 处理，不再作为独立 rounded card。
4. 整体几何风格采用“外层 rounded、内部 flat/docked”的行业取向，既不和 Windows 11 打架，也不会让 terminal chrome 显得软塌。

## 实施步骤

1. 重新定义窗口几何 ownership：
   - `shell-frame` 继续作为整窗唯一完整外轮廓 owner
   - `shell-frame` 只在 `Restored` 状态下保留圆角，在 `Maximized / Snapped` 状态下继续切 flat
   - `Titlebar` 不再拥有独立完整圆角与完整描边
   - `RightPanel` 不再拥有完整 rounded card 语义
2. 收敛顶部状态栏：
   - 移除 `Titlebar` 的独立圆角 card 语义
   - 保留 drag zone、window controls、utility actions 的交互区域
   - 如有需要，仅保留局部分隔线而非完整边框
3. 收敛右侧面板：
   - 改为 square/docked inspector pane
   - 与主工作区之间使用单一 divider 或 surface 差异建立边界
4. 收敛测试契约：
   - 恢复态仅验证外层 `shell-frame` 为 rounded
   - 内层 `Titlebar` / `RightPanel` 改为 flat geometry
   - 最大化与贴靠态继续验证窗口级 flat geometry
5. 回归视觉检查：
   - 恢复态左上/右上不再露出方角背景
   - 最大化/贴靠态无额外圆角残留
   - 暗色与亮色模式都不出现边框堆叠

## 风险与回滚

### 主要风险

- 之前曾有 `flat internal chrome` 版本被回退，说明历史上可能存在审美分歧或未覆盖的视觉副作用。
- 如果 divider 与 surface 层次定义不稳，内部全 flat 后可能显得过于“硬”。
- 如果测试只覆盖几何属性，不覆盖实际截图观感，仍可能遗漏视觉回归。

### 风险缓解

- 保留窗口级 `Restored => Rounded / Maximized|Snapped => Flat` 不变，避免一次性改动过大。
- 内部层次优先通过 `command-tint / panel-tint / terminal-surface / shell-stroke` 控制，而不是重新引入 rounded card。
- 补充 geometry contract 与 UI smoke，确保未来不会重新回流到“双层圆角 card”。

### 回滚策略

- 如果 flat internal chrome 在实际真机上观感不可接受，只允许回滚到“外层 rounded + 顶部轻微圆润”这一类受控折中方案。
- 不建议回滚到“外层 rounded + 内层完整 rounded titlebar card + 完整描边”的旧结构，因为那正是当前问题来源。

## 验证清单

- [ ] 恢复态下，窗口只有最外层圆角，顶部栏自身不再形成独立 rounded card
- [ ] 左上角和右上角不再出现“里面圆、外面方”
- [ ] 最大化态下，外层与内层都为 flat geometry
- [ ] Snap 态下，不出现残留 rounded 角
- [ ] 右侧面板与主工作区接缝自然，无 card 咬边感
- [ ] 暗色与亮色主题下都不存在重复描边或露底
- [ ] drag zone、caption buttons、resize band 行为不受影响
- [ ] 相关测试契约更新后，能够防止 rounded internal chrome 回流

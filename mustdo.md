# Windows Native Terminal Surface Must-Do

日期: 2026-04-02
状态: 部分完成；Task 1-7 与 Linux-host Windows GNU 打包已完成，Windows mainline MSVC 打包与真机补验仍未完成。
适用范围: `mica-term` Windows native terminal surface recovery 最终收口。

## 当前结论

- 已完成 present-driver seam、runtime diagnostics、runtime present path selection、Windows font locator/fallback、OpenType/fallback shaping、real color glyph path、damage/lifecycle hardening。
- `cargo check --workspace` 与 `cargo clippy --workspace -- -D warnings` 已于 2026-04-02 通过。
- `./build-win-x64-software.sh` 已于 2026-04-02 通过，并生成 `dist/mica-term-x86_64-pc-windows-gnu-release-software.zip`。
- `./build-win-x64.sh` 在当前 Linux host 失败：`target 'x86_64-pc-windows-msvc' must be built from a Windows MSVC shell or Git Bash environment.`
- Windows 真机 UI 验证尚未执行，因此还不能把本主题标记为“完全完成”。

## 已完成事项

- [x] present trigger 从单点 notifier 依赖拆成 `NativeSurfacePresentDriver` seam
- [x] runtime / backend diagnostics snapshot 已接入 `NativeTerminalSurfaceDiagnostics`
- [x] packaged profile 已区分 `event-loop` 与 `rendering-notifier` 两条 native present path
- [x] Windows system-backed font locate / fallback discovery 已接入 `WindowsFontLocator` 与 `WindowsFontFallbackResolver`
- [x] OpenType features、fallback shaping、resolved face tracking 已落地
- [x] color glyph / emoji 不再走旧 placeholder accent square
- [x] resize / device-loss / detach 生命周期已补 `damage.rs`、`surface_alive`、`attached` guard
- [x] recovery handoff 文档已创建：`docs/plans/2026-04-01-windows-native-terminal-surface-recovery-tdd-spec.md`

## 仍待完成的最后两项

- [ ] 在 Windows MSVC shell 或 Git Bash 运行 `./build-win-x64.sh`，确认 mainline package 可成功构建并保留日志/产物路径。
- [ ] 在 Windows 真机验证以下流：first-paint text、selection、underline、cursor、IME preview、emoji、resize、close、reconnect。

## 真机补验时的优先观察点

- `NativeTerminalSurfaceDiagnostics` 的 `last_prepared_frame_token`、`last_presented_frame_token`、`render_target_generation` 是否持续推进。
- mainline `rendering-notifier` 路径与 software `event-loop` 路径是否存在可见差异。
- `DirectWriteFontSystem` 当前仍是 staged seam；若真机出现字体/emoji 异常，优先检查 `WindowsFontLocator`、fallback chain 与 `TerminalEmojiRenderer`。
- 若出现 close/dispose 后残留回调，优先检查 `surface_alive`、`attached` 与 `detach()` 调用链。

## 参考文档

- `docs/plans/2026-04-01-windows-native-terminal-surface-recovery-tdd-spec.md`
- `docs/plans/2026-04-01-windows-native-terminal-surface-recovery-design.md`
- `docs/plans/2026-04-01-windows-native-terminal-surface-recovery-implementation-plan.md`

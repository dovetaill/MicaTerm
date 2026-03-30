# Terminal Rendering Stack Follow-up Backlog

Date: 2026-03-30

This backlog tracks the post-Windows-MVP follow-up work for Linux/macOS native backends and the
explicit trigger conditions for switching to the `libghostty` stop-loss route.

| Item | Trigger | Owner | Notes |
| --- | --- | --- | --- |
| `linux_freetype_fontconfig.rs` | Windows MVP native path is stable and bitmap fallback regressions stay green | TBD | Reuse the existing HarfBuzz layout layer and keep the presenter contract unchanged |
| `macos_coretext.rs` | Linux backend scope is estimated and Windows text quality sign-off is complete | TBD | Mirror the shared `FontSystem` seam instead of adding a second shaping path |
| Move cursor/selection fully into the renderer | Native renderer draw list covers text backgrounds and cursor invalidation without Slint overlay regressions | TBD | Remove the remaining split ownership between Slint overlays and renderer-owned text state |
| Explicit fallback telemetry for native setup failures | Native presenter setup fails in packaged Windows sessions often enough to hide real diagnostics | TBD | Keep logging tied to `terminal_render_mode` and fallback cause, not just the Slint renderer mode |
| Switching to the `libghostty` stop-loss route | Slint native surface complexity remains high, multi-platform renderer cost exceeds budget, or text quality still misses product targets after the Windows-first pass | TBD | Preserve existing window/tab/sidebar/session lifecycle and replace the terminal pane renderer with `libghostty` only if the stop-loss triggers are met |

## Stop-loss Trigger Checklist

- Native surface integration still blocks stable redraw, resize, or input mapping after the staged renderer pass
- Linux/macOS native backend delivery would require another architecture rewrite instead of plugging into the existing presenter/model/layout seams
- Windows-first native renderer still fails to reach the expected terminal text quality after reasonable DirectWrite iteration

If any of the above stays true after the current staged renderer roadmap, switch planning to the
`libghostty` stop-loss route and freeze further custom renderer expansion until the stop-loss
decision is made.

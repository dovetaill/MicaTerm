# Terminal Rendering Stack TDD Spec

Date: 2026-03-30

Source documents:

- Design: `docs/plans/2026-03-30-terminal-rendering-stack-design.md`
- Implementation plan: `docs/plans/2026-03-30-terminal-rendering-stack-implementation-plan.md`
- Follow-up backlog: `docs/plans/2026-03-30-terminal-rendering-stack-follow-up.md`

## Core Structs

- `PresentedTerminalFrame`
  - Presenter output contract.
  - `Bitmap(BitmapTerminalFrame)` keeps the existing atlas-backed `slint::Image` path.
  - `Native(NativeTerminalFrame)` carries `frame_token`, `cell_width_px`, and `cell_height_px`.
- `TerminalModelFrame`
  - Pure renderer-facing snapshot projected from `TerminalSurfaceState`.
  - Owns rows, cells, cursor, palette, selection metadata, and `dirty_rows`.
- `BitmapAtlasPresenter`
  - Migration fallback presenter.
  - Projects `TerminalModelFrame` back into the legacy atlas renderer path.
- `WindowsNativePresenter`
  - Windows-first native presenter.
  - Owns `DirectWriteFontSystem`, `HarfBuzzTextShaper`, `WgpuTerminalRenderer`, and the last `TerminalModelFrame`.
- `DirectWriteFontSystem`
  - Current Windows-native font backend seam.
  - Exposes face resolution, metrics, face bytes, and staged glyph rasterization.
- `GlyphRasterRequest` / `RasterizedGlyph`
  - Shared request/result pair for native glyph staging.
- `GlyphAtlas`
  - Tracks rasterized glyph cache entries keyed by face, glyph id, and px size.
- `ShapedTerminalFrame` / `PreparedNativeFrame`
  - `ShapedTerminalFrame` groups shaped rows plus face and metrics metadata.
  - `PreparedNativeFrame` is the renderer-ready native frame summary, including `glyph_cache_entries` and stable `frame_token`.
- `WgpuTerminalRenderer`
  - Current native renderer preparation stage.
  - Converts shaped rows into atlas-backed prepared frames and advances `frame_token` only when the shaped frame fingerprint changes.
- `NativeTerminalSurface` / `NativeTerminalSurfaceRect`
  - Slint rendering-notifier host for the native terminal region.
  - Tracks current rect and latest native frame token, then requests redraw through the Slint window.
- `AppRuntimeProfile` / `TerminalRenderMode`
  - Runtime renderer selection contract.
  - `TerminalRenderMode::{Bitmap, Native}` separates terminal rendering mode from Slint renderer mode.
- `WorkspaceFollowTracker`
  - Preserves local scrollback-follow state while runtime surfaces continue to refresh.

## Traits And Interface Contracts

- `TerminalPresenter`
  - `present(&mut self, surface: &TerminalSurfaceState, options: TerminalPresentationOptions) -> Result<PresentedTerminalFrame>`
  - `default_cell_size(&self) -> (u32, u32)`
- `FontSystem`
  - `resolve_face(&mut self, request: &FontRequest) -> Result<FontFaceKey>`
  - `metrics(&mut self, face: FontFaceKey, px_size: f32) -> Result<FontMetrics>`
  - `face_bytes(&self, face: FontFaceKey) -> Result<&[u8]>`
  - `face_index(&self, face: FontFaceKey) -> u32`
- `TextShaper`
  - `shape_row(&mut self, row: &TerminalModelRow, fonts: &mut dyn FontSystem) -> Result<ShapedRow>`

Contract rules:

- Bootstrap consumes only `PresentedTerminalFrame`, not concrete renderer types.
- `TerminalModelFrame` remains free of Slint, platform, and GPU types.
- Native frame tokens are renderer-owned. Bootstrap must not synthesize them from `TerminalSurfaceState::seqno`.
- Bitmap fallback remains valid on all platforms and after native setup failure.

## Slint Callbacks, Global State, And Bindings

Key app/window properties:

- `workspace-session-render-mode`
- `workspace-session-native-frame-token`
- `layout-workspace-session-native-surface-x`
- `layout-workspace-session-native-surface-y`
- `layout-workspace-session-native-surface-width`
- `layout-workspace-session-native-surface-height`

Binding chain:

- `AppWindow` owns workspace session render mode, native frame token, and native surface layout outputs.
- `WorkspacePane` forwards those values to `TerminalSessionHost`.
- `TerminalSessionHost` keeps `session-surface-image` for bitmap fallback and toggles its `Image` visibility with `session-render-mode`.

Terminal-related callbacks owned by `AppWindow`:

- `workspace-session-text-input`
- `workspace-session-key-input`
- `workspace-session-resize-requested`
- `workspace-session-copy-selection-requested`
- `workspace-session-selection-changed`
- `workspace-session-paste-requested`
- `workspace-session-local-action-requested`
- `workspace-session-scroll-requested`
- `workspace-session-scroll-thumb-drag-requested`
- `workspace-session-scroll-jump-requested`
- `workspace-session-jump-to-latest-requested`
- `workspace-session-mouse-input`

Current ownership split:

- Slint still owns text input focus, cursor overlay, selection overlay, scrollbar, and context-menu interaction.
- Native renderer currently owns only frame preparation and native frame invalidation hooks.

## Tokio Tasks, Channels, And Actor Interactions

- `AppAsyncRuntime` provides the Tokio runtime used by SSH/session actors.
- `SessionManager` owns session lifecycle and is wrapped by `ShellSessionBridge` on the UI side.
- Session runtimes emit `SessionRuntimeEvent` through `tokio::sync::mpsc::UnboundedSender<SessionRuntimeEvent>`.
- `sync_workspace_projection_from_manager()` pulls runtime state into `ShellViewModel`.
- A Slint `Timer` (`session_projection_timer`) runs every 50ms to:
  - pull the latest session projection from `SessionManager`
  - update follow state
  - refresh workspace session UI state
- `NativeTerminalSurface` keeps a weak `AppWindow` reference and requests redraw through Slint instead of performing cross-thread UI mutation.

Actor boundary summary:

- Tokio runtime side:
  - SSH connection, PTY IO, remote bytes, `TerminalSession`, `SessionRuntimeEvent`
- UI thread side:
  - `ShellViewModel`, presenter selection, Slint property sync, native surface rect/frame-token sync

## State Flow

1. Remote PTY output updates `TerminalSession`.
2. Session runtime emits `SessionRuntimeEvent::SurfaceChanged`.
3. `SessionManager` stores the latest `TerminalSurfaceState`.
4. UI projection timer calls `sync_workspace_projection_from_manager()`.
5. `ShellViewModel` updates active workspace surface, visible lines, and follow metadata.
6. `sync_workspace_session_state()`:
   - reconciles visible line models
   - updates cell metrics
   - routes the surface through `TerminalPresenter`
7. Presenter path:
   - Bitmap path: `TerminalModelFrame -> BitmapAtlasPresenter -> slint::Image`
   - Native path: `TerminalModelFrame -> HarfBuzzTextShaper -> DirectWriteFontSystem -> WgpuTerminalRenderer -> NativeTerminalFrame`
8. Bootstrap publishes either:
   - `workspace-session-surface-image` for bitmap mode
   - `workspace-session-native-frame-token` plus render mode for native mode
9. `NativeTerminalSurface` receives rect updates from Slint layout outputs and frame token updates from bootstrap, then requests redraw.

## Key Error Handling Strategies

- Presenter installation:
  - `install_workspace_terminal_presenter()` tries the requested presenter for the runtime profile.
  - If native presenter setup fails, bootstrap logs `app.terminal` and falls back to `BitmapAtlasPresenter`.
- Native surface attachment:
  - `NativeTerminalSurface::attach_or_detach()` warns and stays detached if the Slint backend does not support rendering notifiers.
- Presentation failures:
  - Bootstrap logs the failing session id and clears native frame state instead of leaving stale renderer data active.
- Runtime diagnostics:
  - `emit_runtime_profile_metadata()` logs both Slint `renderer_mode` and terminal `terminal_render_mode`.

## Edge Cases

- Tokio channel blocking or message buildup
  - `SessionRuntimeEvent` currently uses `UnboundedSender`; a burst of remote updates can queue faster than the 50ms UI projection timer drains them.
  - Future work should add queue pressure metrics or bounded coalescing for high-output sessions.
- UI thread update timing
  - Native surface rect and frame token must be updated before redraw requests, otherwise the renderer hook can repaint with stale geometry.
- Data races or shared-state inconsistency
  - Presenters and native surface state are UI-thread-owned (`thread_local!` + `Rc<RefCell<...>>`).
  - Runtime/session state crosses threads only through manager APIs and Tokio channels.
- Resource release ordering
  - `NativeTerminalSurface` resets frame state on rendering teardown and uses a weak window reference so redraw calls do not keep dead windows alive.
- Async task cancellation or window close after callbacks were scheduled
  - The projection timer and native surface both upgrade weak handles before touching the window; dropped windows short-circuit cleanly.
- Slint model updates diverging from the actual terminal data source
  - Visible lines use `VecModel` reconciliation to avoid unnecessary model replacement.
  - Render mode, native frame token, and rect outputs must stay in sync with the active presenter path to avoid blank or stale frames.

## Suggested Next Tests

Unit tests:

- `WgpuTerminalRenderer` should keep `frame_token` stable when the shaped frame fingerprint does not change.
- `GlyphAtlas` should reuse cache entries across repeated glyph rasterization.
- `DirectWriteFontSystem::rasterize()` should cover zero-coverage glyphs and non-zero glyph coverage separately.
- `TerminalModelFrame::from_surface()` should keep dirty-row detection stable when selection changes without text changes.

Integration tests:

- Runtime profile fallback should force bitmap mode on non-Windows even if `mainline_native()` is requested.
- Bootstrap should preserve copy/paste, selection, scrollback, and resize after native presenter fallback.
- Native presenter setup failure should log fallback diagnostics and still refresh terminal output through bitmap mode.

UI interaction tests:

- Windows 11 manual verification for cursor, selection, wheel, resize, and redraw timing in native mode.
- Native-surface unsupported backend smoke test to ensure the terminal still renders through bitmap fallback.
- Follow-mode regression test when remote output arrives while the user is scrolled away from the bottom.

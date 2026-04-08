//! Source-level guards for terminal runtime performance-sensitive paths.

use std::fs;

fn block_between<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
    let start_index = source.find(start).expect("start marker");
    let rest = &source[start_index..];
    let end_index = rest.find(end).expect("end marker");
    &rest[..end_index]
}

#[test]
fn runtime_visible_projection_limits_iteration_to_visible_phys_range() {
    let adapter_source = fs::read_to_string("src/app/terminal_core/wezterm_adapter.rs")
        .expect("read wezterm adapter source");
    let visible_rows_block = block_between(
        &adapter_source,
        "    pub fn visible_rows(&self) -> Vec<TerminalRowState> {",
        "    pub fn visible_lines(&self) -> Vec<String> {",
    );
    let visible_cells_block = block_between(
        &adapter_source,
        "    fn visible_cells(&self, palette: &ColorPalette) -> Vec<TerminalCellState> {",
        "    fn cursor_state(&self, palette: &ColorPalette) -> TerminalCursorState {",
    );

    assert!(
        visible_rows_block.contains("lines_in_phys_range(visible_start..visible_end"),
        "visible row projection should iterate only the currently visible phys range so large scrollback histories do not get scanned on every local scroll update"
    );
    assert!(
        !visible_rows_block.contains("for_each_phys_line"),
        "visible row projection should not walk the full scrollback when only the visible viewport needs to be projected"
    );
    assert!(
        visible_cells_block.contains("lines_in_phys_range(visible_start..visible_end"),
        "visible cell projection should iterate only the visible phys range so bitmap/native presenters do not rebuild from a full scrollback scan during scrollbar drags"
    );
    assert!(
        !visible_cells_block.contains("for_each_phys_line"),
        "visible cell projection should not walk the full scrollback when projecting the current viewport"
    );
    assert!(
        adapter_source.contains(
            "let visible_end = visible_start.saturating_add(visible_rows).min(total_rows);"
        ) && adapter_source
            .contains("let visible_start = visible_end.saturating_sub(visible_rows);"),
        "visible phys bounds should clamp against the live screen length so wrapped scrollback layouts cannot feed out-of-range phys indexes into the viewport projection path"
    );
}

#[test]
fn runtime_dirty_notifications_expose_a_fast_input_active_flush_contract() {
    let runtime_source = fs::read_to_string("src/app/ssh/runtime.rs").expect("read runtime source");
    let pump_source =
        fs::read_to_string("src/app/ssh/runtime/pump.rs").expect("read runtime pump source");

    assert!(
        runtime_source.contains(
            "const FAST_SURFACE_DIRTY_NOTIFICATION_INTERVAL: Duration = Duration::from_millis("
        ),
        "runtime should expose a dedicated fast dirty-notification cadence so local key repeat does not wait for the conservative idle 40ms output flush budget"
    );
    assert!(
        pump_source.contains("note_local_input("),
        "runtime pump should record recent local input activity so echoed output can request the fast dirty-notification cadence"
    );
    assert!(
        pump_source.contains("preferred_interval("),
        "surface dirty coalescing should choose between idle and input-active flush intervals instead of hard-coding one timer budget for every workload"
    );
}

#[test]
fn bootstrap_exposes_a_coalesced_fast_input_projection_refresh_path() {
    let bootstrap_source = fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap");
    let workspace_terminal_source = fs::read_to_string("src/app/bootstrap/workspace_terminal.rs")
        .expect("read workspace terminal source");

    assert!(
        bootstrap_source.contains("input_projection_refresh_timer")
            && bootstrap_source.contains("input_projection_refresh_gate"),
        "bootstrap should keep a dedicated timer and gate for local-input projection refreshes so repeated key input can repaint faster than the idle 50ms polling loop without forcing a full synchronous refresh on every keystroke"
    );
    assert!(
        workspace_terminal_source.contains("schedule_workspace_input_projection_refresh("),
        "workspace terminal helpers should centralize the coalesced fast input-refresh scheduler instead of inlining ad-hoc immediate projection polls in each key/text callback"
    );
    assert!(
        workspace_terminal_source.contains("refresh_active_workspace_surface_projection("),
        "the fast input-refresh path should use a surface-only projection helper so held-key repaint stays off the heavier full tab/SFTP projection walk"
    );
    assert!(
        !workspace_terminal_source.contains(
            "schedule_workspace_input_projection_refresh(\n    window: &AppWindow,\n    state: Rc<RefCell<ShellViewModel>>,\n    bridge: Option<Rc<ShellSessionBridge>>,\n    follow_tracker: Rc<RefCell<WorkspaceFollowTracker>>,\n    timer: Rc<Timer>,\n    gate: Rc<RefCell<DeferredWorkspaceProjectionRefreshGate>>,\n) {\n    {\n        let mut gate = gate.borrow_mut();\n        if !gate.mark_scheduled() {\n            return;\n        }\n    }\n\n    let window_handle = window.as_weak();\n    timer.start(\n        TimerMode::SingleShot,\n        Duration::from_millis(WORKSPACE_INPUT_PROJECTION_DEBOUNCE_MS),\n        move || {\n            gate.borrow_mut().clear();\n            let Some(window) = window_handle.upgrade() else {\n                return;\n            };\n            let mut state = state.borrow_mut();\n            refresh_active_workspace_projection("
        ),
        "the coalesced fast input timer should no longer call the full workspace projection walker because that keeps held-key input tied to the slower global projection workload"
    );
}

#[test]
fn bootstrap_exposes_surface_only_scroll_projection_refresh_paths() {
    let workspace_terminal_source = fs::read_to_string("src/app/bootstrap/workspace_terminal.rs")
        .expect("read workspace terminal source");
    let scroll_refresh_block = block_between(
        &workspace_terminal_source,
        "pub(super) fn schedule_workspace_scroll_projection_refresh(",
        "pub(super) fn schedule_workspace_scroll_thumb_drag_update(",
    );
    let thumb_drag_block = block_between(
        &workspace_terminal_source,
        "pub(super) fn schedule_workspace_scroll_thumb_drag_update(",
        "pub(super) fn forward_active_workspace_text_input(",
    );

    assert!(
        scroll_refresh_block.contains("refresh_active_terminal_scroll_projection_only("),
        "wheel and scroll-jump refresh timers should update only the active terminal render payload so scrollbar motion does not rebuild unrelated workspace/session chrome"
    );
    assert!(
        !scroll_refresh_block.contains("refresh_active_workspace_projection("),
        "wheel and scroll-jump refresh timers should stay off the full workspace projection walker during high-frequency viewport updates"
    );
    assert!(
        thumb_drag_block.contains("refresh_active_terminal_scroll_projection_only("),
        "scroll thumb drag refreshes should update only the active terminal render payload so continuous thumb motion stays on the cheap viewport projection path"
    );
    assert!(
        !thumb_drag_block.contains("refresh_active_workspace_projection("),
        "scroll thumb drag refreshes should not call the full workspace projection walker after each coalesced runtime update"
    );
}

#[test]
fn terminal_scroll_refresh_avoids_full_workspace_projection() {
    let workspace_terminal_source = fs::read_to_string("src/app/bootstrap/workspace_terminal.rs")
        .expect("read workspace terminal source");
    let scroll_refresh_block = block_between(
        &workspace_terminal_source,
        "pub(super) fn schedule_workspace_scroll_projection_refresh(",
        "pub(super) fn schedule_workspace_scroll_thumb_drag_update(",
    );

    assert!(
        workspace_terminal_source
            .contains("pub(super) fn refresh_active_terminal_scroll_projection_only("),
        "terminal scroll refreshes should target a dedicated render-only helper so viewport updates can skip unrelated session-chrome syncing"
    );
    assert!(
        !scroll_refresh_block.contains("refresh_active_terminal_surface_only("),
        "terminal scroll refreshes should stop calling the broader surface helper once a tighter render-only path exists"
    );
    assert!(
        workspace_terminal_source
            .contains("super::sync_workspace_terminal_surface_projection_only("),
        "the dedicated scroll helper should hand off directly to a render-only bootstrap sync routine instead of rebuilding visible-line models and session chrome"
    );
    assert!(
        !workspace_terminal_source.contains(
            "pub(super) fn refresh_active_terminal_scroll_projection_only(\n    window: &AppWindow,\n    state: &mut ShellViewModel,\n    bridge: Option<&ShellSessionBridge>,\n    follow_tracker: &mut WorkspaceFollowTracker,\n) {\n    let Some(bridge) = bridge else {\n        return;\n    };\n    if sync_active_workspace_surface_projection_from_manager(state, &bridge.manager) {\n        sync_workspace_session_state_with_manager("
        ),
        "the dedicated scroll helper should not fall back to the full workspace session sync because that reprojects visible lines, chrome tokens, and connection state on every wheel notch"
    );
}

#[test]
fn renderer_hot_paths_consume_terminal_frame_snapshot_contract() {
    let presenter_source = fs::read_to_string("src/app/terminal_presenter.rs")
        .expect("read terminal presenter source");

    assert!(
        presenter_source.contains("TerminalFrameSnapshot"),
        "renderer-facing terminal hot paths should consume a compact TerminalFrameSnapshot contract so glyph prep and present scheduling no longer depend on the heavier surface projection model"
    );
    assert!(
        !presenter_source.contains("TerminalSurfaceState"),
        "renderer-facing hot paths should stop depending directly on TerminalSurfaceState once the compact frame snapshot seam exists"
    );
}

#[test]
fn presenter_and_scene_image_paths_keep_perf_diagnostics_without_runtime_log_spam() {
    let presenter_source = fs::read_to_string("src/app/terminal_presenter.rs")
        .expect("read terminal presenter source");
    let scene_image_source = fs::read_to_string("src/app/terminal_scene_image.rs")
        .expect("read scene image source");

    for expected in [
        "struct TerminalPrepareDiagnostics",
        "prepare_native_terminal_frame_with_diagnostics(",
        "prepare_native_terminal_frame_us",
        "shape_rows_us",
        "renderer_prepare_us",
        "reused_shaped_row_count",
        "prepared_row_reuse_count",
        "glyph_raster_cache_entry_count",
    ] {
        assert!(
            presenter_source.contains(expected),
            "presenter perf instrumentation should expose `{expected}` so prepare-path regressions remain observable without forcing the release build to emit per-frame runtime log spam"
        );
    }

    assert!(
        !presenter_source.contains("app.terminal.perf"),
        "presenter source should stop emitting per-frame app.terminal.perf logs once the scroll tuning is considered good enough for release builds"
    );

    for expected in [
        "pub struct SceneImageRenderDiagnostics",
        "pub fn last_render_diagnostics(&self)",
        "incremental_scroll_render_count",
        "dirty_edge_row_raster_count",
        "base_raster_us",
        "overlay_compose_us",
        "fallback_reason",
        "detect_incremental_scroll_delta(",
        "render_incremental_base_pixels(",
    ] {
        assert!(
            scene_image_source.contains(expected),
            "scene-image renderer should expose `{expected}` so incremental scroll reuse and raster timings stay observable even after runtime perf logging is disabled by default"
        );
    }
}

#[test]
fn presenter_sources_expose_scrollback_row_shape_reuse_contract() {
    let presenter_source = fs::read_to_string("src/app/terminal_presenter.rs")
        .expect("read terminal presenter source");
    let model_source =
        fs::read_to_string("src/app/terminal_model.rs").expect("read terminal model source");
    let font_backend_source =
        fs::read_to_string("src/app/terminal_font/backend.rs").expect("read font backend source");

    assert!(
        presenter_source.contains("previous_shaped_rows"),
        "presenter hot paths should retain the previous frame's shaped rows so adjacent viewport scrolls can reuse overlapping row shaping work instead of re-shaping every visible row"
    );
    assert!(
        presenter_source.contains("PresenterShapedRowCache")
            && presenter_source.contains("LoadedFontKey")
            && presenter_source.contains("content_hash"),
        "presenter hot paths should retain a bounded shaped-row cache keyed by font identity plus row content hash so large scroll jumps and back-scrolls can reuse shaping work even after rows leave the previous viewport"
    );
    assert!(
        presenter_source.contains("content_hash"),
        "presenter hot paths should key scroll reuse off a viewport-stable row content hash rather than the transient viewport row index"
    );
    assert!(
        model_source.contains("pub content_hash: u64"),
        "terminal model rows should expose a viewport-stable content hash so presenters can reuse overlapping scrollback rows even after they move to a new viewport slot"
    );
    assert!(
        font_backend_source.contains("pub struct LoadedFontKey"),
        "the font backend should expose a stable loaded-font cache identity so presenter-side shaping caches can safely separate rows shaped under different terminal font metrics"
    );
}

#[test]
fn presenter_and_scene_image_sources_expose_cache_stats_and_clear_hooks() {
    let presenter_source = fs::read_to_string("src/app/terminal_presenter.rs")
        .expect("read terminal presenter source");
    let scene_image_source = fs::read_to_string("src/app/terminal_scene_image.rs")
        .expect("read scene image source");

    for expected in [
        "pub struct TerminalPresenterCacheStats",
        "fn cache_stats(&self) -> TerminalPresenterCacheStats",
        "fn clear_transient_caches(&mut self)",
    ] {
        assert!(
            presenter_source.contains(expected),
            "terminal presenter source should expose `{expected}` so bootstrap can inspect and shrink retained caches when terminal memory diagnostics are enabled"
        );
    }

    for expected in [
        "pub struct SceneImageCacheStats",
        "pub fn cache_stats(&self) -> SceneImageCacheStats",
        "pub fn clear_transient_caches(&mut self)",
    ] {
        assert!(
            scene_image_source.contains(expected),
            "scene-image renderer should expose `{expected}` so retained bitmap and glyph caches are observable and shrinkable after scroll-heavy sessions close"
        );
    }
}

#[test]
fn renderer_host_exposes_surface_local_present_updates() {
    let host_source =
        fs::read_to_string("src/app/terminal_renderer/host.rs").expect("read renderer host");
    let bootstrap_source = fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap");

    assert!(
        host_source.contains("pub fn present_surface_update("),
        "renderer host should expose a dedicated surface-local present entry point so scroll/theme refreshes can stay on the terminal-only path while dirty-region support is introduced behind the host seam"
    );
    assert!(
        bootstrap_source.contains(".present_surface_update("),
        "bootstrap should consume the renderer host's surface-local present entry point instead of reaching for a generic presenter call during terminal-only refreshes"
    );
}

#[test]
fn native_renderer_prepare_avoids_cloning_cached_rows_and_rehashing_full_glyph_runs() {
    let renderer_source = fs::read_to_string("src/app/terminal_renderer/wgpu_renderer.rs")
        .expect("read wgpu renderer source");
    let shaper_source = fs::read_to_string("src/app/terminal_layout/shaper.rs")
        .expect("read shaper source");

    assert!(
        renderer_source.contains("std::mem::take(&mut self.previous_prepared_rows)"),
        "native renderer prepare should move the previous prepared-row cache out of self instead of cloning the entire retained row-artifact map on every frame"
    );
    assert!(
        !renderer_source.contains("let previous_prepared_rows = self.previous_prepared_rows.clone();"),
        "native renderer prepare should stop cloning the retained row-artifact cache because repeated large viewport scrolls otherwise spend hot-path time duplicating cached draw payload vectors before any reuse happens"
    );
    assert!(
        shaper_source.contains("pub content_hash: u64")
            && shaper_source.contains("pub row_hash: u64"),
        "shaped rows should retain the model-layer content and row hashes so renderer prepare can key row reuse and frame fingerprints off cheap precomputed hashes instead of walking every glyph cluster again"
    );
    assert!(
        renderer_source.contains("row.content_hash.hash(&mut hasher)")
            && renderer_source.contains("row.row_hash.hash(&mut hasher)"),
        "renderer prepare should hash precomputed row hash fields rather than re-hashing each run, cluster, and glyph just to detect cache reuse and frame-token changes"
    );
}

#[test]
fn native_renderer_cached_glyph_hits_keep_raster_payloads_behind_shared_refs() {
    let renderer_source = fs::read_to_string("src/app/terminal_renderer/wgpu_renderer.rs")
        .expect("read wgpu renderer source");

    assert!(
        renderer_source
            .contains("glyph_raster_cache: HashMap<GlyphRasterRequest, Arc<RasterizedGlyph>>"),
        "native renderer should keep cached monochrome raster payloads behind Arc so cache hits do not clone full coverage buffers back onto the hot prepare path"
    );
    assert!(
        renderer_source.contains("return Ok(Arc::clone(rasterized));"),
        "native renderer should hand out shared glyph cache hits by Arc clone so cached raster payloads stay zero-copy until an upload is actually required"
    );
    assert!(
        !renderer_source.contains("return Ok(rasterized.clone());"),
        "native renderer cache hits should stop cloning RasterizedGlyph payloads directly because that duplicates large coverage buffers on every scroll/frame reuse"
    );
}

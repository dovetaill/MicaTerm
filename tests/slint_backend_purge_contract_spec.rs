//! Source-level contract coverage for the vendored Slint/Skia backend purge hook.

use std::fs;

#[test]
fn vendored_winit_backend_exposes_renderer_memory_purge_hook() {
    let backend_source =
        fs::read_to_string("vendor/i-slint-backend-winit/lib.rs").expect("read winit backend");

    assert!(
        backend_source.contains("fn purge_memory_resources(&self) -> Result<(), PlatformError>"),
        "winit-compatible renderers should expose a narrow purge hook so the app can request backend cache cleanup without tearing down the whole window"
    );
    assert!(
        backend_source.contains("pub trait WinitWindowMemoryPurge"),
        "the vendored winit backend should expose a public window-level purge trait so app code can reach the active renderer without downcasting private internals"
    );
    assert!(
        backend_source.contains("adapter.renderer().purge_memory_resources()?;"),
        "the window-level purge helper should forward directly to the active renderer implementation"
    );
}

#[test]
fn vendored_skia_renderer_purge_contract_clears_local_and_global_caches() {
    let renderer_source =
        fs::read_to_string("vendor/i-slint-renderer-skia/lib.rs").expect("read skia renderer");

    assert!(
        renderer_source
            .contains("pub fn purge_memory_resources(&self) -> Result<(), PlatformError>"),
        "the vendored Skia renderer should expose a dedicated purge entry point instead of reusing suspend()"
    );
    assert!(
        renderer_source.contains("self.layer_cache.clear_all();"),
        "the purge path should clear the retained layer cache in addition to image and path caches"
    );
    assert!(
        renderer_source.contains("surface.purge_memory_resources()?;"),
        "the renderer purge path should delegate to the live surface so backend-specific GPU caches can be reclaimed too"
    );
    assert!(
        renderer_source.contains("skia_safe::graphics::purge_all_caches();"),
        "the renderer purge path should call Skia's global cache purge so font/image caches do not stay resident after all tabs are closed"
    );
}

#[test]
fn vendored_d3d_surface_purge_contract_uses_conservative_cleanup() {
    let d3d_source = fs::read_to_string("vendor/i-slint-renderer-skia/d3d_surface.rs")
        .expect("read d3d surface");

    assert!(
        d3d_source.contains("self.gr_context.flush_submit_and_sync_cpu();"),
        "the D3D purge path should synchronize outstanding GPU work before attempting to reclaim backend resources"
    );
    assert!(
        d3d_source.contains("perform_deferred_cleanup(Duration::ZERO, None);"),
        "the D3D purge path should use deferred cleanup on the live DirectContext instead of abandoning the swapchain-backed renderer"
    );
}

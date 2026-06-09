use anyhow::Result;
use mica_term::app::ssh::runtime::TerminalSession;
use mica_term::app::terminal_presenter::{
    BitmapAtlasPresenter, TerminalPresentationOptions, TerminalPresenter,
};
use uuid::Uuid;

#[test]
fn bitmap_presenter_clear_transient_caches_drops_bitmap_retained_state() -> Result<()> {
    let mut presenter = BitmapAtlasPresenter::new()?;
    let mut session = TerminalSession::new(4, 20);
    session.apply_remote_bytes(b"[root@host ~]# echo atlas\r\n");
    let surface = session.surface_state(Uuid::new_v4());

    presenter.present(&surface, TerminalPresentationOptions::default())?;
    let warmed_stats = presenter.cache_stats();

    assert!(
        warmed_stats.previous_frame_rows > 0,
        "bitmap presenter should retain the last styled frame while the terminal surface stays active"
    );
    assert!(
        warmed_stats.bitmap_sprite_cache_entries > 0,
        "bitmap presenter cache stats should surface retained sprite-cache entries so diagnostics can distinguish bitmap fallback pressure from native glyph caches"
    );
    assert!(
        warmed_stats.bitmap_row_hash_entries > 0,
        "bitmap presenter cache stats should expose retained bitmap row hashes so close-path shrink logs can prove dirty-row state was cleared"
    );
    assert!(
        warmed_stats.bitmap_surface_bytes > 0,
        "bitmap presenter cache stats should expose retained bitmap surface bytes so diagnostics can spot atlas pixel buffers that survive past close or no-surface transitions"
    );

    presenter.clear_transient_caches();
    let cleared_stats = presenter.cache_stats();

    assert_eq!(
        cleared_stats.previous_frame_rows, 0,
        "clear_transient_caches should drop the previous styled bitmap frame after the workspace loses its active surface"
    );
    assert_eq!(
        cleared_stats.bitmap_sprite_cache_entries, 0,
        "clear_transient_caches should drop retained bitmap sprite entries instead of only forgetting the presenter-facing frame model"
    );
    assert_eq!(
        cleared_stats.bitmap_row_hash_entries, 0,
        "clear_transient_caches should drop retained bitmap row hashes so the next frame does not reuse stale dirty-row state"
    );
    assert_eq!(
        cleared_stats.bitmap_surface_bytes, 0,
        "clear_transient_caches should release the retained bitmap pixel buffer when no terminal surface remains visible"
    );

    Ok(())
}

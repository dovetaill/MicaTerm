use anyhow::Result;
use mica_term::app::runtime_profile::TerminalRenderMode;
use mica_term::app::ssh::runtime::TerminalSession;
use mica_term::app::terminal_presenter::{BitmapAtlasPresenter, TerminalPresenter};
use mica_term::app::terminal_renderer::host::{TerminalRendererHost, TerminalRendererHostOptions};
use uuid::Uuid;

fn check_zoom(presenter: Box<dyn TerminalPresenter>, mode: TerminalRenderMode) -> Result<()> {
    let mut host = TerminalRendererHost::new(presenter, mode);
    let initial_size = host.font_size_px();
    let initial_cell = host.default_cell_size();
    let mut session = TerminalSession::new(4, 20);
    session.apply_remote_bytes("abc 中文 ┌─┐\r\n123".as_bytes());
    let surface = session.surface_state(Uuid::new_v4());
    host.present(&surface, TerminalRendererHostOptions::default())?;

    assert!(host.adjust_font_size(i32::MIN));
    assert_eq!(host.font_size_px(), 8.0);
    assert!(!host.adjust_font_size(-1));
    let small = host.default_cell_size();
    assert!(small.0 < initial_cell.0 && small.1 < initial_cell.1);
    for (actual, base) in [(small.0, initial_cell.0), (small.1, initial_cell.1)] {
        assert!((actual as f32 - base as f32 * 8.0 / initial_size).abs() <= 1.0);
    }
    host.present(&surface, TerminalRendererHostOptions::default())?;

    assert!(host.adjust_font_size(i32::MAX));
    assert_eq!(host.font_size_px(), 40.0);
    assert!(!host.adjust_font_size(1));
    let large = host.default_cell_size();
    for (actual, base) in [(large.0, initial_cell.0), (large.1, initial_cell.1)] {
        assert!((actual as f32 - base as f32 * 40.0 / initial_size).abs() <= 1.0);
    }
    host.present(&surface, TerminalRendererHostOptions::default())?;

    // DPI changes must preserve the user's logical font size.
    host.set_raster_scale(2.0);
    assert_eq!(host.font_size_px(), 40.0);
    let hidpi = host.default_cell_size();
    assert!((hidpi.0 as i32 - large.0 as i32 * 2).abs() <= 1);
    assert!((hidpi.1 as i32 - large.1 as i32 * 2).abs() <= 1);
    host.set_raster_scale(1.0);
    host.adjust_font_size((initial_size - 40.0) as i32);
    assert_eq!(host.default_cell_size(), initial_cell);
    host.present(&surface, TerminalRendererHostOptions::default())?;
    Ok(())
}

#[test]
fn bitmap_zoom_scales_grid_and_clamps_font_size() -> Result<()> {
    check_zoom(
        Box::new(BitmapAtlasPresenter::new()?),
        TerminalRenderMode::Bitmap,
    )
}

#[cfg(feature = "terminal-native-renderer")]
#[test]
fn native_zoom_scales_grid_and_clamps_font_size() -> Result<()> {
    use mica_term::app::terminal_presenter::WindowsNativePresenter;
    check_zoom(
        Box::new(WindowsNativePresenter::new()?),
        TerminalRenderMode::Native,
    )
}

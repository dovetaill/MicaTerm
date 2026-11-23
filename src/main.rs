#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use anyhow::{Context as _, Result};
use gpui_kit::component::TitleBar;
use gpui_kit::{AppContext as _, Bounds, WindowBounds, WindowOptions, px, size};
use tracing_subscriber::EnvFilter;

use mica_term_gpui::domain::build_default_credential_store;
use mica_term_gpui::storage::{AppPaths, WorkspaceStore};
use mica_term_gpui::ui::MicaWorkspace;

fn main() -> Result<()> {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .try_init();

    let tokio_runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(4)
        .thread_name("mica-tokio")
        .build()
        .context("初始化异步运行时失败")?;
    let tokio_handle = tokio_runtime.handle().clone();

    let paths = AppPaths::resolve()?;
    let store = WorkspaceStore::open(&paths.db_path)?;
    let credentials = build_default_credential_store(&paths.secrets_dir);

    gpui_kit::application()
        .with_assets(gpui_kit::assets::AllAssets)
        .run(move |cx| {
            gpui_kit::init(cx);

            let bounds = Bounds::centered(None, size(px(1280.0), px(800.0)), cx);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(860.0), px(540.0))),
                ..TitleBar::window_options()
            };

            let paths = paths.clone();
            let credentials = credentials.clone();
            let tokio_handle = tokio_handle.clone();

            gpui_kit::open_window(options, cx, move |window, cx| {
                cx.new(|cx| {
                    MicaWorkspace::new(paths, store, credentials, tokio_handle, window, cx)
                })
            })
            .expect("打开 Mica Term 主窗口失败");
        });

    Ok(())
}

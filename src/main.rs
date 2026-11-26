#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use anyhow::{Context as _, Result};
use gpui_kit::component::TitleBar;
use gpui_kit::{AppContext as _, Bounds, WindowBounds, WindowOptions, px, size};

use mica_term_gpui::domain::build_default_credential_store;
use mica_term_gpui::logging::{
    init_logging_and_crash_handlers, maybe_attach_console_from_env, set_debug_logging,
    show_fatal_startup_error,
};
use mica_term_gpui::storage::{AppPaths, WorkspaceStore};
use mica_term_gpui::ui::{MicaWorkspace, register_bundled_fonts, register_keybindings};

fn main() -> Result<()> {
    let console = maybe_attach_console_from_env();
    if let Err(err) = run_app(console) {
        tracing::error!(error = ?err, "Mica Term 启动或运行异常退出");
        show_fatal_startup_error("Mica Term 启动失败", &format!("{err:#}"));
        return Err(err);
    }
    Ok(())
}

fn run_app(console: bool) -> Result<()> {
    let paths = AppPaths::resolve()?;
    init_logging_and_crash_handlers(&paths, console)?;
    tracing::info!(
        root_dir = %paths.root_dir.display(),
        runtime_log = %paths.runtime_log_path.display(),
        crash_log = %paths.crash_log_path.display(),
        "日志与异常诊断系统初始化完成"
    );

    let tokio_runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(4)
        .thread_stack_size(8 * 1024 * 1024)
        .thread_name("mica-tokio")
        .build()
        .context("初始化异步运行时失败")?;
    let tokio_handle = tokio_runtime.handle().clone();

    let store = WorkspaceStore::open(&paths.db_path)?;
    let snapshot = store.load_snapshot().context("加载工作区失败")?;
    if !console
        && snapshot.settings.debug_logging
        && let Err(err) = set_debug_logging(true)
    {
        tracing::error!(error = %err, "启用已保存的调试日志设置失败");
    }
    let credentials = build_default_credential_store(&paths.secrets_dir);

    gpui_kit::application()
        .with_assets(gpui_kit::assets::AllAssets)
        .run(move |cx| {
            gpui_kit::init(cx);
            cx.set_app_identity("dev.micaterm.gpui", "Mica Term GPUI");
            register_keybindings(cx);
            register_bundled_fonts(cx);

            let bounds = Bounds::centered(None, size(px(1320.0), px(820.0)), cx);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(880.0), px(560.0))),
                app_id: Some("dev.micaterm.gpui".into()),
                ..TitleBar::window_options()
            };

            let paths = paths.clone();
            let credentials = credentials.clone();
            let tokio_handle = tokio_handle.clone();

            gpui_kit::open_window(options, cx, move |window, cx| {
                tracing::info!("正在创建主工作区窗口");
                cx.new(|cx| {
                    MicaWorkspace::new(paths, store, snapshot, credentials, tokio_handle, window, cx)
                })
            })
            .expect("打开 Mica Term 主窗口失败");
        });

    Ok(())
}

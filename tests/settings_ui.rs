#![cfg(target_os = "linux")]

use std::sync::Arc;

use gpui_kit::component::{ActiveTheme as _, WindowExt as _};
use gpui_kit::{AppContext as _, Bounds, WindowBounds, WindowOptions, px, size};
use mica_term_gpui::domain::MemoryCredentialStore;
use mica_term_gpui::storage::{AppPaths, WorkspaceStore};
use mica_term_gpui::ui::modals::{SettingsTab, open_settings_modal_at};
use mica_term_gpui::ui::theme::apply_theme_specs;
use mica_term_gpui::ui::theme_catalog::{builtin_themes, contrast_ratio};
use mica_term_gpui::ui::{MicaWorkspace, register_bundled_fonts, register_keybindings};

#[test]
fn settings_pages_render_with_readable_chrome() {
    let dir = std::env::temp_dir().join(format!("mica-settings-{}", uuid::Uuid::new_v4()));
    let paths = AppPaths::from_root(&dir).unwrap();
    let store = WorkspaceStore::open(&paths.db_path).unwrap();
    let snapshot = store.load_snapshot().unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    gpui_kit::platform::headless()
        .with_assets(gpui_kit::assets::AllAssets)
        .run(move |cx| {
            gpui_kit::init(cx);
            register_keybindings(cx);
            register_bundled_fonts(cx);
            let system_font = cx.theme().font_family.clone();
            for spec in builtin_themes() {
                apply_theme_specs(&spec, &spec, None, cx);
                let theme = cx.theme();
                assert_eq!(theme.font_family, system_font);
                let rgb = |color: gpui_kit::Hsla| {
                    let color = color.to_rgb();
                    let [r, g, b] = [color.r, color.g, color.b].map(|v| (v * 255.).round() as u8);
                    u32::from_be_bytes([0, r, g, b])
                };
                for bg in [theme.title_bar, theme.background] {
                    assert!(
                        contrast_ratio(rgb(theme.muted_foreground), rgb(bg)) >= 4.5,
                        "{}",
                        spec.id
                    );
                }
            }
            let bounds = Bounds::centered(None, size(px(1320.), px(820.)), cx);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            };
            let (handle, workspace) = gpui_kit::open_window(options, cx, |window, cx| {
                cx.new(|cx| {
                    MicaWorkspace::new(
                        paths.clone(),
                        store,
                        snapshot,
                        Arc::new(MemoryCredentialStore::default()),
                        runtime.handle().clone(),
                        window,
                        cx,
                    )
                })
            })
            .unwrap();
            for tab in [
                SettingsTab::Themes,
                SettingsTab::General,
                SettingsTab::Shortcuts,
            ] {
                handle
                    .update(cx, |_, window, cx| {
                        open_settings_modal_at(workspace.clone(), paths.clone(), tab, window, cx);
                        assert!(window.has_active_dialog(cx));
                        window.draw(cx).clear(cx);
                        window.close_dialog(cx);
                        assert!(!window.has_active_dialog(cx));
                    })
                    .unwrap();
            }
            cx.spawn(async |cx| cx.update(|cx| cx.quit())).detach();
        });
    std::fs::remove_dir_all(dir).unwrap();
}

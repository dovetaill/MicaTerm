#![cfg(target_os = "linux")]

use std::{path::PathBuf, sync::Arc};

use gpui_kit::component::{ActiveTheme as _, TitleBar, WindowExt as _};
use gpui_kit::{
    App, AppContext as _, Bounds, Hsla, MouseButton, MouseDownEvent, MouseUpEvent, Pixels,
    PlatformInput, Point, Window, WindowBounds, WindowOptions, point, px, size,
};
use mica_term_gpui::domain::{
    DEFAULT_DARK_THEME_ID, DEFAULT_LIGHT_THEME_ID, HostNode, MemoryCredentialStore, SshAuthConfig,
    SshHostConfig, SshProxyConfig,
};
use mica_term_gpui::storage::{AppPaths, WorkspaceStore};
use mica_term_gpui::ui::modals::{
    SettingsTab, open_confirm_dialog, open_logs_modal, open_remote_editor_dialog,
    open_settings_modal_at,
};
use mica_term_gpui::ui::theme::apply_theme_specs;
use mica_term_gpui::ui::theme_catalog::{builtin_themes, contrast_ratio, relative_luminance};
use mica_term_gpui::ui::{MicaWorkspace, register_bundled_fonts, register_keybindings};

struct TestDirectory(PathBuf);

impl Drop for TestDirectory {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).expect("remove settings test data");
    }
}

fn rgb(color: Hsla) -> u32 {
    let color = color.to_rgb();
    let [r, g, b] = [color.r, color.g, color.b].map(|v| (v * 255.).round() as u8);
    u32::from_be_bytes([0, r, g, b])
}

fn click(position: Point<Pixels>, window: &mut Window, cx: &mut App) {
    window.dispatch_event(
        PlatformInput::MouseDown(MouseDownEvent {
            position,
            button: MouseButton::Left,
            click_count: 1,
            ..Default::default()
        }),
        cx,
    );
    window.dispatch_event(
        PlatformInput::MouseUp(MouseUpEvent {
            position,
            button: MouseButton::Left,
            click_count: 1,
            ..Default::default()
        }),
        cx,
    );
}

fn draw_and_close(window: &mut Window, cx: &mut App) {
    assert!(window.has_active_dialog(cx));
    window.refresh();
    window.draw(cx).clear(cx);
    window.close_dialog(cx);
    assert!(!window.has_active_dialog(cx));
}

#[test]
fn settings_and_business_dialogs_render_with_readable_chrome() {
    let path = std::env::temp_dir().join(format!("mica-settings-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&path).unwrap();
    let dir = TestDirectory(path);
    let paths = AppPaths::from_root(&dir.0).unwrap();
    let store = WorkspaceStore::open(&paths.db_path).unwrap();
    let mut snapshot = store.load_snapshot().unwrap();
    let folder = HostNode::new_folder("开发", None);
    let host = HostNode::new_host(
        "测试主机",
        Some(folder.id.clone()),
        SshHostConfig {
            host: "example.invalid".into(),
            port: 2222,
            auth: SshAuthConfig::PrivateKeyInline,
            proxy: SshProxyConfig::Socks5 {
                host: "localhost".into(),
                port: 1081,
                username: None,
            },
            ..Default::default()
        },
    );
    let host_id = host.id.clone();
    snapshot.hosts.nodes.extend([folder, host]);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    gpui_kit::platform::headless()
        .with_assets(gpui_kit::assets::AllAssets)
        .run(move |cx| {
            gpui_kit::init(cx);
            cx.set_reduce_motion(true);
            register_keybindings(cx);
            register_bundled_fonts(cx);
            let system_font = cx.theme().font_family.clone();
            for spec in builtin_themes() {
                apply_theme_specs(&spec, &spec, None, cx);
                let theme = cx.theme();
                assert_eq!(theme.font_family, system_font);
                assert!(!theme.focus_ring);
                assert_eq!((theme.radius, theme.radius_lg), (px(6.), px(10.)));
                assert_eq!(theme.overlay.a, if spec.dark { 0.56 } else { 0.30 });
                for bg in [
                    theme.title_bar,
                    theme.sidebar,
                    theme.background,
                    theme.popover,
                    theme.button,
                    theme.button_hover,
                    theme.button_active,
                ] {
                    assert!(
                        contrast_ratio(rgb(theme.foreground), rgb(bg)) >= 4.5,
                        "foreground on {}",
                        spec.id
                    );
                    assert!(
                        contrast_ratio(rgb(theme.muted_foreground), rgb(bg)) >= 4.5,
                        "{}",
                        spec.id
                    );
                    assert!(
                        contrast_ratio(rgb(bg.blend(theme.ring)), rgb(bg)) >= 3.,
                        "focus on {}",
                        spec.id
                    );
                }
                if spec.id == DEFAULT_LIGHT_THEME_ID {
                    assert_eq!(rgb(theme.sidebar), 0xECEEF2);
                    assert_eq!(rgb(theme.background), 0xFAFAFC);
                    assert_eq!(rgb(theme.popover), 0xFFFFFF);
                    assert!(
                        relative_luminance(rgb(theme.button))
                            < relative_luminance(rgb(theme.popover))
                    );
                } else if spec.id == DEFAULT_DARK_THEME_ID {
                    assert_eq!(rgb(theme.background), 0x12141A);
                    for bg in [theme.background, theme.sidebar] {
                        assert!(
                            relative_luminance(rgb(bg)) < relative_luminance(rgb(theme.popover))
                        );
                    }
                    assert!(
                        relative_luminance(rgb(theme.popover))
                            < relative_luminance(rgb(theme.button))
                    );
                }
            }
            let bounds = Bounds::centered(None, size(px(1320.), px(820.)), cx);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..TitleBar::window_options()
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
            for (w, h, modal_w, modal_h) in [(1320., 820., 1080., 688.), (880., 560., 774.4, 500.)]
            {
                handle
                    .update(cx, |_, window, cx| {
                        let viewport = size(px(w), px(h));
                        window.resize(viewport);
                        window.bounds_changed(cx);
                        assert_eq!(window.viewport_size(), viewport);
                        workspace.update(cx, |ws, cx| ws.set_dark_mode(w == 1320., window, cx));
                        for tab in [
                            SettingsTab::Themes,
                            SettingsTab::General,
                            SettingsTab::Shortcuts,
                        ] {
                            open_settings_modal_at(
                                workspace.clone(),
                                paths.clone(),
                                tab,
                                window,
                                cx,
                            );
                            assert!(window.has_active_dialog(cx));
                            window.refresh();
                            window.draw(cx).clear(cx);
                            let top = (h - modal_h) / 2.;
                            let right = (w + modal_w) / 2.;
                            click(point(px(right - 52.), px(top + 20.)), window, cx);
                            assert!(window.has_active_dialog(cx), "header inside {tab:?}");
                            click(point(px(right - 20.), px(top + 20.)), window, cx);
                            assert!(!window.has_active_dialog(cx), "close {tab:?} at {w}×{h}");
                        }
                        for id in [None, Some(host_id.clone())] {
                            workspace
                                .update(cx, |ws, cx| ws.open_host_dialog(id, None, window, cx));
                            draw_and_close(window, cx);
                        }
                        workspace.update(cx, |ws, cx| ws.open_keychain_dialog(None, window, cx));
                        draw_and_close(window, cx);
                        workspace.update(cx, |ws, cx| ws.open_snippet_dialog(None, window, cx));
                        draw_and_close(window, cx);
                        workspace.update(cx, |ws, cx| ws.open_folder_dialog(None, window, cx));
                        draw_and_close(window, cx);
                        open_confirm_dialog(
                            "删除主机",
                            "确认删除此主机？",
                            "删除",
                            window,
                            cx,
                            |_, _| {},
                        );
                        draw_and_close(window, cx);
                        open_remote_editor_dialog(
                            "empty.txt",
                            "/empty.txt",
                            "",
                            "",
                            14.,
                            window,
                            cx,
                            |_, _| panic!("opening the editor must not save the file"),
                        );
                        draw_and_close(window, cx);
                        open_logs_modal(paths.clone(), window, cx);
                        draw_and_close(window, cx);
                    })
                    .unwrap();
            }
            cx.spawn(async |cx| cx.update(|cx| cx.quit())).detach();
        });
}

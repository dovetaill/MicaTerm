#![cfg(target_os = "linux")]

use std::{path::PathBuf, sync::Arc};

use gpui_kit::component::{ActiveTheme as _, TitleBar, WindowExt as _};
use gpui_kit::{
    App, AppContext as _, Bounds, Hsla, MouseButton, MouseDownEvent, MouseUpEvent, Pixels,
    PlatformInput, Point, Window, WindowBounds, WindowOptions, point, px, size,
};
use mica_term_gpui::domain::{
    CredentialStore as _, DEFAULT_DARK_THEME_ID, DEFAULT_LIGHT_THEME_ID, HostNode,
    MemoryCredentialStore, SshAuthConfig, SshHostConfig, SshProxyConfig,
};
use mica_term_gpui::storage::{AppPaths, WorkspaceStore};
use mica_term_gpui::ui::modals::{
    SettingsTab, open_confirm_dialog, open_logs_modal, open_remote_editor_dialog,
    open_settings_modal_at,
};
use mica_term_gpui::ui::theme::{apply_theme_specs, terminal_tab_chrome};
use mica_term_gpui::ui::theme_catalog::{builtin_themes, contrast_ratio, relative_luminance};
use mica_term_gpui::ui::workspace::{PaneLayoutNode, TabSplitAxis};
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

                let tab_chrome = terminal_tab_chrome(cx);
                let eff_active_bg = theme.title_bar.blend(tab_chrome.active_bg);
                let eff_active_fg = eff_active_bg.blend(tab_chrome.active_fg);
                let eff_inactive_fg = theme.title_bar.blend(tab_chrome.inactive_fg);
                assert!(
                    contrast_ratio(rgb(eff_active_fg), rgb(eff_active_bg)) >= 4.5,
                    "active tab contrast on {}",
                    spec.id
                );
                assert!(
                    contrast_ratio(rgb(eff_inactive_fg), rgb(theme.title_bar)) >= 4.5,
                    "inactive tab contrast on {}",
                    spec.id
                );
                if spec.dark {
                    assert!(
                        relative_luminance(rgb(eff_active_bg))
                            > relative_luminance(rgb(theme.title_bar)),
                        "dark active tab should lift above title_bar on {}",
                        spec.id
                    );
                } else {
                    assert!(
                        relative_luminance(rgb(eff_active_bg))
                            >= relative_luminance(rgb(theme.title_bar)),
                        "light active tab should stay bright on {}",
                        spec.id
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

#[test]
fn rex_tabs_splits_floating_and_header_invariants() {
    let path = std::env::temp_dir().join(format!("mica-rex-ui-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&path).unwrap();
    let dir = TestDirectory(path);
    let paths = AppPaths::from_root(&dir.0).unwrap();
    let store = WorkspaceStore::open(&paths.db_path).unwrap();
    let mut snapshot = store.load_snapshot().unwrap();
    let cred_store = Arc::new(MemoryCredentialStore::default());
    let host = HostNode::new_host(
        "生产节点-B",
        None,
        SshHostConfig {
            host: "127.0.0.1".into(),
            port: 1,
            user: "ops".into(),
            auth: SshAuthConfig::Password,
            ..Default::default()
        },
    );
    let host_id = host.id.clone();
    let mut secret = mica_term_gpui::domain::SecretPayload::default();
    secret.password = Some("secret".into());
    cred_store
        .put_payload(&mica_term_gpui::domain::host_secret_key(&host_id), &secret)
        .unwrap();
    snapshot.hosts.nodes.push(host);

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    gpui_kit::platform::headless()
        .with_assets(gpui_kit::assets::AllAssets)
        .run(move |cx| {
            gpui_kit::init(cx);
            cx.set_reduce_motion(true);
            register_keybindings(cx);
            register_bundled_fonts(cx);

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
                        cred_store,
                        runtime.handle().clone(),
                        window,
                        cx,
                    )
                })
            })
            .unwrap();

            let redraw = |cx: &mut App| {
                handle
                    .update(cx, |_, window, cx| {
                        window.refresh();
                        window.draw(cx).clear(cx);
                    })
                    .unwrap();
            };
            let click_in_window = |pos: Point<Pixels>, cx: &mut App| {
                handle
                    .update(cx, |_, window, cx| {
                        click(pos, window, cx);
                    })
                    .unwrap();
            };

            // 1. Open 1 local shell tab: `+` button hugs the tab and drag space stays wide
            handle
                .update(cx, |_, window, cx| {
                    let viewport = size(px(1320.), px(820.));
                    window.resize(viewport);
                    window.bounds_changed(cx);
                    workspace.update(cx, |ws, cx| ws.open_local_shell_tab(window, cx));
                })
                .unwrap();
            redraw(cx);

            let strip_b = workspace.read(cx).tab_strip_scroll_handle().bounds();
            assert!(
                f32::from(strip_b.size.width) > 40.0 && f32::from(strip_b.size.width) < 250.0,
                "1 tab strip must hug tab width instead of stretching full width, got {:?}",
                strip_b
            );
            assert_eq!(
                workspace.read(cx).tab_strip_scroll_handle().max_offset().x,
                px(0.0)
            );

            // Real hit-test: clicking ~20px to the right of the 1st tab hits `new-local-tab-btn` (`+`)
            click_in_window(point(strip_b.right() + px(20.0), px(20.0)), cx);
            redraw(cx);
            assert_eq!(
                workspace.read(cx).tabs.len(),
                2,
                "clicking immediately after last tab must trigger `+` new-tab button"
            );

            // Clicking empty titlebar drag space (`x = 800`) does NOT steal focus from active terminal pane
            let active_pane_2 = workspace.read(cx).tabs[1].active_pane.clone();
            handle
                .update(cx, |_, window, cx| {
                    assert!(
                        gpui_kit::Focusable::focus_handle(&active_pane_2, cx).is_focused(window)
                    );
                })
                .unwrap();
            click_in_window(point(px(800.0), px(20.0)), cx);
            redraw(cx);
            assert_eq!(workspace.read(cx).tabs.len(), 2);
            handle
                .update(cx, |_, window, cx| {
                    assert!(
                        gpui_kit::Focusable::focus_handle(&active_pane_2, cx).is_focused(window),
                        "clicking empty titlebar drag space must not steal terminal focus or trigger root track_focus"
                    );
                })
                .unwrap();

            let first_tab_id = workspace.read(cx).tabs[0].id.clone();
            handle
                .update(cx, |_, window, cx| {
                    workspace
                        .update(cx, |ws, cx| ws.close_other_tabs(&first_tab_id, window, cx));
                })
                .unwrap();
            redraw(cx);

            // 2. Single pane has no close button (rightmost action is `pane-split-v`, NOT `close-pane`)
            //    and terminal grid sits strictly below the 28px header
            let pane_a = workspace.read(cx).tabs[0].active_pane.clone();
            assert_eq!(pane_a.read(cx).pane_count_in_tab, 1);
            let header_a = pane_a.read(cx).last_header_bounds().unwrap();
            let grid_a = pane_a.read(cx).last_grid_bounds().unwrap();
            assert_eq!(f32::from(header_a.size.height), 28.0);
            assert!(
                f32::from(grid_a.origin.y) >= f32::from(header_a.bottom()),
                "terminal grid ({:?}) must not overlap 28px pane header ({:?})",
                grid_a,
                header_a
            );
            // Clicking rightmost header button on a single pane triggers `pane-split-v` (splits to 2 panes),
            // proving `close-pane` is absent when `pane_count == 1`; then clicking that exact same rightmost
            // slot on the 2-pane header hits `close-pane` and restores back to 1 pane!
            click_in_window(
                point(header_a.right() - px(19.0), header_a.origin.y + px(14.0)),
                cx,
            );
            redraw(cx);
            redraw(cx);
            assert_eq!(
                workspace.read(cx).tabs.len(),
                1,
                "single pane must not close the tab when clicking rightmost header button"
            );
            assert_eq!(
                workspace.read(cx).tabs[0].panes().len(),
                2,
                "rightmost button on single-pane header should be vertical split"
            );
            let temp_split_pane = workspace.read(cx).tabs[0].active_pane.clone();
            let temp_header = temp_split_pane.read(cx).last_header_bounds().unwrap();
            click_in_window(
                point(temp_header.right() - px(19.0), temp_header.origin.y + px(14.0)),
                cx,
            );
            redraw(cx);
            assert_eq!(
                workspace.read(cx).tabs[0].panes(),
                vec![pane_a.clone()],
                "rightmost button on multi-pane header should close the split pane back to 1 pane"
            );

            // 3. Overflowing 15 tabs shrinks horizontally, scrolls to newest tab, and keeps right toolbar intact
            handle
                .update(cx, |_, window, cx| {
                    for _ in 0..14 {
                        workspace.update(cx, |ws, cx| ws.open_local_shell_tab(window, cx));
                    }
                })
                .unwrap();
            redraw(cx);
            let strip_15 = workspace.read(cx).tab_strip_scroll_handle().bounds();
            let max_off_15 = workspace.read(cx).tab_strip_scroll_handle().max_offset();
            let cur_off_15 = workspace.read(cx).tab_strip_scroll_handle().offset();
            assert!(
                f32::from(strip_15.right()) <= 1120.0,
                "15-tab strip must shrink so `+`, drag_space, and right toolbar fit inside 1320px: {:?}",
                strip_15
            );
            assert!(
                f32::from(max_off_15.x) > 200.0,
                "15 tabs must be horizontally scrollable, got max_offset {:?}",
                max_off_15
            );
            assert!(
                f32::from(cur_off_15.x) < -100.0,
                "opening 15th tab must auto-scroll tab strip into view, got offset {:?}",
                cur_off_15
            );

            handle
                .update(cx, |_, window, cx| {
                    workspace
                        .update(cx, |ws, cx| ws.close_other_tabs(&first_tab_id, window, cx));
                })
                .unwrap();
            redraw(cx);

            // 4. Split horizontally with SSH host -> 6px shell gap & hover-target split clones target
            handle
                .update(cx, |_, window, cx| {
                    workspace.update(cx, |ws, cx| {
                        ws.connect_host_in_split(&host_id, true, window, cx);
                    });
                })
                .unwrap();
            redraw(cx);
            redraw(cx);

            let pane_b = workspace.read(cx).tabs[0].active_pane.clone();
            assert_ne!(pane_a, pane_b);
            assert!(pane_b.read(cx).session.read(cx).is_ssh());

            let header_a_split = pane_a.read(cx).last_header_bounds().unwrap();
            let header_b_split = pane_b.read(cx).last_header_bounds().unwrap();
            // Inner header gap = 1px right border of card A + 6px shell gap + 1px left border of card B = 8px
            let inner_header_gap = f32::from(header_b_split.origin.x - header_a_split.right());
            assert!(
                (7.5..=8.5).contains(&inner_header_gap),
                "expected 6px outer shell gap (8px between inner header boxes), got {inner_header_gap} (a={header_a_split:?}, b={header_b_split:?})"
            );

            // Activate local `pane_a`, then split non-active SSH `pane_b` -> new pane C must clone SSH `pane_b`
            handle
                .update(cx, |_, window, cx| {
                    workspace.update(cx, |ws, cx| ws.activate_pane(&pane_a, Some(window), cx));
                    workspace.update(cx, |ws, cx| {
                        ws.split_pane(&pane_b, TabSplitAxis::Vertical, window, cx);
                    });
                })
                .unwrap();
            // Draw 2 frames so newly mounted `v_resizable` settles its container bounds
            redraw(cx);
            redraw(cx);

            let pane_c = workspace.read(cx).tabs[0].active_pane.clone();
            assert_eq!(workspace.read(cx).tabs[0].panes().len(), 3);
            assert!(
                pane_c.read(cx).session.read(cx).is_ssh(),
                "splitting hovered non-active SSH pane B must clone B, not active local pane A"
            );

            // Real hit-test on `close-pane` button in `pane_c`'s 28px header (flushes `TerminalPaneEvent::CloseRequested`)
            let header_c = pane_c.read(cx).last_header_bounds().unwrap();
            click_in_window(
                point(header_c.right() - px(19.0), header_c.origin.y + px(14.0)),
                cx,
            );
            redraw(cx);
            assert_eq!(
                workspace.read(cx).tabs[0].panes().len(),
                2,
                "clicking top-right X button on split pane header must close that split pane (header_c={header_c:?})"
            );

            // 5. Unequal split ratio -> Maximize -> Restore preserves custom `Entity<ResizableState>` proportions
            let split_state_before = match &workspace.read(cx).tabs[0].root {
                PaneLayoutNode::Split { state, .. } => state.clone(),
                PaneLayoutNode::Leaf(_) => panic!("expected split root"),
            };
            handle
                .update(cx, |_, window, cx| {
                    split_state_before.update(cx, |s, cx| {
                        s.resize_panel(0, px(240.0), window, cx);
                    });
                })
                .unwrap();
            redraw(cx);
            redraw(cx);
            let sizes_before = split_state_before.read(cx).sizes().clone();
            let w_split = f32::from(pane_a.read(cx).last_header_bounds().unwrap().size.width);
            let w_split_b = f32::from(pane_b.read(cx).last_header_bounds().unwrap().size.width);
            assert!(
                w_split_b > w_split + 80.0,
                "expected unequal split widths before maximize ({w_split} vs {w_split_b})"
            );

            // Clicking `pane_a`'s header focuses `pane_a` without `#mica-workspace-stage` stealing focus
            let header_a_click = pane_a.read(cx).last_header_bounds().unwrap();
            click_in_window(
                point(header_a_click.origin.x + px(40.0), header_a_click.origin.y + px(14.0)),
                cx,
            );
            redraw(cx);
            handle
                .update(cx, |_, window, cx| {
                    assert!(
                        gpui_kit::Focusable::focus_handle(&pane_a, cx).is_focused(window),
                        "clicking tiled pane header must keep terminal pane focused"
                    );
                })
                .unwrap();

            handle
                .update(cx, |_, window, cx| {
                    workspace
                        .update(cx, |ws, cx| ws.toggle_maximize_pane(&pane_a, window, cx));
                })
                .unwrap();
            redraw(cx);
            assert!(pane_a.read(cx).is_maximized_pane);
            let w_max = f32::from(pane_a.read(cx).last_header_bounds().unwrap().size.width);
            assert!(
                w_max > w_split * 2.2,
                "maximized pane must fill full center stage width ({w_max} vs split {w_split})"
            );

            handle
                .update(cx, |_, window, cx| {
                    workspace
                        .update(cx, |ws, cx| ws.toggle_maximize_pane(&pane_a, window, cx));
                })
                .unwrap();
            redraw(cx);
            assert!(!pane_a.read(cx).is_maximized_pane);
            let w_restored = f32::from(pane_a.read(cx).last_header_bounds().unwrap().size.width);
            assert!((w_restored - w_split).abs() < 1.0);
            let split_state_after = match &workspace.read(cx).tabs[0].root {
                PaneLayoutNode::Split { state, .. } => state.clone(),
                PaneLayoutNode::Leaf(_) => panic!("expected split root after restore"),
            };
            assert_eq!(split_state_before, split_state_after);
            assert_eq!(split_state_after.read(cx).sizes(), &sizes_before);

            // 6. Floating pane continuous drag over `.occlude()` body + collapse tab-switch focus + promotion
            handle
                .update(cx, |_, window, cx| {
                    workspace.update(cx, |ws, cx| ws.toggle_float_pane(&pane_b, window, cx));
                })
                .unwrap();
            redraw(cx);
            assert!(pane_b.read(cx).is_floating_pane);
            assert_eq!(workspace.read(cx).tabs[0].tiled_panes(), vec![pane_a.clone()]);
            assert_eq!(workspace.read(cx).tabs[0].floating.len(), 1);

            // Drag floating `pane_b` by its 28px header while cursor remains inside `.occlude()` card
            let rect_before_drag = workspace.read(cx).tabs[0].floating[0].rect;
            let float_hdr = pane_b.read(cx).last_header_bounds().unwrap();
            let start_pt = point(float_hdr.origin.x + px(60.0), float_hdr.origin.y + px(14.0));
            let mid_pt = point(start_pt.x - px(45.0), start_pt.y + px(30.0));
            handle
                .update(cx, |_, window, cx| {
                    window.dispatch_event(
                        PlatformInput::MouseDown(MouseDownEvent {
                            position: start_pt,
                            button: MouseButton::Left,
                            click_count: 1,
                            ..Default::default()
                        }),
                        cx,
                    );
                })
                .unwrap();
            redraw(cx);
            handle
                .update(cx, |_, window, cx| {
                    window.dispatch_event(
                        PlatformInput::MouseMove(gpui_kit::MouseMoveEvent {
                            position: mid_pt,
                            pressed_button: Some(MouseButton::Left),
                            modifiers: Default::default(),
                        }),
                        cx,
                    );
                    window.dispatch_event(
                        PlatformInput::MouseUp(MouseUpEvent {
                            position: mid_pt,
                            button: MouseButton::Left,
                            click_count: 1,
                            ..Default::default()
                        }),
                        cx,
                    );
                })
                .unwrap();
            redraw(cx);
            let rect_after_drag = workspace.read(cx).tabs[0].floating[0].rect;
            assert!(
                (rect_after_drag.x - (rect_before_drag.x - 45.0)).abs() < 1.0,
                "floating drag over occluded body must track continuously ({rect_before_drag:?} -> {rect_after_drag:?})"
            );

            let expanded_h = workspace.read(cx).tabs[0].floating[0].rect.height;

            // Collapse floating `pane_b`, switch to a temporary tab and back -> focus stays mounted & active!
            handle
                .update(cx, |_, _, cx| {
                    workspace
                        .update(cx, |ws, cx| ws.toggle_collapse_floating_pane(&pane_b, cx));
                })
                .unwrap();
            redraw(cx);
            assert!(pane_b.read(cx).is_floating_collapsed);
            handle
                .update(cx, |_, window, cx| {
                    workspace.update(cx, |ws, cx| ws.open_local_shell_tab(window, cx));
                })
                .unwrap();
            redraw(cx);
            let extra_tid = workspace.read(cx).tabs[1].id.clone();
            handle
                .update(cx, |_, window, cx| {
                    workspace.update(cx, |ws, cx| ws.close_tab(&extra_tid, window, cx));
                })
                .unwrap();
            redraw(cx);
            handle
                .update(cx, |_, window, cx| {
                    assert!(
                        gpui_kit::Focusable::focus_handle(&pane_b, cx).is_focused(window),
                        "collapsed active floating pane must retain mounted focus across tab switches"
                    );
                })
                .unwrap();

            handle
                .update(cx, |_, _, cx| {
                    workspace
                        .update(cx, |ws, cx| ws.toggle_collapse_floating_pane(&pane_b, cx));
                })
                .unwrap();
            redraw(cx);
            assert!(!pane_b.read(cx).is_floating_collapsed);
            assert_eq!(
                workspace.read(cx).tabs[0].floating[0].rect.height,
                expanded_h
            );

            // Floating sole tiled pane `pane_a` swaps `pane_b` onto tiled floor so root stays non-empty
            handle
                .update(cx, |_, window, cx| {
                    workspace.update(cx, |ws, cx| ws.toggle_float_pane(&pane_a, window, cx));
                })
                .unwrap();
            redraw(cx);
            assert_eq!(workspace.read(cx).tabs[0].tiled_panes(), vec![pane_b.clone()]);
            assert_eq!(workspace.read(cx).tabs[0].floating.len(), 1);
            assert!(pane_a.read(cx).is_floating_pane);

            // Closing sole tiled floor `pane_b` promotes floating `pane_a` to sole tiled root
            handle
                .update(cx, |_, window, cx| {
                    workspace.update(cx, |ws, cx| ws.close_pane(&pane_b, window, cx));
                })
                .unwrap();
            redraw(cx);
            assert_eq!(workspace.read(cx).tabs.len(), 1);
            assert_eq!(workspace.read(cx).tabs[0].panes(), vec![pane_a.clone()]);
            assert!(workspace.read(cx).tabs[0].floating.is_empty());
            assert!(!pane_a.read(cx).is_floating_pane);
            assert_eq!(pane_a.read(cx).pane_count_in_tab, 1);

            cx.spawn(async |cx| cx.update(|cx| cx.quit())).detach();
        });
}

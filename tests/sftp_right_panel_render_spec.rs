#![cfg(feature = "slint-renderer-software")]

use std::rc::Rc;
use std::time::{Duration, Instant};

use mica_term::AppWindow;
use mica_term::SftpPanelItem;
use slint::platform::software_renderer::{MinimalSoftwareWindow, RepaintBufferType};
use slint::platform::{Platform, PlatformError, WindowAdapter};
use slint::{ComponentHandle, ModelRc, PhysicalSize, Rgb8Pixel, SharedPixelBuffer, VecModel};

const WINDOW_WIDTH: u32 = 1440;
const WINDOW_HEIGHT: u32 = 900;
const PANEL_X: u32 = WINDOW_WIDTH - 392;

struct SoftwareTestPlatform {
    window: Rc<MinimalSoftwareWindow>,
    started_at: Instant,
}

impl Platform for SoftwareTestPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        Ok(self.window.clone())
    }

    fn duration_since_start(&self) -> Duration {
        self.started_at.elapsed()
    }
}

fn pixel_at(buffer: &SharedPixelBuffer<Rgb8Pixel>, x: u32, y: u32) -> Rgb8Pixel {
    buffer.as_slice()[(y * buffer.width() + x) as usize]
}

fn color_distance(a: Rgb8Pixel, b: Rgb8Pixel) -> u16 {
    let dr = (i16::from(a.r) - i16::from(b.r)).unsigned_abs();
    let dg = (i16::from(a.g) - i16::from(b.g)).unsigned_abs();
    let db = (i16::from(a.b) - i16::from(b.b)).unsigned_abs();

    dr + dg + db
}

fn count_distinct_pixels(
    buffer: &SharedPixelBuffer<Rgb8Pixel>,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    baseline: Rgb8Pixel,
    threshold: u16,
) -> usize {
    let mut distinct = 0usize;

    for sample_y in y..(y + height) {
        for sample_x in x..(x + width) {
            if color_distance(pixel_at(buffer, sample_x, sample_y), baseline) >= threshold {
                distinct += 1;
            }
        }
    }

    distinct
}

fn render_app(setup: impl FnOnce(&AppWindow)) -> SharedPixelBuffer<Rgb8Pixel> {
    let window = MinimalSoftwareWindow::new(RepaintBufferType::ReusedBuffer);
    slint::platform::set_platform(Box::new(SoftwareTestPlatform {
        window: window.clone(),
        started_at: Instant::now(),
    }))
    .unwrap();

    let app = AppWindow::new().unwrap();
    app.set_dark_mode(false);
    app.window()
        .set_size(PhysicalSize::new(WINDOW_WIDTH, WINDOW_HEIGHT));
    setup(&app);
    app.show().unwrap();

    let mut buffer = SharedPixelBuffer::<Rgb8Pixel>::new(WINDOW_WIDTH, WINDOW_HEIGHT);
    let stride = buffer.width() as usize;
    assert!(window.draw_if_needed(|renderer| {
        renderer.render(buffer.make_mut_slice(), stride);
    }));
    buffer
}

#[test]
fn empty_sftp_panel_renders_empty_state_copy() {
    let buffer = render_app(|app| {
        app.set_show_right_panel(true);
        app.set_effective_show_right_panel(true);
        app.set_right_panel_view("sftp".into());
        app.set_sftp_panel_mode("empty".into());
    });

    let panel_surface = pixel_at(&buffer, PANEL_X + 280, 100);
    let headline_pixels =
        count_distinct_pixels(&buffer, PANEL_X + 12, 72, 340, 48, panel_surface, 14);
    let body_pixels =
        count_distinct_pixels(&buffer, PANEL_X + 12, 150, 340, 120, panel_surface, 14);

    assert!(
        headline_pixels >= 1200,
        "empty sftp panel should render a visible headline, only found {headline_pixels} distinct pixels"
    );
    assert!(
        body_pixels >= 2200,
        "empty sftp panel should render explanatory body copy, only found {body_pixels} distinct pixels"
    );
}

#[test]
fn right_panel_source_defines_single_line_toolbar_and_path_bar_contract() {
    let source = std::fs::read_to_string("ui/shell/right-panel.slint").unwrap();

    assert!(
        source.contains("toolbar-row"),
        "right panel should define a compact single-line toolbar row"
    );
    assert!(
        source.contains("breadcrumb-row"),
        "right panel should define a dedicated breadcrumb/path row shell"
    );
    assert!(
        !source.contains("session-strip"),
        "right panel should remove the legacy session strip"
    );
}

#[test]
fn right_panel_source_uses_two_line_rows_instead_of_dense_file_headers() {
    let source = std::fs::read_to_string("ui/shell/right-panel.slint").unwrap();

    assert!(
        source.contains("meta-text := Text"),
        "right panel should project a dedicated meta text row for compact file metadata"
    );
    assert!(
        !source.contains("text: \"Name\"") && !source.contains("label: \"Name\""),
        "right panel should stop rendering the legacy Name table header"
    );
    assert!(
        !source.contains("text: \"Type\"") && !source.contains("label: \"Type\""),
        "right panel should stop rendering the legacy Type table header"
    );
    assert!(
        !source.contains("text: \"Modified\"") && !source.contains("label: \"Modified\""),
        "right panel should stop rendering the legacy Modified table header"
    );
    assert!(
        !source.contains("text: \"Size\"") && !source.contains("label: \"Size\""),
        "right panel should stop rendering the legacy Size table header"
    );
}

#[test]
fn right_panel_source_keeps_runtime_sort_contract_without_table_handles() {
    let source = std::fs::read_to_string("ui/shell/right-panel.slint").unwrap();

    assert!(
        source.contains("in property <string> sftp-panel-sort-column")
            && source.contains("in property <string> sftp-panel-sort-direction"),
        "right panel should accept runtime sort state from the shell view model"
    );
    assert!(
        source.contains("in property <length> sftp-panel-name-column-width")
            && source.contains("in property <length> sftp-panel-type-column-width")
            && source.contains("in property <length> sftp-panel-modified-column-width")
            && source.contains("in property <length> sftp-panel-size-column-width"),
        "right panel should bind runtime column widths instead of hard-coded file table widths"
    );
    assert!(
        source.contains("callback sftp-panel-sort-requested(string);")
            && source
                .contains("callback sftp-panel-column-width-change-requested(string, length);"),
        "right panel should expose sort and column-resize callbacks"
    );
    assert!(
        !source.contains("sort-indicator") && !source.contains("resize-handle"),
        "the compact quick browser should drop explicit table sort indicators and resize handles from the rendered chrome"
    );
}

#[test]
fn right_panel_source_uses_compact_toolbar_buttons_without_horizontal_file_scroll() {
    let source = std::fs::read_to_string("ui/shell/right-panel.slint").unwrap();

    assert!(
        source.contains("SidebarToolbarIconButton"),
        "sftp toolbar should reuse the shared compact icon button component"
    );
    assert!(
        !source.contains("horizontal-scrollbar-policy: always-on;"),
        "the quick browser should not force horizontal scrolling for the two-line row layout"
    );
    assert!(
        source.contains("item.kind == \"parent-directory\""),
        "sftp rows should reserve a dedicated parent-directory row contract for navigating up"
    );
}

#[test]
fn app_window_source_threads_sftp_table_state_into_right_panel() {
    let source = std::fs::read_to_string("ui/app-window.slint").unwrap();

    assert!(
        source.contains("sftp-panel-sort-column: root.sftp-panel-sort-column;")
            && source.contains("sftp-panel-sort-direction: root.sftp-panel-sort-direction;"),
        "app window should forward sort state into the right panel"
    );
    assert!(
        source.contains("sftp-panel-name-column-width: root.sftp-panel-name-column-width;")
            && source.contains("sftp-panel-type-column-width: root.sftp-panel-type-column-width;")
            && source.contains(
                "sftp-panel-modified-column-width: root.sftp-panel-modified-column-width;"
            )
            && source.contains("sftp-panel-size-column-width: root.sftp-panel-size-column-width;"),
        "app window should forward current column widths into the right panel"
    );
    assert!(
        source.contains("sftp-panel-sort-requested(column-id) => {")
            && source.contains("root.sftp-panel-sort-requested(column-id);")
            && source.contains("sftp-panel-column-width-change-requested(column-id, width) => {")
            && source.contains("root.sftp-panel-column-width-change-requested(column-id, width);"),
        "app window should proxy header sort and column resize callbacks from the right panel"
    );
}

#[test]
fn right_panel_source_uses_fluent_toolbar_icons_and_actions_menu_trigger() {
    let source = std::fs::read_to_string("ui/shell/right-panel.slint").unwrap();

    for asset in [
        "arrow-hook-up-left-20-regular.svg",
        "arrow-sync-20-regular.svg",
        "panel-right-expand-20-regular.svg",
        "link-20-regular.svg",
        "folder-20-regular.svg",
        "document-20-regular.svg",
    ] {
        assert!(
            source.contains(asset),
            "right panel toolbar should load the Fluent `{asset}` icon asset"
        );
    }

    assert!(
        source.contains("sftp-panel-context-menu-requested(") && source.contains("\"sftp-blank\""),
        "blank-area context menu affordances should stay wired for low-frequency actions"
    );
    assert!(
        !source.contains("glyph: \"<\"")
            && !source.contains("glyph: \">\"")
            && !source.contains("glyph: \"^\"")
            && !source.contains("glyph: \"+\"")
            && !source.contains("glyph: \"R\""),
        "toolbar should stop rendering ASCII placeholder glyphs"
    );
    assert!(
        !source.contains("arrow-next-20-regular.svg")
            && !source.contains("arrow-sort-up-20-regular.svg"),
        "toolbar should drop the forward and up buttons from the compact browser rail"
    );
    assert!(
        !source.contains("? \"BROWSE\" : \"LIVE\"") && !source.contains("text: \"Follow\""),
        "path bar should stop rendering the legacy live/follow chrome"
    );
    assert!(
        source.contains("double-clicked => {") && source.contains("sftp-panel-item-activated("),
        "sftp rows should expose a double-click activation callback"
    );
}

#[test]
fn right_panel_source_no_longer_renders_queue_summary_inside_panel() {
    let source = std::fs::read_to_string("ui/shell/right-panel.slint").unwrap();

    assert!(
        !source.contains("Transfer queue"),
        "right panel should remove the embedded transfer queue strip"
    );
    assert!(
        !source.contains("queue-strip :="),
        "right panel should remove the queue strip container"
    );
    assert!(
        !source.contains("queue-drawer :="),
        "right panel should remove the queue drawer container"
    );
}

#[test]
fn quick_browser_header_keeps_low_frequency_actions_out_of_the_main_toolbar() {
    let source = std::fs::read_to_string("ui/shell/right-panel.slint").unwrap();
    let toolbar_source = source
        .split("toolbar-row := Rectangle {")
        .nth(1)
        .and_then(|rest| rest.split("table-card := Rectangle {").next())
        .expect("toolbar section should exist");

    assert!(
        !toolbar_source.contains("root.sftp-panel-upload-requested();"),
        "upload should remain a low-frequency action outside the main quick-browser toolbar"
    );
    assert!(
        !toolbar_source.contains("root.sftp-panel-new-folder-requested();"),
        "new folder should remain a low-frequency action outside the main quick-browser toolbar"
    );
}

#[test]
fn ready_sftp_panel_renders_compact_toolbar_and_file_table() {
    let rows = vec![
        SftpPanelItem {
            id: "entry-app".into(),
            name: "app".into(),
            meta_label: "Folder · 2026-03-31 10:05".into(),
            type_label: "Folder".into(),
            modified_label: "2026-03-31 10:05".into(),
            size_label: "".into(),
            kind: "directory".into(),
            selected: true,
        },
        SftpPanelItem {
            id: "entry-release".into(),
            name: "release.tar.gz".into(),
            meta_label: "File · 14 KB · 2026-03-31 10:11".into(),
            type_label: "File".into(),
            modified_label: "2026-03-31 10:11".into(),
            size_label: "14 KB".into(),
            kind: "file".into(),
            selected: false,
        },
    ];

    let buffer = render_app(|app| {
        app.set_show_right_panel(true);
        app.set_effective_show_right_panel(true);
        app.set_right_panel_view("sftp".into());
        app.set_sftp_panel_mode("ready".into());
        app.set_sftp_panel_host_label("Prod Bastion".into());
        app.set_sftp_panel_path("/srv/app/releases".into());
        app.set_sftp_panel_follow_mode("manual-browse".into());
        app.set_sftp_panel_actions_enabled(true);
        app.set_sftp_panel_items(ModelRc::new(VecModel::from(rows)));
        app.set_sftp_panel_queue_active(2);
        app.set_sftp_panel_queue_failed(1);
        app.set_sftp_panel_queue_current_session(3);
    });

    let panel_surface = pixel_at(&buffer, PANEL_X + 20, 28);
    let toolbar_pixels =
        count_distinct_pixels(&buffer, PANEL_X + 12, 12, 360, 56, panel_surface, 14);
    let path_bar_pixels =
        count_distinct_pixels(&buffer, PANEL_X + 84, 12, 276, 56, panel_surface, 14);
    let list_pixels = count_distinct_pixels(&buffer, PANEL_X + 12, 88, 360, 260, panel_surface, 14);

    assert!(
        toolbar_pixels >= 1400,
        "ready sftp panel should render the browser toolbar and path bar, only found {toolbar_pixels} distinct pixels"
    );
    assert!(
        path_bar_pixels >= 850,
        "ready sftp panel should render a visible path bar shell, only found {path_bar_pixels} distinct pixels"
    );
    assert!(
        list_pixels >= 6200,
        "ready sftp panel should render the file list shell, only found {list_pixels} distinct pixels"
    );
}

#[test]
fn disconnected_sftp_panel_renders_retry_guidance_shell() {
    let buffer = render_app(|app| {
        app.set_show_right_panel(true);
        app.set_effective_show_right_panel(true);
        app.set_right_panel_view("sftp".into());
        app.set_sftp_panel_mode("disconnected".into());
        app.set_sftp_panel_host_label("Prod Bastion".into());
        app.set_sftp_panel_path("/srv/app/current".into());
        app.set_sftp_panel_follow_mode("follow-cwd".into());
        app.set_sftp_panel_actions_enabled(false);
    });

    let panel_surface = pixel_at(&buffer, PANEL_X + 280, 100);
    let headline_pixels =
        count_distinct_pixels(&buffer, PANEL_X + 12, 96, 340, 52, panel_surface, 14);
    let body_pixels =
        count_distinct_pixels(&buffer, PANEL_X + 12, 150, 340, 160, panel_surface, 14);
    let retry_pixels =
        count_distinct_pixels(&buffer, PANEL_X + 300, 102, 84, 36, panel_surface, 14);

    assert!(
        headline_pixels >= 1100,
        "disconnected sftp panel should render a visible retry headline, only found {headline_pixels} distinct pixels"
    );
    assert!(
        body_pixels >= 2600,
        "disconnected sftp panel should render recovery guidance copy, only found {body_pixels} distinct pixels"
    );
    assert!(
        retry_pixels >= 480,
        "disconnected sftp panel should render a retry action shell, only found {retry_pixels} distinct pixels"
    );
}

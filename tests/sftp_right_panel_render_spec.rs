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

    let panel_surface = pixel_at(&buffer, PANEL_X + 24, 80);
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
fn ready_sftp_panel_renders_session_toolbar_list_and_queue_summary() {
    let rows = vec![
        SftpPanelItem {
            id: "entry-app".into(),
            name: "app".into(),
            detail: "Directory".into(),
            kind: "directory".into(),
            selected: true,
        },
        SftpPanelItem {
            id: "entry-release".into(),
            name: "release.tar.gz".into(),
            detail: "14 KB".into(),
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
    let session_pixels =
        count_distinct_pixels(&buffer, PANEL_X + 12, 12, 360, 52, panel_surface, 14);
    let toolbar_pixels =
        count_distinct_pixels(&buffer, PANEL_X + 12, 70, 360, 82, panel_surface, 14);
    let list_pixels =
        count_distinct_pixels(&buffer, PANEL_X + 12, 164, 360, 220, panel_surface, 14);
    let queue_pixels =
        count_distinct_pixels(&buffer, PANEL_X + 12, 780, 360, 72, panel_surface, 14);

    assert!(
        session_pixels >= 1800,
        "ready sftp panel should render the session strip, only found {session_pixels} distinct pixels"
    );
    assert!(
        toolbar_pixels >= 3500,
        "ready sftp panel should render the browser toolbar and path bar, only found {toolbar_pixels} distinct pixels"
    );
    assert!(
        list_pixels >= 6000,
        "ready sftp panel should render the file list shell, only found {list_pixels} distinct pixels"
    );
    assert!(
        queue_pixels >= 1800,
        "ready sftp panel should render the queue summary strip, only found {queue_pixels} distinct pixels"
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

    let panel_surface = pixel_at(&buffer, PANEL_X + 24, 80);
    let headline_pixels =
        count_distinct_pixels(&buffer, PANEL_X + 12, 96, 340, 52, panel_surface, 14);
    let body_pixels =
        count_distinct_pixels(&buffer, PANEL_X + 12, 150, 340, 160, panel_surface, 14);
    let retry_pixels =
        count_distinct_pixels(&buffer, PANEL_X + 12, 420, 180, 48, panel_surface, 14);

    assert!(
        headline_pixels >= 1100,
        "disconnected sftp panel should render a visible retry headline, only found {headline_pixels} distinct pixels"
    );
    assert!(
        body_pixels >= 2600,
        "disconnected sftp panel should render recovery guidance copy, only found {body_pixels} distinct pixels"
    );
    assert!(
        retry_pixels >= 900,
        "disconnected sftp panel should render a retry action shell, only found {retry_pixels} distinct pixels"
    );
}

use std::rc::Rc;
use std::time::{Duration, Instant};

use mica_term::AppWindow;
use mica_term::app::bootstrap::bind_top_status_bar_with_store;
use mica_term::shell::metrics::ShellMetrics;
use slint::platform::software_renderer::{MinimalSoftwareWindow, RepaintBufferType};
use slint::platform::{Platform, PlatformError, WindowAdapter};
use slint::{ComponentHandle, PhysicalSize, Rgb8Pixel, SharedPixelBuffer};

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

#[test]
fn titlebar_renders_visible_chrome_in_software_renderer() {
    let window = MinimalSoftwareWindow::new(RepaintBufferType::ReusedBuffer);
    slint::platform::set_platform(Box::new(SoftwareTestPlatform {
        window: window.clone(),
        started_at: Instant::now(),
    }))
    .unwrap();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);
    app.set_dark_mode(false);
    app.window().set_size(PhysicalSize::new(1440, 900));
    app.show().unwrap();

    assert_eq!(
        app.get_layout_titlebar_width() as u32,
        ShellMetrics::WINDOW_DEFAULT_WIDTH,
        "software renderer should receive the full titlebar width"
    );
    assert_eq!(
        app.get_layout_titlebar_content_width() as u32,
        ShellMetrics::WINDOW_DEFAULT_WIDTH - 12,
        "software renderer should lay out the titlebar content row"
    );

    let mut buffer = SharedPixelBuffer::<Rgb8Pixel>::new(1440, 900);
    let stride = buffer.width() as usize;
    assert!(window.draw_if_needed(|renderer| {
        renderer.render(buffer.make_mut_slice(), stride);
    }));

    let titlebar_background = pixel_at(&buffer, 720, 20);
    let body_background = pixel_at(&buffer, 720, 120);

    let nav_zone_pixels = count_distinct_pixels(&buffer, 8, 8, 36, 28, titlebar_background, 24);
    let brand_zone_pixels =
        count_distinct_pixels(&buffer, 48, 8, 180, 28, titlebar_background, 24);
    let window_control_pixels =
        count_distinct_pixels(&buffer, 1300, 8, 128, 28, titlebar_background, 24);

    eprintln!(
        "titlebar bg=({}, {}, {}), body bg=({}, {}, {}), nav={}, brand={}, controls={}",
        titlebar_background.r,
        titlebar_background.g,
        titlebar_background.b,
        body_background.r,
        body_background.g,
        body_background.b,
        nav_zone_pixels,
        brand_zone_pixels,
        window_control_pixels
    );

    assert!(
        color_distance(titlebar_background, body_background) >= 18,
        "titlebar background should differ from shell body, got top=({}, {}, {}) body=({}, {}, {})",
        titlebar_background.r,
        titlebar_background.g,
        titlebar_background.b,
        body_background.r,
        body_background.g,
        body_background.b
    );

    assert!(
        nav_zone_pixels >= 40,
        "navigation zone should render a visible icon, only found {nav_zone_pixels} distinct pixels"
    );
    assert!(
        brand_zone_pixels >= 180,
        "brand zone should render the header logotype, only found {brand_zone_pixels} distinct pixels"
    );
    assert!(
        window_control_pixels >= 60,
        "window controls should render visible icons, only found {window_control_pixels} distinct pixels"
    );
}

#[test]
fn shell_body_fills_to_window_bottom_in_software_renderer() {
    let window = MinimalSoftwareWindow::new(RepaintBufferType::ReusedBuffer);
    slint::platform::set_platform(Box::new(SoftwareTestPlatform {
        window: window.clone(),
        started_at: Instant::now(),
    }))
    .unwrap();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);
    app.set_dark_mode(false);
    app.window().set_size(PhysicalSize::new(1440, 900));
    app.show().unwrap();

    let mut buffer = SharedPixelBuffer::<Rgb8Pixel>::new(1440, 900);
    let stride = buffer.width() as usize;
    assert!(window.draw_if_needed(|renderer| {
        renderer.render(buffer.make_mut_slice(), stride);
    }));

    let upper_body = pixel_at(&buffer, 720, 120);
    let lower_body = pixel_at(&buffer, 720, 850);

    eprintln!(
        "upper body=({}, {}, {}), lower body=({}, {}, {})",
        upper_body.r,
        upper_body.g,
        upper_body.b,
        lower_body.r,
        lower_body.g,
        lower_body.b
    );

    assert!(
        color_distance(upper_body, lower_body) <= 2,
        "shell body should extend to the bottom of the window, got upper=({}, {}, {}) lower=({}, {}, {})",
        upper_body.r,
        upper_body.g,
        upper_body.b,
        lower_body.r,
        lower_body.g,
        lower_body.b
    );
}

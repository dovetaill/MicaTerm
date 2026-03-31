#![cfg(feature = "slint-renderer-software")]

use std::rc::Rc;
use std::time::{Duration, Instant};
use std::{fs, path::Path};

use mica_term::AppWindow;
use mica_term::app::bootstrap::bind_top_status_bar_with_store;
use slint::platform::software_renderer::{MinimalSoftwareWindow, RepaintBufferType};
use slint::platform::{Platform, PlatformError, WindowAdapter};
use slint::{ComponentHandle, PhysicalSize, Rgb8Pixel, SharedPixelBuffer};

const WINDOW_WIDTH: u32 = 1440;
const WINDOW_HEIGHT: u32 = 900;
const TITLEBAR_HEIGHT: u32 = 48;
const VIEWPORT_MARGIN: u32 = 24;

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

#[derive(Clone, Copy)]
struct ModalRect {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

fn blocking_modal_rect(modal_width: u32, modal_height: u32) -> ModalRect {
    blocking_modal_rect_for_viewport(WINDOW_WIDTH, WINDOW_HEIGHT, modal_width, modal_height)
}

fn blocking_modal_rect_for_viewport(
    window_width: u32,
    window_height: u32,
    modal_width: u32,
    modal_height: u32,
) -> ModalRect {
    let available_height = window_height - TITLEBAR_HEIGHT;
    let x = ((window_width - modal_width) / 2).max(VIEWPORT_MARGIN);
    let y = (TITLEBAR_HEIGHT + (available_height - modal_height) / 2)
        .max(TITLEBAR_HEIGHT + VIEWPORT_MARGIN);

    ModalRect {
        x,
        y,
        width: modal_width,
        height: modal_height,
    }
}

fn render_app(setup: impl FnOnce(&AppWindow)) -> SharedPixelBuffer<Rgb8Pixel> {
    render_app_with_size(WINDOW_WIDTH, WINDOW_HEIGHT, setup)
}

fn render_app_with_size(
    window_width: u32,
    window_height: u32,
    setup: impl FnOnce(&AppWindow),
) -> SharedPixelBuffer<Rgb8Pixel> {
    let window = MinimalSoftwareWindow::new(RepaintBufferType::ReusedBuffer);
    slint::platform::set_platform(Box::new(SoftwareTestPlatform {
        window: window.clone(),
        started_at: Instant::now(),
    }))
    .unwrap();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);
    app.set_dark_mode(false);
    app.window()
        .set_size(PhysicalSize::new(window_width, window_height));
    setup(&app);
    app.show().unwrap();

    let mut buffer = SharedPixelBuffer::<Rgb8Pixel>::new(window_width, window_height);
    let stride = buffer.width() as usize;
    assert!(window.draw_if_needed(|renderer| {
        renderer.render(buffer.make_mut_slice(), stride);
    }));
    buffer
}

fn write_ppm(buffer: &SharedPixelBuffer<Rgb8Pixel>, path: impl AsRef<Path>) {
    let path = path.as_ref();
    let mut bytes = format!("P6\n{} {}\n255\n", buffer.width(), buffer.height()).into_bytes();
    for pixel in buffer.as_slice() {
        bytes.push(pixel.r);
        bytes.push(pixel.g);
        bytes.push(pixel.b);
    }
    fs::write(path, bytes).expect("write ppm");
}

#[test]
fn new_folder_modal_renders_visible_footer_actions() {
    let modal = blocking_modal_rect(420, 230);
    let buffer = render_app(|app| {
        app.set_asset_modal_open(true);
        app.set_asset_modal_kind("new-folder".into());
        app.set_asset_folder_modal_name("Folder 1".into());
        app.set_asset_modal_can_confirm(true);
    });
    write_ppm(&buffer, "/tmp/new-folder-modal.ppm");

    let modal_surface = pixel_at(&buffer, modal.x + 10, modal.y + 10);
    let footer_pixels = count_distinct_pixels(
        &buffer,
        modal.x + 210,
        modal.y + modal.height - 70,
        190,
        54,
        modal_surface,
        18,
    );
    let footer_surface = pixel_at(&buffer, modal.x + 6, modal.y + modal.height - 20);
    let footer_panel_pixels = count_distinct_pixels(
        &buffer,
        modal.x + 28,
        modal.y + modal.height - 56,
        modal.width - 156,
        30,
        footer_surface,
        12,
    );

    assert!(
        footer_pixels >= 3000,
        "new folder modal footer action zone should render visible controls, only found {footer_pixels} distinct pixels"
    );
    assert!(
        footer_panel_pixels >= 1900,
        "new folder modal footer should keep an integrated action region, only found {footer_panel_pixels} distinct pixels"
    );
}

#[test]
fn new_ssh_modal_renders_footer_actions_and_balanced_top_row() {
    let modal = blocking_modal_rect(640, 560);
    let buffer = render_app(|app| {
        app.set_asset_modal_open(true);
        app.set_asset_modal_kind("new-ssh-connection".into());
        app.set_asset_ssh_modal_name("SSH Connection 1".into());
        app.set_asset_ssh_modal_host("10.0.0.12".into());
        app.set_asset_ssh_modal_user("ops".into());
        app.set_asset_ssh_modal_port("22".into());
        app.set_asset_ssh_modal_connect_family_enabled(true);
        app.set_asset_modal_can_confirm(true);
    });
    write_ppm(&buffer, "/tmp/new-ssh-modal.ppm");

    let modal_surface = pixel_at(&buffer, modal.x + 10, modal.y + 10);
    let footer_pixels = count_distinct_pixels(
        &buffer,
        modal.x + 80,
        modal.y + modal.height - 92,
        modal.width - 120,
        62,
        modal_surface,
        18,
    );
    let left_field_pixels = count_distinct_pixels(
        &buffer,
        modal.x + 24,
        modal.y + 168,
        420,
        54,
        modal_surface,
        14,
    );
    let right_field_pixels = count_distinct_pixels(
        &buffer,
        modal.x + 458,
        modal.y + 168,
        150,
        54,
        modal_surface,
        14,
    );
    let left_action_pixels = count_distinct_pixels(
        &buffer,
        modal.x + 18,
        modal.y + modal.height - 50,
        88,
        36,
        modal_surface,
        14,
    );
    let right_action_pixels = count_distinct_pixels(
        &buffer,
        modal.x + modal.width - 106,
        modal.y + modal.height - 50,
        88,
        36,
        modal_surface,
        14,
    );
    let footer_surface = pixel_at(&buffer, modal.x + 6, modal.y + modal.height - 22);
    let footer_panel_pixels = count_distinct_pixels(
        &buffer,
        modal.x + 28,
        modal.y + modal.height - 64,
        modal.width - 56,
        40,
        footer_surface,
        12,
    );

    assert!(
        footer_pixels >= 1500,
        "ssh modal footer action zone should render visible controls, only found {footer_pixels} distinct pixels"
    );
    assert!(
        left_action_pixels >= 250,
        "ssh modal left-side test action should render as a distinct button, only found {left_action_pixels} distinct pixels"
    );
    assert!(
        right_action_pixels >= 250,
        "ssh modal right-side save action should render as a distinct button, only found {right_action_pixels} distinct pixels"
    );
    assert!(
        footer_panel_pixels >= 5000,
        "ssh modal footer should keep an integrated action region, only found {footer_panel_pixels} distinct pixels"
    );
    assert!(
        right_field_pixels >= 120,
        "ssh modal top-right field should render with visible width, only found {right_field_pixels} distinct pixels"
    );
    assert!(
        left_field_pixels <= right_field_pixels * 6,
        "ssh modal leading field should not starve the sibling field; left={left_field_pixels}, right={right_field_pixels}"
    );
}

#[test]
fn sync_modal_renders_state_driven_content_and_footer_actions() {
    let modal = blocking_modal_rect(560, 360);
    let buffer = render_app(|app| {
        app.set_sync_modal_open(true);
        app.set_sync_modal_mode("not-configured".into());
        app.set_sync_modal_title("Sync".into());
        app.set_sync_modal_headline("Set up sync".into());
        app.set_sync_modal_status_text("Configure a Gitee remote to enable sync.".into());
        app.set_sync_modal_primary_action_label("Set up sync".into());
        app.set_sync_modal_secondary_action_label("Close".into());
    });
    write_ppm(&buffer, "/tmp/sync-vault-modal.ppm");

    let modal_surface = pixel_at(&buffer, modal.x + 10, modal.y + 10);
    let body_pixels = count_distinct_pixels(
        &buffer,
        modal.x + 28,
        modal.y + 72,
        modal.width - 56,
        modal.height - 156,
        modal_surface,
        14,
    );
    let footer_pixels = count_distinct_pixels(
        &buffer,
        modal.x + 24,
        modal.y + modal.height - 60,
        modal.width - 48,
        40,
        modal_surface,
        14,
    );

    assert!(
        body_pixels >= 2200,
        "sync modal body should render visible state-driven content, only found {body_pixels} distinct pixels"
    );
    assert!(
        footer_pixels >= 1100,
        "sync modal footer should render visible action controls, only found {footer_pixels} distinct pixels"
    );
}

#[test]
fn sync_modal_footer_stays_visible_in_short_viewport() {
    let short_height = 640;
    let modal_height = 528;
    let modal = blocking_modal_rect_for_viewport(WINDOW_WIDTH, short_height, 640, modal_height);
    let buffer = render_app_with_size(WINDOW_WIDTH, short_height, |app| {
        app.set_sync_modal_open(true);
        app.set_sync_modal_mode("not-configured".into());
        app.set_sync_modal_title("Sync".into());
        app.set_sync_modal_headline("Unlock sync".into());
        app.set_sync_modal_status_text(
            "Configure a Gitee remote and unlock the vault before automatic sync can run."
                .into(),
        );
        app.set_sync_modal_error_text(
            "Primary remote must contain a valid vault-head.json before unlock can continue."
                .into(),
        );
        app.set_sync_modal_primary_action_label("Unlock".into());
        app.set_sync_modal_secondary_action_label("Close".into());
        app.set_sync_modal_mirror_enabled(true);
    });

    let modal_surface = pixel_at(&buffer, modal.x + 12, modal.y + 12);
    let footer_pixels = count_distinct_pixels(
        &buffer,
        modal.x + 24,
        modal.y + modal.height - 68,
        modal.width - 48,
        44,
        modal_surface,
        14,
    );

    assert!(
        footer_pixels >= 1200,
        "sync modal footer should stay visible in short viewports, only found {footer_pixels} distinct pixels"
    );
}

#[test]
fn sync_modal_short_viewport_keeps_master_password_field_actionable() {
    let short_height = 640;
    let modal_height = 528;
    let modal = blocking_modal_rect_for_viewport(WINDOW_WIDTH, short_height, 640, modal_height);
    let footer_height = 82;
    let footer_top = modal.y + modal.height - footer_height;
    let buffer = render_app_with_size(WINDOW_WIDTH, short_height, |app| {
        app.set_sync_modal_open(true);
        app.set_sync_modal_mode("locked".into());
        app.set_sync_modal_title("Unlock sync".into());
        app.set_sync_modal_headline("Unlock sync".into());
        app.set_sync_modal_status_text(
            "Run sync automatically after local changes when the vault is unlocked.".into(),
        );
        app.set_sync_modal_provider_label("Gitee".into());
        app.set_sync_modal_target_label("1 target configured".into());
        app.set_sync_modal_primary_gist_id("yemd6wft9jgpv5bhlux3o60".into());
        app.set_sync_modal_primary_pat("token-value".into());
        app.set_sync_modal_primary_action_label("Unlock".into());
        app.set_sync_modal_secondary_action_label("Close".into());
    });
    write_ppm(&buffer, "/tmp/sync-vault-short-modal.ppm");

    let body_surface = pixel_at(&buffer, modal.x + 36, modal.y + 124);
    let master_password_field_pixels = count_distinct_pixels(
        &buffer,
        modal.x + 36,
        footer_top - 42,
        modal.width - 92,
        28,
        body_surface,
        10,
    );

    assert!(
        master_password_field_pixels >= 2600,
        "sync modal should keep the master password field visibly above the sticky footer in short viewports, only found {master_password_field_pixels} distinct pixels"
    );
}

#[test]
fn blocking_modal_tints_workspace_behind_the_dialog() {
    let short_height = 640;
    let modal_height = 528;
    let modal = blocking_modal_rect_for_viewport(WINDOW_WIDTH, short_height, 640, modal_height);
    let sample_x = modal.x + modal.width + 64;
    let sample_y = modal.y + 180;
    let workspace_surface = Rgb8Pixel {
        r: 255,
        g: 255,
        b: 255,
    };

    let modal_buffer = render_app_with_size(WINDOW_WIDTH, short_height, |app| {
        app.set_sync_modal_open(true);
        app.set_sync_modal_mode("locked".into());
        app.set_sync_modal_title("Unlock sync".into());
        app.set_sync_modal_headline("Unlock sync".into());
        app.set_sync_modal_status_text(
            "Run sync automatically after local changes when the vault is unlocked.".into(),
        );
        app.set_sync_modal_provider_label("Gitee".into());
        app.set_sync_modal_target_label("1 target configured".into());
        app.set_sync_modal_primary_gist_id("yemd6wft9jgpv5bhlux3o60".into());
        app.set_sync_modal_primary_pat("token-value".into());
        app.set_sync_modal_primary_action_label("Unlock".into());
        app.set_sync_modal_secondary_action_label("Close".into());
    });
    let tinted_pixel = pixel_at(&modal_buffer, sample_x, sample_y);

    assert!(
        color_distance(workspace_surface, tinted_pixel) >= 24,
        "blocking modal should tint the workspace behind the dialog; expected the workspace pixel to differ from pure white, got {tinted_pixel:?}"
    );
}

#[test]
fn ssh_modal_footer_stays_visible_in_short_viewport() {
    let short_height = 640;
    let modal_height = 528;
    let modal = blocking_modal_rect_for_viewport(WINDOW_WIDTH, short_height, 640, modal_height);
    let buffer = render_app_with_size(WINDOW_WIDTH, short_height, |app| {
        app.set_asset_modal_open(true);
        app.set_asset_modal_kind("new-ssh-connection".into());
        app.set_asset_ssh_modal_name("SSH Connection 1".into());
        app.set_asset_ssh_modal_host("10.0.0.12".into());
        app.set_asset_ssh_modal_user("ops".into());
        app.set_asset_ssh_modal_port("22".into());
        app.set_asset_ssh_modal_remark(
            "This is a long SSH note to prove the body scrolls while the footer remains reachable."
                .into(),
        );
        app.set_asset_ssh_modal_feedback_state("busy".into());
        app.set_asset_ssh_modal_feedback_message("Testing connection...".into());
        app.set_asset_ssh_modal_connect_family_enabled(true);
        app.set_asset_modal_can_confirm(true);
    });

    let modal_surface = pixel_at(&buffer, modal.x + 12, modal.y + 12);
    let footer_pixels = count_distinct_pixels(
        &buffer,
        modal.x + 18,
        modal.y + modal.height - 72,
        modal.width - 36,
        48,
        modal_surface,
        14,
    );

    assert!(
        footer_pixels >= 1300,
        "ssh modal footer should stay visible in short viewports, only found {footer_pixels} distinct pixels"
    );
}

#[test]
fn ssh_modal_short_viewport_keeps_primary_auth_field_actionable() {
    let short_height = 640;
    let modal_height = 528;
    let modal = blocking_modal_rect_for_viewport(WINDOW_WIDTH, short_height, 640, modal_height);
    let footer_height = 84;
    let footer_top = modal.y + modal.height - footer_height;
    let buffer = render_app_with_size(WINDOW_WIDTH, short_height, |app| {
        app.set_asset_modal_open(true);
        app.set_asset_modal_kind("new-ssh-connection".into());
        app.set_asset_ssh_modal_name("Sharon".into());
        app.set_asset_ssh_modal_host("157.254.53.77".into());
        app.set_asset_ssh_modal_user("root".into());
        app.set_asset_ssh_modal_port("57722".into());
        app.set_asset_ssh_modal_auth_source("manual".into());
        app.set_asset_ssh_modal_auth_method("password".into());
        app.set_asset_ssh_modal_connect_family_enabled(true);
        app.set_asset_modal_can_confirm(true);
    });
    write_ppm(&buffer, "/tmp/new-ssh-short-modal.ppm");

    let body_surface = pixel_at(&buffer, modal.x + 36, modal.y + 140);
    let password_field_pixels = count_distinct_pixels(
        &buffer,
        modal.x + 36,
        footer_top - 48,
        modal.width - 92,
        32,
        body_surface,
        10,
    );

    assert!(
        password_field_pixels >= 2400,
        "ssh modal should keep the first authentication field visibly above the sticky footer in short viewports, only found {password_field_pixels} distinct pixels"
    );
}

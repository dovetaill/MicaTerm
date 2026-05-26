#![cfg(feature = "slint-renderer-software")]

use std::rc::Rc;
use std::time::{Duration, Instant};
use std::{fs, path::Path};

use mica_term::AppWindow;
use mica_term::app::bootstrap::bind_top_status_bar_with_store;
use slint::platform::software_renderer::{MinimalSoftwareWindow, RepaintBufferType};
use slint::platform::{Platform, PlatformError, WindowAdapter, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, PhysicalSize, Rgb8Pixel, SharedPixelBuffer};

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

fn count_changed_pixels(
    before: &SharedPixelBuffer<Rgb8Pixel>,
    after: &SharedPixelBuffer<Rgb8Pixel>,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    threshold: u16,
) -> usize {
    let mut changed = 0usize;

    for sample_y in y..(y + height) {
        for sample_x in x..(x + width) {
            if color_distance(
                pixel_at(before, sample_x, sample_y),
                pixel_at(after, sample_x, sample_y),
            ) >= threshold
            {
                changed += 1;
            }
        }
    }

    changed
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
    let viewport_margin = if window_width < 960 || window_height - TITLEBAR_HEIGHT < 720 {
        8
    } else {
        VIEWPORT_MARGIN
    };
    let available_width = (window_width - (viewport_margin * 2)).max(280);
    let available_height = (window_height - TITLEBAR_HEIGHT - (viewport_margin * 2)).max(220);
    let resolved_width = modal_width.min(available_width);
    let resolved_height =
        if available_height > modal_height && available_height - modal_height <= 40 {
            available_height
        } else {
            modal_height.min(available_height)
        };
    let x = ((window_width - resolved_width) / 2)
        .min(window_width - viewport_margin - resolved_width)
        .max(viewport_margin);
    let y = (TITLEBAR_HEIGHT + ((window_height - TITLEBAR_HEIGHT - resolved_height) / 2))
        .min(window_height - viewport_margin - resolved_height)
        .max(TITLEBAR_HEIGHT + viewport_margin);

    ModalRect {
        x,
        y,
        width: resolved_width,
        height: resolved_height,
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
    render_app_with_size_and_after_show(window_width, window_height, setup, |_| {})
}

fn render_app_with_size_and_after_show(
    window_width: u32,
    window_height: u32,
    setup: impl FnOnce(&AppWindow),
    after_show: impl FnOnce(&AppWindow),
) -> SharedPixelBuffer<Rgb8Pixel> {
    render_app_with_size_and_theme_and_after_show(
        window_width,
        window_height,
        false,
        setup,
        after_show,
    )
}

fn render_app_with_size_and_theme_and_after_show(
    window_width: u32,
    window_height: u32,
    dark_mode: bool,
    setup: impl FnOnce(&AppWindow),
    after_show: impl FnOnce(&AppWindow),
) -> SharedPixelBuffer<Rgb8Pixel> {
    let window = MinimalSoftwareWindow::new(RepaintBufferType::ReusedBuffer);
    slint::platform::set_platform(Box::new(SoftwareTestPlatform {
        window: window.clone(),
        started_at: Instant::now(),
    }))
    .unwrap();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);
    app.set_dark_mode(dark_mode);
    app.window()
        .set_size(PhysicalSize::new(window_width, window_height));
    setup(&app);
    app.show().unwrap();
    after_show(&app);
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
fn rename_modal_renders_simple_name_field_and_footer_actions() {
    let modal = blocking_modal_rect(420, 230);
    let buffer = render_app(|app| {
        app.set_asset_rename_modal_open(true);
        app.set_asset_rename_modal_name("Prod Bastion".into());
        app.set_asset_rename_modal_can_confirm(true);
    });

    let modal_surface = pixel_at(&buffer, modal.x + 10, modal.y + 10);
    let body_field_pixels = count_distinct_pixels(
        &buffer,
        modal.x + 24,
        modal.y + 78,
        modal.width - 48,
        88,
        modal_surface,
        14,
    );
    let footer_surface = pixel_at(&buffer, modal.x + 6, modal.y + modal.height - 20);
    let footer_panel_pixels = count_distinct_pixels(
        &buffer,
        modal.x + 24,
        modal.y + modal.height - 56,
        modal.width - 48,
        30,
        footer_surface,
        12,
    );

    assert!(
        body_field_pixels >= 1200,
        "rename modal should render a visible simple name field without the old heavy section card, only found {body_field_pixels} distinct pixels"
    );
    assert!(
        footer_panel_pixels >= 1000,
        "rename modal footer should render a visible shared action rail, only found {footer_panel_pixels} distinct pixels"
    );
}

#[test]
fn delete_confirm_modal_renders_structured_warning_and_destructive_footer() {
    let modal = blocking_modal_rect(440, 268);
    let buffer = render_app(|app| {
        app.set_asset_delete_confirm_modal_open(true);
        app.set_asset_delete_confirm_target_label("Prod Bastion".into());
        app.set_asset_delete_confirm_descendant_count(3);
    });

    let modal_surface = pixel_at(&buffer, modal.x + 10, modal.y + 10);
    let warning_card_pixels = count_distinct_pixels(
        &buffer,
        modal.x + 24,
        modal.y + 84,
        modal.width - 48,
        118,
        modal_surface,
        14,
    );
    let footer_surface = pixel_at(&buffer, modal.x + 6, modal.y + modal.height - 20);
    let footer_panel_pixels = count_distinct_pixels(
        &buffer,
        modal.x + 24,
        modal.y + modal.height - 56,
        modal.width - 48,
        30,
        footer_surface,
        12,
    );

    assert!(
        warning_card_pixels >= 2600,
        "delete confirm modal should render a structured warning card instead of flat body copy, only found {warning_card_pixels} distinct pixels"
    );
    assert!(
        footer_panel_pixels >= 1100,
        "delete confirm modal footer should render a visible destructive action rail, only found {footer_panel_pixels} distinct pixels"
    );
}

#[test]
fn ssh_host_key_modal_renders_verification_card_and_action_row() {
    let modal = blocking_modal_rect(420, 332);
    let buffer = render_app(|app| {
        app.set_ssh_host_key_modal_open(true);
        app.set_ssh_host_key_modal_host("db.example.com".into());
        app.set_ssh_host_key_modal_fingerprint(
            "SHA256:X2j2f3Ag9rQm0tJ4wQ4Jw7WiyR5QH1O3m2Q2n4v6QzA".into(),
        );
    });

    let modal_surface = pixel_at(&buffer, modal.x + 10, modal.y + 10);
    let verification_card_pixels = count_distinct_pixels(
        &buffer,
        modal.x + 24,
        modal.y + 86,
        modal.width - 48,
        164,
        modal_surface,
        14,
    );
    let footer_surface = pixel_at(&buffer, modal.x + 6, modal.y + modal.height - 20);
    let footer_panel_pixels = count_distinct_pixels(
        &buffer,
        modal.x + 24,
        modal.y + modal.height - 56,
        modal.width - 48,
        30,
        footer_surface,
        12,
    );

    assert!(
        verification_card_pixels >= 3200,
        "host-key confirm modal should render a visible structured verification surface, only found {verification_card_pixels} distinct pixels"
    );
    assert!(
        footer_panel_pixels >= 1000,
        "host-key confirm modal should render a shared action row for reject/accept, only found {footer_panel_pixels} distinct pixels"
    );
}

#[test]
fn sftp_remote_file_modal_renders_editor_surface_status_and_footer_actions() {
    let modal = blocking_modal_rect(860, 620);
    let buffer = render_app(|app| {
        app.set_sftp_remote_file_modal_open(true);
        app.set_sftp_remote_file_modal_title("Edit Remote File".into());
        app.set_sftp_remote_file_modal_path("/srv/app/.env".into());
        app.set_sftp_remote_file_modal_content("APP_ENV=production\nAPI_PORT=443\n".into());
        app.set_sftp_remote_file_modal_status_text("Remote file loaded".into());
        app.set_sftp_remote_file_modal_can_save(true);
    });

    let modal_surface = pixel_at(&buffer, modal.x + 10, modal.y + 10);
    let editor_surface_pixels = count_distinct_pixels(
        &buffer,
        modal.x + 28,
        modal.y + 128,
        modal.width - 56,
        modal.height - 236,
        modal_surface,
        14,
    );
    let footer_surface = pixel_at(&buffer, modal.x + 6, modal.y + modal.height - 20);
    let footer_panel_pixels = count_distinct_pixels(
        &buffer,
        modal.x + 24,
        modal.y + modal.height - 58,
        modal.width - 48,
        32,
        footer_surface,
        12,
    );

    assert!(
        editor_surface_pixels >= 12000,
        "remote-file modal should render a visible elevated editor work surface, only found {editor_surface_pixels} distinct pixels"
    );
    assert!(
        footer_panel_pixels >= 2500,
        "remote-file modal footer should render a visible shared action rail, only found {footer_panel_pixels} distinct pixels"
    );
}

#[test]
fn migrated_remaining_modals_preserve_distinct_light_and_dark_shells() {
    let modal = blocking_modal_rect(860, 620);
    let window = MinimalSoftwareWindow::new(RepaintBufferType::ReusedBuffer);
    slint::platform::set_platform(Box::new(SoftwareTestPlatform {
        window: window.clone(),
        started_at: Instant::now(),
    }))
    .unwrap();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);
    app.window()
        .set_size(PhysicalSize::new(WINDOW_WIDTH, WINDOW_HEIGHT));
    app.set_dark_mode(false);
    app.set_sftp_remote_file_modal_open(true);
    app.set_sftp_remote_file_modal_title("Edit Remote File".into());
    app.set_sftp_remote_file_modal_path("/srv/app/.env".into());
    app.set_sftp_remote_file_modal_content("APP_ENV=production\nAPI_PORT=443\n".into());
    app.set_sftp_remote_file_modal_status_text("Remote file loaded".into());
    app.set_sftp_remote_file_modal_can_save(true);
    app.show().unwrap();

    let mut light = SharedPixelBuffer::<Rgb8Pixel>::new(WINDOW_WIDTH, WINDOW_HEIGHT);
    let light_stride = light.width() as usize;
    assert!(window.draw_if_needed(|renderer| {
        renderer.render(light.make_mut_slice(), light_stride);
    }));

    app.set_dark_mode(true);
    slint::platform::update_timers_and_animations();

    let mut dark = SharedPixelBuffer::<Rgb8Pixel>::new(WINDOW_WIDTH, WINDOW_HEIGHT);
    let dark_stride = dark.width() as usize;
    assert!(window.draw_if_needed(|renderer| {
        renderer.render(dark.make_mut_slice(), dark_stride);
    }));

    let shell_delta = count_changed_pixels(
        &light,
        &dark,
        modal.x + 4,
        modal.y + 4,
        modal.width - 8,
        modal.height - 8,
        18,
    );

    assert!(
        shell_delta >= 25000,
        "migrated remaining dialogs should preserve a clearly distinct shell between light and dark themes, only found {shell_delta} changed pixels"
    );
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
        footer_panel_pixels >= 300,
        "new folder modal footer should keep an integrated action region, only found {footer_panel_pixels} distinct pixels"
    );
}

#[test]
fn new_ssh_modal_renders_footer_actions_and_balanced_top_row() {
    let modal = blocking_modal_rect(640, 720);
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
        modal.x + 28,
        modal.y + 170,
        250,
        48,
        modal_surface,
        14,
    );
    let right_field_pixels = count_distinct_pixels(
        &buffer,
        modal.x + 300,
        modal.y + 170,
        250,
        48,
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
        footer_panel_pixels >= 2500,
        "ssh modal footer should keep an integrated action region, only found {footer_panel_pixels} distinct pixels"
    );
    assert!(
        right_field_pixels >= 600,
        "ssh modal top-right field should render with visible width, only found {right_field_pixels} distinct pixels"
    );
    assert!(
        left_field_pixels <= right_field_pixels * 2,
        "ssh modal leading field should not starve the sibling field; left={left_field_pixels}, right={right_field_pixels}"
    );
}

#[test]
fn sync_modal_renders_state_driven_content_and_footer_actions() {
    let modal = blocking_modal_rect(640, 680);
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
fn sync_modal_renders_sync_status_card_with_timestamps() {
    let modal = blocking_modal_rect(640, 680);
    let buffer = render_app(|app| {
        app.set_sync_modal_open(true);
        app.set_sync_modal_mode("ready".into());
        app.set_sync_modal_title("Sync Settings".into());
        app.set_sync_modal_headline("Sync ready".into());
        app.set_sync_modal_status_text(
            "Sync is configured. Use the titlebar Sync button to run an immediate check.".into(),
        );
        app.set_sync_modal_provider_label("Gitee".into());
        app.set_sync_modal_target_label("1 Git primary configured".into());
        app.set_sync_modal_local_last_sync_text("2026-04-02 10:30".into());
        app.set_sync_modal_remote_last_update_text("2026-04-02 10:31".into());
        app.set_sync_modal_primary_revision_text("rev-0042".into());
        app.set_sync_modal_remote_status_text("Primary remote is currently at rev-0042.".into());
        app.set_sync_modal_git_remote_url("https://example.com/org/mica-term.git".into());
        app.set_sync_modal_primary_action_label("Sync now".into());
        app.set_sync_modal_secondary_action_label("Close".into());
    });

    let modal_surface = pixel_at(&buffer, modal.x + 10, modal.y + 10);
    let status_card_pixels = count_distinct_pixels(
        &buffer,
        modal.x + 28,
        modal.y + 150,
        modal.width - 56,
        120,
        modal_surface,
        14,
    );

    assert!(
        status_card_pixels >= 1800,
        "sync modal status card should render visible timestamp content, only found {status_card_pixels} distinct pixels"
    );
}

#[test]
fn sync_modal_footer_stays_visible_in_short_viewport() {
    let short_height = 640;
    let modal = blocking_modal_rect_for_viewport(WINDOW_WIDTH, short_height, 640, 680);
    let buffer = render_app_with_size(WINDOW_WIDTH, short_height, |app| {
        app.set_sync_modal_open(true);
        app.set_sync_modal_mode("not-configured".into());
        app.set_sync_modal_title("Sync Settings".into());
        app.set_sync_modal_headline("Configure sync".into());
        app.set_sync_modal_status_text(
            "Configure a sync target once. After that, background sync runs automatically.".into(),
        );
        app.set_sync_modal_error_text(
            "Primary remote is missing the required configuration.".into(),
        );
        app.set_sync_modal_primary_action_label("Save and enable".into());
        app.set_sync_modal_secondary_action_label("Close".into());
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
    let source = fs::read_to_string("ui/components/sync-vault-modal.slint").unwrap();

    assert!(
        source.contains(
            "trailing-icon-source: root.master-password-visible ? root.eye-off-icon : root.eye-icon;"
        ),
        "sync modal should expose a Fluent eye toggle for the master password field"
    );
    assert!(
        source.contains(
            "trailing-icon-source: root.git-https-secret-visible ? root.eye-off-icon : root.eye-icon;"
        ),
        "sync modal should expose a Fluent eye toggle for the HTTPS secret field"
    );
    assert!(
        source.contains(
            "trailing-icon-source: root.git-ssh-passphrase-visible ? root.eye-off-icon : root.eye-icon;"
        ),
        "sync modal should expose a Fluent eye toggle for the SSH passphrase field"
    );

    let short_height = 640;
    let modal = blocking_modal_rect_for_viewport(WINDOW_WIDTH, short_height, 640, 680);
    let buffer = render_app_with_size(WINDOW_WIDTH, short_height, |app| {
        app.set_sync_modal_open(true);
        app.set_sync_modal_mode("not-configured".into());
        app.set_sync_modal_title("Sync Settings".into());
        app.set_sync_modal_headline("Configure sync".into());
        app.set_sync_modal_status_text(
            "Enter a master password once to enable sync and keep recovery state available.".into(),
        );
        app.set_sync_modal_git_remote_url("https://example.com/org/mica-term.git".into());
        app.set_sync_modal_primary_action_label("Save and enable".into());
        app.set_sync_modal_secondary_action_label("Close".into());
    });
    write_ppm(&buffer, "/tmp/sync-vault-short-modal.ppm");

    let body_surface = pixel_at(&buffer, modal.x + 36, modal.y + 124);
    // The sticky footer stays pinned while provider/repository controls remain visible below the
    // bootstrap field, so sample the actual rendered password band instead of a footer-adjacent
    // strip that can miss the control even when it is still clearly actionable on screen.
    let field_band_top = modal.y + 300;
    let master_password_field_pixels = count_distinct_pixels(
        &buffer,
        modal.x + 36,
        field_band_top,
        modal.width - 92,
        40,
        body_surface,
        10,
    );

    assert!(
        master_password_field_pixels >= 1500,
        "sync modal should keep the master password field visibly above the sticky footer in short viewports, only found {master_password_field_pixels} distinct pixels"
    );
}

#[test]
fn blocking_modal_tints_workspace_behind_the_dialog() {
    let short_height = 640;
    let modal = blocking_modal_rect_for_viewport(WINDOW_WIDTH, short_height, 640, 680);
    let sample_x = modal.x + modal.width + 64;
    let sample_y = modal.y + 180;
    let workspace_surface = Rgb8Pixel {
        r: 255,
        g: 255,
        b: 255,
    };

    let modal_buffer = render_app_with_size(WINDOW_WIDTH, short_height, |app| {
        app.set_sync_modal_open(true);
        app.set_sync_modal_mode("ready".into());
        app.set_sync_modal_title("Sync Settings".into());
        app.set_sync_modal_headline("Sync ready".into());
        app.set_sync_modal_status_text(
            "Sync is configured. Use the titlebar Sync button to run an immediate check.".into(),
        );
        app.set_sync_modal_provider_label("Git".into());
        app.set_sync_modal_target_label("1 target configured".into());
        app.set_sync_modal_git_remote_url("https://example.com/org/mica-term.git".into());
        app.set_sync_modal_git_https_username("git".into());
        app.set_sync_modal_git_https_secret("token-value".into());
        app.set_sync_modal_primary_action_label("Sync now".into());
        app.set_sync_modal_secondary_action_label("Close".into());
    });
    let tinted_pixel = pixel_at(&modal_buffer, sample_x, sample_y);

    assert!(
        color_distance(workspace_surface, tinted_pixel) >= 24,
        "blocking modal should tint the workspace behind the dialog; expected the workspace pixel to differ from pure white, got {tinted_pixel:?}"
    );
}

#[test]
fn sync_modal_narrow_viewport_preserves_right_gutter_for_form_fields() {
    let buffer = render_app_with_size(663, 744, |app| {
        app.set_sync_modal_open(true);
        app.set_sync_modal_mode("ready".into());
        app.set_sync_modal_title("Sync Settings".into());
        app.set_sync_modal_headline("Sync ready".into());
        app.set_sync_modal_status_text(
            "Sync is configured. Use the titlebar Sync action to run an immediate check.".into(),
        );
        app.set_sync_modal_provider_label("Git".into());
        app.set_sync_modal_target_label("1 target configured".into());
        app.set_sync_modal_git_remote_url("https://example.com/org/mica-term.git".into());
        app.set_sync_modal_git_https_username("git".into());
        app.set_sync_modal_git_https_secret("token-value".into());
        app.set_sync_modal_primary_action_label("Sync now".into());
        app.set_sync_modal_secondary_action_label("Close".into());
    });

    let field_border = pixel_at(&buffer, 620, 420);
    let right_gutter_pixels = count_distinct_pixels(&buffer, 639, 420, 9, 4, field_border, 10);

    assert!(
        right_gutter_pixels >= 24,
        "sync modal should preserve a visible right gutter inside the body panel in narrow viewports, only found {right_gutter_pixels} distinct pixels"
    );
}

#[test]
fn sync_modal_source_no_longer_advertises_lock_unlock_or_auto_sync_copy() {
    let source = fs::read_to_string("ui/components/sync-vault-modal.slint").unwrap();

    assert!(!source.contains("label: \"Auto sync\""));
    assert!(!source.contains("auto-sync-enabled"));
    assert!(!source.contains("mode == \"locked\""));
    assert!(!source.contains("Unlock"));
    assert!(!source.contains("Lock"));
}

#[test]
fn ssh_modal_narrow_viewport_preserves_right_gutter_after_trailing_action() {
    let chrome = fs::read_to_string("ui/components/modal-chrome.slint").unwrap();

    assert!(
        chrome.contains("root.trailing-icon-visible ? 36px : 0px")
            && chrome.contains("x: parent.width - self.width - 6px;"),
        "shared dialog fields should reserve a stable trailing icon slot with a 6px gutter for reveal actions in narrow SSH layouts"
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
    let source = fs::read_to_string("ui/components/assets-ssh-connection-modal.slint").unwrap();

    assert!(
        source.contains("DialogSelectField"),
        "ssh modal should switch its modal select triggers to the shared dialog select primitive"
    );
    assert!(
        !source.contains("ComboBox {"),
        "ssh modal should not keep stock ComboBox popups inside modal content"
    );

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
        password_field_pixels >= 1200,
        "ssh modal should keep the first authentication field visibly above the sticky footer in short viewports, only found {password_field_pixels} distinct pixels"
    );
}

#[test]
fn snippet_modal_source_uses_dialog_select_field_for_package_picker() {
    let source = fs::read_to_string("ui/components/assets-snippet-modal.slint").unwrap();

    assert!(
        source.contains("DialogSelectField"),
        "snippet modal should switch its package picker to the shared dialog select primitive"
    );
    assert!(
        !source.contains("ComboBox {"),
        "snippet modal should not keep stock ComboBox popups inside modal content"
    );
}

#[test]
fn sftp_conflict_modal_renders_info_cards_scope_card_and_footer_actions() {
    let modal = blocking_modal_rect(520, 396);
    let buffer = render_app(|app| {
        app.set_sftp_conflict_modal_open(true);
        app.set_sftp_conflict_modal_source_path("/tmp/releases/release-a.tar.gz".into());
        app.set_sftp_conflict_modal_target_path("/srv/app/releases/release-a.tar.gz".into());
        app.set_sftp_conflict_modal_batch_conflict_count(2);
        app.set_sftp_conflict_modal_apply_to_batch(true);
    });

    let modal_surface = pixel_at(&buffer, modal.x + 10, modal.y + 10);
    let incoming_card_pixels = count_distinct_pixels(
        &buffer,
        modal.x + 20,
        modal.y + 140,
        modal.width - 40,
        54,
        modal_surface,
        14,
    );
    let target_card_pixels = count_distinct_pixels(
        &buffer,
        modal.x + 20,
        modal.y + 204,
        modal.width - 40,
        54,
        modal_surface,
        14,
    );
    let scope_card_pixels = count_distinct_pixels(
        &buffer,
        modal.x + 20,
        modal.y + 268,
        modal.width - 40,
        92,
        modal_surface,
        14,
    );
    let footer_pixels = count_distinct_pixels(
        &buffer,
        modal.x + modal.width - 312,
        modal.y + modal.height - 56,
        292,
        40,
        modal_surface,
        14,
    );

    assert!(
        incoming_card_pixels >= 1800,
        "conflict modal should render a visible incoming-item card, only found {incoming_card_pixels} distinct pixels"
    );
    assert!(
        target_card_pixels >= 1800,
        "conflict modal should render a visible existing-target card, only found {target_card_pixels} distinct pixels"
    );
    assert!(
        scope_card_pixels >= 2600,
        "conflict modal should render the destination scope card and checkbox affordance, only found {scope_card_pixels} distinct pixels"
    );
    assert!(
        footer_pixels >= 1400,
        "conflict modal footer should keep the cancel/skip/replace action cluster visible, only found {footer_pixels} distinct pixels"
    );
}

#[test]
fn sftp_conflict_modal_close_tooltip_renders_when_close_affordance_is_hovered() {
    let modal = blocking_modal_rect(520, 396);
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
        .set_size(PhysicalSize::new(WINDOW_WIDTH, WINDOW_HEIGHT));
    app.set_sftp_conflict_modal_open(true);
    app.set_sftp_conflict_modal_source_path("/tmp/releases/release-a.tar.gz".into());
    app.set_sftp_conflict_modal_target_path("/srv/app/releases/release-a.tar.gz".into());
    app.set_sftp_conflict_modal_batch_conflict_count(1);
    app.show().unwrap();

    let mut base = SharedPixelBuffer::<Rgb8Pixel>::new(WINDOW_WIDTH, WINDOW_HEIGHT);
    let stride = base.width() as usize;
    assert!(window.draw_if_needed(|renderer| {
        renderer.render(base.make_mut_slice(), stride);
    }));

    let close_position =
        LogicalPosition::new((modal.x + modal.width - 26) as f32, (modal.y + 26) as f32);
    app.window().dispatch_event(WindowEvent::PointerMoved {
        position: close_position,
    });
    std::thread::sleep(Duration::from_millis(320));
    slint::platform::update_timers_and_animations();

    let mut hovered = SharedPixelBuffer::<Rgb8Pixel>::new(WINDOW_WIDTH, WINDOW_HEIGHT);
    let hovered_stride = hovered.width() as usize;
    assert!(window.draw_if_needed(|renderer| {
        renderer.render(hovered.make_mut_slice(), hovered_stride);
    }));

    let tooltip_delta = count_changed_pixels(
        &base,
        &hovered,
        modal.x + modal.width - 162,
        modal.y + 36,
        144,
        42,
        12,
    );

    assert!(
        tooltip_delta >= 1200,
        "hovering the conflict-modal close affordance should render a visible tooltip pill, only found {tooltip_delta} changed pixels"
    );
}

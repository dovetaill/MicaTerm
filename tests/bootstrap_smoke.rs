use mica_term::app::bootstrap::{app_title, default_window_size};

#[test]
fn bootstrap_exposes_app_title_and_default_window_size() {
    assert_eq!(app_title(), "Mica Term");
    assert_eq!(default_window_size(), (1440, 900));
}

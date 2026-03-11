use mica_term::app::bootstrap::{app_title, default_window_size};
use mica_term::shell::metrics::ShellMetrics;

#[test]
fn bootstrap_exposes_shell_default_window_budget() {
    assert_eq!(app_title(), "Mica Term");
    assert_eq!(
        default_window_size(),
        (
            ShellMetrics::WINDOW_DEFAULT_WIDTH,
            ShellMetrics::WINDOW_DEFAULT_HEIGHT,
        )
    );
}

//! Panic-hook coverage for crash record creation and startup failure messaging.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use mica_term::app::bootstrap::startup_failure_message;
use mica_term::app::logging::panic::install_panic_hook;
use mica_term::app::runtime_profile::AppRuntimeProfile;

#[test]
fn panic_hook_writes_crash_file_for_child_process() {
    if std::env::var("MICA_TERM_PANIC_CHILD").ok().as_deref() == Some("1") {
        let crash_dir = PathBuf::from(std::env::var("MICA_TERM_CRASH_DIR").unwrap());
        install_panic_hook(crash_dir).unwrap();
        panic!("panic hook smoke");
    }

    let temp_root = std::env::temp_dir()
        .join("mica-term")
        .join("tests")
        .join("panic-hook");
    let _ = fs::remove_dir_all(&temp_root);
    fs::create_dir_all(temp_root.join("crash")).unwrap();

    let output = Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg("panic_hook_writes_crash_file_for_child_process")
        .env("MICA_TERM_PANIC_CHILD", "1")
        .env("MICA_TERM_CRASH_DIR", temp_root.join("crash"))
        .output()
        .unwrap();

    assert!(!output.status.success());
    let combined_output = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        combined_output.contains("panic hook smoke"),
        "child panic run should still crash for real while the parent test keeps that noise out of the main suite output"
    );

    let crash_file = fs::read_dir(temp_root.join("crash"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let content = fs::read_to_string(crash_file).unwrap();
    assert!(content.contains("panic hook smoke"));
    assert!(content.contains("thread="));
    assert!(content.contains("backtrace="));
}

#[test]
fn startup_failure_message_uses_selected_renderer_label() {
    let message = startup_failure_message(AppRuntimeProfile::mainline(), "mock init failure")
        .expect("mainline profile should expose a startup message");

    assert!(message.contains("Mica Term"));
    assert!(message.contains("winit-skia"));
    assert!(message.contains("mock init failure"));
}

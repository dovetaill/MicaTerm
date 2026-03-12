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

    let status = Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg("panic_hook_writes_crash_file_for_child_process")
        .env("MICA_TERM_PANIC_CHILD", "1")
        .env("MICA_TERM_CRASH_DIR", temp_root.join("crash"))
        .status()
        .unwrap();

    assert!(!status.success());

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
fn startup_failure_message_is_absent_for_formal_profile() {
    assert_eq!(startup_failure_message(AppRuntimeProfile::formal(), "mock init failure"), None);
}

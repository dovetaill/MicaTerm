//! Standalone as well as Cargo-testable: no UI initialization required.
#[path = "../src/app/logging/lifecycle.rs"]
mod lifecycle;

#[test]
fn journal_survives_exit_without_destructors() {
    const CHILD: &str = "MICA_LIFECYCLE_TEST_DIR";
    if let Some(dir) = std::env::var_os(CHILD) {
        lifecycle::install(std::path::Path::new(&dir)).unwrap();
        lifecycle::record("event-loop-enter");
        // Deliberately bypass destructors, just like abrupt process termination.
        std::process::exit(23);
    }
    let dir = std::env::temp_dir().join(format!("mica-lifecycle-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "journal_survives_exit_without_destructors"])
        .env(CHILD, &dir)
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(23));
    let path = std::fs::read_dir(&dir)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let text = std::fs::read_to_string(path).unwrap();
    assert!(text.contains("event=process-start"));
    assert!(text.contains("event=event-loop-enter"));
    assert!(!text.contains("shutdown-complete"));
    std::fs::remove_dir_all(dir).unwrap();
}

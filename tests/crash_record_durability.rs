#[path = "../src/app/logging/panic.rs"]
mod panic_logging;

#[test]
fn fatal_and_panic_records_include_context() {
    let dir = std::env::temp_dir().join(format!("mica-crash-record-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = panic_logging::write_fatal_record(&dir, "renderer-select", "outer: inner").unwrap();
    let text = std::fs::read_to_string(path).unwrap();
    assert!(text.contains("phase=renderer-select\nerror=outer: inner"));
    assert!(text.contains("pid="));
    assert!(text.contains("backtrace="));

    let previous = std::panic::take_hook();
    panic_logging::install_panic_hook(dir.clone()).unwrap();
    let result = std::panic::catch_unwind(|| panic!("diagnostic panic"));
    std::panic::set_hook(previous);
    assert!(result.is_err());
    let path = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("panic-")
        })
        .unwrap();
    let text = std::fs::read_to_string(path).unwrap();
    assert!(text.contains("message=diagnostic panic"));
    assert!(text.contains("location="));
    assert!(text.contains("thread="));
    assert!(text.contains("backtrace="));
    std::fs::remove_dir_all(dir).unwrap();
}

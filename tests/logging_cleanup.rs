use std::fs;
use std::time::{Duration, SystemTime};

use filetime::{FileTime, set_file_mtime};
use mica_term::app::logging::cleanup::{CleanupPolicy, cleanup_logging_dirs};

#[test]
fn cleanup_removes_files_older_than_max_age() {
    let temp_root = std::env::temp_dir()
        .join("mica-term")
        .join("tests")
        .join("logging-cleanup-age");
    let logs_dir = temp_root.join("logs");
    let crash_dir = temp_root.join("crash");
    let _ = fs::remove_dir_all(&temp_root);
    fs::create_dir_all(&logs_dir).unwrap();
    fs::create_dir_all(&crash_dir).unwrap();

    let old_file = logs_dir.join("old.log");
    fs::write(&old_file, "old").unwrap();
    let old_time =
        FileTime::from_system_time(SystemTime::now() - Duration::from_secs(60 * 60 * 24 * 30));
    set_file_mtime(&old_file, old_time).unwrap();

    cleanup_logging_dirs(
        &logs_dir,
        &crash_dir,
        CleanupPolicy {
            max_age: Duration::from_secs(60 * 60 * 24 * 14),
            max_total_bytes: 1024 * 1024,
        },
    )
    .unwrap();

    assert!(!old_file.exists());
}

#[test]
fn cleanup_trims_oldest_files_when_total_size_exceeds_limit() {
    let temp_root = std::env::temp_dir()
        .join("mica-term")
        .join("tests")
        .join("logging-cleanup-size");
    let logs_dir = temp_root.join("logs");
    let crash_dir = temp_root.join("crash");
    let _ = fs::remove_dir_all(&temp_root);
    fs::create_dir_all(&logs_dir).unwrap();
    fs::create_dir_all(&crash_dir).unwrap();

    let oldest = logs_dir.join("a.log");
    let newer = logs_dir.join("b.log");
    fs::write(&oldest, vec![b'a'; 32]).unwrap();
    fs::write(&newer, vec![b'b'; 32]).unwrap();
    set_file_mtime(
        &oldest,
        FileTime::from_system_time(SystemTime::now() - Duration::from_secs(120)),
    )
    .unwrap();

    cleanup_logging_dirs(
        &logs_dir,
        &crash_dir,
        CleanupPolicy {
            max_age: Duration::from_secs(60 * 60 * 24 * 14),
            max_total_bytes: 40,
        },
    )
    .unwrap();

    assert!(!oldest.exists());
    assert!(newer.exists());
}

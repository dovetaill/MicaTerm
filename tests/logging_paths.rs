use std::fs;

use mica_term::app::logging::paths::{LoggingPathInputs, LoggingRootSource, resolve_logging_paths};

#[test]
fn logging_paths_prefer_env_override_over_portable_and_standard_dirs() {
    let temp_root = std::env::temp_dir()
        .join("mica-term")
        .join("tests")
        .join("logging-paths-override");
    let _ = fs::remove_dir_all(&temp_root);
    fs::create_dir_all(temp_root.join("portable-root")).unwrap();
    fs::write(
        temp_root.join("portable-root").join(".mica-term-portable"),
        "",
    )
    .unwrap();

    let paths = resolve_logging_paths(&LoggingPathInputs {
        env_log_dir: Some(temp_root.join("override-root")),
        executable_dir: temp_root.join("portable-root"),
        standard_local_data_dir: temp_root.join("standard-root"),
        portable_marker_name: ".mica-term-portable",
    })
    .unwrap();

    assert_eq!(paths.root_source, LoggingRootSource::EnvOverride);
    assert_eq!(paths.root_dir, temp_root.join("override-root"));
    assert_eq!(paths.logs_dir, temp_root.join("override-root").join("logs"));
    assert_eq!(
        paths.crash_dir,
        temp_root.join("override-root").join("crash")
    );
}

#[test]
fn logging_paths_fall_back_to_portable_marker_when_present() {
    let temp_root = std::env::temp_dir()
        .join("mica-term")
        .join("tests")
        .join("logging-paths-portable");
    let _ = fs::remove_dir_all(&temp_root);
    fs::create_dir_all(temp_root.join("portable-root")).unwrap();
    fs::write(
        temp_root.join("portable-root").join(".mica-term-portable"),
        "",
    )
    .unwrap();

    let paths = resolve_logging_paths(&LoggingPathInputs {
        env_log_dir: None,
        executable_dir: temp_root.join("portable-root"),
        standard_local_data_dir: temp_root.join("standard-root"),
        portable_marker_name: ".mica-term-portable",
    })
    .unwrap();

    assert_eq!(paths.root_source, LoggingRootSource::PortableMarker);
    assert_eq!(paths.root_dir, temp_root.join("portable-root"));
}

#[test]
fn logging_paths_use_standard_local_data_dir_when_no_override_exists() {
    let temp_root = std::env::temp_dir()
        .join("mica-term")
        .join("tests")
        .join("logging-paths-standard");
    let _ = fs::remove_dir_all(&temp_root);
    fs::create_dir_all(temp_root.join("app-bin")).unwrap();

    let paths = resolve_logging_paths(&LoggingPathInputs {
        env_log_dir: None,
        executable_dir: temp_root.join("app-bin"),
        standard_local_data_dir: temp_root.join("standard-root"),
        portable_marker_name: ".mica-term-portable",
    })
    .unwrap();

    assert_eq!(paths.root_source, LoggingRootSource::StandardLocalData);
    assert_eq!(paths.root_dir, temp_root.join("standard-root"));
}

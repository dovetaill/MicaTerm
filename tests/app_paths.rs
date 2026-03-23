//! Shared application root path resolution coverage for portable, override, and standard roots.

use std::fs;

use mica_term::app::app_paths::{AppRootPathInputs, AppRootSource, resolve_app_root_paths};

#[test]
fn app_root_prefers_explicit_override() {
    let temp_root = std::env::temp_dir()
        .join("mica-term")
        .join("tests")
        .join("app-paths-override");
    let _ = fs::remove_dir_all(&temp_root);
    fs::create_dir_all(temp_root.join("portable-root")).unwrap();
    fs::write(
        temp_root.join("portable-root").join(".mica-term-portable"),
        "",
    )
    .unwrap();

    let paths = resolve_app_root_paths(&AppRootPathInputs {
        env_root_dir: Some(temp_root.join("override-root")),
        executable_dir: temp_root.join("portable-root"),
        standard_local_data_dir: temp_root.join("standard-root"),
        portable_marker_name: ".mica-term-portable",
    })
    .unwrap();

    assert_eq!(paths.root_source, AppRootSource::EnvOverride);
    assert_eq!(paths.root_dir, temp_root.join("override-root"));
    assert_eq!(paths.data_dir, temp_root.join("override-root").join("data"));
    assert_eq!(paths.logs_dir, temp_root.join("override-root").join("logs"));
    assert_eq!(
        paths.crash_dir,
        temp_root.join("override-root").join("crash")
    );
}

#[test]
fn app_root_uses_executable_dir_when_portable_marker_exists() {
    let temp_root = std::env::temp_dir()
        .join("mica-term")
        .join("tests")
        .join("app-paths-portable");
    let _ = fs::remove_dir_all(&temp_root);
    fs::create_dir_all(temp_root.join("portable-root")).unwrap();
    fs::write(
        temp_root.join("portable-root").join(".mica-term-portable"),
        "",
    )
    .unwrap();

    let paths = resolve_app_root_paths(&AppRootPathInputs {
        env_root_dir: None,
        executable_dir: temp_root.join("portable-root"),
        standard_local_data_dir: temp_root.join("standard-root"),
        portable_marker_name: ".mica-term-portable",
    })
    .unwrap();

    assert_eq!(paths.root_source, AppRootSource::PortableMarker);
    assert_eq!(paths.root_dir, temp_root.join("portable-root"));
    assert_eq!(paths.data_dir, temp_root.join("portable-root").join("data"));
}

#[test]
fn app_root_uses_platform_local_data_when_marker_is_absent() {
    let temp_root = std::env::temp_dir()
        .join("mica-term")
        .join("tests")
        .join("app-paths-standard");
    let _ = fs::remove_dir_all(&temp_root);
    fs::create_dir_all(temp_root.join("app-bin")).unwrap();

    let paths = resolve_app_root_paths(&AppRootPathInputs {
        env_root_dir: None,
        executable_dir: temp_root.join("app-bin"),
        standard_local_data_dir: temp_root.join("standard-root"),
        portable_marker_name: ".mica-term-portable",
    })
    .unwrap();

    assert_eq!(paths.root_source, AppRootSource::StandardLocalData);
    assert_eq!(paths.root_dir, temp_root.join("standard-root"));
    assert_eq!(paths.data_dir, temp_root.join("standard-root").join("data"));
}

#[test]
fn app_root_creates_data_logs_and_crash_directories() {
    let temp_root = std::env::temp_dir()
        .join("mica-term")
        .join("tests")
        .join("app-paths-directories");
    let _ = fs::remove_dir_all(&temp_root);
    fs::create_dir_all(temp_root.join("app-bin")).unwrap();

    let paths = resolve_app_root_paths(&AppRootPathInputs {
        env_root_dir: None,
        executable_dir: temp_root.join("app-bin"),
        standard_local_data_dir: temp_root.join("standard-root"),
        portable_marker_name: ".mica-term-portable",
    })
    .unwrap();

    assert!(paths.data_dir.is_dir());
    assert!(paths.logs_dir.is_dir());
    assert!(paths.crash_dir.is_dir());
}

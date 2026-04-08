#[path = "../build_support/xwin_link.rs"]
mod xwin_link;

use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use xwin_link::{ShimConfig, maybe_prepare_advapi32_shim};

#[test]
fn creates_case_exact_advapi32_shim_from_xwin_cache() {
    let test_root = temp_test_dir("xwin-advapi32-cache");
    let home_dir = test_root.join("home");
    let out_dir = test_root.join("out");
    let source_dir = home_dir
        .join(".cache")
        .join("cargo-xwin")
        .join("xwin")
        .join("sdk")
        .join("lib")
        .join("um")
        .join("x86_64");
    fs::create_dir_all(&source_dir).unwrap();
    fs::create_dir_all(&out_dir).unwrap();

    let source_path = source_dir.join("ADVAPI32.lib");
    fs::write(&source_path, b"shim-me").unwrap();

    let config = ShimConfig {
        target_os: "windows".into(),
        target_env: "msvc".into(),
        host_os: "linux".into(),
        target_arch: "x86_64".into(),
        out_dir: out_dir.clone(),
        xwin_cache_dir: None,
        home_dir: Some(home_dir),
        lib_paths: Vec::new(),
    };

    let shim_dir = maybe_prepare_advapi32_shim(&config)
        .unwrap()
        .expect("expected xwin shim directory");
    let shim_path = shim_dir.join("Advapi32.lib");

    assert_eq!(fs::read(&shim_path).unwrap(), b"shim-me");
}

#[test]
fn skips_advapi32_shim_on_windows_hosts() {
    let test_root = temp_test_dir("xwin-advapi32-skip");
    let out_dir = test_root.join("out");
    fs::create_dir_all(&out_dir).unwrap();

    let config = ShimConfig {
        target_os: "windows".into(),
        target_env: "msvc".into(),
        host_os: "windows".into(),
        target_arch: "x86_64".into(),
        out_dir,
        xwin_cache_dir: None,
        home_dir: None,
        lib_paths: Vec::new(),
    };

    assert!(maybe_prepare_advapi32_shim(&config).unwrap().is_none());
}

fn temp_test_dir(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("mica-term-{label}-{nanos}"));
    reset_dir(&path);
    path
}

fn reset_dir(path: &Path) {
    let _ = fs::remove_dir_all(path);
    fs::create_dir_all(path).unwrap();
}

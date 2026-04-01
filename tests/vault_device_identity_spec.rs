use std::fs;
use std::path::PathBuf;

use mica_term::app::vault::device_identity::{git_remote_cache_dir, load_or_create_device_id};
use uuid::Uuid;

fn sample_vault_runtime_root(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "mica-term-vault-device-identity-{label}-{}",
        Uuid::new_v4()
    ))
}

#[test]
fn device_id_persists_for_the_same_vault_root() {
    let root = sample_vault_runtime_root("persist");

    let first = load_or_create_device_id(root.as_path()).expect("create first device id");
    let second = load_or_create_device_id(root.as_path()).expect("reload device id");

    assert_eq!(first, second);
    assert!(first.starts_with("device-"));
}

#[test]
fn device_id_regenerates_only_after_identity_file_is_removed() {
    let root = sample_vault_runtime_root("regenerate");

    let first = load_or_create_device_id(root.as_path()).expect("create first device id");
    fs::remove_file(root.join("device-id")).expect("remove persisted device id");
    let second = load_or_create_device_id(root.as_path()).expect("regenerate device id");

    assert_ne!(first, second);
    assert!(second.starts_with("device-"));
}

#[test]
fn git_remote_cache_dir_uses_remote_scoped_layout() {
    let root = sample_vault_runtime_root("git-remote-cache");

    assert_eq!(
        git_remote_cache_dir(root.as_path(), "remote-primary"),
        root.join("git-remotes").join("remote-primary")
    );
}

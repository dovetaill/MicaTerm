use std::fs;

use mica_term::app::vault::conflict_inbox::{
    ConflictInboxEntry, load_conflict_entries, persist_conflict_entries,
};
use uuid::Uuid;

fn sample_conflict_root() -> std::path::PathBuf {
    std::env::temp_dir().join(format!("mica-term-conflict-inbox-{}", Uuid::new_v4()))
}

fn sample_conflict_entry(captured_at: &str, target_id: &str) -> ConflictInboxEntry {
    ConflictInboxEntry {
        vault_id: "vault-main".into(),
        target_id: target_id.into(),
        conflict_kind: "asset-delete-vs-modify".into(),
        local_device_id: "device-local".into(),
        remote_device_id: "device-remote".into(),
        captured_at: captured_at.into(),
    }
}

#[test]
fn conflict_inbox_entries_round_trip_through_disk_storage() {
    let conflict_root = sample_conflict_root();
    let expected = sample_conflict_entry("00000000000000000042", "asset-prod");

    persist_conflict_entries(&conflict_root, &[expected.clone()]).expect("persist conflict entry");

    let loaded = load_conflict_entries(&conflict_root, "vault-main").expect("load conflict inbox");

    assert_eq!(loaded, vec![expected]);
    let _ = fs::remove_dir_all(conflict_root);
}

#[test]
fn conflict_inbox_loads_newest_entries_first() {
    let conflict_root = sample_conflict_root();
    let older = sample_conflict_entry("00000000000000000041", "asset-older");
    let newer = sample_conflict_entry("00000000000000000042", "asset-newer");

    persist_conflict_entries(&conflict_root, &[older.clone(), newer.clone()])
        .expect("persist conflict entries");

    let loaded = load_conflict_entries(&conflict_root, "vault-main").expect("load conflict inbox");

    assert_eq!(loaded, vec![newer, older]);
    let _ = fs::remove_dir_all(conflict_root);
}

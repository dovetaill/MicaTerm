use mica_term::app::vault::model::{
    CipherKind, CompressionKind, KdfConfig, PackLayout, VaultHead,
};
use mica_term::app::vault::sync_decision::{
    LocalSyncState, SyncAction, decide_sync_action,
};

fn sample_kdf() -> KdfConfig {
    KdfConfig::Argon2id {
        memory_cost_kib: 19_456,
        time_cost: 2,
        parallelism: 1,
        salt_b64: "sync-decision-salt".into(),
    }
}

fn sample_remote_head(revision: &str, payload_hash: &str, committed_at: &str) -> VaultHead {
    VaultHead {
        format_version: 1,
        vault_id: "vault-main".into(),
        vault_revision: revision.into(),
        parent_revision: Some("rev-0001".into()),
        device_id: "device-remote".into(),
        committed_at: committed_at.into(),
        committed_by_device: "device-remote".into(),
        payload_hash: payload_hash.into(),
        manifest_ref: format!("manifest/{revision}.bin"),
        wrapped_vault_key: "wrapped-key".into(),
        kdf: sample_kdf(),
        cipher: CipherKind::XChaCha20Poly1305,
        compression: CompressionKind::Zstd,
        pack_layout: PackLayout::ObjectSet,
    }
}

#[test]
fn newer_local_snapshot_pushes_and_stashes_remote_backup() {
    let local = LocalSyncState {
        base_revision: Some("rev-0001".into()),
        local_snapshot_hash: Some("sha256:local-new".into()),
        last_local_change_at: Some("00000000000000000200".into()),
        last_successful_push_at: Some("00000000000000000100".into()),
        last_successful_pull_at: Some("00000000000000000100".into()),
    };
    let remote = sample_remote_head(
        "rev-0002",
        "sha256:remote-new",
        "00000000000000000150",
    );

    let decision = decide_sync_action(&local, Some(&remote));

    assert_eq!(decision.action, SyncAction::Push);
    assert!(!decision.backup_local_snapshot);
    assert!(decision.backup_remote_snapshot);
}

#[test]
fn newer_remote_revision_pulls_and_stashes_local_backup() {
    let local = LocalSyncState {
        base_revision: Some("rev-0001".into()),
        local_snapshot_hash: Some("sha256:local-new".into()),
        last_local_change_at: Some("00000000000000000120".into()),
        last_successful_push_at: Some("00000000000000000100".into()),
        last_successful_pull_at: Some("00000000000000000100".into()),
    };
    let remote = sample_remote_head(
        "rev-0002",
        "sha256:remote-new",
        "00000000000000000150",
    );

    let decision = decide_sync_action(&local, Some(&remote));

    assert_eq!(decision.action, SyncAction::Pull);
    assert!(decision.backup_local_snapshot);
    assert!(!decision.backup_remote_snapshot);
}

#[test]
fn identical_hashes_short_circuit_to_noop() {
    let local = LocalSyncState {
        base_revision: Some("rev-0001".into()),
        local_snapshot_hash: Some("sha256:same".into()),
        last_local_change_at: Some("00000000000000000120".into()),
        last_successful_push_at: Some("00000000000000000100".into()),
        last_successful_pull_at: Some("00000000000000000100".into()),
    };
    let remote = sample_remote_head("rev-0002", "sha256:same", "00000000000000000150");

    let decision = decide_sync_action(&local, Some(&remote));

    assert_eq!(decision.action, SyncAction::Noop);
    assert!(!decision.backup_local_snapshot);
    assert!(!decision.backup_remote_snapshot);
}

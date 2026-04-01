use crate::app::vault::model::VaultHead;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalSyncState {
    pub base_revision: Option<String>,
    pub local_snapshot_hash: Option<String>,
    pub last_local_change_at: Option<String>,
    pub last_successful_push_at: Option<String>,
    pub last_successful_pull_at: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncAction {
    Noop,
    PullOnly,
    PushOnly,
    MergeThenPush,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncDecision {
    pub action: SyncAction,
}

pub fn decide_sync_action(local: &LocalSyncState, remote: Option<&VaultHead>) -> SyncDecision {
    let Some(local_snapshot_hash) = local.local_snapshot_hash.as_deref() else {
        return SyncDecision {
            action: if remote.is_some() {
                SyncAction::PullOnly
            } else {
                SyncAction::Noop
            },
        };
    };

    let Some(remote) = remote else {
        return SyncDecision {
            action: SyncAction::PushOnly,
        };
    };

    if remote.payload_hash == local_snapshot_hash {
        return SyncDecision {
            action: SyncAction::Noop,
        };
    }

    let remote_changed = local.base_revision.as_deref() != Some(remote.vault_revision.as_str());
    let local_changed = local_has_unsynced_changes(local);

    match (local_changed, remote_changed) {
        (false, false) => SyncDecision {
            action: SyncAction::PushOnly,
        },
        (true, false) => SyncDecision {
            action: SyncAction::PushOnly,
        },
        (false, true) => SyncDecision {
            action: SyncAction::PullOnly,
        },
        (true, true) => SyncDecision {
            action: SyncAction::MergeThenPush,
        },
    }
}

fn local_has_unsynced_changes(local: &LocalSyncState) -> bool {
    let Some(last_local_change_at) = local.last_local_change_at.as_deref() else {
        return false;
    };

    let last_success = local
        .last_successful_push_at
        .as_deref()
        .max(local.last_successful_pull_at.as_deref());

    match last_success {
        Some(last_success) => last_local_change_at > last_success,
        None => true,
    }
}

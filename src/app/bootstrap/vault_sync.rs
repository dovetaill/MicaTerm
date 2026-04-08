//! Bootstrap vault sync module.

use super::*;

#[derive(Clone)]
pub(super) struct VaultSyncBackgroundSuccess {
    pub(super) projection: Option<VaultProjectionUpdate>,
    pub(super) sync_modal_state: SyncModalViewState,
    pub(super) vault_panel_state: VaultPanelViewState,
    pub(super) local_state: Option<LocalVaultBootstrapState>,
    pub(super) decrypted_snapshot: Option<VaultSnapshot>,
    pub(super) should_clear_dirty: bool,
}

#[derive(Clone)]
pub(super) struct VaultSyncBackgroundFailure {
    pub(super) sync_modal_state: SyncModalViewState,
    pub(super) vault_panel_state: VaultPanelViewState,
    pub(super) local_state: Option<LocalVaultBootstrapState>,
    pub(super) should_clear_dirty: bool,
}

#[allow(clippy::large_enum_variant)]
pub(super) enum VaultSyncBackgroundMessage {
    Completed {
        trigger: VaultSyncTrigger,
        result: std::result::Result<VaultSyncBackgroundSuccess, VaultSyncBackgroundFailure>,
    },
    RemoteHeadRefreshed {
        snapshot: RemoteHeadSnapshot,
    },
}

pub(super) fn persist_sync_modal_settings(
    state: &mut ShellViewModel,
    vault: &mut VaultSessionState,
    credential_store: &dyn CredentialStore,
) -> Result<()> {
    let existing_bundle = configured_sync_bundle(vault);
    let bundle = build_sync_bundle_from_modal(state, existing_bundle)?;
    let modal = state.sync_modal_state();
    let credential_material = build_git_repo_credential_material(modal)?;

    persist_provider_credential(
        credential_store,
        bootstrap_provider_credential_ref(sync_settings_remote_id(RemoteRole::Primary)).as_str(),
        Some(credential_material.as_str()),
    )?;

    let bootstrap_state_path = vault.bootstrap_state_path();
    if let Some(local_state) = vault.local_state.as_mut() {
        local_state.bundle = bundle;
        save_local_vault_bootstrap_state(bootstrap_state_path.as_path(), local_state)?;
    } else {
        vault.bootstrap_template = Some(bundle);
    }

    update_sync_modal_for_local_state(state, vault);
    Ok(())
}

pub(super) fn update_sync_modal_for_local_state(
    state: &mut ShellViewModel,
    vault: &VaultSessionState,
) {
    let has_primary_remote = vault_primary_remote(vault).is_some();
    let git_repo_setup = GitRepoRemoteDraft::default();
    let (conflict_count, conflict_summary, conflict_review_available) =
        sync_modal_conflict_projection(vault);
    let modal = state.sync_modal_state_mut();

    modal.title = "Sync Settings".into();
    modal.provider_label = first_release_formal_provider_label().into();
    modal.target_label = if has_primary_remote {
        "1 Git primary configured".into()
    } else {
        String::new()
    };
    modal.conflict_count = conflict_count;
    modal.conflict_summary = conflict_summary;
    modal.conflict_review_available = conflict_review_available;
    modal.error_text.clear();
    let local_last_sync = latest_local_sync_timestamp(vault);
    modal.local_last_sync_text = local_last_sync
        .as_deref()
        .map(|raw| format_sync_timestamp_for_ui(Some(raw)))
        .unwrap_or_else(|| "Never synced".into());
    modal.remote_last_update_text = "Unknown".into();
    modal.primary_revision_text = vault
        .local_state
        .as_ref()
        .and_then(|local_state| local_state.current_revision.clone())
        .unwrap_or_else(|| "Unknown".into());
    modal.remote_status_text.clear();
    modal.remote_status_loading = false;

    if vault.local_state.is_none() && vault.unlocked_vault_key.is_some() {
        modal.mode = SyncModalMode::SyncError;
        modal.headline = "Sync state is inconsistent".into();
        modal.status_text = "The local vault state could not be resolved.".into();
        modal.error_text = "Missing local bootstrap state".into();
        modal.primary_action_label = "Close".into();
        modal.secondary_action_label.clear();
        return;
    }

    match (&vault.local_state, has_primary_remote) {
        (None, false) => {
            modal.mode = SyncModalMode::NotConfigured;
            modal.headline = "Configure sync".into();
            modal.status_text = git_repo_setup.setup_summary();
            modal.primary_action_label = "Save and enable".into();
            modal.secondary_action_label = "Close".into();
        }
        (None, true) => {
            modal.mode = SyncModalMode::NotConfigured;
            modal.headline = "Enable or recover sync".into();
            modal.status_text = "The Git primary is configured. Enter a master password to recover from the remote if it already has data, or create a new local vault if it is still empty.".into();
            modal.primary_action_label = "Save and enable".into();
            modal.secondary_action_label = "Close".into();
        }
        (Some(_), false) => {
            modal.mode = SyncModalMode::NotConfigured;
            modal.headline = "Finish sync settings".into();
            modal.status_text =
                "Add a Gitee Git remote plus HTTPS credentials or SSH key before sync can run."
                    .into();
            modal.primary_action_label = "Save settings".into();
            modal.secondary_action_label = "Close".into();
        }
        (Some(_), true) => {
            modal.mode = SyncModalMode::Ready;
            modal.headline = "Sync ready".into();
            modal.status_text = if vault.unlocked_vault_key.is_some() {
                "Sync is configured. Use the titlebar Sync button to run an immediate check.".into()
            } else {
                "Sync is configured. Use the titlebar Sync button to run an immediate check. Diagnostics appear here if sync needs attention."
                    .into()
            };
            modal.primary_action_label = "Sync now".into();
            modal.secondary_action_label = "Close".into();
        }
    }
}

pub(super) fn vault_primary_remote(vault: &VaultSessionState) -> Option<&BootstrapRemoteConfig> {
    vault
        .local_state
        .as_ref()
        .and_then(|local_state| local_state.bundle.primary_remote())
        .or_else(|| {
            vault
                .bootstrap_template
                .as_ref()
                .and_then(BootstrapBundle::primary_remote)
        })
}

pub(super) fn read_primary_remote_head_snapshot(
    vault: &VaultSessionState,
    credential_store: &dyn CredentialStore,
) -> RemoteHeadSnapshot {
    let Some(primary_remote) = vault_primary_remote(vault).cloned() else {
        return RemoteHeadSnapshot::default();
    };
    let resolved = match resolve_remote_for_sync(&primary_remote, credential_store) {
        Ok(resolved) => resolved,
        Err(err) => {
            return RemoteHeadSnapshot {
                error: Some(err.to_string()),
                loading: false,
                ..RemoteHeadSnapshot::default()
            };
        }
    };
    let provider = match vault
        .provider_factory
        .build_provider_for_vault(&resolved, vault.root_dir.as_path())
    {
        Ok(provider) => provider,
        Err(err) => {
            return RemoteHeadSnapshot {
                error: Some(err.to_string()),
                loading: false,
                ..RemoteHeadSnapshot::default()
            };
        }
    };
    match provider.read_head() {
        Ok(result) => {
            if let Some(head) = result.head {
                RemoteHeadSnapshot {
                    revision: Some(head.vault_revision),
                    committed_at: Some(head.committed_at),
                    loading: false,
                    ..RemoteHeadSnapshot::default()
                }
            } else {
                RemoteHeadSnapshot {
                    loading: false,
                    ..RemoteHeadSnapshot::default()
                }
            }
        }
        Err(err) => RemoteHeadSnapshot {
            error: Some(format!(
                "failed to inspect primary remote `{}`: {err}",
                primary_remote.remote_id
            )),
            loading: false,
            ..RemoteHeadSnapshot::default()
        },
    }
}

pub(super) fn apply_remote_head_snapshot_to_sync_modal(
    state: &mut ShellViewModel,
    snapshot: RemoteHeadSnapshot,
) {
    let modal = state.sync_modal_state_mut();
    modal.remote_status_loading = false;
    modal.primary_revision_text = snapshot
        .revision
        .clone()
        .unwrap_or_else(|| "Unknown".into());
    modal.remote_last_update_text = format_sync_timestamp_for_ui(snapshot.committed_at.as_deref());

    if let Some(error) = snapshot.error {
        modal.remote_status_text = "Failed to refresh remote status.".into();
        modal.status_text = modal.remote_status_text.clone();
        modal.error_text = error;
        return;
    }

    modal.error_text.clear();
    modal.remote_status_text = if let Some(revision) = snapshot.revision.clone() {
        format!("Primary remote is currently at {revision}.")
    } else {
        "Primary remote is empty.".into()
    };
    modal.status_text = modal.remote_status_text.clone();
}

pub(super) fn request_sync_modal_remote_head_refresh(
    state: &mut ShellViewModel,
    vault: &VaultSessionState,
    credential_store: Arc<dyn CredentialStore>,
    vault_sync_result_tx: &std::sync::mpsc::Sender<VaultSyncBackgroundMessage>,
) {
    if vault_primary_remote(vault).is_none() || state.sync_modal_state().remote_status_loading {
        return;
    }

    {
        let modal = state.sync_modal_state_mut();
        modal.remote_status_loading = true;
        modal.remote_status_text = "Refreshing remote status...".into();
    }

    let worker_vault = vault.clone();
    let completion_tx = vault_sync_result_tx.clone();
    std::thread::spawn(move || {
        let snapshot = read_primary_remote_head_snapshot(&worker_vault, credential_store.as_ref());
        let _ = completion_tx.send(VaultSyncBackgroundMessage::RemoteHeadRefreshed { snapshot });
    });
}

pub(super) fn set_sync_modal_error(
    state: &mut ShellViewModel,
    vault: &VaultSessionState,
    error: impl Into<String>,
) {
    update_sync_modal_for_local_state(state, vault);
    state.set_sync_modal_error(error);
}

pub(super) fn set_sync_modal_error_without_opening(
    state: &mut ShellViewModel,
    vault: &VaultSessionState,
    error: impl Into<String>,
) {
    update_sync_modal_for_local_state(state, vault);
    state.sync_modal_state_mut().error_text = error.into();
}

pub(super) fn submit_sync_modal_master_password(
    state: &mut ShellViewModel,
    vault: &mut VaultSessionState,
    credential_store: &dyn CredentialStore,
    password: &secrecy::SecretString,
) -> Result<()> {
    if vault.local_state.is_some() {
        unlock_local_vault_into_shell(state, vault, credential_store, password)
    } else {
        if recover_local_vault_from_primary_remote(state, vault, credential_store, password)? {
            return Ok(());
        }
        create_local_vault_from_shell_state(state, vault, credential_store, password)
    }
}

pub(super) fn silently_restore_vault_session_from_runtime_key(
    state: &mut ShellViewModel,
    vault: &mut VaultSessionState,
    credential_store: &dyn CredentialStore,
) -> Option<String> {
    let vault_id = vault
        .local_state
        .as_ref()
        .map(|local_state| local_state.bundle.vault_id.clone())?;

    let runtime_vault_key = match load_runtime_vault_key(credential_store, &vault_id) {
        Ok(Some(key)) => key,
        Ok(None) => return None,
        Err(err) => {
            let credential_ref = vault_runtime_key_credential_ref(&vault_id);
            if let Err(delete_err) = credential_store.delete_secret(credential_ref.as_str()) {
                tracing::error!(
                    target: "app.vault",
                    vault_id,
                    credential_ref,
                    error = %delete_err,
                    "failed to clear unreadable runtime vault key material"
                );
            }
            return Some(format!(
                "Automatic vault recovery is unavailable until you re-enter the master password: {err}"
            ));
        }
    };

    let recovery_attempt = (|| -> Result<VaultSnapshot> {
        let encrypted_snapshot = load_encrypted_cache(vault.cache_root().as_path(), &vault_id)?
            .ok_or_else(|| anyhow!("encrypted cache is unavailable"))?;
        let snapshot = normalize_snapshot_secret_refs(decrypt_snapshot(
            &encrypted_snapshot,
            &runtime_vault_key,
        )?);
        apply_vault_snapshot_to_shell(
            state,
            &snapshot,
            credential_store,
            vault.known_hosts_path().as_path(),
        )?;
        Ok(snapshot)
    })();

    match recovery_attempt {
        Ok(snapshot) => {
            vault.unlocked_vault_key = Some(runtime_vault_key);
            vault.decrypted_snapshot = Some(snapshot);
            None
        }
        Err(err) => {
            let credential_ref = vault_runtime_key_credential_ref(&vault_id);
            if let Err(delete_err) = credential_store.delete_secret(credential_ref.as_str()) {
                tracing::error!(
                    target: "app.vault",
                    vault_id,
                    credential_ref,
                    error = %delete_err,
                    "failed to clear invalid runtime vault key material"
                );
            }
            Some(format!(
                "Automatic vault recovery failed. Re-enter the master password to restore sync: {err}"
            ))
        }
    }
}

pub(super) fn sync_preferences_for_bundle(
    bundle: &BootstrapBundle,
    last_sync_result: Option<String>,
) -> SnapshotSyncPreferences {
    SnapshotSyncPreferences {
        auto_sync_enabled: bundle.primary_remote().is_some(),
        selected_primary_remote_id: bundle
            .primary_remote()
            .map(|remote| remote.remote_id.clone()),
        selected_mirror_remote_ids: bundle
            .remotes
            .iter()
            .filter(|remote| remote.role == RemoteRole::Mirror)
            .map(|remote| remote.remote_id.clone())
            .collect(),
        last_sync_result,
    }
}

pub(super) fn shell_has_materialized_local_data(state: &ShellViewModel) -> bool {
    !state.console_asset_tree().root_ids().is_empty()
        || !state.snippet_asset_tree().root_ids().is_empty()
        || !state.keychain_catalog().nodes.is_empty()
}

pub(super) fn apply_vault_snapshot_to_shell(
    state: &mut ShellViewModel,
    snapshot: &VaultSnapshot,
    credential_store: &dyn CredentialStore,
    known_hosts_path: &std::path::Path,
) -> Result<()> {
    let applied = apply_vault_snapshot(snapshot, credential_store, known_hosts_path)?;
    let (console_tree, snippet_tree) =
        catalog_to_asset_trees(&asset_tree_to_catalog(&applied.asset_tree));
    state.replace_vault_projection(console_tree, snippet_tree, applied.keychain_catalog);
    Ok(())
}

pub(super) fn clear_vault_decrypted_state(
    state: &mut ShellViewModel,
    snapshot: Option<&VaultSnapshot>,
    credential_store: &dyn CredentialStore,
) -> Result<()> {
    if let Some(snapshot) = snapshot {
        for node in snapshot.asset_catalog.nodes.values() {
            let VaultAssetPayload::SshConnection(spec) = &node.payload else {
                continue;
            };
            restore_snapshot_secret_bundle(credential_store, spec.credential_ref.as_deref(), None)?;
        }

        for node in snapshot.keychain_catalog.nodes.values() {
            match &node.payload {
                KeychainNodePayload::Folder => {}
                KeychainNodePayload::Identity(spec) => restore_snapshot_secret_bundle(
                    credential_store,
                    spec.credential_ref.as_deref(),
                    None,
                )?,
                KeychainNodePayload::SshKey(spec) => restore_snapshot_secret_bundle(
                    credential_store,
                    spec.credential_ref.as_deref(),
                    None,
                )?,
            }
        }
    }
    state.clear_vault_projection();
    Ok(())
}

pub(super) fn create_local_vault_from_shell_state(
    state: &mut ShellViewModel,
    vault: &mut VaultSessionState,
    credential_store: &dyn CredentialStore,
    password: &secrecy::SecretString,
) -> Result<()> {
    let mut bundle = vault
        .bootstrap_template
        .clone()
        .ok_or_else(|| anyhow!("Configure a Gitee Git remote first"))?;
    if bundle.primary_remote().is_none() {
        return Err(anyhow!("Configure a Gitee Git remote first"));
    }
    if bundle.vault_id.trim().is_empty() {
        bundle.vault_id = format!("vault-{}", Uuid::new_v4().simple());
    }
    ensure_primary_remote_is_empty_before_first_local_bootstrap(vault, &bundle, credential_store)?;
    let kdf = default_vault_kdf();
    let vault_key = generate_vault_key();
    let wrapped_vault_key = serde_json::to_string(&wrap_vault_key(password, &kdf, &vault_key)?)
        .context("failed to encode wrapped vault key")?;
    let snapshot = export_vault_snapshot(
        &combined_asset_tree(state),
        state.keychain_catalog(),
        credential_store,
        vault.known_hosts_path().as_path(),
        sync_preferences_for_bundle(&bundle, None),
        &UiPreferences::from(&*state),
    )?;
    let encrypted_snapshot = encrypt_snapshot(&snapshot, &vault_key)?;
    let bootstrap_created_at = current_sync_timestamp();
    let device_id = load_or_create_device_id(vault.root_dir.as_path())?;
    store_encrypted_cache(
        vault.cache_root().as_path(),
        &bundle.vault_id,
        &encrypted_snapshot,
    )?;
    let local_state = LocalVaultBootstrapState {
        bundle,
        wrapped_vault_key,
        kdf: kdf.clone(),
        device_id,
        logical_revision: None,
        transport_revision_hint: None,
        current_revision: None,
        local_snapshot_hash: Some(payload_hash_from_encrypted_snapshot_sha(
            &encrypted_snapshot.payload_sha256,
        )),
        last_local_change_at: Some(bootstrap_created_at),
        last_successful_push_at: None,
        last_successful_pull_at: None,
        last_sync_error: None,
    };
    save_local_vault_bootstrap_state(vault.bootstrap_state_path().as_path(), &local_state)?;
    persist_runtime_vault_key(credential_store, &local_state.bundle.vault_id, &vault_key)?;
    vault.local_state = Some(local_state);
    vault.unlocked_vault_key = Some(vault_key);
    vault.decrypted_snapshot = Some(snapshot);
    update_vault_panel_for_local_state(state, vault);
    update_sync_modal_for_local_state(state, vault);

    Ok(())
}

pub(super) fn recover_local_vault_from_primary_remote(
    state: &mut ShellViewModel,
    vault: &mut VaultSessionState,
    credential_store: &dyn CredentialStore,
    password: &secrecy::SecretString,
) -> Result<bool> {
    let Some(mut bundle) = vault.bootstrap_template.clone() else {
        return Ok(false);
    };
    let Some(primary_remote) = bundle.primary_remote().cloned() else {
        return Ok(false);
    };
    let primary_remote = resolve_remote_for_sync(&primary_remote, credential_store)?;
    let provider = vault
        .provider_factory
        .build_provider_for_vault(&primary_remote, vault.root_dir.as_path())?;
    let Some(remote_head) = provider
        .read_head()
        .with_context(|| {
            format!(
                "failed to inspect primary remote `{}` before enabling sync",
                primary_remote.remote_id
            )
        })?
        .head
    else {
        return Ok(false);
    };
    let remote_revision = provider.read_revision(&remote_head).map_err(|err| {
        anyhow!(
            "failed to read recoverable revision `{}` from primary remote `{}`: {err}",
            remote_head.vault_revision,
            primary_remote.remote_id
        )
    })?;
    let wrapped: WrappedVaultKey = serde_json::from_str(&remote_head.wrapped_vault_key)
        .context("failed to decode wrapped vault key from remote head")?;
    let vault_key = unwrap_vault_key(password, &wrapped)?;
    let device_id = load_or_create_device_id(vault.root_dir.as_path())?;
    let remote_snapshot = normalize_snapshot_secret_refs(decrypt_snapshot(
        &remote_revision.encrypted_snapshot,
        &vault_key,
    )?);
    let local_snapshot = export_vault_snapshot(
        &combined_asset_tree(state),
        state.keychain_catalog(),
        credential_store,
        vault.known_hosts_path().as_path(),
        sync_preferences_for_bundle(&bundle, None),
        &UiPreferences::from(&*state),
    )?;
    if shell_has_materialized_local_data(state) {
        let merge_remote_snapshot = prepare_remote_snapshot_for_merge(
            &VaultSnapshot::default(),
            &local_snapshot,
            &remote_snapshot,
        );
        let merge_result = merge_snapshots(MergeInput {
            base: VaultSnapshot::default(),
            local: local_snapshot.clone(),
            remote: merge_remote_snapshot.clone(),
            device_id: device_id.clone(),
        });
        if !merge_result.conflicts.is_empty() {
            let captured_at = current_sync_timestamp();
            persist_merge_conflict_recovery_snapshots(
                vault.root_dir.join("recovery").as_path(),
                &remote_head.vault_id,
                None,
                &local_snapshot,
                &remote_head,
                &merge_remote_snapshot,
            )?;
            persist_merge_conflict_inbox_entries(
                vault.root_dir.join("conflicts").as_path(),
                &remote_head.vault_id,
                merge_result.conflicts.as_slice(),
                device_id.as_str(),
                remote_head.device_id.as_str(),
                captured_at.as_str(),
            )?;
        }
        let merged_snapshot = normalize_snapshot_secret_refs(merge_result.merged);
        let next_revision = next_vault_revision(Some(remote_head.vault_revision.as_str()));
        let committed_at = current_sync_timestamp();
        let request = SyncRequest {
            vault_id: remote_head.vault_id.clone(),
            snapshot: merged_snapshot.clone(),
            next_revision: next_revision.clone(),
            parent_revision: Some(remote_head.vault_revision.clone()),
            device_id: device_id.clone(),
            committed_at: committed_at.clone(),
            committed_by_device: device_id.clone(),
            wrapped_vault_key: remote_head.wrapped_vault_key.clone(),
            kdf: remote_head.kdf.clone(),
            provider_kind: primary_remote.provider,
            vault_key,
        };
        let mirror_providers = build_mirror_providers(&bundle, vault, credential_store)?;
        let engine = SyncEngine::new(provider, mirror_providers);
        let report = engine
            .sync(request)
            .map_err(|err| anyhow!(err.to_string()))?;

        bundle.vault_id = remote_head.vault_id.clone();
        store_encrypted_cache(
            vault.cache_root().as_path(),
            &bundle.vault_id,
            &report.encrypted_snapshot,
        )?;
        let local_state = LocalVaultBootstrapState {
            bundle,
            wrapped_vault_key: remote_head.wrapped_vault_key.clone(),
            kdf: remote_head.kdf.clone(),
            device_id,
            logical_revision: Some(report.primary_revision.clone()),
            transport_revision_hint: None,
            current_revision: Some(report.primary_revision.clone()),
            local_snapshot_hash: Some(report.head.payload_hash.clone()),
            last_local_change_at: Some(committed_at.clone()),
            last_successful_push_at: Some(committed_at),
            last_successful_pull_at: None,
            last_sync_error: None,
        };
        save_local_vault_bootstrap_state(vault.bootstrap_state_path().as_path(), &local_state)?;
        persist_runtime_vault_key(credential_store, &local_state.bundle.vault_id, &vault_key)?;
        apply_vault_snapshot_to_shell(
            state,
            &merged_snapshot,
            credential_store,
            vault.known_hosts_path().as_path(),
        )?;
        vault.local_state = Some(local_state);
        vault.unlocked_vault_key = Some(vault_key);
        vault.decrypted_snapshot = Some(merged_snapshot);
        update_vault_panel_for_local_state(state, vault);
        update_sync_modal_for_local_state(state, vault);
        state.vault_panel_state_mut().primary_status_label =
            format!("Attached and merged {}", report.primary_revision);
        state.sync_modal_state_mut().status_text = format!(
            "Attached local assets to primary remote and pushed merged revision {}.",
            report.primary_revision
        );

        return Ok(true);
    }

    let recovery_pulled_at = current_sync_timestamp();
    bundle.vault_id = remote_head.vault_id.clone();
    store_encrypted_cache(
        vault.cache_root().as_path(),
        &bundle.vault_id,
        &remote_revision.encrypted_snapshot,
    )?;
    let local_state = LocalVaultBootstrapState {
        bundle,
        wrapped_vault_key: remote_head.wrapped_vault_key.clone(),
        kdf: remote_head.kdf.clone(),
        device_id,
        logical_revision: Some(remote_head.vault_revision.clone()),
        transport_revision_hint: None,
        current_revision: Some(remote_head.vault_revision.clone()),
        local_snapshot_hash: Some(remote_head.payload_hash.clone()),
        last_local_change_at: None,
        last_successful_push_at: None,
        last_successful_pull_at: Some(recovery_pulled_at),
        last_sync_error: None,
    };
    save_local_vault_bootstrap_state(vault.bootstrap_state_path().as_path(), &local_state)?;
    persist_runtime_vault_key(credential_store, &local_state.bundle.vault_id, &vault_key)?;
    apply_vault_snapshot_to_shell(
        state,
        &remote_snapshot,
        credential_store,
        vault.known_hosts_path().as_path(),
    )?;
    vault.local_state = Some(local_state);
    vault.unlocked_vault_key = Some(vault_key);
    vault.decrypted_snapshot = Some(remote_snapshot);
    update_vault_panel_for_local_state(state, vault);
    update_sync_modal_for_local_state(state, vault);
    state.vault_panel_state_mut().primary_status_label =
        format!("Recovered from primary {}", remote_head.vault_revision);
    state.sync_modal_state_mut().status_text = format!(
        "Recovered local vault from primary remote at {}.",
        remote_head.vault_revision
    );

    Ok(true)
}

pub(super) fn ensure_primary_remote_is_empty_before_first_local_bootstrap(
    vault: &VaultSessionState,
    bundle: &BootstrapBundle,
    credential_store: &dyn CredentialStore,
) -> Result<()> {
    let primary_remote = bundle
        .primary_remote()
        .cloned()
        .ok_or_else(|| anyhow!("primary remote is not configured"))?;
    let resolved = resolve_remote_for_sync(&primary_remote, credential_store)?;
    let provider = vault
        .provider_factory
        .build_provider_for_vault(&resolved, vault.root_dir.as_path())?;
    let remote_head = provider
        .read_head()
        .with_context(|| {
            format!(
                "failed to inspect primary remote `{}` before enabling sync",
                primary_remote.remote_id
            )
        })?
        .head;

    if let Some(head) = remote_head {
        return Err(anyhow!(
            "primary remote `{}` already contains revision `{}`. Local recovery from remote is not implemented yet, so refusing to initialize a new empty local vault over existing remote data.",
            primary_remote.remote_id,
            head.vault_revision
        ));
    }

    Ok(())
}

pub(super) fn unlock_local_vault_into_shell(
    state: &mut ShellViewModel,
    vault: &mut VaultSessionState,
    credential_store: &dyn CredentialStore,
    password: &secrecy::SecretString,
) -> Result<()> {
    let local_state = vault
        .local_state
        .as_ref()
        .ok_or_else(|| anyhow!("vault bootstrap is not initialized"))?;
    let wrapped: WrappedVaultKey = serde_json::from_str(&local_state.wrapped_vault_key)
        .context("failed to decode wrapped vault key")?;
    let vault_key = unwrap_vault_key(password, &wrapped)?;
    let encrypted_snapshot =
        load_encrypted_cache(vault.cache_root().as_path(), &local_state.bundle.vault_id)?
            .ok_or_else(|| anyhow!("encrypted cache is unavailable"))?;
    let snapshot =
        normalize_snapshot_secret_refs(decrypt_snapshot(&encrypted_snapshot, &vault_key)?);
    apply_vault_snapshot_to_shell(
        state,
        &snapshot,
        credential_store,
        vault.known_hosts_path().as_path(),
    )?;
    persist_runtime_vault_key(credential_store, &local_state.bundle.vault_id, &vault_key)?;
    let cached_snapshot_hash =
        payload_hash_from_encrypted_snapshot_sha(&encrypted_snapshot.payload_sha256);
    let bootstrap_state_path = vault.bootstrap_state_path();
    vault.unlocked_vault_key = Some(vault_key);
    vault.decrypted_snapshot = Some(snapshot);
    if let Some(local_state) = vault.local_state.as_mut() {
        let needs_save = local_state.local_snapshot_hash.as_deref()
            != Some(cached_snapshot_hash.as_str())
            || local_state.last_sync_error.is_some();
        if needs_save {
            local_state.local_snapshot_hash = Some(cached_snapshot_hash);
        }
        local_state.last_sync_error = None;
        if needs_save {
            save_local_vault_bootstrap_state(bootstrap_state_path.as_path(), local_state)?;
        }
    }
    update_vault_panel_for_local_state(state, vault);
    update_sync_modal_for_local_state(state, vault);
    Ok(())
}

pub(super) fn resolve_remote_for_sync(
    remote: &BootstrapRemoteConfig,
    credential_store: &dyn CredentialStore,
) -> Result<BootstrapRemoteConfig> {
    let mut resolved = remote.clone();

    if remote.provider == ProviderKind::GiteeGist && remote.auth_kind == ProviderAuthKind::Pat {
        let inline_secret =
            load_provider_credential(credential_store, remote.credential_ref.as_deref())?;
        let inline_secret = inline_secret.ok_or_else(|| {
            anyhow!(
                "missing saved provider credential for remote `{}`",
                remote.remote_id
            )
        })?;
        resolved.credential_ref = Some(inline_secret);
    }

    if remote.provider == ProviderKind::GitRepo
        && matches!(
            remote.auth_kind,
            ProviderAuthKind::HttpsCredentials | ProviderAuthKind::SshKey
        )
    {
        let inline_secret =
            load_provider_credential(credential_store, remote.credential_ref.as_deref())?;
        let inline_secret = inline_secret.ok_or_else(|| {
            anyhow!(
                "missing saved provider credential for remote `{}`",
                remote.remote_id
            )
        })?;
        resolved.credential_ref = Some(inline_secret);
    }

    Ok(resolved)
}

pub(super) fn build_mirror_providers(
    bundle: &BootstrapBundle,
    vault: &VaultSessionState,
    credential_store: &dyn CredentialStore,
) -> Result<Vec<Arc<dyn VaultProvider>>> {
    bundle
        .remotes
        .iter()
        .filter(|remote| remote.role == RemoteRole::Mirror)
        .map(|remote| {
            let resolved = resolve_remote_for_sync(remote, credential_store)?;
            vault
                .provider_factory
                .build_provider_for_vault(&resolved, vault.root_dir.as_path())
        })
        .collect()
}

pub(super) fn prepare_remote_snapshot_for_merge(
    base: &VaultSnapshot,
    local: &VaultSnapshot,
    remote: &VaultSnapshot,
) -> VaultSnapshot {
    let mut remote = remote.clone();
    let asset_id_remap = concurrent_addition_asset_id_remap(
        &base.asset_catalog,
        &local.asset_catalog,
        &remote.asset_catalog,
    );
    if !asset_id_remap.is_empty() {
        apply_remote_asset_id_remap(&mut remote, &asset_id_remap);
    }
    let keychain_id_remap = concurrent_addition_keychain_id_remap(
        &base.keychain_catalog,
        &local.keychain_catalog,
        &remote.keychain_catalog,
    );
    if !keychain_id_remap.is_empty() {
        apply_remote_keychain_id_remap(&mut remote, &keychain_id_remap);
    }
    remote
}

pub(super) fn concurrent_addition_asset_id_remap(
    base: &crate::app::vault::model::VaultAssetCatalog,
    local: &crate::app::vault::model::VaultAssetCatalog,
    remote: &crate::app::vault::model::VaultAssetCatalog,
) -> BTreeMap<String, String> {
    let mut occupied = base
        .nodes
        .keys()
        .chain(local.nodes.keys())
        .chain(remote.nodes.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut remap = BTreeMap::new();
    for (node_id, remote_node) in &remote.nodes {
        let Some(local_node) = local.nodes.get(node_id) else {
            continue;
        };
        if base.nodes.contains_key(node_id) || local_node == remote_node {
            continue;
        }
        let next_id = next_merge_collision_id(&occupied, node_id);
        occupied.insert(next_id.clone());
        remap.insert(node_id.clone(), next_id);
    }
    remap
}

pub(super) fn concurrent_addition_keychain_id_remap(
    base: &crate::app::keychain::model::KeychainCatalog,
    local: &crate::app::keychain::model::KeychainCatalog,
    remote: &crate::app::keychain::model::KeychainCatalog,
) -> BTreeMap<String, String> {
    let mut occupied = base
        .nodes
        .keys()
        .chain(local.nodes.keys())
        .chain(remote.nodes.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut remap = BTreeMap::new();
    for (node_id, remote_node) in &remote.nodes {
        let Some(local_node) = local.nodes.get(node_id) else {
            continue;
        };
        if base.nodes.contains_key(node_id) || local_node == remote_node {
            continue;
        }
        let next_id = next_merge_collision_id(&occupied, node_id);
        occupied.insert(next_id.clone());
        remap.insert(node_id.clone(), next_id);
    }
    remap
}

pub(super) fn next_merge_collision_id(occupied: &BTreeSet<String>, original: &str) -> String {
    let mut suffix = 1_u64;
    loop {
        let candidate = format!("{original}-remote-merge-{suffix}");
        if !occupied.contains(&candidate) {
            return candidate;
        }
        suffix += 1;
    }
}

pub(super) fn apply_remote_asset_id_remap(
    snapshot: &mut VaultSnapshot,
    remap: &BTreeMap<String, String>,
) {
    snapshot.asset_catalog.root_ids = snapshot
        .asset_catalog
        .root_ids
        .iter()
        .map(|node_id| {
            remap
                .get(node_id)
                .cloned()
                .unwrap_or_else(|| node_id.clone())
        })
        .collect();
    snapshot.asset_catalog.nodes = snapshot
        .asset_catalog
        .nodes
        .iter()
        .map(|(node_id, node)| {
            let mut node = node.clone();
            node.id = remap
                .get(node_id)
                .cloned()
                .unwrap_or_else(|| node_id.clone());
            node.parent_id = node
                .parent_id
                .and_then(|parent_id| remap.get(&parent_id).cloned().or(Some(parent_id)));
            node.child_ids = node
                .child_ids
                .iter()
                .map(|child_id| {
                    remap
                        .get(child_id)
                        .cloned()
                        .unwrap_or_else(|| child_id.clone())
                })
                .collect();
            match &mut node.payload {
                VaultAssetPayload::Folder | VaultAssetPayload::SnippetPackage => {}
                VaultAssetPayload::SshConnection(spec) => {
                    if let VaultSshProxySpec::SshAsset { asset_id } = &mut spec.proxy
                        && let Some(next_id) = remap.get(asset_id)
                    {
                        *asset_id = next_id.clone();
                    }
                }
                VaultAssetPayload::Snippet(spec) => {
                    if let Some(package_id) = &mut spec.package_id
                        && let Some(next_id) = remap.get(package_id)
                    {
                        *package_id = next_id.clone();
                    }
                }
            }
            (node.id.clone(), node)
        })
        .collect();
    snapshot.ssh_secret_bundles = snapshot
        .ssh_secret_bundles
        .iter()
        .map(|(node_id, bundle)| {
            (
                remap
                    .get(node_id)
                    .cloned()
                    .unwrap_or_else(|| node_id.clone()),
                bundle.clone(),
            )
        })
        .collect();
    snapshot.asset_catalog.merge_metadata = snapshot
        .asset_catalog
        .merge_metadata
        .iter()
        .map(|(node_id, metadata)| {
            (
                remap
                    .get(node_id)
                    .cloned()
                    .unwrap_or_else(|| node_id.clone()),
                metadata.clone(),
            )
        })
        .collect();
}

pub(super) fn apply_remote_keychain_id_remap(
    snapshot: &mut VaultSnapshot,
    remap: &BTreeMap<String, String>,
) {
    snapshot.keychain_catalog.root_ids = snapshot
        .keychain_catalog
        .root_ids
        .iter()
        .map(|node_id| {
            remap
                .get(node_id)
                .cloned()
                .unwrap_or_else(|| node_id.clone())
        })
        .collect();
    snapshot.keychain_catalog.nodes = snapshot
        .keychain_catalog
        .nodes
        .iter()
        .map(|(node_id, node)| {
            let mut node = node.clone();
            node.id = remap
                .get(node_id)
                .cloned()
                .unwrap_or_else(|| node_id.clone());
            node.parent_id = node
                .parent_id
                .and_then(|parent_id| remap.get(&parent_id).cloned().or(Some(parent_id)));
            node.child_ids = node
                .child_ids
                .iter()
                .map(|child_id| {
                    remap
                        .get(child_id)
                        .cloned()
                        .unwrap_or_else(|| child_id.clone())
                })
                .collect();
            match &mut node.payload {
                KeychainNodePayload::Folder => {}
                KeychainNodePayload::Identity(spec) => {
                    if let Some(ssh_key_id) = &mut spec.ssh_key_id
                        && let Some(next_id) = remap.get(ssh_key_id)
                    {
                        *ssh_key_id = next_id.clone();
                    }
                }
                KeychainNodePayload::SshKey(_) => {}
            }
            (node.id.clone(), node)
        })
        .collect();
    snapshot.keychain_identity_secret_bundles = snapshot
        .keychain_identity_secret_bundles
        .iter()
        .map(|(node_id, bundle)| {
            (
                remap
                    .get(node_id)
                    .cloned()
                    .unwrap_or_else(|| node_id.clone()),
                bundle.clone(),
            )
        })
        .collect();
    snapshot.keychain_key_secret_bundles = snapshot
        .keychain_key_secret_bundles
        .iter()
        .map(|(node_id, bundle)| {
            (
                remap
                    .get(node_id)
                    .cloned()
                    .unwrap_or_else(|| node_id.clone()),
                bundle.clone(),
            )
        })
        .collect();
    snapshot.keychain_catalog.merge_metadata = snapshot
        .keychain_catalog
        .merge_metadata
        .iter()
        .map(|(node_id, metadata)| {
            (
                remap
                    .get(node_id)
                    .cloned()
                    .unwrap_or_else(|| node_id.clone()),
                metadata.clone(),
            )
        })
        .collect();

    for node in snapshot.asset_catalog.nodes.values_mut() {
        let VaultAssetPayload::SshConnection(spec) = &mut node.payload else {
            continue;
        };
        if let Some(identity_id) = &mut spec.keychain_identity_id
            && let Some(next_id) = remap.get(identity_id)
        {
            *identity_id = next_id.clone();
        }
    }
}

pub(super) fn sync_local_vault(
    state: &mut ShellViewModel,
    vault: &mut VaultSessionState,
    credential_store: &dyn CredentialStore,
) -> Result<()> {
    let known_hosts_path = vault.known_hosts_path();
    let bootstrap_state_path = vault.bootstrap_state_path();
    let cache_root = vault.cache_root();
    let local_state = vault
        .local_state
        .as_ref()
        .ok_or_else(|| anyhow!("vault bootstrap is not initialized"))?;
    let local_bundle = local_state.bundle.clone();
    let current_revision = local_state.current_revision.clone();
    let local_device_id = local_state.device_id.clone();
    let wrapped_vault_key = local_state.wrapped_vault_key.clone();
    let kdf = local_state.kdf.clone();
    let vault_key = vault
        .unlocked_vault_key
        .ok_or_else(|| anyhow!("vault is locked"))?;
    let snapshot = export_vault_snapshot(
        &combined_asset_tree(state),
        state.keychain_catalog(),
        credential_store,
        known_hosts_path.as_path(),
        sync_preferences_for_bundle(&local_bundle, None),
        &UiPreferences::from(&*state),
    )?;
    let current_encrypted_snapshot = encrypt_snapshot(&snapshot, &vault_key)?;
    let local_snapshot_hash =
        payload_hash_from_encrypted_snapshot_sha(&current_encrypted_snapshot.payload_sha256);
    let local_sync_state = local_sync_state_for_snapshot(local_state, local_snapshot_hash.clone());

    let primary_remote = local_bundle
        .primary_remote()
        .cloned()
        .ok_or_else(|| anyhow!("primary remote is not configured"))?;
    let primary_remote = resolve_remote_for_sync(&primary_remote, credential_store)?;
    let primary_provider = vault
        .provider_factory
        .build_provider_for_vault(&primary_remote, vault.root_dir.as_path())?;
    let primary_head = primary_provider
        .read_head()
        .map_err(|err| {
            anyhow!(
                "failed to inspect primary remote `{}`: {err}",
                primary_remote.remote_id
            )
        })?
        .head;
    let base_snapshot = vault
        .decrypted_snapshot
        .clone()
        .map(normalize_snapshot_secret_refs)
        .unwrap_or_default();
    let decision = decide_sync_action(&local_sync_state, primary_head.as_ref());
    match decision.action {
        SyncAction::Noop => {
            store_encrypted_cache(
                cache_root.as_path(),
                &local_bundle.vault_id,
                &current_encrypted_snapshot,
            )?;
            if let Some(remote_head) = primary_head.as_ref() {
                let local_state = vault
                    .local_state
                    .as_mut()
                    .ok_or_else(|| anyhow!("vault bootstrap is not initialized"))?;
                local_state.current_revision = Some(remote_head.vault_revision.clone());
                local_state.local_snapshot_hash = Some(local_snapshot_hash);
                if current_revision.as_deref() != Some(remote_head.vault_revision.as_str()) {
                    local_state.last_successful_pull_at = Some(remote_head.committed_at.clone());
                }
                local_state.last_sync_error = None;
                save_local_vault_bootstrap_state(bootstrap_state_path.as_path(), local_state)?;
                vault.decrypted_snapshot = Some(snapshot);
                update_vault_panel_for_local_state(state, vault);
                update_sync_modal_for_local_state(state, vault);
                state.vault_panel_state_mut().primary_status_label =
                    format!("Already synced {}", remote_head.vault_revision);
                state.sync_modal_state_mut().status_text = format!(
                    "Local and remote snapshots already match at {}.",
                    remote_head.vault_revision
                );
            } else {
                update_vault_panel_for_local_state(state, vault);
                update_sync_modal_for_local_state(state, vault);
                state.vault_panel_state_mut().primary_status_label = "Primary empty".into();
                state.sync_modal_state_mut().status_text =
                    "No remote revision exists yet. Run sync after the next local change.".into();
            }
            return Ok(());
        }
        SyncAction::PullOnly => {
            let remote_head = primary_head.ok_or_else(|| {
                anyhow!(
                    "primary remote `{}` is missing a readable head",
                    primary_remote.remote_id
                )
            })?;
            let remote_revision = primary_provider
                .read_revision(&remote_head)
                .map_err(|err| {
                    anyhow!(
                        "failed to read primary revision `{}` from remote `{}`: {err}",
                        remote_head.vault_revision,
                        primary_remote.remote_id
                    )
                })?;
            let remote_snapshot = normalize_snapshot_secret_refs(decrypt_snapshot(
                &remote_revision.encrypted_snapshot,
                &vault_key,
            )?);
            clear_vault_decrypted_state(
                state,
                vault.decrypted_snapshot.as_ref(),
                credential_store,
            )?;
            apply_vault_snapshot_to_shell(
                state,
                &remote_snapshot,
                credential_store,
                vault.known_hosts_path().as_path(),
            )?;
            let pulled_at = current_sync_timestamp();
            let local_state = vault
                .local_state
                .as_mut()
                .ok_or_else(|| anyhow!("vault bootstrap is not initialized"))?;
            local_state.wrapped_vault_key = remote_head.wrapped_vault_key.clone();
            local_state.kdf = remote_head.kdf.clone();
            local_state.current_revision = Some(remote_head.vault_revision.clone());
            local_state.local_snapshot_hash = Some(remote_head.payload_hash.clone());
            local_state.last_successful_pull_at = Some(pulled_at);
            local_state.last_sync_error = None;
            save_local_vault_bootstrap_state(bootstrap_state_path.as_path(), local_state)?;
            store_encrypted_cache(
                cache_root.as_path(),
                &local_bundle.vault_id,
                &remote_revision.encrypted_snapshot,
            )?;
            vault.decrypted_snapshot = Some(remote_snapshot);
            update_vault_panel_for_local_state(state, vault);
            update_sync_modal_for_local_state(state, vault);
            state.vault_panel_state_mut().primary_status_label =
                format!("Pulled primary {}", remote_head.vault_revision);
            state.sync_modal_state_mut().status_text = format!(
                "Pulled remote changes from primary {}.",
                remote_head.vault_revision
            );
            return Ok(());
        }
        SyncAction::PushOnly | SyncAction::MergeThenPush => {}
    }

    let mirror_providers = build_mirror_providers(&local_bundle, vault, credential_store)?;
    let mut snapshot_to_commit = snapshot.clone();
    let mut snapshot_to_display = snapshot.clone();
    let mut parent_revision = primary_head
        .as_ref()
        .map(|head| head.vault_revision.clone())
        .or(current_revision.clone());
    let mut merge_conflicts_present = false;

    if matches!(decision.action, SyncAction::MergeThenPush) {
        let remote_head = primary_head.clone().ok_or_else(|| {
            anyhow!(
                "primary remote `{}` is missing a readable head",
                primary_remote.remote_id
            )
        })?;
        let remote_revision = primary_provider
            .read_revision(&remote_head)
            .map_err(|err| {
                anyhow!(
                    "failed to read primary revision `{}` from remote `{}`: {err}",
                    remote_head.vault_revision,
                    primary_remote.remote_id
                )
            })?;
        let remote_snapshot = normalize_snapshot_secret_refs(decrypt_snapshot(
            &remote_revision.encrypted_snapshot,
            &vault_key,
        )?);
        let merge_remote_snapshot =
            prepare_remote_snapshot_for_merge(&base_snapshot, &snapshot, &remote_snapshot);
        let merge_result = merge_snapshots(MergeInput {
            base: base_snapshot,
            local: snapshot.clone(),
            remote: merge_remote_snapshot.clone(),
            device_id: local_device_id.clone(),
        });
        if !merge_result.conflicts.is_empty() {
            let captured_at = current_sync_timestamp();
            persist_merge_conflict_recovery_snapshots(
                vault.root_dir.join("recovery").as_path(),
                &local_bundle.vault_id,
                current_revision.clone(),
                &snapshot,
                &remote_head,
                &merge_remote_snapshot,
            )?;
            persist_merge_conflict_inbox_entries(
                vault.root_dir.join("conflicts").as_path(),
                &local_bundle.vault_id,
                merge_result.conflicts.as_slice(),
                local_device_id.as_str(),
                remote_head.device_id.as_str(),
                captured_at.as_str(),
            )?;
            merge_conflicts_present = true;
        }
        parent_revision = Some(remote_head.vault_revision.clone());
        snapshot_to_display = normalize_snapshot_secret_refs(merge_result.merged.clone());
        snapshot_to_commit = snapshot_to_display.clone();
    }

    let request = SyncRequest {
        vault_id: local_bundle.vault_id.clone(),
        snapshot: snapshot_to_commit.clone(),
        next_revision: next_vault_revision(parent_revision.as_deref()),
        parent_revision,
        device_id: local_device_id.clone(),
        committed_at: current_sync_timestamp(),
        committed_by_device: local_device_id,
        wrapped_vault_key,
        kdf,
        provider_kind: primary_remote.provider,
        vault_key,
    };
    let engine = SyncEngine::new(primary_provider, mirror_providers);

    match engine.sync(request) {
        Ok(report) => {
            let local_state = vault
                .local_state
                .as_mut()
                .ok_or_else(|| anyhow!("vault bootstrap is not initialized"))?;
            store_encrypted_cache(
                cache_root.as_path(),
                &local_bundle.vault_id,
                &report.encrypted_snapshot,
            )?;
            local_state.current_revision = Some(report.primary_revision.clone());
            local_state.local_snapshot_hash = Some(report.head.payload_hash.clone());
            local_state.last_successful_push_at = Some(report.head.committed_at.clone());
            local_state.last_sync_error = None;
            save_local_vault_bootstrap_state(bootstrap_state_path.as_path(), local_state)?;
            if matches!(decision.action, SyncAction::MergeThenPush) {
                clear_vault_decrypted_state(state, Some(&snapshot), credential_store)?;
                apply_vault_snapshot_to_shell(
                    state,
                    &snapshot_to_display,
                    credential_store,
                    vault.known_hosts_path().as_path(),
                )?;
            }
            vault.decrypted_snapshot = Some(snapshot_to_display.clone());
            update_vault_panel_for_local_state(state, vault);
            update_sync_modal_for_local_state(state, vault);
            let primary_status = if matches!(decision.action, SyncAction::MergeThenPush) {
                if merge_conflicts_present {
                    format!("Merged with conflicts {}", report.primary_revision)
                } else {
                    format!("Merged and synced {}", report.primary_revision)
                }
            } else {
                format!("Primary synced {}", report.primary_revision)
            };
            let sync_status = if matches!(decision.action, SyncAction::MergeThenPush) {
                if merge_conflicts_present {
                    format!(
                        "Merged local and remote changes with conflict copies saved locally. Primary is now at {}.",
                        report.primary_revision
                    )
                } else {
                    format!(
                        "Merged local and remote changes. Primary is now at {}.",
                        report.primary_revision
                    )
                }
            } else {
                format!(
                    "Sync completed. Primary is now at {}.",
                    report.primary_revision
                )
            };
            if report.is_mirror_degraded() {
                let mirror_degraded_message = format!(
                    "Mirror degraded: {}",
                    report
                        .mirror_failures
                        .first()
                        .map(|failure| failure.message.as_str())
                        .unwrap_or("unknown mirror failure")
                );
                state.vault_panel_state_mut().primary_status_label = primary_status.clone();
                state.sync_modal_state_mut().status_text =
                    format!("{} {}", sync_status, mirror_degraded_message);
            } else {
                state.vault_panel_state_mut().primary_status_label = primary_status;
                state.sync_modal_state_mut().status_text = sync_status;
            }
            Ok(())
        }
        Err(err) => {
            let bootstrap_state_path = vault.bootstrap_state_path();
            if let Some(local_state) = vault.local_state.as_mut() {
                local_state.last_sync_error = Some(err.to_string());
                save_local_vault_bootstrap_state(bootstrap_state_path.as_path(), local_state)?;
            }
            update_vault_panel_for_local_state(state, vault);
            update_sync_modal_for_local_state(state, vault);
            state.vault_panel_state_mut().primary_status_label = match &err {
                SyncError::PrimaryReadFailed { message, .. }
                | SyncError::PrimaryWriteFailed { message, .. } => {
                    format!("Provider auth error: {message}")
                }
                SyncError::Conflict { .. } => "Remote conflict".into(),
                SyncError::PayloadAssemblyFailed { message } => {
                    format!("Vault decrypt error: {message}")
                }
            };
            state.sync_modal_state_mut().error_text = err.to_string();
            Err(anyhow!(err.to_string()))
        }
    }
}

pub(super) fn refresh_local_vault_from_primary_remote_if_changed(
    state: &mut ShellViewModel,
    vault: &mut VaultSessionState,
    credential_store: &dyn CredentialStore,
) -> Result<bool> {
    let local_state = vault
        .local_state
        .as_ref()
        .ok_or_else(|| anyhow!("vault bootstrap is not initialized"))?;
    let current_revision = local_state.current_revision.clone();
    let primary_remote = local_state
        .bundle
        .primary_remote()
        .cloned()
        .ok_or_else(|| anyhow!("primary remote is not configured"))?;
    let primary_remote = resolve_remote_for_sync(&primary_remote, credential_store)?;
    let provider = vault
        .provider_factory
        .build_provider_for_vault(&primary_remote, vault.root_dir.as_path())?;
    let Some(remote_head) = provider
        .read_head()
        .map_err(|err| {
            anyhow!(
                "failed to inspect primary remote `{}`: {err}",
                primary_remote.remote_id
            )
        })?
        .head
    else {
        return Ok(false);
    };
    if current_revision.as_deref() == Some(remote_head.vault_revision.as_str()) {
        update_vault_panel_for_local_state(state, vault);
        update_sync_modal_for_local_state(state, vault);
        state.vault_panel_state_mut().primary_status_label =
            format!("Already synced {}", remote_head.vault_revision);
        state.sync_modal_state_mut().status_text = format!(
            "No remote changes found. Primary stays at {}.",
            remote_head.vault_revision
        );
        return Ok(false);
    }

    let remote_revision = provider.read_revision(&remote_head).map_err(|err| {
        anyhow!(
            "failed to read primary revision `{}` from remote `{}`: {err}",
            remote_head.vault_revision,
            primary_remote.remote_id
        )
    })?;
    let vault_key = vault
        .unlocked_vault_key
        .ok_or_else(|| anyhow!("vault is locked"))?;
    let snapshot = normalize_snapshot_secret_refs(decrypt_snapshot(
        &remote_revision.encrypted_snapshot,
        &vault_key,
    )?);
    clear_vault_decrypted_state(state, vault.decrypted_snapshot.as_ref(), credential_store)?;
    apply_vault_snapshot_to_shell(
        state,
        &snapshot,
        credential_store,
        vault.known_hosts_path().as_path(),
    )?;

    let bootstrap_state_path = vault.bootstrap_state_path();
    let cache_root = vault.cache_root();
    let pulled_at = current_sync_timestamp();
    let local_state = vault
        .local_state
        .as_mut()
        .ok_or_else(|| anyhow!("vault bootstrap is not initialized"))?;
    local_state.wrapped_vault_key = remote_head.wrapped_vault_key.clone();
    local_state.kdf = remote_head.kdf.clone();
    local_state.current_revision = Some(remote_head.vault_revision.clone());
    local_state.local_snapshot_hash = Some(remote_head.payload_hash.clone());
    local_state.last_successful_pull_at = Some(pulled_at);
    local_state.last_sync_error = None;
    save_local_vault_bootstrap_state(bootstrap_state_path.as_path(), local_state)?;
    store_encrypted_cache(
        cache_root.as_path(),
        &local_state.bundle.vault_id,
        &remote_revision.encrypted_snapshot,
    )?;
    vault.decrypted_snapshot = Some(snapshot);
    update_vault_panel_for_local_state(state, vault);
    update_sync_modal_for_local_state(state, vault);
    state.vault_panel_state_mut().primary_status_label =
        format!("Pulled primary {}", remote_head.vault_revision);
    state.sync_modal_state_mut().status_text = format!(
        "Pulled remote changes from primary {}.",
        remote_head.vault_revision
    );

    Ok(true)
}

pub(super) fn vault_sync_background_success(
    initial_state: &ShellViewModel,
    worker_state: ShellViewModel,
    worker_vault: VaultSessionState,
    should_clear_dirty: bool,
) -> VaultSyncBackgroundSuccess {
    let projection = (initial_state.console_asset_tree() != worker_state.console_asset_tree()
        || initial_state.snippet_asset_tree() != worker_state.snippet_asset_tree()
        || initial_state.keychain_catalog() != worker_state.keychain_catalog())
    .then(|| VaultProjectionUpdate {
        console_tree: worker_state.console_asset_tree().clone(),
        snippet_tree: worker_state.snippet_asset_tree().clone(),
        keychain_catalog: worker_state.keychain_catalog().clone(),
    });

    VaultSyncBackgroundSuccess {
        projection,
        sync_modal_state: worker_state.sync_modal_state().clone(),
        vault_panel_state: worker_state.vault_panel_state().clone(),
        local_state: worker_vault.local_state.clone(),
        decrypted_snapshot: worker_vault.decrypted_snapshot.clone(),
        should_clear_dirty,
    }
}

pub(super) fn vault_sync_background_failure(
    worker_state: ShellViewModel,
    worker_vault: VaultSessionState,
    should_clear_dirty: bool,
) -> VaultSyncBackgroundFailure {
    VaultSyncBackgroundFailure {
        sync_modal_state: worker_state.sync_modal_state().clone(),
        vault_panel_state: worker_state.vault_panel_state().clone(),
        local_state: worker_vault.local_state.clone(),
        should_clear_dirty,
    }
}

pub(super) fn vault_background_sync_ready(vault: &VaultSessionState) -> bool {
    vault.local_state.is_some() && vault.unlocked_vault_key.is_some()
}

pub(super) fn vault_requires_initial_remote_sync(vault: &VaultSessionState) -> bool {
    vault.local_state.as_ref().is_some_and(|local_state| {
        local_state.current_revision.is_none() && local_state.local_snapshot_hash.is_some()
    })
}

pub(super) fn mark_local_vault_dirty_and_arm_sync(
    state: &mut ShellViewModel,
    vault: &mut VaultSessionState,
    scheduler: &Rc<RefCell<VaultSyncSchedulerState>>,
    sync_debounce_timer: &Rc<Timer>,
    run_sync: Rc<dyn Fn(VaultSyncTrigger)>,
) {
    scheduler.borrow_mut().dirty = true;
    let bootstrap_state_path = vault.bootstrap_state_path();
    if let Some(local_state) = vault.local_state.as_mut() {
        local_state.last_local_change_at = Some(next_local_change_timestamp(local_state));
        if let Err(err) =
            save_local_vault_bootstrap_state(bootstrap_state_path.as_path(), local_state)
        {
            tracing::error!(
                target: "app.vault",
                error = %err,
                "failed to persist local vault sync metadata after local mutation"
            );
        }
    }
    state.sync_modal_state_mut().status_text = "Local changes queued for background sync.".into();

    if vault_background_sync_ready(vault) {
        sync_debounce_timer.start(
            TimerMode::SingleShot,
            Duration::from_millis(VAULT_AUTO_SYNC_DEBOUNCE_MS),
            move || {
                run_sync(VaultSyncTrigger::DebouncedAuto);
            },
        );
    }
}

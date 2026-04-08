//! ShellViewModel asset modal execution helpers.

use super::*;

impl ShellViewModel {
    pub fn can_confirm_asset_modal(&self) -> bool {
        match &self.asset_modal_state {
            Some(AssetModalState::NewFolder { .. })
            | Some(AssetModalState::SftpNewFolder { .. })
            | Some(AssetModalState::NewSnippet { .. })
            | Some(AssetModalState::NewSnippetPackage { .. })
            | Some(AssetModalState::NewKeychainIdentity { .. })
            | Some(AssetModalState::NewKeychainSshKey { .. })
            | Some(AssetModalState::NewSshConnection { .. }) => {
                self.asset_create_modal_can_confirm()
            }
            Some(AssetModalState::RenameAsset {
                asset_id,
                draft_name,
                ..
            }) => {
                self.rename_asset_modal_validation(asset_id, draft_name)
                    == AssetNameValidation::Valid
            }
            Some(AssetModalState::SftpRenameEntry {
                entry_id,
                draft_name,
                ..
            }) => {
                self.sftp_name_validation(draft_name, Some(entry_id.as_str()))
                    == AssetNameValidation::Valid
            }
            Some(AssetModalState::SftpDeleteEntriesConfirm { .. }) => true,
            Some(AssetModalState::DeleteAssetConfirm { .. }) => true,
            None => false,
        }
    }

    pub fn confirm_asset_modal(&mut self) -> bool {
        let Some(modal_state) = self.asset_modal_state.clone() else {
            return false;
        };

        let (parent_id, kind, draft_label, payload) = match modal_state {
            AssetModalState::NewFolder {
                parent_id,
                draft_name,
            } => (
                parent_id,
                ConsoleAssetKind::Folder,
                draft_name,
                AssetNodePayload::Folder,
            ),
            AssetModalState::SftpNewFolder { draft_name } => {
                if self.sftp_name_validation(&draft_name, None) != AssetNameValidation::Valid {
                    return false;
                }

                let Some(session_id) = self.active_workspace_session_id().map(str::to_string)
                else {
                    return false;
                };
                let path = sftp_child_path(self.sftp_panel_path().as_str(), draft_name.trim());
                let entry_id = format!("sftp-dir-{}", path);
                let next_entry = SftpDirectoryEntry {
                    id: entry_id.clone(),
                    name: draft_name.trim().to_string(),
                    path,
                    kind: crate::app::sftp::SftpDirectoryEntryKind::Directory,
                    modified_unix_seconds: None,
                    size_bytes: None,
                };

                if let Some(state) = self.sftp_sessions.get_mut(&session_id) {
                    state.entries.push(next_entry);
                    state.selected_entry_ids = vec![entry_id.clone()];
                }
                self.context_target_asset_id = Some(entry_id);
                self.asset_modal_state = None;
                return true;
            }
            AssetModalState::NewSnippet {
                parent_package_id,
                editing_asset_id,
                draft,
            } => {
                if !self
                    .snippet_modal_validation_message(
                        parent_package_id.as_deref(),
                        editing_asset_id.as_deref(),
                        &draft,
                    )
                    .is_empty()
                {
                    return false;
                }

                let resolved_parent_id =
                    self.resolve_snippet_package_id_by_label(draft.package.trim());
                if let Some(asset_id) = editing_asset_id {
                    self.snippet_asset_tree
                        .set_title(&asset_id, draft.name.trim().to_string());
                    if !self.snippet_asset_tree.set_snippet_spec(
                        &asset_id,
                        crate::shell::assets::AssetSnippetSpec {
                            script: draft.script,
                            package_id: resolved_parent_id,
                        },
                    ) {
                        return false;
                    }
                    self.selected_asset_ids = vec![asset_id.clone()];
                    self.focused_asset_id = Some(asset_id.clone());
                    self.context_target_asset_id = Some(asset_id);
                    self.asset_modal_state = None;
                    return true;
                }

                (
                    resolved_parent_id.clone(),
                    ConsoleAssetKind::Snippet,
                    draft.name,
                    AssetNodePayload::Snippet(crate::shell::assets::AssetSnippetSpec {
                        script: draft.script,
                        package_id: resolved_parent_id.clone(),
                    }),
                )
            }
            AssetModalState::NewSnippetPackage {
                editing_asset_id,
                draft_name,
            } => {
                if self.snippet_asset_tree.validate_name_in_parent(
                    None,
                    &draft_name,
                    editing_asset_id.as_deref(),
                ) != AssetNameValidation::Valid
                {
                    return false;
                }

                if let Some(asset_id) = editing_asset_id {
                    self.snippet_asset_tree
                        .set_title(&asset_id, draft_name.trim().to_string());
                    self.selected_asset_ids = vec![asset_id.clone()];
                    self.focused_asset_id = Some(asset_id.clone());
                    self.context_target_asset_id = Some(asset_id);
                    self.asset_modal_state = None;
                    return true;
                }

                (
                    None,
                    ConsoleAssetKind::SnippetPackage,
                    draft_name,
                    AssetNodePayload::SnippetPackage,
                )
            }
            AssetModalState::NewKeychainIdentity {
                parent_id,
                editing_item_id,
                draft,
            } => {
                if !self.keychain_identity_modal_can_confirm(
                    parent_id.as_deref(),
                    editing_item_id.as_deref(),
                    &draft,
                ) {
                    return false;
                }

                let item_id = if let Some(item_id) = editing_item_id {
                    let auth_kind = keychain_identity_auth_kind_from_id(draft.auth_kind.as_str());
                    if self
                        .rename_keychain_item(&item_id, draft.name.trim())
                        .is_err()
                    {
                        return false;
                    }
                    let credential_ref = (auth_kind == KeychainIdentityAuthKind::Password)
                        .then(|| keychain_identity_credential_ref(item_id.as_str()));
                    let spec = crate::app::keychain::KeychainIdentitySpec {
                        username: draft.username.trim().to_string(),
                        auth_kind,
                        ssh_key_id: (normalized_keychain_identity_auth_kind_id(
                            draft.auth_kind.as_str(),
                        ) == "ssh-key")
                            .then(|| draft.ssh_key_id.trim().to_string())
                            .filter(|value| !value.is_empty()),
                        credential_ref,
                        remark: draft.remark.trim().to_string(),
                    };
                    if let Some(node) = self.keychain_catalog.nodes.get_mut(&item_id) {
                        node.payload = KeychainNodePayload::Identity(spec);
                    }
                    item_id
                } else {
                    let item_id = create_keychain_node(
                        &mut self.keychain_catalog,
                        parent_id.as_deref(),
                        KeychainItemKind::Identity,
                        Some(draft.name.trim()),
                    );
                    let auth_kind = keychain_identity_auth_kind_from_id(draft.auth_kind.as_str());
                    let credential_ref = (auth_kind == KeychainIdentityAuthKind::Password)
                        .then(|| keychain_identity_credential_ref(item_id.as_str()));
                    let spec = crate::app::keychain::KeychainIdentitySpec {
                        username: draft.username.trim().to_string(),
                        auth_kind,
                        ssh_key_id: (normalized_keychain_identity_auth_kind_id(
                            draft.auth_kind.as_str(),
                        ) == "ssh-key")
                            .then(|| draft.ssh_key_id.trim().to_string())
                            .filter(|value| !value.is_empty()),
                        credential_ref,
                        remark: draft.remark.trim().to_string(),
                    };
                    if let Some(node) = self.keychain_catalog.nodes.get_mut(&item_id) {
                        node.payload = KeychainNodePayload::Identity(spec);
                    }
                    if let Some(parent_id) = parent_id {
                        self.keychain_expanded_ids.insert(parent_id);
                    }
                    item_id
                };

                self.selected_keychain_ids = vec![item_id.clone()];
                self.focused_keychain_id = Some(item_id.clone());
                self.context_target_asset_id = Some(item_id);
                self.asset_modal_state = None;
                return true;
            }
            AssetModalState::NewKeychainSshKey {
                parent_id,
                editing_item_id,
                draft,
            } => {
                if !self.keychain_ssh_key_modal_can_confirm(
                    parent_id.as_deref(),
                    editing_item_id.as_deref(),
                    &draft,
                ) {
                    return false;
                }

                let trimmed_public_key = draft.public_key.trim().to_string();
                let comment = trimmed_public_key
                    .split_whitespace()
                    .nth(2)
                    .unwrap_or_default()
                    .to_string();
                let algorithm = trimmed_public_key
                    .split_whitespace()
                    .next()
                    .unwrap_or_default()
                    .to_string();
                let item_id = if let Some(item_id) = editing_item_id {
                    if self
                        .rename_keychain_item(&item_id, draft.name.trim())
                        .is_err()
                    {
                        return false;
                    }
                    let credential_ref = (!draft.private_key.trim().is_empty())
                        .then(|| keychain_key_credential_ref(item_id.as_str()));
                    let spec = KeychainSshKeySpec {
                        algorithm,
                        fingerprint: draft.fingerprint.trim().to_string(),
                        public_key: trimmed_public_key,
                        comment,
                        credential_ref,
                        remark: String::new(),
                    };
                    if let Some(node) = self.keychain_catalog.nodes.get_mut(&item_id) {
                        node.payload = KeychainNodePayload::SshKey(spec);
                    }
                    item_id
                } else {
                    let item_id = create_keychain_node(
                        &mut self.keychain_catalog,
                        parent_id.as_deref(),
                        KeychainItemKind::SshKey,
                        Some(draft.name.trim()),
                    );
                    let credential_ref = (!draft.private_key.trim().is_empty())
                        .then(|| keychain_key_credential_ref(item_id.as_str()));
                    let spec = KeychainSshKeySpec {
                        algorithm,
                        fingerprint: draft.fingerprint.trim().to_string(),
                        public_key: trimmed_public_key,
                        comment,
                        credential_ref,
                        remark: String::new(),
                    };
                    if let Some(node) = self.keychain_catalog.nodes.get_mut(&item_id) {
                        node.payload = KeychainNodePayload::SshKey(spec);
                    }
                    if let Some(parent_id) = parent_id {
                        self.keychain_expanded_ids.insert(parent_id);
                    }
                    item_id
                };
                self.selected_keychain_ids = vec![item_id.clone()];
                self.focused_keychain_id = Some(item_id);
                self.asset_modal_state = None;
                self.pending_ssh_modal_action = None;
                self.ssh_modal_action_state = SshModalActionState::Idle;
                return true;
            }
            AssetModalState::NewSshConnection {
                parent_id,
                editing_asset_id,
                draft,
                ..
            } => {
                if self
                    .ssh_modal_submit_validation_message(
                        parent_id.as_deref(),
                        editing_asset_id.as_deref(),
                        &draft,
                    )
                    .is_some()
                {
                    return false;
                }

                let label = draft.name.trim().to_string();
                if let Some(asset_id) = editing_asset_id {
                    let existing_spec = self.console_asset_tree.ssh_connection_spec(&asset_id);
                    let payload = build_saved_ssh_connection_spec(&asset_id, &draft, existing_spec);

                    self.console_asset_tree
                        .set_title(&asset_id, label.trim().to_string());
                    if !self
                        .console_asset_tree
                        .set_ssh_connection_spec(&asset_id, payload)
                    {
                        return false;
                    }
                    self.selected_asset_ids = vec![asset_id.clone()];
                    self.focused_asset_id = Some(asset_id.clone());
                    self.context_target_asset_id = Some(asset_id);
                    self.asset_modal_state = None;
                    self.pending_ssh_modal_action = None;
                    self.ssh_modal_action_state = SshModalActionState::Idle;
                    return true;
                }

                let payload = AssetNodePayload::SshConnection(AssetSshConnectionSpec {
                    host: draft.host,
                    user: draft.user,
                    port: draft.port,
                    auth_method: draft.auth_method,
                    auth_source: SSH_AUTH_SOURCE_MANUAL.into(),
                    keychain_identity_id: None,
                    private_key_source: draft.private_key_source,
                    private_key_path: draft.private_key_path,
                    environment: draft.environment,
                    proxy: AssetSshProxySpec::None,
                    proxy_method: draft.proxy_method,
                    remark: draft.remark,
                    credential_ref: None,
                });
                (parent_id, ConsoleAssetKind::SshConnection, label, payload)
            }
            AssetModalState::SftpRenameEntry {
                entry_id,
                draft_name,
                ..
            } => {
                if self.sftp_name_validation(&draft_name, Some(entry_id.as_str()))
                    != AssetNameValidation::Valid
                {
                    return false;
                }

                let current_path = self.sftp_panel_path();
                let next_name = draft_name.trim().to_string();
                if let Some(state) = self.active_sftp_session_state_mut()
                    && let Some(entry) = state.entries.iter_mut().find(|entry| entry.id == entry_id)
                {
                    entry.name = next_name.clone();
                    entry.path = sftp_child_path(current_path.as_str(), next_name.as_str());
                    state.selected_entry_ids = vec![entry.id.clone()];
                    self.context_target_asset_id = Some(entry.id.clone());
                    self.asset_modal_state = None;
                    return true;
                }

                return false;
            }
            AssetModalState::RenameAsset {
                asset_id,
                draft_name,
                ..
            } => {
                if !self.can_confirm_asset_modal() {
                    return false;
                }

                if self.keychain_catalog.nodes.contains_key(&asset_id) {
                    if self
                        .rename_keychain_item(&asset_id, draft_name.trim())
                        .is_err()
                    {
                        return false;
                    }
                    self.focused_keychain_id = Some(asset_id.clone());
                    self.selected_keychain_ids = vec![asset_id.clone()];
                    self.focused_asset_id = None;
                    self.selected_asset_ids.clear();
                } else {
                    self.console_asset_tree
                        .set_title(&asset_id, draft_name.trim().to_string());
                    self.focused_asset_id = Some(asset_id.clone());
                    self.selected_asset_ids = vec![asset_id.clone()];
                    self.focused_keychain_id = None;
                    self.selected_keychain_ids.clear();
                }
                self.context_target_asset_id = Some(asset_id);
                self.asset_modal_state = None;
                return true;
            }
            AssetModalState::DeleteAssetConfirm { .. } => {
                return self.confirm_delete_asset();
            }
            AssetModalState::SftpDeleteEntriesConfirm { .. } => {
                return self.confirm_delete_asset();
            }
        };

        let use_snippet_tree = kind.domain() == crate::shell::assets::AssetDomain::Snippets;
        let label = if draft_label.trim().is_empty() {
            if use_snippet_tree {
                self.snippet_asset_tree
                    .next_default_name_for_parent(parent_id.as_deref(), kind)
            } else {
                self.console_asset_tree
                    .next_default_name_for_parent(parent_id.as_deref(), kind)
            }
        } else {
            let validation = if use_snippet_tree {
                self.snippet_asset_tree.validate_name_in_parent(
                    parent_id.as_deref(),
                    &draft_label,
                    None,
                )
            } else {
                self.create_asset_modal_validation(parent_id.as_deref(), &draft_label)
            };
            if validation != AssetNameValidation::Valid {
                return false;
            }
            draft_label.trim().to_string()
        };
        let asset_id = if use_snippet_tree {
            if let Some(parent_id) = parent_id.as_deref() {
                let asset_id = self
                    .snippet_asset_tree
                    .insert_child_with_payload(parent_id, kind, label, payload);
                self.snippet_asset_tree.set_expanded(parent_id, true);
                asset_id
            } else {
                self.snippet_asset_tree
                    .insert_root_with_payload(kind, label, payload)
            }
        } else {
            if let Some(parent_id) = parent_id.as_deref() {
                let asset_id = self
                    .console_asset_tree
                    .insert_child_with_payload(parent_id, kind, label, payload);
                self.console_asset_tree.set_expanded(parent_id, true);
                asset_id
            } else {
                self.console_asset_tree
                    .insert_root_with_payload(kind, label, payload)
            }
        };

        if let Some(AssetModalState::NewSshConnection { draft, .. }) = &self.asset_modal_state {
            let payload = build_saved_ssh_connection_spec(&asset_id, draft, None);
            let _ = self
                .console_asset_tree
                .set_ssh_connection_spec(&asset_id, payload);
        }

        self.selected_asset_ids = vec![asset_id.clone()];
        self.focused_asset_id = Some(asset_id.clone());
        self.context_target_asset_id = Some(asset_id);
        self.asset_modal_state = None;
        self.pending_ssh_modal_action = None;
        self.ssh_modal_action_state = SshModalActionState::Idle;
        true
    }
}

fn build_saved_ssh_connection_spec(
    asset_id: &str,
    draft: &AssetSshConnectionDraft,
    existing_spec: Option<&AssetSshConnectionSpec>,
) -> AssetSshConnectionSpec {
    let uses_saved_auth_secret = if draft.auth_source == SSH_AUTH_SOURCE_KEYCHAIN_IDENTITY {
        false
    } else {
        match draft.auth_method.as_str() {
            "password" => !draft.password.trim().is_empty(),
            "private-key" if draft.private_key_source == "content" => {
                !draft.private_key_content.trim().is_empty()
            }
            "private-key" if draft.private_key_source == "path" => {
                !draft.passphrase.trim().is_empty()
            }
            _ => false,
        }
    };
    let uses_saved_proxy_secret = matches!(draft.proxy_type.as_str(), "socks5" | "http")
        && !draft.proxy_socks5_password.trim().is_empty();
    let saved_secret_ref = (uses_saved_auth_secret || uses_saved_proxy_secret)
        .then(|| saved_ssh_credential_ref(asset_id, existing_spec));
    let credential_ref = if uses_saved_auth_secret || uses_saved_proxy_secret {
        saved_secret_ref.clone()
    } else {
        None
    };
    let mut proxy = build_draft_proxy_spec(draft);
    match &mut proxy {
        AssetSshProxySpec::Socks5(spec) | AssetSshProxySpec::Http(spec) => {
            spec.password_credential_ref = if uses_saved_proxy_secret {
                saved_secret_ref.clone()
            } else {
                None
            };
        }
        AssetSshProxySpec::None | AssetSshProxySpec::SshAsset { .. } => {}
    }

    AssetSshConnectionSpec {
        host: draft.host.clone(),
        user: if draft.auth_source == SSH_AUTH_SOURCE_KEYCHAIN_IDENTITY {
            String::new()
        } else {
            draft.user.clone()
        },
        port: draft.port.clone(),
        auth_method: draft.auth_method.clone(),
        auth_source: draft.auth_source.clone(),
        keychain_identity_id: (draft.auth_source == SSH_AUTH_SOURCE_KEYCHAIN_IDENTITY)
            .then(|| draft.keychain_identity_id.trim().to_string())
            .filter(|value| !value.is_empty()),
        private_key_source: draft.private_key_source.clone(),
        private_key_path: if draft.auth_source == SSH_AUTH_SOURCE_KEYCHAIN_IDENTITY
            || draft.private_key_source == "content"
        {
            String::new()
        } else {
            draft.private_key_path.clone()
        },
        environment: draft.environment.clone(),
        proxy,
        proxy_method: String::new(),
        remark: draft.remark.clone(),
        credential_ref,
    }
}

fn build_draft_proxy_spec(draft: &AssetSshConnectionDraft) -> AssetSshProxySpec {
    match draft.proxy_type.as_str() {
        "socks5" => AssetSshProxySpec::Socks5(AssetSocks5ProxySpec {
            host: draft.proxy_socks5_host.clone(),
            port: draft.proxy_socks5_port.clone(),
            username: draft.proxy_socks5_username.clone(),
            password_credential_ref: None,
        }),
        "http" => AssetSshProxySpec::Http(AssetSocks5ProxySpec {
            host: draft.proxy_socks5_host.clone(),
            port: draft.proxy_socks5_port.clone(),
            username: draft.proxy_socks5_username.clone(),
            password_credential_ref: None,
        }),
        "ssh-asset" => AssetSshProxySpec::SshAsset {
            asset_id: draft.proxy_ssh_asset_id.clone(),
        },
        _ => AssetSshProxySpec::None,
    }
}

fn saved_ssh_credential_ref(
    asset_id: &str,
    existing_spec: Option<&AssetSshConnectionSpec>,
) -> String {
    existing_spec
        .and_then(|spec| spec.credential_ref.clone())
        .unwrap_or_else(|| ssh_credential_ref(asset_id, SshCredentialKind::SavedSecrets))
}

pub(super) fn normalized_keychain_identity_auth_kind_id(value: &str) -> &'static str {
    match value.trim() {
        "ssh-key" => "ssh-key",
        _ => "password",
    }
}

fn keychain_identity_auth_kind_from_id(value: &str) -> KeychainIdentityAuthKind {
    match normalized_keychain_identity_auth_kind_id(value) {
        "ssh-key" => KeychainIdentityAuthKind::SshKey,
        _ => KeychainIdentityAuthKind::Password,
    }
}

pub(super) fn keychain_identity_auth_kind_id(value: KeychainIdentityAuthKind) -> &'static str {
    match value {
        KeychainIdentityAuthKind::Password => "password",
        KeychainIdentityAuthKind::SshKey => "ssh-key",
    }
}

pub fn welcome_actions() -> &'static [WelcomeAction] {
    &[
        WelcomeAction::NewConnection,
        WelcomeAction::OpenRecent,
        WelcomeAction::Snippets,
        WelcomeAction::Sftp,
    ]
}

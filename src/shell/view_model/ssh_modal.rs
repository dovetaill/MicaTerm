//! ShellViewModel SSH modal heavy-flow helpers.

use super::*;

impl ShellViewModel {
    pub fn update_ssh_modal_field(&mut self, field: &str, value: String) {
        let selected_proxy_asset_id = if field == "proxy_ssh_asset_label" {
            self.resolve_ssh_proxy_target_asset_id_from_label(value.as_str())
        } else if field == "keychain_identity_label" {
            self.resolve_ssh_keychain_identity_id_from_label(value.as_str())
        } else {
            None
        };
        let Some(AssetModalState::NewSshConnection { draft, .. }) = self.asset_modal_state.as_mut()
        else {
            return;
        };
        match field {
            "name" => draft.name = value,
            "host" => draft.host = value,
            "user" => draft.user = value,
            "port" => draft.port = value,
            "auth_source" => {
                if value == SSH_AUTH_SOURCE_MANUAL || value == SSH_AUTH_SOURCE_KEYCHAIN_IDENTITY {
                    draft.auth_source = value;
                    if draft.auth_source == SSH_AUTH_SOURCE_KEYCHAIN_IDENTITY {
                        draft.password.clear();
                        draft.private_key_content.clear();
                        draft.passphrase.clear();
                        draft.password_visible = false;
                    }
                }
            }
            "keychain_identity_label" => {
                draft.keychain_identity_id = selected_proxy_asset_id.unwrap_or_default();
            }
            "keychain_identity_id" => draft.keychain_identity_id = value,
            "auth_method" => {
                if matches!(value.as_str(), "password" | "private-key") {
                    draft.auth_method = value;
                }
            }
            "private_key_source" => {
                if matches!(value.as_str(), "content" | "path") {
                    draft.private_key_source = value;
                }
            }
            "password" => draft.password = value,
            "private_key_content" => {
                if !value.trim().is_empty() {
                    draft.auth_method = "private-key".into();
                    draft.private_key_source = "content".into();
                    draft.private_key_path.clear();
                }
                draft.private_key_content = value;
            }
            "private_key_path" => draft.private_key_path = value,
            "passphrase" => draft.passphrase = value,
            "password_visibility" => {
                draft.password_visible = matches!(value.as_str(), "visible" | "show" | "true");
            }
            "remark" => draft.remark = value,
            "environment" => draft.environment = value,
            "proxy_type" => {
                if matches!(value.as_str(), "none" | "socks5" | "http" | "ssh-asset") {
                    draft.proxy_type = value;
                }
            }
            "proxy_socks5_host" => draft.proxy_socks5_host = value,
            "proxy_socks5_port" => draft.proxy_socks5_port = value,
            "proxy_socks5_username" => draft.proxy_socks5_username = value,
            "proxy_socks5_password" => draft.proxy_socks5_password = value,
            "proxy_ssh_asset_label" => {
                draft.proxy_ssh_asset_id = selected_proxy_asset_id.unwrap_or_default();
            }
            "proxy_socks5_password_visibility" => {
                draft.proxy_socks5_password_visible =
                    matches!(value.as_str(), "visible" | "show" | "true");
            }
            "proxy_ssh_asset_id" => draft.proxy_ssh_asset_id = value,
            "proxy_method" => draft.proxy_method = value,
            _ => {}
        }

        draft.validation_message.clear();
        self.ssh_modal_action_state = SshModalActionState::Idle;
    }

    pub fn update_ssh_modal_name(&mut self, value: String) {
        self.update_ssh_modal_field("name", value);
    }

    pub fn update_ssh_modal_host(&mut self, value: String) {
        self.update_ssh_modal_field("host", value);
    }

    pub fn begin_ssh_modal_action(&mut self, action_id: &str) -> bool {
        if self.ssh_modal_is_busy() {
            return false;
        }

        let Some(AssetModalState::NewSshConnection {
            parent_id,
            editing_asset_id,
            draft,
            ..
        }) = self.asset_modal_state.as_ref()
        else {
            return false;
        };

        let draft = draft.clone();
        let validation_message = self.ssh_modal_submit_validation_message(
            parent_id.as_deref(),
            editing_asset_id.as_deref(),
            &draft,
        );

        if let Some(AssetModalState::NewSshConnection { draft, .. }) =
            self.asset_modal_state.as_mut()
        {
            draft.validation_message = validation_message.clone().unwrap_or_default();
        }

        self.pending_ssh_modal_action = None;
        self.ssh_modal_action_state = SshModalActionState::Idle;
        if validation_message.is_some() {
            return false;
        }

        let action = match action_id {
            "save" => SshModalAction::Save,
            "connect" => SshModalAction::Connect,
            "test" => SshModalAction::TestConnection,
            "save-and-connect" => SshModalAction::SaveAndConnect,
            _ => {
                return false;
            }
        };

        if matches!(
            action,
            SshModalAction::Connect
                | SshModalAction::TestConnection
                | SshModalAction::SaveAndConnect
        ) && !self.ssh_modal_connect_family_enabled()
        {
            return false;
        }

        self.pending_ssh_modal_action = Some(PendingSshModalAction { action, draft });
        self.ssh_modal_action_state = SshModalActionState::Busy(action);
        true
    }


}

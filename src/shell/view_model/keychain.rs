//! ShellViewModel keychain domain impls.

use super::*;
use super::asset_modal_executor::{
    keychain_identity_auth_kind_id, normalized_keychain_identity_auth_kind_id,
};

impl ShellViewModel {
    pub fn keychain_catalog(&self) -> &KeychainCatalog {
        &self.keychain_catalog
    }

    pub fn replace_keychain_catalog(&mut self, catalog: KeychainCatalog) {
        self.keychain_expanded_ids = catalog
            .root_ids
            .iter()
            .filter(|node_id| {
                catalog.nodes.get(*node_id).is_some_and(|node| {
                    matches!(
                        node.payload,
                        crate::app::keychain::KeychainNodePayload::Folder
                    )
                })
            })
            .cloned()
            .collect();
        self.keychain_catalog = catalog;
        self.selected_keychain_ids.clear();
        self.focused_keychain_id = None;
        self.keychain_search_query.clear();
    }

    pub fn set_keychain_search_query(&mut self, query: String) {
        self.keychain_search_query = query;
    }

    pub fn toggle_keychain_folder_expanded(&mut self, item_id: &str) {
        if !self
            .keychain_catalog
            .nodes
            .get(item_id)
            .is_some_and(|node| {
                matches!(
                    node.payload,
                    crate::app::keychain::KeychainNodePayload::Folder
                )
            })
        {
            return;
        }

        if !self.keychain_expanded_ids.insert(item_id.to_string()) {
            self.keychain_expanded_ids.remove(item_id);
        }
    }

    pub fn select_keychain_item(&mut self, item_id: &str) {
        if !self.keychain_catalog.nodes.contains_key(item_id) {
            return;
        }

        self.selected_keychain_ids = vec![item_id.to_string()];
        self.focused_keychain_id = Some(item_id.to_string());
        self.asset_create_menu_open = false;
    }

    pub fn create_keychain_item(
        &mut self,
        parent_id: Option<String>,
        kind: KeychainItemKind,
    ) -> String {
        let parent_id = self.normalize_keychain_folder_parent_id(parent_id);
        let item_id =
            create_keychain_node(&mut self.keychain_catalog, parent_id.as_deref(), kind, None);
        if let Some(parent_id) = parent_id {
            self.keychain_expanded_ids.insert(parent_id);
        }
        self.selected_keychain_ids = vec![item_id.clone()];
        self.focused_keychain_id = Some(item_id.clone());
        self.asset_create_menu_open = false;
        item_id
    }

    pub fn rename_keychain_item(&mut self, item_id: &str, title: &str) -> anyhow::Result<()> {
        rename_keychain_node(&mut self.keychain_catalog, item_id, title)
    }

    pub fn delete_keychain_item(&mut self, item_id: &str) -> Result<bool, KeychainDeleteError> {
        let removed = delete_keychain_node(
            &mut self.keychain_catalog,
            item_id,
            &self.console_asset_tree,
        )?;
        if removed.removed_ids.is_empty() {
            return Ok(false);
        }

        self.selected_keychain_ids.retain(|selected_id| {
            !removed
                .removed_ids
                .iter()
                .any(|removed_id| removed_id == selected_id)
        });
        if self
            .focused_keychain_id
            .as_deref()
            .is_some_and(|focused_id| {
                removed
                    .removed_ids
                    .iter()
                    .any(|removed_id| removed_id == focused_id)
            })
        {
            self.focused_keychain_id = None;
        }
        Ok(true)
    }

    pub fn open_new_keychain_identity_modal(&mut self, parent_id: Option<String>) {
        let parent_id = self.normalize_keychain_folder_parent_id(parent_id);
        self.dismiss_active_asset_rename();
        self.close_context_menu();
        self.close_asset_create_menu();
        self.context_target_asset_id = parent_id.clone();
        self.asset_modal_state = Some(AssetModalState::NewKeychainIdentity {
            editing_item_id: None,
            draft: KeychainIdentityDraft {
                name: next_default_keychain_name_for_parent(
                    &self.keychain_catalog,
                    parent_id.as_deref(),
                    KeychainItemKind::Identity,
                ),
                auth_kind: "password".into(),
                ..KeychainIdentityDraft::default()
            },
            parent_id,
        });
    }

    pub fn open_edit_keychain_identity_modal(
        &mut self,
        item_id: String,
        password: Option<String>,
    ) {
        let Some(node) = self.keychain_catalog.nodes.get(&item_id).cloned() else {
            return;
        };
        let KeychainNodePayload::Identity(spec) = node.payload.clone() else {
            return;
        };

        let ssh_key_id = spec.ssh_key_id.unwrap_or_default();
        let ssh_key_label = self
            .keychain_ssh_key_options()
            .into_iter()
            .find(|option| option.key_id == ssh_key_id)
            .map(|option| option.label)
            .unwrap_or_else(|| ssh_key_id.clone());

        self.dismiss_active_asset_rename();
        self.close_context_menu();
        self.close_asset_create_menu();
        self.focused_keychain_id = Some(item_id.clone());
        self.selected_keychain_ids = vec![item_id.clone()];
        self.context_target_asset_id = Some(item_id.clone());
        let auth_kind = keychain_identity_auth_kind_id(spec.auth_kind).to_string();
        self.asset_modal_state = Some(AssetModalState::NewKeychainIdentity {
            parent_id: node.parent_id,
            editing_item_id: Some(item_id),
            draft: KeychainIdentityDraft {
                name: node.title,
                username: spec.username,
                auth_kind: auth_kind.clone(),
                password: if auth_kind == "password" {
                    password.unwrap_or_default()
                } else {
                    String::new()
                },
                ssh_key_id,
                ssh_key_label,
                remark: spec.remark,
            },
        });
    }

    pub fn open_new_keychain_ssh_key_modal(&mut self, parent_id: Option<String>) {
        let parent_id = self.normalize_keychain_folder_parent_id(parent_id);
        self.dismiss_active_asset_rename();
        self.close_context_menu();
        self.close_asset_create_menu();
        self.asset_modal_state = Some(AssetModalState::NewKeychainSshKey {
            editing_item_id: None,
            draft: KeychainSshKeyDraft {
                name: next_default_keychain_name_for_parent(
                    &self.keychain_catalog,
                    parent_id.as_deref(),
                    KeychainItemKind::SshKey,
                ),
                ..KeychainSshKeyDraft::default()
            },
            parent_id,
        });
    }

    pub fn open_edit_keychain_ssh_key_modal(&mut self, item_id: String, private_key: Option<String>) {
        let Some(node) = self.keychain_catalog.nodes.get(&item_id).cloned() else {
            return;
        };
        let KeychainNodePayload::SshKey(spec) = node.payload.clone() else {
            return;
        };

        self.dismiss_active_asset_rename();
        self.close_context_menu();
        self.close_asset_create_menu();
        self.focused_keychain_id = Some(item_id.clone());
        self.selected_keychain_ids = vec![item_id.clone()];
        self.context_target_asset_id = Some(item_id.clone());
        self.asset_modal_state = Some(AssetModalState::NewKeychainSshKey {
            parent_id: node.parent_id,
            editing_item_id: Some(item_id),
            draft: KeychainSshKeyDraft {
                name: node.title,
                private_key: private_key.unwrap_or_default(),
                public_key: spec.public_key,
                fingerprint: spec.fingerprint,
            },
        });
    }

    pub fn update_keychain_identity_modal_field(&mut self, field: &str, value: String) {
        let Some(AssetModalState::NewKeychainIdentity { draft, .. }) =
            self.asset_modal_state.as_mut()
        else {
            return;
        };

        match field {
            "name" => draft.name = value,
            "username" => draft.username = value,
            "auth_kind" => {
                let next_auth_kind = normalized_keychain_identity_auth_kind_id(value.as_str());
                if draft.auth_kind != next_auth_kind {
                    draft.auth_kind = next_auth_kind.to_string();
                    if next_auth_kind == "password" {
                        draft.ssh_key_id.clear();
                        draft.ssh_key_label.clear();
                    } else {
                        draft.password.clear();
                    }
                }
            }
            "password" => draft.password = value,
            "ssh_key_id" => draft.ssh_key_id = value,
            "ssh_key_label" => draft.ssh_key_label = value,
            "remark" => draft.remark = value,
            _ => {}
        }
    }

    pub fn hydrate_edit_keychain_identity_secret(&mut self, password: Option<String>) {
        let Some(AssetModalState::NewKeychainIdentity {
            editing_item_id: Some(_),
            draft,
            ..
        }) = self.asset_modal_state.as_mut()
        else {
            return;
        };

        draft.password = if draft.auth_kind == "password" {
            password.unwrap_or_default()
        } else {
            String::new()
        };
    }

    pub fn select_first_keychain_identity_modal_ssh_key(&mut self) -> bool {
        let Some(option) = self.keychain_ssh_key_options().into_iter().next() else {
            return false;
        };
        let Some(AssetModalState::NewKeychainIdentity { draft, .. }) =
            self.asset_modal_state.as_mut()
        else {
            return false;
        };
        draft.ssh_key_id = option.key_id;
        draft.ssh_key_label = option.label;
        true
    }

    pub fn update_keychain_ssh_key_modal_field(&mut self, field: &str, value: String) {
        let Some(AssetModalState::NewKeychainSshKey { draft, .. }) =
            self.asset_modal_state.as_mut()
        else {
            return;
        };

        match field {
            "name" => draft.name = value,
            "private_key" => draft.private_key = value,
            "public_key" => draft.public_key = value,
            "fingerprint" => draft.fingerprint = value,
            _ => {}
        }
    }

    pub fn hydrate_edit_keychain_ssh_key_secret(&mut self, private_key: Option<String>) {
        let Some(AssetModalState::NewKeychainSshKey {
            editing_item_id: Some(_),
            draft,
            ..
        }) = self.asset_modal_state.as_mut()
        else {
            return;
        };

        draft.private_key = private_key.unwrap_or_default();
    }
}

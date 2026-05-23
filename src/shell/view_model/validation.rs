//! ShellViewModel validation domain impls.

use super::*;

impl ShellViewModel {
    pub(super) fn sftp_name_validation_for_session(
        &self,
        file_browser_session_id: &str,
        draft_name: &str,
        editing_entry_id: Option<&str>,
    ) -> AssetNameValidation {
        let trimmed = draft_name.trim();
        if trimmed.is_empty() {
            return AssetNameValidation::Empty;
        }

        let duplicate = self
            .file_browser_sessions
            .get(file_browser_session_id)
            .into_iter()
            .flat_map(|state| state.entries.iter())
            .filter(|entry| Some(entry.id.as_str()) != editing_entry_id)
            .any(|entry| entry.name.trim() == trimmed);

        if duplicate {
            AssetNameValidation::Duplicate
        } else {
            AssetNameValidation::Valid
        }
    }

    pub(super) fn create_asset_modal_validation(
        &self,
        parent_id: Option<&str>,
        draft_name: &str,
    ) -> AssetNameValidation {
        self.console_asset_tree
            .validate_name_in_parent(parent_id, draft_name, None)
    }

    pub(super) fn rename_asset_modal_validation(
        &self,
        asset_id: &str,
        draft_name: &str,
    ) -> AssetNameValidation {
        if self.console_asset_tree.contains(asset_id) {
            let parent_id = self.console_asset_tree.parent_id(asset_id).flatten();
            return self.console_asset_tree.validate_name_in_parent(
                parent_id,
                draft_name,
                Some(asset_id),
            );
        }

        if let Some(node) = self.keychain_catalog.nodes.get(asset_id) {
            return self.keychain_name_validation(
                node.parent_id.as_deref(),
                draft_name,
                Some(asset_id),
            );
        }

        AssetNameValidation::Empty
    }

    pub(super) fn sftp_name_validation(
        &self,
        draft_name: &str,
        editing_entry_id: Option<&str>,
    ) -> AssetNameValidation {
        if let Some(file_browser_session_id) = self.active_file_browser_session_id() {
            self.sftp_name_validation_for_session(
                file_browser_session_id,
                draft_name,
                editing_entry_id,
            )
        } else if draft_name.trim().is_empty() {
            AssetNameValidation::Empty
        } else {
            AssetNameValidation::Valid
        }
    }

    pub fn asset_rename_modal_validation_message(&self) -> String {
        match &self.asset_modal_state {
            Some(AssetModalState::RenameAsset {
                asset_id,
                draft_name,
                ..
            }) => asset_name_validation_message(
                self.rename_asset_modal_validation(asset_id, draft_name),
            ),
            Some(AssetModalState::SftpRenameEntry {
                file_browser_session_id,
                entry_id,
                draft_name,
                ..
            }) => asset_name_validation_message(self.sftp_name_validation_for_session(
                file_browser_session_id,
                draft_name,
                Some(entry_id.as_str()),
            )),
            _ => String::new(),
        }
    }

    pub fn asset_create_modal_validation_message(&self) -> String {
        match &self.asset_modal_state {
            Some(AssetModalState::NewFolder {
                parent_id,
                draft_name,
            }) => asset_name_validation_message(
                self.create_asset_modal_validation(parent_id.as_deref(), draft_name),
            ),
            Some(AssetModalState::NewSnippet {
                parent_package_id,
                editing_asset_id,
                draft,
            }) => self.snippet_modal_validation_message(
                parent_package_id.as_deref(),
                editing_asset_id.as_deref(),
                draft,
            ),
            Some(AssetModalState::NewSnippetPackage {
                editing_asset_id,
                draft_name,
            }) => asset_name_validation_message(self.snippet_asset_tree.validate_name_in_parent(
                None,
                draft_name,
                editing_asset_id.as_deref(),
            )),
            Some(AssetModalState::NewKeychainIdentity {
                parent_id,
                editing_item_id,
                draft,
            }) => self.keychain_identity_modal_validation_message(
                parent_id.as_deref(),
                editing_item_id.as_deref(),
                draft,
            ),
            Some(AssetModalState::NewKeychainSshKey {
                parent_id,
                editing_item_id,
                draft,
            }) => self.keychain_ssh_key_modal_validation_message(
                parent_id.as_deref(),
                editing_item_id.as_deref(),
                draft,
            ),
            Some(AssetModalState::NewSshConnection {
                parent_id,
                editing_asset_id,
                draft,
                ..
            }) => self.ssh_modal_validation_message(
                parent_id.as_deref(),
                editing_asset_id.as_deref(),
                draft,
            ),
            Some(AssetModalState::SftpNewFile { draft_name }) => {
                asset_name_validation_message(self.sftp_name_validation(draft_name, None))
            }
            Some(AssetModalState::SftpNewFolder { draft_name }) => {
                asset_name_validation_message(self.sftp_name_validation(draft_name, None))
            }
            _ => String::new(),
        }
    }

    pub fn asset_create_modal_can_confirm(&self) -> bool {
        match &self.asset_modal_state {
            Some(AssetModalState::NewFolder {
                parent_id,
                draft_name,
            }) => {
                self.create_asset_modal_validation(parent_id.as_deref(), draft_name)
                    == AssetNameValidation::Valid
            }
            Some(AssetModalState::NewSnippet {
                parent_package_id,
                editing_asset_id,
                draft,
            }) => self
                .snippet_modal_validation_message(
                    parent_package_id.as_deref(),
                    editing_asset_id.as_deref(),
                    draft,
                )
                .is_empty(),
            Some(AssetModalState::NewSnippetPackage {
                editing_asset_id,
                draft_name,
            }) => {
                self.snippet_asset_tree.validate_name_in_parent(
                    None,
                    draft_name,
                    editing_asset_id.as_deref(),
                ) == AssetNameValidation::Valid
            }
            Some(AssetModalState::NewKeychainIdentity {
                parent_id,
                editing_item_id,
                draft,
            }) => self.keychain_identity_modal_can_confirm(
                parent_id.as_deref(),
                editing_item_id.as_deref(),
                draft,
            ),
            Some(AssetModalState::NewKeychainSshKey {
                parent_id,
                editing_item_id,
                draft,
            }) => self.keychain_ssh_key_modal_can_confirm(
                parent_id.as_deref(),
                editing_item_id.as_deref(),
                draft,
            ),
            Some(AssetModalState::NewSshConnection {
                parent_id,
                editing_asset_id,
                draft,
                ..
            }) => {
                self.ssh_modal_can_confirm(parent_id.as_deref(), editing_asset_id.as_deref(), draft)
            }
            Some(AssetModalState::SftpNewFile { draft_name }) => {
                self.sftp_name_validation(draft_name, None) == AssetNameValidation::Valid
            }
            Some(AssetModalState::SftpNewFolder { draft_name }) => {
                self.sftp_name_validation(draft_name, None) == AssetNameValidation::Valid
            }
            _ => false,
        }
    }
}

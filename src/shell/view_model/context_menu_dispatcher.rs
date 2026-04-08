//! ShellViewModel context menu dispatch helpers.

use super::*;

impl ShellViewModel {
    pub fn handle_context_menu_leaf_action(&mut self, action_id: &str) {
        if self
            .context_menu_target_kind
            .is_some_and(is_sftp_context_target)
        {
            self.handle_sftp_context_menu_leaf_action(action_id);
            return;
        }

        if matches!(action_id, "new-folder" | "new-identity" | "new-ssh-key")
            && self
                .context_menu_target_kind
                .is_some_and(is_keychain_context_target)
        {
            let parent_id = match (
                self.context_menu_target_kind,
                self.context_target_asset_id.as_deref(),
            ) {
                (Some(ContextTargetKind::KeychainFolder), Some(asset_id))
                    if self.keychain_catalog.nodes.contains_key(asset_id) =>
                {
                    Some(asset_id.to_string())
                }
                _ => None,
            };

            match action_id {
                "new-folder" => {
                    self.close_context_menu();
                    self.create_keychain_item(parent_id, KeychainItemKind::Folder);
                }
                "new-identity" => self.open_new_keychain_identity_modal(parent_id),
                "new-ssh-key" => self.open_new_keychain_ssh_key_modal(parent_id),
                _ => {}
            }
            return;
        }

        if matches!(
            action_id,
            "new-snippet" | "new-package" | "new-snippet-package"
        ) {
            match action_id {
                "new-snippet" => {
                    let parent_id = match (
                        self.context_menu_target_kind,
                        self.context_target_asset_id.as_deref(),
                    ) {
                        (Some(ContextTargetKind::SnippetPackage), Some(asset_id))
                            if self.snippet_asset_tree.contains(asset_id) =>
                        {
                            Some(asset_id.to_string())
                        }
                        _ => None,
                    };
                    self.open_new_snippet_modal(parent_id);
                }
                "new-package" | "new-snippet-package" => self.open_new_snippet_package_modal(),
                _ => {}
            }
            return;
        }

        if ConsoleAssetKind::from_create_action_id(action_id).is_some() {
            let parent_id = match (
                self.context_menu_target_kind,
                self.context_target_asset_id.as_deref(),
            ) {
                (Some(ContextTargetKind::Folder), Some(asset_id))
                    if self.console_asset_tree.contains(asset_id) =>
                {
                    Some(asset_id.to_string())
                }
                _ => None,
            };

            match action_id {
                "new-folder" => self.open_new_folder_modal(parent_id),
                "new-ssh-connection" => self.open_new_ssh_modal(parent_id),
                _ => {}
            }
            return;
        }

        let roots = self.context_menu_roots();
        let Some(action) = find_action_node_by_id(&roots, action_id) else {
            return;
        };

        match action.state {
            ContextMenuActionState::Planned => {
                self.set_context_menu_feedback(format!("{} is not wired yet.", action.label));
            }
            ContextMenuActionState::Enabled => match action_id {
                "edit-connection" => {
                    if let Some(asset_id) = self
                        .context_target_asset_id
                        .clone()
                        .filter(|asset_id| self.console_asset_tree.contains(asset_id))
                    {
                        self.open_edit_ssh_modal(asset_id);
                    } else {
                        self.close_context_menu();
                    }
                }
                "rename-asset" => {
                    if let Some(asset_id) =
                        self.context_target_asset_id.clone().filter(|asset_id| {
                            self.console_asset_tree.contains(asset_id)
                                || self.keychain_catalog.nodes.contains_key(asset_id)
                        })
                    {
                        if self.console_asset_tree.kind(&asset_id)
                            == Some(ConsoleAssetKind::SshConnection)
                        {
                            self.open_edit_ssh_modal(asset_id);
                        } else {
                            self.open_rename_asset_modal(asset_id);
                        }
                    } else {
                        self.close_context_menu();
                    }
                }
                "delete-asset" => {
                    if let Some(asset_id) =
                        self.context_target_asset_id.clone().filter(|asset_id| {
                            self.console_asset_tree.contains(asset_id)
                                || self.keychain_catalog.nodes.contains_key(asset_id)
                                || self.snippet_asset_tree.contains(asset_id)
                        })
                    {
                        self.open_delete_asset_confirm(asset_id);
                    } else {
                        self.close_context_menu();
                    }
                }
                "edit-snippet" => {
                    if let Some(asset_id) =
                        self.context_target_asset_id.clone().filter(|asset_id| {
                            self.snippet_asset_tree.kind(asset_id)
                                == Some(ConsoleAssetKind::Snippet)
                        })
                    {
                        self.open_edit_snippet_modal(asset_id);
                    } else {
                        self.close_context_menu();
                    }
                }
                "edit-package" => {
                    if let Some(asset_id) =
                        self.context_target_asset_id.clone().filter(|asset_id| {
                            self.snippet_asset_tree.kind(asset_id)
                                == Some(ConsoleAssetKind::SnippetPackage)
                        })
                    {
                        self.open_edit_snippet_package_modal(asset_id);
                    } else {
                        self.close_context_menu();
                    }
                }
                "edit-keychain-identity" => {
                    if let Some(item_id) = self.context_target_asset_id.clone().filter(|item_id| {
                        self.keychain_catalog
                            .nodes
                            .get(item_id)
                            .is_some_and(|node| {
                                matches!(node.payload, KeychainNodePayload::Identity(_))
                            })
                    }) {
                        self.open_edit_keychain_identity_modal(item_id, None);
                    } else {
                        self.close_context_menu();
                    }
                }
                "edit-keychain-ssh-key" => {
                    if let Some(item_id) = self.context_target_asset_id.clone().filter(|item_id| {
                        self.keychain_catalog
                            .nodes
                            .get(item_id)
                            .is_some_and(|node| {
                                matches!(node.payload, KeychainNodePayload::SshKey(_))
                            })
                    }) {
                        self.open_edit_keychain_ssh_key_modal(item_id, None);
                    } else {
                        self.close_context_menu();
                    }
                }
                "delete-snippet" | "delete-package" => {
                    if let Some(asset_id) = self
                        .context_target_asset_id
                        .clone()
                        .filter(|asset_id| self.snippet_asset_tree.contains(asset_id))
                    {
                        self.open_delete_asset_confirm(asset_id);
                    } else {
                        self.close_context_menu();
                    }
                }
                "paste-snippet" => {
                    if let Some(asset_id) =
                        self.context_target_asset_id.clone().filter(|asset_id| {
                            self.snippet_asset_tree.kind(asset_id)
                                == Some(ConsoleAssetKind::Snippet)
                        })
                    {
                        self.begin_snippet_activation(&asset_id, SnippetActivation::Paste);
                        self.close_context_menu();
                    } else {
                        self.close_context_menu();
                    }
                }
                "run-snippet" => {
                    if let Some(asset_id) =
                        self.context_target_asset_id.clone().filter(|asset_id| {
                            self.snippet_asset_tree.kind(asset_id)
                                == Some(ConsoleAssetKind::Snippet)
                        })
                    {
                        self.begin_snippet_activation(&asset_id, SnippetActivation::Run);
                        self.close_context_menu();
                    } else {
                        self.close_context_menu();
                    }
                }
                _ => self.close_context_menu(),
            },
            ContextMenuActionState::Disabled => {}
        }
    }

    fn handle_sftp_context_menu_leaf_action(&mut self, action_id: &str) {
        match action_id {
            "new-folder" => self.open_sftp_new_folder_modal(),
            "rename-sftp-entry" => {
                if let Some(entry_id) = self.context_target_asset_id.clone() {
                    self.open_sftp_rename_entry_modal(entry_id);
                } else {
                    self.close_context_menu();
                }
            }
            "delete-sftp-entry" => {
                if let Some(entry_id) = self.context_target_asset_id.clone() {
                    self.open_sftp_delete_confirm(vec![entry_id]);
                } else {
                    self.close_context_menu();
                }
            }
            "delete-selected" => {
                let entry_ids = self.sftp_panel_selected_entry_ids().to_vec();
                self.open_sftp_delete_confirm(entry_ids);
            }
            "refresh-sftp" => {
                let _ = self.refresh_sftp_panel();
                self.close_context_menu();
            }
            _ => self.close_context_menu(),
        }
    }

    pub(super) fn context_menu_roots(&self) -> Vec<ContextMenuActionNode> {
        let Some(target_kind) = self.context_menu_target_kind else {
            return Vec::new();
        };

        resolve_action_tree(target_kind, &self.context_menu_selection())
    }
}

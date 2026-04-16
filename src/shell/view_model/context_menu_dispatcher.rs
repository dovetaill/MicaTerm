//! ShellViewModel context menu dispatch helpers.

use super::*;

impl ShellViewModel {
    pub fn handle_context_menu_leaf_action(&mut self, action_id: &str) {
        if self
            .context_menu_target_kind
            .is_some_and(is_sftp_context_target)
        {
            let roots = self.context_menu_roots();
            let Some(action) = find_action_node_by_id(&roots, action_id) else {
                return;
            };

            match action.state {
                ContextMenuActionState::Enabled => {
                    self.handle_sftp_context_menu_leaf_action(action_id);
                }
                ContextMenuActionState::Planned => {
                    self.handle_planned_sftp_context_menu_action(action_id, action.label);
                }
                ContextMenuActionState::Disabled => {
                    self.handle_disabled_sftp_context_menu_action(action_id);
                }
            }
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
            "open-remote" => {
                if let Some(entry_id) = self.context_target_asset_id.clone() {
                    self.pending_sftp_context_action =
                        Some(PendingSftpContextAction::OpenRemote { entry_id });
                    self.close_context_menu();
                }
            }
            "open-local" => {
                if let Some(entry_id) = self.context_target_asset_id.clone() {
                    self.pending_sftp_context_action =
                        Some(PendingSftpContextAction::OpenLocal { entry_id });
                    self.close_context_menu();
                }
            }
            "edit-locally" => {
                if let Some(entry_id) = self.context_target_asset_id.clone() {
                    self.pending_sftp_context_action =
                        Some(PendingSftpContextAction::EditLocally { entry_id });
                    self.close_context_menu();
                }
            }
            "new-folder" => self.open_sftp_new_folder_modal(),
            "upload-files" => {
                self.pending_sftp_context_action = Some(PendingSftpContextAction::UploadFiles);
                self.close_context_menu();
            }
            "upload-folder" => {
                self.pending_sftp_context_action = Some(PendingSftpContextAction::UploadFolder);
                self.close_context_menu();
            }
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
            "download" => {
                let entry_ids = self
                    .context_target_asset_id
                    .clone()
                    .into_iter()
                    .collect::<Vec<_>>();
                if !entry_ids.is_empty() {
                    self.pending_sftp_context_action =
                        Some(PendingSftpContextAction::DownloadSelection { entry_ids });
                    self.close_context_menu();
                }
            }
            "download-selected" => {
                let entry_ids = self.sftp_panel_selected_entry_ids().to_vec();
                if !entry_ids.is_empty() {
                    self.pending_sftp_context_action =
                        Some(PendingSftpContextAction::DownloadSelection { entry_ids });
                    self.close_context_menu();
                }
            }
            "refresh-sftp" => {
                let _ = self.refresh_sftp_panel();
                self.close_context_menu();
            }
            "select-all-sftp" => {
                let _ = self.select_all_sftp_entries();
                self.close_context_menu();
            }
            "sort-name" => {
                let _ = self.set_sftp_panel_sort_column("name");
                self.close_context_menu();
            }
            "sort-size" => {
                let _ = self.set_sftp_panel_sort_column("size");
                self.close_context_menu();
            }
            "sort-modified" => {
                let _ = self.set_sftp_panel_sort_column("modified");
                self.close_context_menu();
            }
            "copy-current-path" => {
                self.copy_sftp_text_to_clipboard(self.sftp_panel_path(), "Current path");
            }
            "copy-file-path" | "copy-folder-path" => {
                if let Some(text) = self.context_target_sftp_entry_text(|entry| entry.path.clone())
                {
                    self.copy_sftp_text_to_clipboard(text, "Remote path");
                }
            }
            "copy-file-name" => {
                if let Some(text) = self.context_target_sftp_entry_text(|entry| entry.name.clone())
                {
                    self.copy_sftp_text_to_clipboard(text, "File name");
                }
            }
            "copy-paths" => {
                let text = self
                    .sftp_selected_entries()
                    .into_iter()
                    .map(|entry| entry.path.clone())
                    .collect::<Vec<_>>()
                    .join("\n");
                self.copy_sftp_text_to_clipboard(text, "Remote paths");
            }
            "open-sftp-workspace" => {
                let _ = self.expand_quick_browser_to_workspace();
                self.close_context_menu();
            }
            _ => self.close_context_menu(),
        }
    }

    fn handle_planned_sftp_context_menu_action(&mut self, action_id: &str, label: &str) {
        let message = match action_id {
            "copy-sftp-entry" | "cut-sftp-entry" | "paste-sftp" => {
                "Remote copy and paste are planned. This quick browser does not yet expose server-side copy or relay paste.".to_string()
            }
            _ => format!("{label} is not wired yet."),
        };
        self.set_context_menu_feedback(message);
    }

    fn handle_disabled_sftp_context_menu_action(&mut self, action_id: &str) {
        let message = match action_id {
            "copy-sftp-entry" => Some("Copy is not available for SFTP yet."),
            "cut-sftp-entry" => Some("Cut is not available for SFTP yet."),
            "paste-sftp" => Some("Paste is not available for SFTP yet."),
            "permissions-sftp" => Some("Permissions are not available for SFTP yet."),
            _ => None,
        };
        if let Some(message) = message {
            self.set_context_menu_feedback(message);
        }
    }

    fn copy_sftp_text_to_clipboard(&mut self, text: String, label: &str) {
        if text.trim().is_empty() {
            self.set_context_menu_feedback(format!("{label} is empty."));
            return;
        }

        match set_sftp_clipboard_text(text.as_str()) {
            Ok(()) => self.close_context_menu(),
            Err(err) => self.set_context_menu_feedback(format!("Failed to copy {label}: {err}")),
        }
    }

    fn context_target_sftp_entry_text(
        &self,
        project: impl FnOnce(&SftpDirectoryEntry) -> String,
    ) -> Option<String> {
        let entry_id = self.context_target_asset_id.as_deref()?;
        self.active_sftp_entry(entry_id).map(project)
    }

    fn sftp_selected_entries(&self) -> Vec<&SftpDirectoryEntry> {
        let Some(state) = self.active_sftp_session_state() else {
            return Vec::new();
        };

        state
            .entries
            .iter()
            .filter(|entry| {
                state
                    .selected_entry_ids
                    .iter()
                    .any(|selected_id| selected_id == &entry.id)
            })
            .collect()
    }

    pub(super) fn context_menu_roots(&self) -> Vec<ContextMenuActionNode> {
        let Some(target_kind) = self.context_menu_target_kind else {
            return Vec::new();
        };

        resolve_action_tree(target_kind, &self.context_menu_selection())
    }
}

fn set_sftp_clipboard_text(text: &str) -> anyhow::Result<()> {
    i_slint_backend_selector::with_platform(|platform| {
        platform.set_clipboard_text(text, slint::platform::Clipboard::DefaultClipboard);
        Ok(())
    })
    .map_err(anyhow::Error::from)
}

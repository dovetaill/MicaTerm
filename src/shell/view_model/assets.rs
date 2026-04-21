//! ShellViewModel assets domain impls.

use super::*;

impl ShellViewModel {
    fn clamp_assets_sidebar_expanded_width(width_px: f32) -> f32 {
        width_px.clamp(
            ShellMetrics::ASSETS_SIDEBAR_MIN_WIDTH as f32,
            ShellMetrics::ASSETS_SIDEBAR_MAX_WIDTH as f32,
        )
    }

    fn collapse_assets_sidebar(&mut self) {
        self.show_assets_sidebar = false;
        self.asset_search_expanded = false;
        self.asset_create_menu_open = false;
    }

    pub(super) fn enter_workspace_focus_mode(&mut self) {
        if self.workspace_focus_mode {
            return;
        }

        self.saved_workspace_focus_assets_sidebar = self.show_assets_sidebar;
        self.saved_workspace_focus_right_panel = self.show_right_panel;
        self.workspace_focus_mode = true;
        self.collapse_assets_sidebar();
        self.show_right_panel = false;
    }

    pub(super) fn exit_workspace_focus_mode(&mut self) {
        if !self.workspace_focus_mode {
            return;
        }

        self.workspace_focus_mode = false;
        if self.saved_workspace_focus_assets_sidebar {
            self.show_assets_sidebar = true;
        } else {
            self.collapse_assets_sidebar();
        }
        self.show_right_panel = self.saved_workspace_focus_right_panel;
    }

    pub fn toggle_workspace_focus_mode(&mut self) {
        if self.workspace_focus_mode {
            self.exit_workspace_focus_mode();
        } else {
            self.enter_workspace_focus_mode();
        }
    }

    pub fn toggle_global_menu(&mut self) {
        self.show_global_menu = !self.show_global_menu;
    }

    pub fn close_global_menu(&mut self) {
        self.show_global_menu = false;
    }

    pub fn toggle_assets_sidebar(&mut self) {
        if self.workspace_focus_mode && !self.show_assets_sidebar {
            self.exit_workspace_focus_mode();
            if self.show_assets_sidebar {
                return;
            }

            self.show_assets_sidebar = true;
            return;
        }

        if self.show_assets_sidebar {
            self.collapse_assets_sidebar();
        } else {
            self.show_assets_sidebar = true;
        }
    }

    pub fn assets_sidebar_expanded_width_px(&self) -> f32 {
        self.assets_sidebar_expanded_width
    }

    pub fn set_assets_sidebar_expanded_width(&mut self, width_px: f32) -> bool {
        let width_px = Self::clamp_assets_sidebar_expanded_width(width_px);
        if (self.assets_sidebar_expanded_width - width_px).abs() <= f32::EPSILON {
            return false;
        }

        self.assets_sidebar_expanded_width = width_px;
        true
    }

    pub fn apply_assets_sidebar_resize(&mut self, width_px: f32) -> bool {
        if width_px < ShellMetrics::ASSETS_SIDEBAR_COLLAPSE_THRESHOLD as f32 {
            if !self.show_assets_sidebar {
                return false;
            }

            self.collapse_assets_sidebar();
            return true;
        }

        let width_changed = self.set_assets_sidebar_expanded_width(width_px);
        let reopened = !self.show_assets_sidebar;
        self.show_assets_sidebar = true;
        width_changed || reopened
    }

    pub fn select_sidebar_destination(&mut self, destination: SidebarDestination) {
        if self.workspace_focus_mode && !self.show_assets_sidebar {
            self.exit_workspace_focus_mode();
            if self.show_assets_sidebar && self.active_sidebar_destination == destination {
                return;
            }
        }

        if self.active_sidebar_destination == destination && self.show_assets_sidebar {
            self.collapse_assets_sidebar();
            return;
        }

        self.active_sidebar_destination = destination;
        self.show_assets_sidebar = true;
        if destination != SidebarDestination::Console {
            self.asset_create_menu_open = false;
        }
    }

    pub fn toggle_asset_view_mode(&mut self) {
        self.asset_view_mode = self.asset_view_mode.toggle();
    }

    pub fn toggle_asset_search(&mut self) {
        self.activate_asset_search();
    }

    pub fn activate_asset_search(&mut self) {
        self.asset_search_expanded = true;
        self.asset_create_menu_open = false;
    }

    pub fn close_asset_search(&mut self) {
        self.asset_search_expanded = false;
    }

    pub fn set_asset_search_query(&mut self, query: String) {
        self.asset_search_query = query;
    }

    pub fn collapse_asset_search_if_empty(&mut self) {
        if self.asset_search_query.is_empty() {
            self.asset_search_expanded = false;
        }
    }

    pub fn dismiss_empty_asset_search_on_shell_interaction(&mut self) -> bool {
        if self.asset_search_expanded && self.asset_search_query.is_empty() {
            self.asset_search_expanded = false;
            true
        } else {
            false
        }
    }

    pub fn toggle_asset_tree_expansion(&mut self) {
        if self.asset_view_mode != AssetViewMode::Tree {
            return;
        }

        self.asset_tree_fully_expanded = !self.asset_tree_fully_expanded;
        self.console_asset_tree
            .set_all_expanded(self.asset_tree_fully_expanded);
    }

    pub fn toggle_asset_create_menu(&mut self) {
        if self.asset_create_menu_open {
            self.asset_create_menu_open = false;
        } else {
            self.asset_create_menu_open = true;
            self.asset_search_expanded = false;
        }
    }

    pub fn close_asset_create_menu(&mut self) {
        self.asset_create_menu_open = false;
    }

    pub fn open_new_folder_modal(&mut self, parent_id: Option<String>) {
        let parent_id = self.normalize_folder_parent_id(parent_id);
        self.dismiss_active_asset_rename();
        let draft_name = self
            .console_asset_tree
            .next_default_name_for_parent(parent_id.as_deref(), ConsoleAssetKind::Folder);
        self.close_context_menu();
        self.close_asset_create_menu();
        self.context_target_asset_id = parent_id.clone();
        self.asset_modal_state = Some(AssetModalState::NewFolder {
            parent_id,
            draft_name,
        });
    }

    pub fn open_sftp_new_folder_modal(&mut self) {
        self.dismiss_active_asset_rename();
        self.close_context_menu();
        self.close_asset_create_menu();
        self.asset_modal_state = Some(AssetModalState::SftpNewFolder {
            draft_name: self.next_default_sftp_folder_name(),
        });
    }

    pub fn open_sftp_new_file_modal(&mut self) {
        self.dismiss_active_asset_rename();
        self.close_context_menu();
        self.close_asset_create_menu();
        self.asset_modal_state = Some(AssetModalState::SftpNewFile {
            draft_name: self.next_default_sftp_file_name(),
        });
    }

    pub fn open_new_snippet_modal(&mut self, parent_package_id: Option<String>) {
        let parent_package_id = self.normalize_snippet_package_parent_id(parent_package_id);
        self.dismiss_active_asset_rename();
        let draft_name = self
            .snippet_asset_tree
            .next_default_name_for_parent(parent_package_id.as_deref(), ConsoleAssetKind::Snippet);
        let package = parent_package_id
            .as_deref()
            .and_then(|asset_id| self.snippet_asset_tree.title(asset_id))
            .unwrap_or_default()
            .to_string();
        self.close_context_menu();
        self.close_asset_create_menu();
        self.context_target_asset_id = parent_package_id.clone();
        self.asset_modal_state = Some(AssetModalState::NewSnippet {
            parent_package_id,
            editing_asset_id: None,
            draft: AssetSnippetDraft {
                name: draft_name,
                script: String::new(),
                package,
            },
        });
    }

    pub fn open_edit_snippet_modal(&mut self, asset_id: String) {
        let Some(node) = self.snippet_asset_tree.node(&asset_id).cloned() else {
            return;
        };
        let AssetNodePayload::Snippet(spec) = node.payload else {
            return;
        };

        self.dismiss_active_asset_rename();
        self.close_context_menu();
        self.close_asset_create_menu();
        self.focused_asset_id = Some(asset_id.clone());
        self.selected_asset_ids = vec![asset_id.clone()];
        self.context_target_asset_id = Some(asset_id.clone());
        self.asset_modal_state = Some(AssetModalState::NewSnippet {
            parent_package_id: spec.package_id.clone(),
            editing_asset_id: Some(asset_id),
            draft: AssetSnippetDraft {
                name: node.title,
                script: spec.script,
                package: spec
                    .package_id
                    .as_deref()
                    .and_then(|package_id| self.snippet_asset_tree.title(package_id))
                    .unwrap_or_default()
                    .to_string(),
            },
        });
    }

    pub fn open_new_snippet_package_modal(&mut self) {
        self.dismiss_active_asset_rename();
        let draft_name = self
            .snippet_asset_tree
            .next_default_name_for_parent(None, ConsoleAssetKind::SnippetPackage);
        self.close_context_menu();
        self.close_asset_create_menu();
        self.context_target_asset_id = None;
        self.asset_modal_state = Some(AssetModalState::NewSnippetPackage {
            editing_asset_id: None,
            draft_name,
        });
    }

    pub fn open_edit_snippet_package_modal(&mut self, asset_id: String) {
        if self.snippet_asset_tree.kind(&asset_id) != Some(ConsoleAssetKind::SnippetPackage) {
            return;
        }
        let Some(original_name) = self.snippet_asset_tree.title(&asset_id).map(str::to_string)
        else {
            return;
        };

        self.dismiss_active_asset_rename();
        self.close_context_menu();
        self.close_asset_create_menu();
        self.focused_asset_id = Some(asset_id.clone());
        self.selected_asset_ids = vec![asset_id.clone()];
        self.context_target_asset_id = Some(asset_id.clone());
        self.asset_modal_state = Some(AssetModalState::NewSnippetPackage {
            editing_asset_id: Some(asset_id),
            draft_name: original_name,
        });
    }

    pub fn update_new_folder_modal_name(&mut self, value: String) {
        let Some(modal_state) = self.asset_modal_state.as_mut() else {
            return;
        };

        match modal_state {
            AssetModalState::NewFolder { draft_name, .. }
            | AssetModalState::SftpNewFile { draft_name }
            | AssetModalState::SftpNewFolder { draft_name } => {
                *draft_name = value;
            }
            _ => {}
        }
    }

    pub fn update_snippet_modal_field(&mut self, field: &str, value: String) {
        let Some(AssetModalState::NewSnippet { draft, .. }) = self.asset_modal_state.as_mut()
        else {
            return;
        };

        match field {
            "name" => draft.name = value,
            "script" => draft.script = value,
            "package" => draft.package = value,
            _ => {}
        }
    }

    pub fn update_snippet_package_modal_name(&mut self, value: String) {
        let Some(AssetModalState::NewSnippetPackage { draft_name, .. }) =
            self.asset_modal_state.as_mut()
        else {
            return;
        };

        *draft_name = value;
    }

    pub fn open_rename_asset_modal(&mut self, asset_id: String) {
        let original_name = if let Some(name) = self.console_asset_tree.title(&asset_id) {
            self.focused_asset_id = Some(asset_id.clone());
            self.selected_asset_ids = vec![asset_id.clone()];
            self.focused_keychain_id = None;
            self.selected_keychain_ids.clear();
            name.to_string()
        } else if let Some(node) = self.keychain_catalog.nodes.get(&asset_id) {
            self.focused_keychain_id = Some(asset_id.clone());
            self.selected_keychain_ids = vec![asset_id.clone()];
            self.focused_asset_id = None;
            self.selected_asset_ids.clear();
            node.title.clone()
        } else {
            return;
        };

        self.dismiss_active_asset_rename();
        self.close_context_menu();
        self.close_asset_create_menu();
        self.context_target_asset_id = Some(asset_id.clone());
        self.asset_modal_state = Some(AssetModalState::RenameAsset {
            asset_id,
            original_name: original_name.clone(),
            draft_name: original_name,
        });
    }

    pub fn open_sftp_rename_entry_modal(&mut self, entry_id: String) {
        let Some(entry) = self
            .active_sftp_session_state()
            .and_then(|state| state.entries.iter().find(|entry| entry.id == entry_id))
            .cloned()
        else {
            return;
        };

        self.dismiss_active_asset_rename();
        self.close_context_menu();
        self.close_asset_create_menu();
        let _ = self.replace_active_sftp_selection(vec![entry.id.clone()]);
        self.context_target_asset_id = Some(entry.id.clone());
        self.asset_modal_state = Some(AssetModalState::SftpRenameEntry {
            entry_id: entry.id,
            original_name: entry.name.clone(),
            draft_name: entry.name,
        });
    }

    pub fn update_rename_asset_modal_name(&mut self, value: String) {
        let Some(modal_state) = self.asset_modal_state.as_mut() else {
            return;
        };

        match modal_state {
            AssetModalState::RenameAsset { draft_name, .. }
            | AssetModalState::SftpRenameEntry { draft_name, .. } => {
                *draft_name = value;
            }
            _ => {}
        }
    }

    pub fn open_delete_asset_confirm(&mut self, asset_id: String) {
        let asset_summary = if let Some(node) = self.keychain_catalog.nodes.get(&asset_id) {
            Some((
                node.title.clone(),
                self.keychain_descendant_count(&asset_id)
                    .unwrap_or_default(),
            ))
        } else {
            let snippet_first = self.active_sidebar_destination == SidebarDestination::Snippets;
            if snippet_first {
                self.snippet_asset_tree
                    .title(&asset_id)
                    .map(str::to_string)
                    .zip(self.snippet_asset_tree.descendant_count(&asset_id))
                    .or_else(|| {
                        self.console_asset_tree
                            .title(&asset_id)
                            .map(str::to_string)
                            .zip(self.console_asset_tree.descendant_count(&asset_id))
                    })
            } else {
                self.console_asset_tree
                    .title(&asset_id)
                    .map(str::to_string)
                    .zip(self.console_asset_tree.descendant_count(&asset_id))
                    .or_else(|| {
                        self.snippet_asset_tree
                            .title(&asset_id)
                            .map(str::to_string)
                            .zip(self.snippet_asset_tree.descendant_count(&asset_id))
                    })
            }
        };
        let Some((label, descendant_count)) = asset_summary else {
            return;
        };

        self.dismiss_active_asset_rename();
        self.close_context_menu();
        self.close_asset_create_menu();
        if self.keychain_catalog.nodes.contains_key(&asset_id) {
            self.focused_keychain_id = Some(asset_id.clone());
            self.selected_keychain_ids = vec![asset_id.clone()];
            self.focused_asset_id = None;
            self.selected_asset_ids.clear();
        } else {
            self.focused_asset_id = Some(asset_id.clone());
            self.selected_asset_ids = vec![asset_id.clone()];
            self.focused_keychain_id = None;
            self.selected_keychain_ids.clear();
        }
        self.context_target_asset_id = Some(asset_id.clone());
        self.asset_modal_state = Some(AssetModalState::DeleteAssetConfirm {
            asset_id,
            label,
            descendant_count,
        });
    }

    pub fn open_sftp_delete_confirm(&mut self, entry_ids: Vec<String>) {
        let Some(state) = self.active_sftp_session_state() else {
            return;
        };
        let selected_entries = state
            .entries
            .iter()
            .filter(|entry| entry_ids.iter().any(|id| id == &entry.id))
            .cloned()
            .collect::<Vec<_>>();
        if selected_entries.is_empty() {
            return;
        }

        let label = if selected_entries.len() == 1 {
            selected_entries[0].name.clone()
        } else {
            format!("{} items", selected_entries.len())
        };

        self.dismiss_active_asset_rename();
        self.close_context_menu();
        self.close_asset_create_menu();
        let _ = self.replace_active_sftp_selection(entry_ids.clone());
        self.context_target_asset_id = entry_ids.first().cloned();
        self.asset_modal_state = Some(AssetModalState::SftpDeleteEntriesConfirm {
            entry_ids,
            label,
            descendant_count: 0,
        });
    }

    pub fn confirm_delete_asset(&mut self) -> bool {
        if let Some(AssetModalState::SftpDeleteEntriesConfirm { entry_ids, .. }) =
            self.asset_modal_state.clone()
        {
            if self.quick_browser_linked_terminal_session_id().is_none() {
                return false;
            }
            let selected_entries = self
                .active_sftp_session_state()
                .map(|state| {
                    state
                        .entries
                        .iter()
                        .filter(|entry| entry_ids.iter().any(|id| id == &entry.id))
                        .cloned()
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            if selected_entries.is_empty() {
                return false;
            }

            self.pending_sftp_context_action = Some(PendingSftpContextAction::DeleteEntries {
                entries: selected_entries,
                refresh_path: self.sftp_panel_path(),
            });
            self.asset_modal_state = None;
            return true;
        }

        let Some(AssetModalState::DeleteAssetConfirm { asset_id, .. }) =
            self.asset_modal_state.clone()
        else {
            return false;
        };

        if self.keychain_catalog.nodes.contains_key(&asset_id) {
            match self.delete_keychain_item(&asset_id) {
                Ok(true) => {
                    self.asset_modal_state = None;
                    if self.context_target_asset_id.as_deref() == Some(asset_id.as_str()) {
                        self.context_target_asset_id = self.focused_keychain_id.clone();
                    }
                    return true;
                }
                Ok(false) => return false,
                Err(err) => {
                    self.set_context_menu_feedback(err.to_string());
                    return false;
                }
            }
        }

        let snippet_first = self.active_sidebar_destination == SidebarDestination::Snippets;
        let removed = if snippet_first {
            if self.snippet_asset_tree.remove_subtree(&asset_id).is_some() {
                self.selected_asset_ids.clear();
                self.focused_asset_id = None;
                self.context_target_asset_id = None;
                true
            } else {
                self.remove_asset_subtree(&asset_id)
            }
        } else if self.remove_asset_subtree(&asset_id) {
            true
        } else if self.snippet_asset_tree.remove_subtree(&asset_id).is_some() {
            self.selected_asset_ids.clear();
            self.focused_asset_id = None;
            self.context_target_asset_id = None;
            true
        } else {
            false
        };
        if removed {
            self.asset_modal_state = None;
            self.pending_snippet_activation = None;
            return true;
        }

        false
    }

    pub fn open_new_ssh_modal(&mut self, parent_id: Option<String>) {
        let parent_id = self.normalize_folder_parent_id(parent_id);
        self.dismiss_active_asset_rename();
        let draft_name = self
            .console_asset_tree
            .next_default_name_for_parent(parent_id.as_deref(), ConsoleAssetKind::SshConnection);
        self.close_context_menu();
        self.close_asset_create_menu();
        self.context_target_asset_id = parent_id.clone();
        self.pending_ssh_modal_action = None;
        self.ssh_modal_action_state = SshModalActionState::Idle;
        self.asset_modal_state = Some(AssetModalState::NewSshConnection {
            parent_id,
            editing_asset_id: None,
            draft: AssetSshConnectionDraft {
                name: draft_name,
                ..AssetSshConnectionDraft::default()
            },
        });
    }

    pub fn open_edit_ssh_modal(&mut self, asset_id: String) {
        let Some(node) = self.console_asset_tree.node(&asset_id).cloned() else {
            return;
        };
        let AssetNodePayload::SshConnection(spec) = node.payload else {
            return;
        };
        let auth_method = if spec.auth_method.trim().is_empty() {
            "password".to_string()
        } else {
            spec.auth_method.clone()
        };
        let auth_source = normalized_ssh_auth_source(&spec.auth_source).to_string();
        let private_key_source = if spec.private_key_source.trim().is_empty() {
            "content".to_string()
        } else {
            spec.private_key_source.clone()
        };
        let (
            proxy_type,
            proxy_socks5_host,
            proxy_socks5_port,
            proxy_socks5_username,
            proxy_ssh_asset_id,
        ) = match &spec.proxy {
            AssetSshProxySpec::None => (
                "none".to_string(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
            ),
            AssetSshProxySpec::Socks5(proxy) => (
                "socks5".to_string(),
                proxy.host.clone(),
                proxy.port.clone(),
                proxy.username.clone(),
                String::new(),
            ),
            AssetSshProxySpec::Http(proxy) => (
                "http".to_string(),
                proxy.host.clone(),
                proxy.port.clone(),
                proxy.username.clone(),
                String::new(),
            ),
            AssetSshProxySpec::SshAsset { asset_id } => (
                "ssh-asset".to_string(),
                String::new(),
                String::new(),
                String::new(),
                asset_id.clone(),
            ),
        };
        self.dismiss_active_asset_rename();
        self.close_context_menu();
        self.close_asset_create_menu();
        self.focused_asset_id = Some(asset_id.clone());
        self.selected_asset_ids = vec![asset_id.clone()];
        self.context_target_asset_id = Some(asset_id.clone());
        self.pending_ssh_modal_action = None;
        self.ssh_modal_action_state = SshModalActionState::Idle;
        self.asset_modal_state = Some(AssetModalState::NewSshConnection {
            parent_id: node.parent_id,
            editing_asset_id: Some(asset_id),
            draft: AssetSshConnectionDraft {
                name: node.title,
                host: spec.host,
                user: spec.user,
                port: spec.port,
                auth_source,
                keychain_identity_id: spec.keychain_identity_id.unwrap_or_default(),
                auth_method,
                private_key_source,
                password: String::new(),
                private_key_content: String::new(),
                private_key_path: spec.private_key_path,
                passphrase: String::new(),
                password_visible: false,
                remark: spec.remark,
                environment: spec.environment,
                proxy_type,
                proxy_socks5_host,
                proxy_socks5_port,
                proxy_socks5_username,
                proxy_socks5_password: String::new(),
                proxy_socks5_password_visible: false,
                proxy_ssh_asset_id,
                proxy_method: spec.proxy_method,
                validation_message: String::new(),
            },
        });
    }

    pub fn hydrate_edit_ssh_modal_secret(
        &mut self,
        password: Option<String>,
        private_key_content: Option<String>,
        passphrase: Option<String>,
        inline_error: Option<String>,
    ) {
        let Some(AssetModalState::NewSshConnection {
            editing_asset_id: Some(_),
            draft,
            ..
        }) = self.asset_modal_state.as_mut()
        else {
            return;
        };

        draft.password = password.unwrap_or_default();
        draft.private_key_content = private_key_content.unwrap_or_default();
        draft.passphrase = passphrase.unwrap_or_default();
        draft.password_visible = false;
        draft.proxy_socks5_password_visible = false;
        draft.validation_message = inline_error.unwrap_or_default();
    }
}

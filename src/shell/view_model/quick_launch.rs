//! ShellViewModel quick launch domain impls.

use super::*;

impl ShellViewModel {
    pub fn quick_launch_preferences(&self) -> &QuickLaunchPreferences {
        &self.quick_launch_preferences
    }

    pub fn quick_launch_search_query(&self) -> &str {
        &self.quick_launch_search_query
    }

    pub fn quick_launch_selected_asset_id(&self) -> Option<&str> {
        self.quick_launch_selected_asset_id.as_deref()
    }

    pub fn quick_launch_active_group_id(&self) -> Option<&str> {
        self.quick_launch_active_group_id.as_deref()
    }

    pub fn apply_quick_launch_preferences(&mut self, prefs: QuickLaunchPreferences) {
        self.quick_launch_selected_asset_id = prefs.last_selected_asset_id.clone();
        self.quick_launch_preferences = prefs;
        self.sync_quick_launch_group_from_selected();
        self.ensure_quick_launch_selection();
    }

    pub fn record_recent_saved_ssh_asset(&mut self, asset_id: &str) {
        if self
            .console_asset_tree
            .ssh_connection_spec(asset_id)
            .is_none()
        {
            return;
        }

        self.quick_launch_preferences.recent_asset_ids = record_recent_asset_id(
            self.quick_launch_preferences.recent_asset_ids.clone(),
            asset_id,
            QUICK_LAUNCH_RECENT_LIMIT,
        );
        self.quick_launch_preferences.last_selected_asset_id = Some(asset_id.to_string());
        self.quick_launch_selected_asset_id = Some(asset_id.to_string());
        self.sync_quick_launch_group_from_selected();
        self.ensure_quick_launch_selection();
    }

    pub fn toggle_quick_launch_favorite(&mut self, asset_id: &str) {
        if self
            .console_asset_tree
            .ssh_connection_spec(asset_id)
            .is_none()
        {
            return;
        }

        if let Some(index) = self
            .quick_launch_preferences
            .favorite_asset_ids
            .iter()
            .position(|current| current == asset_id)
        {
            self.quick_launch_preferences
                .favorite_asset_ids
                .remove(index);
        } else {
            self.quick_launch_preferences
                .favorite_asset_ids
                .insert(0, asset_id.to_string());
        }

        self.ensure_quick_launch_selection();
    }

    pub fn select_quick_launch_asset(&mut self, asset_id: String) {
        if self
            .console_asset_tree
            .ssh_connection_spec(&asset_id)
            .is_none()
        {
            return;
        }

        self.quick_launch_preferences.last_selected_asset_id = Some(asset_id.clone());
        self.quick_launch_selected_asset_id = Some(asset_id);
        self.sync_quick_launch_group_from_selected();
        self.ensure_quick_launch_selection();
    }

    pub fn set_quick_launch_search_query(&mut self, query: String) {
        self.quick_launch_search_query = query;
        self.ensure_quick_launch_selection();
    }

    pub fn quick_launch_recent_items(&self) -> Vec<QuickLaunchCardItem> {
        let records = self.matching_quick_launch_records();
        self.ordered_quick_launch_cards_from_ids(
            &self.quick_launch_preferences.recent_asset_ids,
            &records,
        )
    }

    pub fn quick_launch_favorite_items(&self) -> Vec<QuickLaunchCardItem> {
        let records = self.matching_quick_launch_records();
        self.ordered_quick_launch_cards_from_ids(
            &self.quick_launch_preferences.favorite_asset_ids,
            &records,
        )
    }

    pub fn quick_launch_group_items(&self) -> Vec<QuickLaunchGroupItem> {
        let records = self.matching_quick_launch_records();
        let mut groups = Vec::<QuickLaunchGroupItem>::new();
        let mut positions = HashMap::<String, usize>::new();

        for record in records {
            let Some(group) = record.group else {
                continue;
            };

            if let Some(position) = positions.get(&group.id).copied() {
                groups[position].count += 1;
            } else {
                positions.insert(group.id.clone(), groups.len());
                groups.push(QuickLaunchGroupItem {
                    group_id: group.id,
                    label: group.label,
                    count: 1,
                });
            }
        }

        groups
    }

    pub fn quick_launch_visible_group_items(&self) -> Vec<QuickLaunchCardItem> {
        let records = self.matching_quick_launch_records();
        self.visible_group_records(&records)
            .into_iter()
            .map(|record| {
                project_card_item(
                    &record,
                    self.is_quick_launch_favorite(record.asset_id.as_str()),
                )
            })
            .collect()
    }

    pub fn quick_launch_selected_detail(&self) -> Option<QuickLaunchDetailItem> {
        let selected_asset_id = self.quick_launch_selected_asset_id.as_deref()?;
        let records = self.quick_launch_records();
        let record = records
            .iter()
            .find(|record| record.asset_id == selected_asset_id)?;

        Some(project_detail_item(
            record,
            self.quick_launch_recent_label(selected_asset_id),
        ))
    }

    pub fn ensure_quick_launch_selection(&mut self) {
        let records = self.matching_quick_launch_records();
        let visible_asset_ids = self.visible_asset_ids_from_records(&records);
        if self
            .quick_launch_selected_asset_id
            .as_deref()
            .is_some_and(|asset_id| {
                visible_asset_ids
                    .iter()
                    .any(|visible_asset_id| visible_asset_id == asset_id)
            })
        {
            self.sync_quick_launch_group_from_selected();
            return;
        }

        self.quick_launch_selected_asset_id =
            self.first_visible_quick_launch_asset_id_from_records(&records);
        self.quick_launch_preferences.last_selected_asset_id =
            self.quick_launch_selected_asset_id.clone();
        self.sync_quick_launch_group_from_selected();
    }

    fn quick_launch_records(&self) -> Vec<QuickLaunchAssetRecord> {
        collect_quick_launch_records(&self.console_asset_tree)
    }

    fn matching_quick_launch_records(&self) -> Vec<QuickLaunchAssetRecord> {
        self.quick_launch_records()
            .into_iter()
            .filter(|record| {
                matches_quick_launch_query(record, self.quick_launch_search_query.as_str())
            })
            .collect()
    }

    fn ordered_quick_launch_cards_from_ids(
        &self,
        ids: &[String],
        records: &[QuickLaunchAssetRecord],
    ) -> Vec<QuickLaunchCardItem> {
        self.ordered_quick_launch_asset_ids_from_preferences(ids, records)
            .into_iter()
            .filter_map(|asset_id| {
                records
                    .iter()
                    .find(|record| record.asset_id == asset_id)
                    .map(|record| {
                        project_card_item(record, self.is_quick_launch_favorite(asset_id.as_str()))
                    })
            })
            .collect()
    }

    fn ordered_quick_launch_asset_ids_from_preferences(
        &self,
        ids: &[String],
        records: &[QuickLaunchAssetRecord],
    ) -> Vec<String> {
        let mut ordered = Vec::new();
        let mut seen = BTreeSet::new();

        for asset_id in ids {
            if !seen.insert(asset_id.clone()) {
                continue;
            }
            if records.iter().any(|record| record.asset_id == *asset_id) {
                ordered.push(asset_id.clone());
            }
        }

        ordered
    }

    fn visible_group_records(
        &self,
        records: &[QuickLaunchAssetRecord],
    ) -> Vec<QuickLaunchAssetRecord> {
        let Some(group_id) = self.active_quick_launch_group_id_for_records(records) else {
            return records.to_vec();
        };

        records
            .iter()
            .filter(|record| {
                record
                    .group
                    .as_ref()
                    .is_some_and(|group| group.id == group_id)
            })
            .cloned()
            .collect()
    }

    fn active_quick_launch_group_id_for_records<'a>(
        &'a self,
        records: &[QuickLaunchAssetRecord],
    ) -> Option<&'a str> {
        self.quick_launch_active_group_id
            .as_deref()
            .filter(|group_id| {
                records.iter().any(|record| {
                    record
                        .group
                        .as_ref()
                        .is_some_and(|group| group.id == *group_id)
                })
            })
    }

    fn visible_asset_ids_from_records(&self, records: &[QuickLaunchAssetRecord]) -> Vec<String> {
        self.visible_group_records(records)
            .into_iter()
            .map(|record| record.asset_id)
            .collect()
    }

    fn first_visible_quick_launch_asset_id_from_records(
        &self,
        records: &[QuickLaunchAssetRecord],
    ) -> Option<String> {
        self.ordered_quick_launch_asset_ids_from_preferences(
            &self.quick_launch_preferences.recent_asset_ids,
            records,
        )
        .into_iter()
        .next()
        .or_else(|| {
            self.ordered_quick_launch_asset_ids_from_preferences(
                &self.quick_launch_preferences.favorite_asset_ids,
                records,
            )
            .into_iter()
            .next()
        })
        .or_else(|| {
            self.visible_group_records(records)
                .into_iter()
                .map(|record| record.asset_id)
                .next()
        })
    }

    fn is_quick_launch_favorite(&self, asset_id: &str) -> bool {
        self.quick_launch_preferences
            .favorite_asset_ids
            .iter()
            .any(|favorite_id| favorite_id == asset_id)
    }

    fn quick_launch_recent_label(&self, asset_id: &str) -> String {
        self.quick_launch_preferences
            .recent_asset_ids
            .iter()
            .position(|recent_id| recent_id == asset_id)
            .map(|index| format!("Recent #{}", index + 1))
            .unwrap_or_default()
    }

    fn sync_quick_launch_group_from_selected(&mut self) {
        self.quick_launch_active_group_id = self
            .quick_launch_selected_asset_id
            .as_deref()
            .and_then(|asset_id| group_id_for_asset(&self.console_asset_tree, asset_id));
    }
}

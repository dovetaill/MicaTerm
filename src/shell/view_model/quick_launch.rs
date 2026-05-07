//! ShellViewModel recent SSH launcher domain impls.

use super::*;
use crate::app::quick_launch_preferences::{QuickLaunchRecentAsset, record_recent_asset_opened};
use chrono::Utc;

impl ShellViewModel {
    pub fn quick_launch_preferences(&self) -> &QuickLaunchPreferences {
        &self.quick_launch_preferences
    }

    pub fn apply_quick_launch_preferences(&mut self, prefs: QuickLaunchPreferences) {
        self.quick_launch_preferences = prefs;
    }

    pub fn record_recent_saved_ssh_asset(&mut self, asset_id: &str) {
        if self
            .console_asset_tree
            .ssh_connection_spec(asset_id)
            .is_none()
        {
            return;
        }

        self.quick_launch_preferences.recent_asset_ids = record_recent_asset_opened(
            self.quick_launch_preferences.recent_asset_ids.clone(),
            asset_id,
            Utc::now().timestamp(),
            QUICK_LAUNCH_RECENT_LIMIT,
        );
    }

    pub fn quick_launch_recent_items(&self) -> Vec<QuickLaunchCardItem> {
        self.quick_launch_recent_items_at(Utc::now().timestamp())
    }

    pub fn quick_launch_recent_items_at(&self, now_unix_seconds: i64) -> Vec<QuickLaunchCardItem> {
        let records = self.quick_launch_records();
        self.ordered_new_tab_connection_cards(
            &self.quick_launch_preferences.recent_asset_ids,
            &records,
            now_unix_seconds,
        )
    }

    fn quick_launch_records(&self) -> Vec<QuickLaunchAssetRecord> {
        collect_quick_launch_records(&self.console_asset_tree)
    }

    fn ordered_new_tab_connection_cards(
        &self,
        entries: &[QuickLaunchRecentAsset],
        records: &[QuickLaunchAssetRecord],
        now_unix_seconds: i64,
    ) -> Vec<QuickLaunchCardItem> {
        let mut seen = BTreeSet::new();
        let mut items = Vec::new();

        for asset_id in self.connected_quick_launch_asset_ids(records) {
            if items.len() >= QUICK_LAUNCH_RECENT_LIMIT {
                break;
            }
            if !seen.insert(asset_id.clone()) {
                continue;
            }
            if let Some(record) = records.iter().find(|record| record.asset_id == asset_id) {
                items.push(project_connected_card_item(record));
            }
        }

        for entry in entries {
            if items.len() >= QUICK_LAUNCH_RECENT_LIMIT {
                break;
            }
            if !seen.insert(entry.asset_id.clone()) {
                continue;
            }
            if let Some(record) = records
                .iter()
                .find(|record| record.asset_id == entry.asset_id)
            {
                items.push(project_recent_card_item(
                    record,
                    format_recent_time_label(entry.opened_at_unix_seconds, now_unix_seconds),
                    recent_accent_kind(record.asset_id.as_str()).into(),
                ));
            }
        }

        items
    }

    fn connected_quick_launch_asset_ids(&self, records: &[QuickLaunchAssetRecord]) -> Vec<String> {
        let mut ordered_tabs = Vec::new();
        if let Some(active_tab_id) = self.active_workspace_tab_id() {
            if let Some(tab) = self
                .workspace_tabs()
                .iter()
                .find(|tab| tab.tab_id == active_tab_id)
            {
                ordered_tabs.push(tab);
            }
        }
        for tab in self.workspace_tabs() {
            if ordered_tabs
                .iter()
                .any(|ordered_tab| ordered_tab.tab_id == tab.tab_id)
            {
                continue;
            }
            ordered_tabs.push(tab);
        }

        let mut seen = BTreeSet::new();
        ordered_tabs
            .into_iter()
            .filter(|tab| {
                tab.kind == crate::shell::tabs::WorkspaceTabKind::Terminal
                    && tab.state == "connected"
                    && !tab.asset_id.is_empty()
                    && !self.workspace_terminal_session_hidden(tab.session_id.as_str())
            })
            .filter(|tab| records.iter().any(|record| record.asset_id == tab.asset_id))
            .filter_map(|tab| {
                seen.insert(tab.asset_id.clone())
                    .then(|| tab.asset_id.clone())
            })
            .collect()
    }
}

fn recent_accent_kind(asset_id: &str) -> &'static str {
    const ACCENTS: [&str; QUICK_LAUNCH_RECENT_LIMIT] =
        ["lime", "violet", "blue", "amber", "cyan", "yellow", "pink"];

    let hash = asset_id
        .as_bytes()
        .iter()
        .fold(0xcbf29ce484222325_u64, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
        });

    ACCENTS[(hash as usize) % ACCENTS.len()]
}

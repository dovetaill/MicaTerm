//! Projection helpers for the New Tab SSH launcher.

use crate::shell::assets::{
    AssetSshConnectionSpec, AssetSshProxySpec, AssetTree, ConsoleAssetKind,
};

pub const QUICK_LAUNCH_RECENT_LIMIT: usize = 7;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuickLaunchCardItem {
    pub asset_id: String,
    pub title: String,
    pub subtitle: String,
    pub badge: String,
    pub meta: String,
    pub time_label: String,
    pub state_label: String,
    pub icon_kind: String,
    pub accent_kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct QuickLaunchAssetRecord {
    pub asset_id: String,
    pub title: String,
    pub subtitle: String,
    pub meta: String,
    pub icon_kind: String,
    pub accent_kind: String,
    pub proxy_summary: String,
}

pub(crate) fn collect_quick_launch_records(tree: &AssetTree) -> Vec<QuickLaunchAssetRecord> {
    let mut records = Vec::new();
    for root_id in tree.root_ids() {
        collect_records_from_node(tree, root_id, &mut records);
    }
    records
}

pub(crate) fn project_recent_card_item(
    record: &QuickLaunchAssetRecord,
    time_label: String,
    accent_kind: String,
) -> QuickLaunchCardItem {
    QuickLaunchCardItem {
        asset_id: record.asset_id.clone(),
        title: record.title.clone(),
        subtitle: record.subtitle.clone(),
        badge: String::new(),
        meta: recent_meta_for_record(record),
        time_label,
        state_label: String::new(),
        icon_kind: "server-stack".into(),
        accent_kind,
    }
}

pub(crate) fn project_connected_card_item(record: &QuickLaunchAssetRecord) -> QuickLaunchCardItem {
    QuickLaunchCardItem {
        asset_id: record.asset_id.clone(),
        title: record.title.clone(),
        subtitle: record.subtitle.clone(),
        badge: String::new(),
        meta: recent_meta_for_record(record),
        time_label: String::new(),
        state_label: "Connected".into(),
        icon_kind: "server-stack".into(),
        accent_kind: "connected".into(),
    }
}

pub(crate) fn format_recent_time_label(
    opened_at_unix_seconds: i64,
    now_unix_seconds: i64,
) -> String {
    if opened_at_unix_seconds <= 0 {
        return "Recently".into();
    }

    let elapsed_seconds = now_unix_seconds
        .saturating_sub(opened_at_unix_seconds)
        .max(0);
    match elapsed_seconds {
        0..=59 => "Just now".into(),
        60..=3599 => format!("{}m ago", elapsed_seconds / 60),
        3600..=86_399 => format!("{}h ago", elapsed_seconds / 3600),
        86_400..=172_799 => "Yesterday".into(),
        _ => format!("{}d ago", elapsed_seconds / 86_400),
    }
}

fn collect_records_from_node(
    tree: &AssetTree,
    node_id: &str,
    records: &mut Vec<QuickLaunchAssetRecord>,
) {
    let Some(node) = tree.node(node_id) else {
        return;
    };

    match node.kind {
        ConsoleAssetKind::Folder => {
            for child_id in &node.children {
                collect_records_from_node(tree, child_id, records);
            }
        }
        ConsoleAssetKind::SshConnection => {
            if let Some(spec) = tree.ssh_connection_spec(node_id) {
                records.push(record_from_spec(
                    tree,
                    node.id.as_str(),
                    node.title.as_str(),
                    spec,
                ));
            }
        }
        ConsoleAssetKind::SnippetPackage | ConsoleAssetKind::Snippet => {}
    }
}

fn record_from_spec(
    tree: &AssetTree,
    asset_id: &str,
    title: &str,
    spec: &AssetSshConnectionSpec,
) -> QuickLaunchAssetRecord {
    QuickLaunchAssetRecord {
        asset_id: asset_id.into(),
        title: title.into(),
        subtitle: format!("{}@{}", spec.user.trim(), spec.host.trim()),
        meta: meta_for_spec(spec),
        icon_kind: icon_kind_for_asset(title, spec).into(),
        accent_kind: accent_kind_for_environment(&spec.environment).into(),
        proxy_summary: proxy_summary_for_spec(tree, spec),
    }
}

fn meta_for_spec(spec: &AssetSshConnectionSpec) -> String {
    let mut parts = Vec::new();
    let port = spec.port.trim();
    if !port.is_empty() && port != "22" {
        parts.push(format!("Port {port}"));
    }
    let remark = spec.remark.trim();
    if !remark.is_empty() {
        parts.push(remark.to_string());
    }

    parts.join(" - ")
}

fn recent_meta_for_record(record: &QuickLaunchAssetRecord) -> String {
    if record.proxy_summary.starts_with("SSH jump via ") {
        return format!(
            "via {}",
            record
                .proxy_summary
                .trim_start_matches("SSH jump via ")
                .trim()
        );
    }

    record.meta.clone()
}

fn icon_kind_for_asset(title: &str, spec: &AssetSshConnectionSpec) -> &'static str {
    let haystack = format!(
        "{} {} {}",
        title.to_ascii_lowercase(),
        spec.environment.to_ascii_lowercase(),
        spec.remark.to_ascii_lowercase()
    );
    if ["database", "db", "mysql", "postgres", "redis"]
        .iter()
        .any(|keyword| haystack.contains(keyword))
    {
        "database"
    } else if ["bastion", "jump", "gateway", "proxy"]
        .iter()
        .any(|keyword| haystack.contains(keyword))
    {
        "gateway"
    } else {
        "window-console"
    }
}

fn accent_kind_for_environment(environment: &str) -> &'static str {
    let normalized = environment.trim().to_ascii_lowercase();
    if normalized.contains("prod") {
        "danger"
    } else if normalized.contains("stage") || normalized.contains("test") {
        "warning"
    } else if normalized.contains("dev") {
        "info"
    } else {
        "neutral"
    }
}

fn proxy_summary_for_spec(tree: &AssetTree, spec: &AssetSshConnectionSpec) -> String {
    match &spec.proxy {
        AssetSshProxySpec::None => "No proxy".into(),
        AssetSshProxySpec::Socks5(proxy) => {
            format!("SOCKS5 {}:{}", proxy.host.trim(), proxy.port.trim())
        }
        AssetSshProxySpec::Http(proxy) => {
            format!("HTTP {}:{}", proxy.host.trim(), proxy.port.trim())
        }
        AssetSshProxySpec::SshAsset { asset_id } => tree
            .node(asset_id)
            .map(|node| format!("SSH jump via {}", node.title))
            .unwrap_or_else(|| format!("SSH jump via {asset_id}")),
    }
}

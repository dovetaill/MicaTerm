//! Projection helpers for the welcome quick launch dashboard.

use crate::shell::assets::{
    AssetSshConnectionSpec, AssetSshProxySpec, AssetTree, ConsoleAssetKind,
    SSH_AUTH_SOURCE_KEYCHAIN_IDENTITY, normalized_ssh_auth_source,
};

pub const QUICK_LAUNCH_RECENT_LIMIT: usize = 12;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuickLaunchCardItem {
    pub asset_id: String,
    pub title: String,
    pub subtitle: String,
    pub badge: String,
    pub meta: String,
    pub icon_kind: String,
    pub accent_kind: String,
    pub favorite: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuickLaunchGroupItem {
    pub group_id: String,
    pub label: String,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuickLaunchDetailItem {
    pub asset_id: String,
    pub title: String,
    pub subtitle: String,
    pub environment: String,
    pub auth_summary: String,
    pub proxy_summary: String,
    pub remark: String,
    pub recent_label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct QuickLaunchAssetGroup {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct QuickLaunchAssetRecord {
    pub asset_id: String,
    pub title: String,
    pub subtitle: String,
    pub badge: String,
    pub meta: String,
    pub icon_kind: String,
    pub accent_kind: String,
    pub environment: String,
    pub auth_summary: String,
    pub proxy_summary: String,
    pub remark: String,
    pub group: Option<QuickLaunchAssetGroup>,
}

pub(crate) fn collect_quick_launch_records(tree: &AssetTree) -> Vec<QuickLaunchAssetRecord> {
    let mut records = Vec::new();
    for root_id in tree.root_ids() {
        collect_records_from_node(tree, root_id, None, &mut records);
    }
    records
}

pub(crate) fn matches_quick_launch_query(record: &QuickLaunchAssetRecord, query: &str) -> bool {
    let needle = query.trim().to_ascii_lowercase();
    if needle.is_empty() {
        return true;
    }

    [
        record.title.as_str(),
        record.subtitle.as_str(),
        record.badge.as_str(),
        record.meta.as_str(),
        record.environment.as_str(),
        record.remark.as_str(),
    ]
    .into_iter()
    .any(|value| value.to_ascii_lowercase().contains(&needle))
}

pub(crate) fn project_card_item(
    record: &QuickLaunchAssetRecord,
    favorite: bool,
) -> QuickLaunchCardItem {
    QuickLaunchCardItem {
        asset_id: record.asset_id.clone(),
        title: record.title.clone(),
        subtitle: record.subtitle.clone(),
        badge: record.badge.clone(),
        meta: record.meta.clone(),
        icon_kind: record.icon_kind.clone(),
        accent_kind: record.accent_kind.clone(),
        favorite,
    }
}

pub(crate) fn project_detail_item(
    record: &QuickLaunchAssetRecord,
    recent_label: String,
) -> QuickLaunchDetailItem {
    QuickLaunchDetailItem {
        asset_id: record.asset_id.clone(),
        title: record.title.clone(),
        subtitle: record.subtitle.clone(),
        environment: record.environment.clone(),
        auth_summary: record.auth_summary.clone(),
        proxy_summary: record.proxy_summary.clone(),
        remark: record.remark.clone(),
        recent_label,
    }
}

pub(crate) fn group_id_for_asset(tree: &AssetTree, asset_id: &str) -> Option<String> {
    let parent_id = tree.parent_id(asset_id).flatten()?;
    let node = tree.node(parent_id)?;
    (node.kind == ConsoleAssetKind::Folder).then(|| node.id.clone())
}

fn collect_records_from_node(
    tree: &AssetTree,
    node_id: &str,
    current_group: Option<QuickLaunchAssetGroup>,
    records: &mut Vec<QuickLaunchAssetRecord>,
) {
    let Some(node) = tree.node(node_id) else {
        return;
    };

    match node.kind {
        ConsoleAssetKind::Folder => {
            let group = QuickLaunchAssetGroup {
                id: node.id.clone(),
                label: node.title.clone(),
            };
            for child_id in &node.children {
                collect_records_from_node(tree, child_id, Some(group.clone()), records);
            }
        }
        ConsoleAssetKind::SshConnection => {
            if let Some(spec) = tree.ssh_connection_spec(node_id) {
                records.push(record_from_spec(
                    tree,
                    node.id.as_str(),
                    node.title.as_str(),
                    spec,
                    current_group,
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
    group: Option<QuickLaunchAssetGroup>,
) -> QuickLaunchAssetRecord {
    let badge = badge_for_environment(&spec.environment);
    let subtitle = format!("{}@{}", spec.user.trim(), spec.host.trim());
    let meta = meta_for_spec(spec);

    QuickLaunchAssetRecord {
        asset_id: asset_id.into(),
        title: title.into(),
        subtitle,
        badge,
        meta,
        icon_kind: icon_kind_for_asset(title, spec).into(),
        accent_kind: accent_kind_for_environment(&spec.environment).into(),
        environment: spec.environment.trim().to_string(),
        auth_summary: auth_summary_for_spec(spec),
        proxy_summary: proxy_summary_for_spec(tree, spec),
        remark: spec.remark.trim().to_string(),
        group,
    }
}

fn badge_for_environment(environment: &str) -> String {
    let trimmed = environment.trim();
    if trimmed.is_empty() {
        "SSH".into()
    } else {
        trimmed.into()
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

    parts.join(" · ")
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

fn auth_summary_for_spec(spec: &AssetSshConnectionSpec) -> String {
    match normalized_ssh_auth_source(&spec.auth_source) {
        SSH_AUTH_SOURCE_KEYCHAIN_IDENTITY => "Keychain identity".into(),
        _ => match spec.auth_method.trim() {
            "private-key" => match spec.private_key_source.trim() {
                "path" => "Private key file".into(),
                _ => "Inline private key".into(),
            },
            _ => "Password".into(),
        },
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

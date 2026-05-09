use mica_term::app::quick_launch_preferences::{QuickLaunchPreferences, QuickLaunchRecentAsset};
use mica_term::shell::assets::{
    AssetNodePayload, AssetSnippetSpec, AssetSocks5ProxySpec, AssetSshConnectionSpec,
    AssetSshProxySpec, AssetTree, ConsoleAssetKind,
};
use mica_term::shell::tabs::{WorkspaceTab, WorkspaceTabKind};
use mica_term::shell::view_model::ShellViewModel;

struct SeededQuickLaunchIds {
    prod: String,
    db: String,
    snippet: String,
}

fn seeded_view_model() -> (ShellViewModel, SeededQuickLaunchIds) {
    let mut tree = AssetTree::new();
    let folder_prod = tree.insert_root(ConsoleAssetKind::Folder, "Production");
    let prod = tree.insert_child_with_payload(
        &folder_prod,
        ConsoleAssetKind::SshConnection,
        "Prod Bastion",
        AssetNodePayload::SshConnection(AssetSshConnectionSpec {
            host: "prod.example.com".into(),
            user: "ops".into(),
            port: "22".into(),
            auth_method: "password".into(),
            auth_source: "manual".into(),
            environment: "prod".into(),
            remark: "Primary bastion".into(),
            ..AssetSshConnectionSpec::default()
        }),
    );
    let folder_db = tree.insert_root(ConsoleAssetKind::Folder, "Databases");
    let db = tree.insert_child_with_payload(
        &folder_db,
        ConsoleAssetKind::SshConnection,
        "DB Admin",
        AssetNodePayload::SshConnection(AssetSshConnectionSpec {
            host: "db.example.com".into(),
            user: "dba".into(),
            port: "2222".into(),
            auth_method: "private-key".into(),
            auth_source: "manual".into(),
            private_key_source: "path".into(),
            private_key_path: "/keys/db-admin".into(),
            environment: "staging".into(),
            proxy: AssetSshProxySpec::Socks5(AssetSocks5ProxySpec {
                host: "proxy.internal".into(),
                port: "1080".into(),
                username: "db-proxy".into(),
                password_credential_ref: None,
            }),
            proxy_method: "socks5".into(),
            remark: "Database maintenance".into(),
            ..AssetSshConnectionSpec::default()
        }),
    );
    let snippet_package = tree.insert_root(ConsoleAssetKind::SnippetPackage, "Ops Snippets");
    let snippet = tree.insert_child_with_payload(
        &snippet_package,
        ConsoleAssetKind::Snippet,
        "Restart Service",
        AssetNodePayload::Snippet(AssetSnippetSpec {
            script: "systemctl restart app".into(),
            package_id: Some(snippet_package.clone()),
        }),
    );

    let mut view_model = ShellViewModel::default();
    view_model.replace_console_asset_tree(tree);

    (view_model, SeededQuickLaunchIds { prod, db, snippet })
}

#[test]
fn quick_launch_recent_projection_prefers_mru_order_and_ssh_assets_only() {
    let (mut view_model, ids) = seeded_view_model();

    view_model.apply_quick_launch_preferences(QuickLaunchPreferences {
        recent_asset_ids: vec![
            QuickLaunchRecentAsset::new(ids.db.clone(), 1_700_000_000),
            QuickLaunchRecentAsset::new(ids.snippet.clone(), 1_699_999_900),
            QuickLaunchRecentAsset::new(ids.prod.clone(), 1_699_999_000),
        ],
    });

    let recent = view_model.quick_launch_recent_items_at(1_700_000_120);
    let projected_ids = recent
        .iter()
        .map(|item| item.asset_id.as_str())
        .collect::<Vec<_>>();

    assert_eq!(projected_ids, vec![ids.db.as_str(), ids.prod.as_str()]);
    assert_eq!(recent[0].subtitle, "dba@db.example.com");
    assert_eq!(recent[0].time_label, "2m ago");
    let db_accent = recent[0].accent_kind.clone();

    view_model.apply_quick_launch_preferences(QuickLaunchPreferences {
        recent_asset_ids: vec![
            QuickLaunchRecentAsset::new(ids.prod.clone(), 1_700_000_030),
            QuickLaunchRecentAsset::new(ids.db.clone(), 1_700_000_000),
        ],
    });

    let reordered = view_model.quick_launch_recent_items_at(1_700_000_120);
    let reordered_db = reordered
        .iter()
        .find(|item| item.asset_id == ids.db)
        .expect("db recent row after reorder");
    assert_eq!(
        reordered_db.accent_kind, db_accent,
        "recent row accent should remain stable for the same saved SSH asset"
    );
}

#[test]
fn quick_launch_recent_projection_caps_new_tab_rows_to_seven() {
    let (mut view_model, ids) = seeded_view_model();
    let mut tree = view_model.console_asset_tree().clone();
    let mut recent_asset_ids = vec![
        QuickLaunchRecentAsset::new(ids.db.clone(), 1_700_000_000),
        QuickLaunchRecentAsset::new(ids.prod.clone(), 1_699_999_000),
    ];
    for index in 0..8 {
        let asset_id = tree.insert_root_with_payload(
            ConsoleAssetKind::SshConnection,
            format!("Host {index}"),
            AssetNodePayload::SshConnection(AssetSshConnectionSpec {
                host: format!("10.0.0.{index}"),
                user: "ops".into(),
                ..AssetSshConnectionSpec::default()
            }),
        );
        recent_asset_ids.push(QuickLaunchRecentAsset::new(
            asset_id,
            1_699_998_000 - index as i64,
        ));
    }
    view_model.replace_console_asset_tree(tree);

    view_model.apply_quick_launch_preferences(QuickLaunchPreferences { recent_asset_ids });

    let recent = view_model.quick_launch_recent_items_at(1_700_000_120);

    assert_eq!(recent.len(), 7);
}

#[test]
fn quick_launch_recent_projection_includes_connected_saved_ssh_tabs() {
    let (mut view_model, ids) = seeded_view_model();
    view_model.apply_quick_launch_preferences(QuickLaunchPreferences {
        recent_asset_ids: vec![],
    });
    view_model.set_workspace_tabs(vec![WorkspaceTab {
        tab_id: "tab-prod".into(),
        session_id: "session-prod".into(),
        file_browser_session_id: String::new(),
        asset_id: ids.prod.clone(),
        display_name: "Prod Bastion".into(),
        host: "10.0.0.12".into(),
        username: "ops".into(),
        port: 22,
        connection_status: "connected".into(),
        title: "Prod Bastion".into(),
        subtitle: "ops@10.0.0.12:22".into(),
        state: "connected".into(),
        enhanced_session_state: String::new(),
        error_detail: String::new(),
        active: true,
        kind: WorkspaceTabKind::Terminal,
        reconnectable: false,
        connection_profile: None,
    }]);

    let recent = view_model.quick_launch_recent_items_at(1_700_000_120);

    assert_eq!(recent.len(), 1);
    assert_eq!(recent[0].asset_id, ids.prod);
    assert_eq!(recent[0].state_label, "Connected");
    assert_eq!(recent[0].time_label, "");
}

#[test]
fn quick_launch_recent_projection_deduplicates_connected_tabs_ahead_of_history() {
    let (mut view_model, ids) = seeded_view_model();
    view_model.apply_quick_launch_preferences(QuickLaunchPreferences {
        recent_asset_ids: vec![
            QuickLaunchRecentAsset::new(ids.db.clone(), 1_700_000_060),
            QuickLaunchRecentAsset::new(ids.prod.clone(), 1_700_000_000),
        ],
    });
    view_model.set_workspace_tabs(vec![WorkspaceTab {
        tab_id: "tab-prod".into(),
        session_id: "session-prod".into(),
        file_browser_session_id: String::new(),
        asset_id: ids.prod.clone(),
        display_name: "Prod Bastion".into(),
        host: "10.0.0.12".into(),
        username: "ops".into(),
        port: 22,
        connection_status: "connected".into(),
        title: "Prod Bastion".into(),
        subtitle: "ops@10.0.0.12:22".into(),
        state: "connected".into(),
        enhanced_session_state: String::new(),
        error_detail: String::new(),
        active: true,
        kind: WorkspaceTabKind::Terminal,
        reconnectable: false,
        connection_profile: None,
    }]);

    let recent = view_model.quick_launch_recent_items_at(1_700_000_120);
    let projected_ids = recent
        .iter()
        .map(|item| item.asset_id.as_str())
        .collect::<Vec<_>>();

    assert_eq!(projected_ids, vec![ids.prod.as_str(), ids.db.as_str()]);
    assert_eq!(recent[0].state_label, "Connected");
    assert_eq!(recent[1].state_label, "");
    assert_eq!(recent[1].time_label, "1m ago");
}

#[test]
fn saved_ssh_picker_projection_filters_to_saved_ssh_assets_in_tree_order() {
    let (mut view_model, ids) = seeded_view_model();

    view_model.set_saved_ssh_picker_query("db".into());

    let items = view_model.saved_ssh_picker_items();

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].id, ids.db);
    assert_eq!(items[0].kind, "ssh");
    assert!(items[0].compact_flat_mode);
    assert!(items[0].path_hint.contains("dba@db.example.com"));
    assert!(
        items.iter().all(|item| item.id != ids.snippet),
        "saved ssh picker must not project snippet assets"
    );
}

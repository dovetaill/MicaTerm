use mica_term::app::quick_launch_preferences::QuickLaunchPreferences;
use mica_term::shell::assets::{
    AssetNodePayload, AssetSnippetSpec, AssetSocks5ProxySpec, AssetSshConnectionSpec,
    AssetSshProxySpec, AssetTree, ConsoleAssetKind,
};
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
        recent_asset_ids: vec![ids.db.clone(), ids.snippet.clone(), ids.prod.clone()],
        favorite_asset_ids: vec![],
        last_selected_asset_id: None,
    });

    let recent = view_model.quick_launch_recent_items();
    let projected_ids = recent
        .iter()
        .map(|item| item.asset_id.as_str())
        .collect::<Vec<_>>();

    assert_eq!(projected_ids, vec![ids.db.as_str(), ids.prod.as_str()]);
    assert_eq!(recent[0].subtitle, "dba@db.example.com");
}

#[test]
fn quick_launch_group_projection_filters_by_search_query() {
    let (mut view_model, ids) = seeded_view_model();

    view_model.set_quick_launch_search_query("db".into());

    let groups = view_model.quick_launch_group_items();
    let visible = view_model.quick_launch_visible_group_items();

    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].label, "Databases");
    assert_eq!(groups[0].count, 1);
    assert_eq!(visible.len(), 1);
    assert_eq!(visible[0].asset_id, ids.db);
}

#[test]
fn quick_launch_selected_detail_reports_auth_and_proxy_summary() {
    let (mut view_model, ids) = seeded_view_model();

    view_model.select_quick_launch_asset(ids.db.clone());

    let detail = view_model
        .quick_launch_selected_detail()
        .expect("selected quick launch detail");

    assert_eq!(detail.asset_id, ids.db);
    assert_eq!(detail.environment, "staging");
    assert!(detail.auth_summary.contains("Private key"));
    assert!(detail.proxy_summary.contains("SOCKS5"));
}

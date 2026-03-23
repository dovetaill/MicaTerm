//! Basic bootstrap helper coverage for the binary entrypoint.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fs;
use std::rc::Rc;

use anyhow::{Result, anyhow};
use mica_term::AppWindow;
use mica_term::app::assets_catalog::{
    ASSET_CATALOG_SCHEMA_VERSION, AssetCatalogRepository, PersistedAssetCatalog,
    PersistedAssetKind, PersistedAssetNode, PersistedAssetPayload, PersistedSshConnectionSpec,
    catalog_to_asset_tree,
};
use mica_term::app::bootstrap::{
    app_title, bind_top_status_bar_with_store_and_effects_and_asset_repo, default_window_size,
};
use mica_term::app::logging::config::{AppLogMode, AppLoggingConfig};
use mica_term::app::logging::paths::{LoggingPaths, LoggingRootSource};
use mica_term::app::logging::runtime::build_test_logging_runtime;
use mica_term::app::window_effects::default_platform_window_effects;
use mica_term::shell::metrics::ShellMetrics;
use slint::Model;

#[derive(Default)]
struct AssetRepoState {
    load_calls: usize,
    save_attempts: Vec<PersistedAssetCatalog>,
}

struct RecordingAssetRepo {
    loaded_catalog: PersistedAssetCatalog,
    state: Rc<RefCell<AssetRepoState>>,
    save_error: Option<&'static str>,
}

impl RecordingAssetRepo {
    fn new(
        loaded_catalog: PersistedAssetCatalog,
        state: Rc<RefCell<AssetRepoState>>,
        save_error: Option<&'static str>,
    ) -> Self {
        Self {
            loaded_catalog,
            state,
            save_error,
        }
    }
}

impl AssetCatalogRepository for RecordingAssetRepo {
    fn load(&self) -> Result<PersistedAssetCatalog> {
        self.state.borrow_mut().load_calls += 1;
        Ok(self.loaded_catalog.clone())
    }

    fn save(&self, catalog: &PersistedAssetCatalog) -> Result<()> {
        self.state.borrow_mut().save_attempts.push(catalog.clone());
        if let Some(message) = self.save_error {
            return Err(anyhow!(message));
        }

        Ok(())
    }
}

fn loaded_catalog_for_bootstrap() -> PersistedAssetCatalog {
    PersistedAssetCatalog {
        schema_version: ASSET_CATALOG_SCHEMA_VERSION,
        root_ids: vec!["folder-root".into(), "ssh-root".into()],
        nodes: BTreeMap::from([
            (
                "folder-root".into(),
                PersistedAssetNode {
                    id: "folder-root".into(),
                    parent_id: None,
                    title: "Team".into(),
                    kind: PersistedAssetKind::Folder,
                    child_ids: Vec::new(),
                    payload: PersistedAssetPayload::Folder,
                },
            ),
            (
                "ssh-root".into(),
                PersistedAssetNode {
                    id: "ssh-root".into(),
                    parent_id: None,
                    title: "Gateway".into(),
                    kind: PersistedAssetKind::SshConnection,
                    child_ids: Vec::new(),
                    payload: PersistedAssetPayload::SshConnection(PersistedSshConnectionSpec {
                        host: "gateway.example.com".into(),
                        user: "ops".into(),
                        port: "2022".into(),
                        environment: "prod".into(),
                        proxy_method: "jump-host".into(),
                    }),
                },
            ),
        ]),
    }
}

#[test]
fn bootstrap_exposes_shell_default_window_budget() {
    assert_eq!(app_title(), "Mica Term");
    assert_eq!(
        default_window_size(),
        (
            ShellMetrics::WINDOW_DEFAULT_WIDTH,
            ShellMetrics::WINDOW_DEFAULT_HEIGHT,
        )
    );
}

#[test]
fn bootstrap_loads_catalog_before_first_asset_projection_sync() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let repo_state = Rc::new(RefCell::new(AssetRepoState::default()));
    let asset_repo: Rc<dyn AssetCatalogRepository> = Rc::new(RecordingAssetRepo::new(
        loaded_catalog_for_bootstrap(),
        Rc::clone(&repo_state),
        None,
    ));

    bind_top_status_bar_with_store_and_effects_and_asset_repo(
        &app,
        None,
        default_platform_window_effects(),
        Some(asset_repo),
    );

    let rows = app.get_console_asset_items();
    assert_eq!(rows.row_count(), 2);
    assert_eq!(rows.row_data(0).unwrap().label.as_str(), "Team");
    assert_eq!(rows.row_data(1).unwrap().label.as_str(), "Gateway");

    let state = repo_state.borrow();
    assert_eq!(state.load_calls, 1);
    assert!(state.save_attempts.is_empty());
}

#[test]
fn create_rename_delete_and_ssh_edit_trigger_repository_save() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let repo_state = Rc::new(RefCell::new(AssetRepoState::default()));
    let asset_repo: Rc<dyn AssetCatalogRepository> = Rc::new(RecordingAssetRepo::new(
        PersistedAssetCatalog {
            schema_version: ASSET_CATALOG_SCHEMA_VERSION,
            root_ids: Vec::new(),
            nodes: BTreeMap::new(),
        },
        Rc::clone(&repo_state),
        None,
    ));

    bind_top_status_bar_with_store_and_effects_and_asset_repo(
        &app,
        None,
        default_platform_window_effects(),
        Some(asset_repo),
    );

    app.invoke_toggle_assets_search_requested();
    app.invoke_assets_search_query_changed("prod".into());
    app.invoke_toggle_assets_view_mode_requested();
    assert!(repo_state.borrow().save_attempts.is_empty());
    app.invoke_toggle_assets_view_mode_requested();
    app.invoke_assets_search_query_changed("".into());
    app.invoke_close_assets_search_requested();

    app.invoke_assets_create_action_selected("new-folder".into());
    app.invoke_asset_folder_modal_name_changed("Prod".into());
    app.invoke_confirm_asset_modal_requested();
    assert_eq!(repo_state.borrow().save_attempts.len(), 1);

    let folder_id = app
        .get_console_asset_items()
        .row_data(0)
        .unwrap()
        .id
        .to_string();

    app.invoke_asset_context_menu_requested(folder_id.clone().into(), "folder".into(), 96.0, 160.0);
    app.invoke_assets_context_menu_action_invoked("new-ssh-connection".into());
    app.invoke_asset_ssh_modal_draft_changed("name".into(), "Prod Bastion".into());
    app.invoke_asset_ssh_modal_draft_changed("host".into(), "10.0.0.12".into());
    app.invoke_asset_ssh_modal_draft_changed("proxy_method".into(), "jump-host".into());
    app.invoke_confirm_asset_modal_requested();
    assert_eq!(repo_state.borrow().save_attempts.len(), 2);

    app.invoke_asset_context_menu_requested(folder_id.clone().into(), "folder".into(), 96.0, 160.0);
    app.invoke_assets_context_menu_action_invoked("rename-asset".into());
    app.invoke_asset_rename_modal_name_changed("Infra".into());
    app.invoke_confirm_asset_rename_requested();
    assert_eq!(repo_state.borrow().save_attempts.len(), 3);

    let ssh_id = app
        .get_console_asset_items()
        .row_data(1)
        .unwrap()
        .id
        .to_string();
    app.invoke_asset_context_menu_requested(ssh_id.into(), "ssh".into(), 96.0, 160.0);
    app.invoke_assets_context_menu_action_invoked("delete-asset".into());
    app.invoke_confirm_delete_asset_requested();
    assert_eq!(repo_state.borrow().save_attempts.len(), 4);
}

#[test]
fn save_failure_logs_error_without_persisting_ui_session_state() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let temp_root = std::env::temp_dir()
        .join("mica-term")
        .join("tests")
        .join("assets-persistence-save-error");
    let _ = fs::remove_dir_all(&temp_root);
    fs::create_dir_all(temp_root.join("logs")).unwrap();
    fs::create_dir_all(temp_root.join("crash")).unwrap();

    let paths = LoggingPaths {
        root_source: LoggingRootSource::EnvOverride,
        root_dir: temp_root.clone(),
        logs_dir: temp_root.join("logs"),
        crash_dir: temp_root.join("crash"),
    };
    let runtime =
        build_test_logging_runtime(&paths, &AppLoggingConfig::new(AppLogMode::Debug)).unwrap();

    let repo_state = Rc::new(RefCell::new(AssetRepoState::default()));
    let asset_repo: Rc<dyn AssetCatalogRepository> = Rc::new(RecordingAssetRepo::new(
        PersistedAssetCatalog {
            schema_version: ASSET_CATALOG_SCHEMA_VERSION,
            root_ids: vec!["folder-root".into()],
            nodes: BTreeMap::from([(
                "folder-root".into(),
                PersistedAssetNode {
                    id: "folder-root".into(),
                    parent_id: None,
                    title: "Prod".into(),
                    kind: PersistedAssetKind::Folder,
                    child_ids: Vec::new(),
                    payload: PersistedAssetPayload::Folder,
                },
            )]),
        },
        Rc::clone(&repo_state),
        Some("disk full"),
    ));

    tracing::dispatcher::with_default(&runtime.dispatch, || {
        bind_top_status_bar_with_store_and_effects_and_asset_repo(
            &app,
            None,
            default_platform_window_effects(),
            Some(asset_repo),
        );

        let folder_id = app
            .get_console_asset_items()
            .row_data(0)
            .unwrap()
            .id
            .to_string();
        app.invoke_toggle_assets_tree_expansion_requested();
        app.invoke_toggle_assets_search_requested();
        app.invoke_assets_search_query_changed("Prod".into());
        app.invoke_asset_selected(folder_id.clone().into());
        app.invoke_asset_context_menu_requested(folder_id.into(), "folder".into(), 96.0, 160.0);
        app.invoke_assets_context_menu_action_invoked("rename-asset".into());
        app.invoke_asset_rename_modal_name_changed("Infra".into());
        app.invoke_confirm_asset_rename_requested();
    });

    drop(runtime.guard);

    let log_content = fs::read_to_string(paths.logs_dir.join("system-error.log")).unwrap();
    assert!(log_content.contains("failed to save asset catalog"));
    assert!(log_content.contains("error=disk full"));

    let save_attempts = &repo_state.borrow().save_attempts;
    assert_eq!(save_attempts.len(), 1);
    let persisted_tree = catalog_to_asset_tree(&save_attempts[0]);
    assert_eq!(persisted_tree.is_expanded("folder-root"), Some(false));

    let _ = fs::remove_dir_all(temp_root);
}

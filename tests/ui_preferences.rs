//! Persistence coverage for saved UI preferences.

use std::fs;

use mica_term::app::terminal_semantic::OutputRuleProfile;
use mica_term::app::ui_preferences::{DownloadConflictDefault, UiPreferences, UiPreferencesStore};
use mica_term::shell::view_model::RightPanelView;
use mica_term::theme::{SearchMatchHighlightStrength, ThemeMode};

#[test]
fn ui_preferences_default_to_dark_and_not_pinned() {
    let prefs = UiPreferences::default();

    assert_eq!(prefs.theme_mode, ThemeMode::Dark);
    assert!(!prefs.always_on_top);
    assert_eq!(prefs.right_panel_view, "sftp");
}

#[test]
fn ui_preferences_default_terminal_settings_match_memory_plan() {
    let prefs = UiPreferences::default();

    assert_eq!(prefs.terminal_scrollback_limit, 1500);
    assert!(prefs.terminal_active_idle_shrink_enabled);
    assert!(prefs.terminal_input_highlighting_enabled);
    assert!(prefs.terminal_output_rule_highlighting_enabled);
    assert!(prefs.terminal_command_decorations_enabled);
    assert!(
        !prefs.terminal_overview_markers_enabled,
        "overview markers should default off so transcript guesses do not add extra terminal chrome until the user opts in"
    );
    assert_eq!(
        prefs.terminal_output_rule_profile,
        OutputRuleProfile::Focused,
        "terminal output highlighting should default to the focused low-risk profile"
    );
    assert_eq!(
        prefs.terminal_search_match_highlight,
        SearchMatchHighlightStrength::Balanced
    );
}

#[test]
fn ui_preferences_defaults_to_ask_for_download_conflicts() {
    let prefs = UiPreferences::default();

    assert_eq!(
        prefs.download_conflict_default,
        DownloadConflictDefault::Ask
    );
}

#[test]
fn ui_preferences_roundtrip_theme_and_pin_state() {
    let temp_path = std::env::temp_dir()
        .join("mica-term")
        .join("tests")
        .join("ui-preferences-roundtrip.json");

    let store = UiPreferencesStore::new(temp_path.clone());
    let prefs = UiPreferences {
        theme_mode: ThemeMode::Light,
        always_on_top: true,
        right_panel_view: "vault".into(),
        ..UiPreferences::default()
    };

    store.save(&prefs).unwrap();
    let loaded = store.load_or_default().unwrap();

    assert_eq!(loaded, prefs);
    let _ = std::fs::remove_file(temp_path);
}

#[test]
fn ui_preferences_roundtrip_terminal_settings() {
    let temp_path = std::env::temp_dir()
        .join("mica-term")
        .join("tests")
        .join("ui-preferences-terminal-settings-roundtrip.json");

    let store = UiPreferencesStore::new(temp_path.clone());
    let prefs = UiPreferences {
        terminal_scrollback_limit: 3000,
        terminal_active_idle_shrink_enabled: false,
        terminal_input_highlighting_enabled: false,
        terminal_output_rule_highlighting_enabled: false,
        terminal_command_decorations_enabled: false,
        terminal_overview_markers_enabled: false,
        terminal_output_rule_profile: OutputRuleProfile::Focused,
        terminal_search_match_highlight: SearchMatchHighlightStrength::Strong,
        ..UiPreferences::default()
    };

    store.save(&prefs).unwrap();
    let loaded = store.load_or_default().unwrap();

    assert_eq!(loaded, prefs);
    let _ = std::fs::remove_file(temp_path);
}

#[test]
fn ui_preferences_roundtrip_download_conflict_default() {
    let temp_path = std::env::temp_dir()
        .join("mica-term")
        .join("tests")
        .join("ui-preferences-download-conflict-default-roundtrip.json");

    let store = UiPreferencesStore::new(temp_path.clone());
    let prefs = UiPreferences {
        download_conflict_default: DownloadConflictDefault::AutoRename,
        ..UiPreferences::default()
    };

    store.save(&prefs).unwrap();
    let loaded = store.load_or_default().unwrap();

    assert_eq!(loaded, prefs);
    let _ = std::fs::remove_file(temp_path);
}

#[test]
fn ui_preferences_accept_sftp_right_panel_view() {
    let prefs = UiPreferences {
        right_panel_view: "sftp".into(),
        ..UiPreferences::default()
    };

    assert_eq!(prefs.right_panel_view, "sftp");
    assert_eq!(
        RightPanelView::from_id(&prefs.right_panel_view).id(),
        "sftp"
    );
}

#[test]
fn legacy_appearance_preference_migrates_to_sftp() {
    assert_eq!(RightPanelView::from_id("appearance").id(), "sftp");
}

#[test]
fn shell_terminal_tokens_stay_synced_to_theme_backed_terminal_palette_contract() {
    let tokens = fs::read_to_string("ui/theme/tokens.slint").expect("read theme tokens");
    let app_window = fs::read_to_string("ui/app-window.slint").expect("read app window");
    let workspace_pane =
        fs::read_to_string("ui/shell/workspace-pane.slint").expect("read workspace pane");
    let terminal_host =
        fs::read_to_string("ui/shell/terminal-session-host.slint").expect("read terminal host");

    for token in [
        "terminal-default-fg",
        "terminal-default-bg",
        "terminal-cursor-fg",
        "terminal-cursor-bg",
        "terminal-selection-surface",
        "terminal-scrollbar-track-surface",
        "terminal-scrollbar-thumb-surface",
        "terminal-scrollbar-thumb-active-surface",
        "terminal-frame-background",
    ] {
        assert!(
            tokens.contains(token),
            "theme tokens should expose `{token}` so Slint terminal chrome stays synchronized with the Rust terminal palette presets",
        );
    }

    assert!(
        app_window.contains("ThemeTokens.terminal-default-fg"),
        "AppWindow should source the workspace terminal foreground default from ThemeTokens instead of carrying a detached inline color ladder"
    );
    assert!(
        app_window.contains("ThemeTokens.terminal-default-bg"),
        "AppWindow should source the workspace terminal background default from ThemeTokens instead of carrying a detached inline color ladder"
    );
    assert!(
        app_window.contains("ThemeTokens.terminal-cursor-fg")
            && app_window.contains("ThemeTokens.terminal-cursor-bg"),
        "AppWindow should source cursor colors from ThemeTokens so dark/light terminal presets stay aligned with shell theme mode"
    );
    assert!(
        workspace_pane.contains("ThemeTokens.terminal-default-fg")
            && workspace_pane.contains("ThemeTokens.terminal-default-bg"),
        "WorkspacePane should inherit terminal defaults from ThemeTokens rather than restating divergent inline colors"
    );
    assert!(
        workspace_pane.contains("ThemeTokens.terminal-cursor-fg")
            && workspace_pane.contains("ThemeTokens.terminal-cursor-bg"),
        "WorkspacePane should inherit terminal cursor colors from ThemeTokens"
    );
    assert!(
        terminal_host.contains("ThemeTokens.terminal-selection-surface"),
        "TerminalSessionHost should keep a boot-time selection token default so the shell can start in sync before Rust projects the active terminal preset"
    );
    assert!(
        app_window.contains("workspace-session-selection-surface")
            && app_window.contains("workspace-session-scrollbar-track")
            && app_window.contains("workspace-session-frame-surface")
            && app_window.contains("workspace-session-frame-border"),
        "AppWindow should store projected terminal selection, scrollbar-track, and frame colors so Rust can override boot defaults with the active terminal preset"
    );
    assert!(
        workspace_pane.contains("workspace-session-selection-surface")
            && workspace_pane.contains("workspace-session-scrollbar-track")
            && workspace_pane.contains("workspace-session-frame-surface")
            && workspace_pane.contains("workspace-session-frame-border"),
        "WorkspacePane should thread projected terminal selection, scrollbar-track, and frame colors through to TerminalSessionHost"
    );
    assert!(
        terminal_host.contains("session-selection-surface")
            && terminal_host.contains("session-scrollbar-track")
            && terminal_host.contains("session-frame-surface")
            && terminal_host.contains("session-frame-border"),
        "TerminalSessionHost should consume projected terminal selection, scrollbar-track, and frame colors instead of hard-coding a detached shell ladder"
    );
}

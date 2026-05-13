use std::{fs, path::Path};

#[test]
fn bundled_ui_font_assets_exist() {
    assert!(
        Path::new("assets/fonts/JetBrainsMapleMono/JetBrainsMapleMono-Regular.ttf").exists(),
        "the shell UI bundle should ship a JetBrains Maple Mono regular face"
    );
    assert!(
        !Path::new("assets/fonts/JetBrainsMapleMono/JetBrainsMapleMono-Medium.ttf").exists(),
        "the shell UI bundle should stop shipping a separate JetBrains Maple Mono medium face once the shell collapses onto the regular weight"
    );
    assert!(
        !Path::new("assets/fonts/JetBrainsMapleMono/JetBrainsMapleMono-SemiBold.ttf").exists(),
        "the shell UI bundle should stop shipping a separate JetBrains Maple Mono semibold face once the shell collapses onto the regular weight"
    );
    assert!(
        Path::new("assets/fonts/JetBrainsMapleMono/LICENSE.txt").exists(),
        "the shell UI bundle should ship the upstream JetBrains Maple Mono license text"
    );
}

#[test]
fn typography_theme_exposes_the_ui_font_contract() {
    let source = fs::read_to_string("ui/theme/typography.slint").expect("read typography theme");

    assert!(source.contains("ui-font-family: \"JetBrains Maple Mono\";"));
    assert!(source.contains("ui-font-weight-regular: 400;"));
    assert!(source.contains("ui-font-weight-medium: 400;"));
    assert!(source.contains("ui-font-weight-semibold: 400;"));
    assert!(source.contains("ui-font-size-body: 14px;"));
    assert!(source.contains("ui-font-size-caption: 13px;"));
    assert!(!source.contains("SarasaUiSC"));
}

#[test]
fn app_window_uses_jetbrains_maple_mono_as_the_shell_default() {
    let source = fs::read_to_string("ui/app-window.slint").expect("read app window");

    assert!(
        source.contains(
            "import \"../assets/fonts/JetBrainsMapleMono/JetBrainsMapleMono-Regular.ttf\";"
        )
    );
    assert!(!source.contains("JetBrainsMapleMono-Medium.ttf"));
    assert!(!source.contains("JetBrainsMapleMono-SemiBold.ttf"));
    assert!(source.contains("import { AppTypography } from \"theme/typography.slint\";"));
    assert!(source.contains("default-font-family: AppTypography.ui-font-family;"));
    assert!(source.contains("default-font-weight: AppTypography.ui-font-weight-regular;"));
    assert!(source.contains("default-font-size: AppTypography.ui-font-size-body;"));
    assert!(!source.contains("SarasaUiSC"));
}

#[test]
fn popup_menu_uses_the_shared_ui_font_family() {
    let source =
        fs::read_to_string("ui/components/titlebar-menu.slint").expect("read titlebar menu");

    assert!(source.contains("import { AppTypography } from \"../theme/typography.slint\";"));
    assert!(source.contains("font-family: AppTypography.ui-font-family;"));
    assert!(source.contains("font-size: AppTypography.ui-font-size-body;"));
    assert!(source.contains("font-weight: AppTypography.ui-font-weight-regular;"));
}

#[test]
fn shell_chrome_text_uses_regular_weight_and_zero_small_tracking() {
    let active_tab = fs::read_to_string("ui/components/active-tab.slint").expect("read active tab");
    let asset_row =
        fs::read_to_string("ui/components/asset-node-row.slint").expect("read asset row");
    let sidebar = fs::read_to_string("ui/shell/assets-sidebar.slint").expect("read assets sidebar");
    let menu = fs::read_to_string("ui/components/titlebar-menu.slint").expect("read titlebar menu");

    assert!(
        active_tab.contains("font-weight: AppTypography.ui-font-weight-regular;"),
        "tab labels should settle on JetBrains Maple Mono Regular across active and inactive states so the shell chrome stops looking over-inked at Windows small sizes"
    );
    assert!(
        active_tab.contains("letter-spacing: 0px;"),
        "tab labels should zero out extra tracking so small Windows shell text stops picking up synthetic spacing that exaggerates jagged edges"
    );
    assert!(
        asset_row.contains("font-weight: AppTypography.ui-font-weight-regular;"),
        "asset tree labels should request JetBrains Maple Mono Regular explicitly so dense Chinese labels stop looking over-weighted in the shell sidebar"
    );
    assert!(
        asset_row.contains(
            "text: root.label;\n        font-family: AppTypography.ui-font-family;\n        font-size: AppTypography.ui-font-size-caption;"
        ),
        "asset tree labels should drop from body size to the shared caption size so the assets list stops feeling oversized relative to the surrounding shell chrome"
    );
    assert!(
        asset_row.contains("row-height: AppTypography.ui-sidebar-row-height;"),
        "asset tree rows should use the shared sidebar row-height token so list geometry cannot silently drift away from the actual row box"
    );
    assert!(
        sidebar.contains("letter-spacing: 0px;"),
        "sidebar section headings should not inject extra tracking at 14px because Windows shell chrome already gets enough air from row geometry"
    );
    assert!(
        menu.contains("letter-spacing: 0px;"),
        "menu labels should zero small-size tracking so mixed-language shell chrome stops showing exaggerated pixel seams"
    );
}

#[test]
fn small_shell_chrome_controls_use_explicit_shared_regular_contract() {
    let tooltip = fs::read_to_string("ui/components/titlebar-tooltip.slint").expect("read tooltip");
    let context_menu = fs::read_to_string("ui/components/assets-context-menu-row.slint")
        .expect("read assets context menu row");
    let toolbar_menu = fs::read_to_string("ui/components/assets-toolbar-menu-row.slint")
        .expect("read assets toolbar menu row");
    let welcome = fs::read_to_string("ui/welcome/welcome-view.slint").expect("read welcome view");

    for source in [&tooltip, &context_menu, &toolbar_menu, &welcome] {
        assert!(
            source.contains("font-family: AppTypography.ui-font-family;"),
            "small shell chrome text should stop inheriting whatever default happens to be active and instead request the shared JetBrains Maple Mono family explicitly"
        );
    }

    for source in [&tooltip, &context_menu, &toolbar_menu] {
        assert!(
            source.contains("font-size: AppTypography.ui-font-size-body;")
                && source.contains("font-weight: AppTypography.ui-font-weight-regular;")
                && source.contains("letter-spacing: 0px;"),
            "tooltips and asset menus should use the same explicit 14px JetBrains Maple Mono Regular contract with zero extra tracking so hover chrome does not look rougher than nearby shell labels"
        );
    }

    assert!(
        welcome.contains("text: \"Open a recent connection or browse saved SSH targets.\";")
            && welcome.contains("font-size: AppTypography.ui-font-size-body;")
            && welcome.contains("font-weight: AppTypography.ui-font-weight-regular;"),
        "the Welcome helper copy should use the same JetBrains Maple Mono Regular shell chrome contract as menus and tabs instead of staying heavier than nearby explanatory text"
    );
}

#[test]
fn weak_small_text_hotspots_stop_relying_on_tiny_sizes_and_low_opacity() {
    let asset_row =
        fs::read_to_string("ui/components/asset-node-row.slint").expect("read asset row");
    let quick_launch_section = fs::read_to_string("ui/welcome/quick-launch-section.slint")
        .expect("read quick launch section");
    let quick_launch_card =
        fs::read_to_string("ui/welcome/quick-launch-card.slint").expect("read quick launch card");
    let right_panel = fs::read_to_string("ui/shell/right-panel.slint").expect("read right panel");

    assert!(
        asset_row.contains("text: root.path-hint;")
            && asset_row.contains("font-weight: AppTypography.ui-font-weight-regular;")
            && asset_row.contains("color: root.shell-text-secondary;"),
        "asset row helper text should use JetBrains Maple Mono Regular so sidebar metadata stops reading heavier than the surrounding Windows shell copy"
    );

    assert!(
        quick_launch_section.contains("text: root.subtitle;")
            && quick_launch_section.contains("text: root.empty_text;")
            && quick_launch_section.contains("font-weight: AppTypography.ui-font-weight-regular;"),
        "welcome section subtitles and empty-state copy should use the shared regular shell weight so they stay readable without picking up extra small-size darkness"
    );

    assert!(
        !quick_launch_card.contains("opacity: 0.58;")
            && !quick_launch_card.contains("opacity: 0.82;")
            && quick_launch_card.contains("color: ThemeTokens.text-secondary;"),
        "quick launch cards should stop depending on low-opacity primary text for secondary lines because that reads fuzzy in the Windows shell"
    );

    assert!(
        !Path::new("ui/welcome/quick-launch-detail-pane.slint").exists(),
        "the retired quick launch detail pane should be removed once the New Tab flow no longer uses the old quick-launch detail domain"
    );

    assert!(
        right_panel.contains("text: root.sftp-status-copy();")
            && right_panel.contains("text: item.name;")
            && right_panel.contains("text: item.meta_label;")
            && right_panel.contains("color: ThemeTokens.text-primary;")
            && right_panel.contains("color: ThemeTokens.text-secondary;")
            && right_panel.contains("font-size: 12px;")
            && right_panel.contains("font-size: 11px;"),
        "right panel typography should keep the virtualized SFTP rows on explicit semantic colors plus the current 12px name / 11px meta rhythm"
    );

    assert!(
        right_panel.contains("font-size: AppTypography.ui-font-size-caption;")
            && right_panel.contains("font-weight: AppTypography.ui-font-weight-regular;"),
        "right-panel SFTP header labels should use regular-weight caption text so tiny column headings do not look darker than the file rows beneath them"
    );
}

#[test]
fn assets_sidebar_list_height_tracks_the_shared_row_height() {
    let typography = fs::read_to_string("ui/theme/typography.slint").expect("read typography");
    let sidebar = fs::read_to_string("ui/shell/assets-sidebar.slint").expect("read assets sidebar");

    assert!(
        typography.contains("ui-sidebar-row-height: 30px;"),
        "typography should expose a shared sidebar row-height token so shell list geometry and row geometry stay in lock-step"
    );

    for expected in [
        "root.console-asset-items.length * AppTypography.ui-sidebar-row-height",
        "root.snippet-asset-items.length * AppTypography.ui-sidebar-row-height",
        "root.keychain-asset-items.length * AppTypography.ui-sidebar-row-height",
    ] {
        assert!(
            sidebar.contains(expected),
            "assets sidebar should size each list host with `{expected}` so the selected-row frame bottom is not clipped by stale 28px geometry"
        );
    }
}

#[test]
fn build_script_tracks_ui_typography_assets() {
    let source = fs::read_to_string("build.rs").expect("read build script");

    assert!(source.contains("assets/fonts/JetBrainsMapleMono/JetBrainsMapleMono-Regular.ttf"));
    assert!(!source.contains("assets/fonts/JetBrainsMapleMono/JetBrainsMapleMono-Medium.ttf"));
    assert!(!source.contains("assets/fonts/JetBrainsMapleMono/JetBrainsMapleMono-SemiBold.ttf"));
    assert!(!source.contains("assets/fonts/SarasaUiSC"));
}

#[test]
fn font_diagnostics_report_regular_shell_chrome_and_zero_small_tracking() {
    let source = fs::read_to_string("src/app/font_diagnostics.rs").expect("read font diagnostics");

    assert!(source.contains("pub const UI_CHROME_FONT_WEIGHT: i32 = 400;"));
    assert!(source.contains("pub const UI_CHROME_LETTER_SPACING_PX: f32 = 0.0;"));
}

#[test]
fn small_shell_functional_text_stops_requesting_hardcoded_semibold() {
    for path in [
        "ui/components/status-pill.slint",
        "ui/components/modal-chrome.slint",
        "ui/components/settings-modal.slint",
        "ui/components/assets-delete-confirm-modal.slint",
        "ui/components/assets-folder-create-modal.slint",
        "ui/components/assets-keychain-identity-modal.slint",
        "ui/components/assets-keychain-ssh-key-modal.slint",
        "ui/components/assets-rename-modal.slint",
        "ui/components/assets-snippet-package-modal.slint",
        "ui/components/assets-ssh-connection-modal.slint",
        "ui/components/sftp-conflict-modal.slint",
        "ui/components/sftp-remote-file-modal.slint",
        "ui/components/ssh-host-key-confirm-modal.slint",
        "ui/components/vault-provider-card.slint",
        "ui/components/workspace-paste-warning-modal.slint",
        "ui/shell/transfer-center.slint",
    ] {
        let source = fs::read_to_string(path).unwrap_or_else(|_| panic!("read {path}"));
        assert!(
            !source.contains("font-weight: 600;"),
            "{path} should stop hardcoding semibold shell labels now that the bundled shell UI face is regular-only"
        );
    }
}

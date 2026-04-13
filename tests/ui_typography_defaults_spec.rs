use std::{fs, path::Path};

#[test]
fn bundled_ui_font_assets_exist() {
    assert!(
        Path::new("assets/fonts/MiSans/MiSans-Regular.ttf").exists(),
        "the shell UI bundle should ship a MiSans regular face"
    );
    assert!(
        Path::new("assets/fonts/MiSans/MiSans-Medium.ttf").exists(),
        "the shell UI bundle should ship a MiSans medium face for the default Windows chrome weight"
    );
    assert!(
        Path::new("assets/fonts/MiSans/MiSans-Semibold.ttf").exists(),
        "the shell UI bundle should ship a MiSans semibold face for emphasis"
    );
    assert!(
        Path::new("assets/fonts/MiSans/LICENSE.txt").exists(),
        "the shell UI bundle should ship the upstream MiSans license text"
    );
}

#[test]
fn typography_theme_exposes_the_ui_font_contract() {
    let source = fs::read_to_string("ui/theme/typography.slint").expect("read typography theme");

    assert!(source.contains("ui-font-family: \"MiSans\";"));
    assert!(source.contains("ui-font-weight-regular: 400;"));
    assert!(source.contains("ui-font-weight-medium: 500;"));
    assert!(source.contains("ui-font-weight-semibold: 600;"));
    assert!(source.contains("ui-font-size-body: 14px;"));
    assert!(source.contains("ui-font-size-caption: 13px;"));
    assert!(!source.contains("SarasaUiSC"));
}

#[test]
fn app_window_uses_misans_as_the_shell_default() {
    let source = fs::read_to_string("ui/app-window.slint").expect("read app window");

    assert!(source.contains("import \"../assets/fonts/MiSans/MiSans-Regular.ttf\";"));
    assert!(source.contains("import \"../assets/fonts/MiSans/MiSans-Medium.ttf\";"));
    assert!(source.contains("import \"../assets/fonts/MiSans/MiSans-Semibold.ttf\";"));
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
    assert!(source.contains("font-weight: AppTypography.ui-font-weight-medium;"));
}

#[test]
fn shell_chrome_text_uses_medium_weight_and_roomier_rows() {
    let active_tab = fs::read_to_string("ui/components/active-tab.slint").expect("read active tab");
    let asset_row =
        fs::read_to_string("ui/components/asset-node-row.slint").expect("read asset row");
    let sidebar = fs::read_to_string("ui/shell/assets-sidebar.slint").expect("read assets sidebar");
    let menu = fs::read_to_string("ui/components/titlebar-menu.slint").expect("read titlebar menu");

    assert!(
        active_tab.contains("font-weight: AppTypography.ui-font-weight-medium;"),
        "tab labels should settle on MiSans Medium across active and inactive states so the shell chrome stops looking under-inked on Windows"
    );
    assert!(
        active_tab.contains("letter-spacing: 0.08px;"),
        "tab labels should add a slight positive tracking value so English and number-heavy titles stop looking cramped on Windows"
    );
    assert!(
        asset_row.contains("font-weight: AppTypography.ui-font-weight-medium;"),
        "asset tree labels should request MiSans Medium explicitly so dense Chinese labels stop collapsing into a washed-out regular weight"
    );
    assert!(
        asset_row.contains("row-height: AppTypography.ui-sidebar-row-height;"),
        "asset tree rows should use the shared sidebar row-height token so list geometry cannot silently drift away from the actual row box"
    );
    assert!(
        sidebar.contains("letter-spacing: 0.08px;"),
        "sidebar section headings should add a slight tracking bump so MiSans shell chrome keeps a cleaner rhythm at 14px"
    );
    assert!(
        menu.contains("letter-spacing: 0.1px;"),
        "menu labels should add a slight tracking bump so small mixed-language labels stop looking glued together"
    );
}

#[test]
fn small_shell_chrome_controls_use_explicit_misans_medium_contract() {
    let tooltip = fs::read_to_string("ui/components/titlebar-tooltip.slint").expect("read tooltip");
    let context_menu = fs::read_to_string("ui/components/assets-context-menu-row.slint")
        .expect("read assets context menu row");
    let toolbar_menu = fs::read_to_string("ui/components/assets-toolbar-menu-row.slint")
        .expect("read assets toolbar menu row");
    let welcome = fs::read_to_string("ui/welcome/welcome-view.slint").expect("read welcome view");

    for source in [&tooltip, &context_menu, &toolbar_menu, &welcome] {
        assert!(
            source.contains("font-family: AppTypography.ui-font-family;"),
            "small shell chrome text should stop inheriting whatever default happens to be active and instead request the shared MiSans family explicitly"
        );
    }

    for source in [&tooltip, &context_menu, &toolbar_menu] {
        assert!(
            source.contains("font-size: AppTypography.ui-font-size-body;")
                && source.contains("font-weight: AppTypography.ui-font-weight-medium;"),
            "tooltips and asset menus should use the same explicit 14px MiSans Medium chrome contract so right-click menus and hover affordances stop looking thinner than nearby shell labels"
        );
    }

    assert!(
        welcome.contains("font-size: AppTypography.ui-font-size-body;")
            && welcome.contains("font-weight: AppTypography.ui-font-weight-medium;"),
        "the Welcome primary action button should use the same MiSans Medium shell chrome contract as menus and tabs instead of floating on default regular text"
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

    assert!(source.contains("assets/fonts/MiSans/MiSans-Regular.ttf"));
    assert!(source.contains("assets/fonts/MiSans/MiSans-Medium.ttf"));
    assert!(source.contains("assets/fonts/MiSans/MiSans-Semibold.ttf"));
    assert!(!source.contains("assets/fonts/SarasaUiSC"));
}

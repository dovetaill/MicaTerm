use std::fs;

#[test]
fn semantic_theme_tokens_cover_shell_hierarchy_and_states() {
    let tokens = fs::read_to_string("ui/theme/tokens.slint").expect("read theme tokens");

    for token in [
        "out property <brush> titlebar-background:",
        "out property <brush> tabbar-background:",
        "out property <brush> sidebar-background:",
        "out property <brush> sidebar-panel-background:",
        "out property <brush> right-panel-background:",
        "out property <brush> terminal-frame-background:",
        "out property <brush> separator:",
        "out property <brush> hairline:",
        "out property <brush> tabbar-surface:",
        "out property <brush> tab-active-surface:",
        "out property <brush> tab-inactive-surface:",
        "out property <brush> tab-hover-surface:",
        "out property <brush> tab-active-line:",
        "out property <brush> tab-active-indicator:",
        "out property <brush> sidebar-item-hover-background:",
        "out property <brush> sidebar-item-selected-background:",
        "out property <brush> sidebar-item-selected-border:",
        "out property <brush> sidebar-hover-surface:",
        "out property <brush> sidebar-selected-surface:",
        "out property <brush> panel-surface:",
        "out property <brush> input-surface:",
        "out property <brush> input-border:",
        "out property <brush> input-focus-ring:",
        "out property <brush> status-pill-surface:",
        "out property <brush> status-pill-border:",
        "out property <brush> text-secondary:",
        "out property <brush> text-muted:",
        "out property <brush> link-accent:",
        "out property <brush> focus-ring:",
        "out property <brush> terminal-soft-fg:",
        "out property <brush> terminal-dim-fg:",
    ] {
        assert!(
            tokens.contains(token),
            "theme tokens should define `{token}` so dark/light mode share one semantic visual system"
        );
    }
}

#[test]
fn premium_default_tokens_encode_the_new_calm_surface_ladder() {
    let tokens = fs::read_to_string("ui/theme/tokens.slint").expect("read theme tokens");

    assert!(
        tokens.contains("out property <brush> titlebar-background: dark-mode ? #181f27 : #f7f9fc;"),
        "titlebar should use a dedicated calm surface rather than sharing the app sheet"
    );
    assert!(
        tokens.contains(
            "out property <brush> terminal-frame-background: dark-mode ? #11151c : #e6e9ef;"
        ),
        "terminal frame should use the Ayu terminal neighborhood chrome instead of the older graphite/canvas ladder"
    );
    assert!(
        tokens.contains("out property <brush> terminal-default-fg: dark-mode ? #b3b1ad : #5c6166;"),
        "terminal defaults should read as Ayu off-white and cool gray instead of the older premium ladder"
    );
    assert!(
        tokens.contains(
            "out property <brush> sidebar-item-selected-background: dark-mode ? #293846 : #dce6f2;"
        ),
        "selected sidebar items should use a low-saturation filled state instead of a hard control button fill"
    );
}

#[test]
fn shell_chrome_consumes_semantic_tokens_for_tabs_sidebar_inputs_and_pills() {
    let titlebar = fs::read_to_string("ui/shell/titlebar.slint").expect("read titlebar");
    let tabbar = fs::read_to_string("ui/shell/tabbar.slint").expect("read tabbar");
    let sidebar = fs::read_to_string("ui/shell/sidebar.slint").expect("read sidebar");
    let assets_sidebar =
        fs::read_to_string("ui/shell/assets-sidebar.slint").expect("read assets sidebar");
    let right_panel = fs::read_to_string("ui/shell/right-panel.slint").expect("read right panel");
    let app_window = fs::read_to_string("ui/app-window.slint").expect("read app window");
    let workspace = fs::read_to_string("ui/shell/workspace-pane.slint").expect("read workspace");
    let active_tab = fs::read_to_string("ui/components/active-tab.slint").expect("read active tab");
    let sidebar_button =
        fs::read_to_string("ui/components/sidebar-nav-button.slint").expect("read sidebar button");
    let asset_row =
        fs::read_to_string("ui/components/asset-node-row.slint").expect("read asset row");
    let search = fs::read_to_string("ui/components/assets-search-popover.slint")
        .expect("read assets search popover");
    let status_pill =
        fs::read_to_string("ui/components/status-pill.slint").expect("read status pill");
    let terminal_host =
        fs::read_to_string("ui/shell/terminal-session-host.slint").expect("read terminal host");
    let theme_spec = fs::read_to_string("src/theme/spec.rs").expect("read theme spec");

    assert!(
        titlebar.contains("ThemeTokens.titlebar-background"),
        "titlebar should read from the dedicated titlebar background token"
    );
    assert!(
        tabbar.contains("ThemeTokens.tabbar-background"),
        "tab strip should use a dedicated tabbar background token instead of reusing the app sheet"
    );
    assert!(
        sidebar.contains("ThemeTokens.sidebar-background")
            && assets_sidebar.contains("ThemeTokens.sidebar-panel-background")
            && right_panel.contains("ThemeTokens.right-panel-background")
            && workspace.contains("workspace-session-frame-surface"),
        "shell chrome layers should explicitly consume the sidebar / panel ladder while workspace terminal chrome comes from the projected session frame surface"
    );
    assert!(
        active_tab.contains("ThemeTokens.tab-active-surface")
            && active_tab.contains("ThemeTokens.tab-hover-surface")
            && active_tab.contains("ThemeTokens.tab-active-line")
            && active_tab.contains("ThemeTokens.text-secondary"),
        "active tabs should use the calmer tab token family instead of reading like raised buttons"
    );
    assert!(
        sidebar_button.contains("ThemeTokens.sidebar-item-hover-background")
            && sidebar_button.contains("ThemeTokens.sidebar-item-selected-background")
            && sidebar_button.contains("ThemeTokens.sidebar-item-selected-border"),
        "activity buttons should use the new low-saturation selected fill and border tokens"
    );
    assert!(
        asset_row.contains("ThemeTokens.sidebar-item-hover-background")
            && asset_row.contains("ThemeTokens.sidebar-item-selected-background")
            && asset_row.contains("ThemeTokens.sidebar-item-selected-border"),
        "asset tree rows should share the same sidebar item state tokens"
    );
    assert!(
        search.contains("ThemeTokens.input-surface")
            && search.contains("ThemeTokens.input-border")
            && search.contains("ThemeTokens.input-focus-ring")
            && search.contains("ThemeTokens.text-muted"),
        "search input should use semantic input tokens so bright and dark modes stay visually coherent"
    );
    assert!(
        status_pill.contains("ThemeTokens.status-pill-surface")
            && status_pill.contains("ThemeTokens.status-pill-border")
            && status_pill.contains("ThemeTokens.text-secondary"),
        "status pill should use its own semantic pill tokens instead of a generic panel background"
    );
    assert!(
        terminal_host.contains("session-frame-surface")
            && terminal_host.contains("session-frame-border")
            && terminal_host.contains("session-selection-surface")
            && terminal_host.contains("session-scrollbar-track"),
        "terminal host chrome should use projected session-scoped frame, selection, and scrollbar-track colors rather than a detached shell token ladder"
    );
    assert!(
        sidebar.contains("Click to collapse, drag to resize")
            && sidebar.contains("Click to expand")
            && right_panel.contains("Click to collapse, drag to resize")
            && app_window.contains("text: \"Click to expand\""),
        "edge handles and revive affordances should keep explicit, discoverable guidance"
    );
    assert!(
        theme_spec.contains("pub const TERMINAL_ROW_BANDING_ENABLED: bool = false;")
            && theme_spec.contains("pub const TERMINAL_ROW_BANDING_ALPHA: f32 = 0.0;")
            && theme_spec.contains("pub const TERMINAL_BG_GRAIN_ALPHA: f32 = 0.0;")
            && theme_spec.contains("pub const TERMINAL_BG_BASE_DARK: u32 = 0x0a_0e14;")
            && theme_spec.contains("pub const TERMINAL_BG_GRADIENT_TOP_DARK: u32 = 0x0a_0e14;")
            && theme_spec.contains("pub const TERMINAL_BG_GRADIENT_BOTTOM_DARK: u32 = 0x0a_0e14;")
            && theme_spec.contains("pub const TERMINAL_BG_BASE_LIGHT: u32 = 0xfa_fafa;")
            && theme_spec.contains("pub const TERMINAL_BG_GRADIENT_TOP_LIGHT: u32 = 0xfa_fafa;")
            && theme_spec.contains("pub const TERMINAL_BG_GRADIENT_BOTTOM_LIGHT: u32 = 0xfa_fafa;"),
        "terminal palette spec should expose the shared Ayu viewport background constants, disable legacy row banding/grain, and keep the default viewport backgrounds flat across renderers"
    );
    assert!(
        !fs::read_to_string("ui/theme/tokens.slint")
            .expect("read theme tokens")
            .contains("terminal-row-stripe-surface"),
        "unused shell stripe tokens should be removed once terminal viewport banding is no longer part of the design system"
    );
}

#[test]
fn welcome_shell_copy_uses_token_colors_instead_of_opacity_fades() {
    let welcome = fs::read_to_string("ui/welcome/welcome-view.slint").expect("read welcome view");
    let quick_launch =
        fs::read_to_string("ui/welcome/quick-launch-section.slint").expect("read quick launch");

    assert!(
        welcome.contains("color: ThemeTokens.text-secondary;"),
        "welcome hero supporting copy should use a semantic secondary text token instead of fading primary text through opacity on a dark panel"
    );
    assert!(
        !welcome.contains("opacity: 0.72;"),
        "welcome hero supporting copy should stop using opacity fades because that pushes text through transparent compositing and makes shell text look gray on Windows"
    );
    assert!(
        quick_launch.contains("color: ThemeTokens.text-secondary;")
            && quick_launch.contains("color: ThemeTokens.text-muted;"),
        "recent-connections helper copy should use semantic secondary/muted tokens instead of washed-out primary text"
    );
    assert!(
        !quick_launch.contains("opacity: 0.58;") && !quick_launch.contains("opacity: 0.54;"),
        "quick-launch helper copy should not use low-opacity text on dark shell panels because that magnifies the muddy Windows UI text look"
    );
}

use std::fs;

#[test]
fn semantic_theme_tokens_cover_shell_hierarchy_and_states() {
    let tokens = fs::read_to_string("ui/theme/tokens.slint").expect("read theme tokens");

    for token in [
        "out property <brush> tabbar-surface:",
        "out property <brush> tab-active-surface:",
        "out property <brush> tab-inactive-surface:",
        "out property <brush> tab-active-indicator:",
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
    ] {
        assert!(
            tokens.contains(token),
            "theme tokens should define `{token}` so dark/light mode share one semantic visual system"
        );
    }
}

#[test]
fn light_mode_text_tokens_raise_shell_contrast_for_small_misans_copy() {
    let tokens = fs::read_to_string("ui/theme/tokens.slint").expect("read theme tokens");

    assert!(
        tokens.contains("out property <brush> text-secondary: dark-mode ? #b9c3d0 : #3f4d5d;"),
        "light-mode secondary text should move to a darker shell contrast so 14px shell body copy stops reading as gray haze on Windows"
    );
    assert!(
        tokens.contains("out property <brush> text-muted: dark-mode ? #8794a6 : #5f7084;"),
        "light-mode muted text should keep enough density for small shell captions instead of collapsing into low-contrast gray"
    );
}

#[test]
fn shell_chrome_consumes_semantic_tokens_for_tabs_sidebar_inputs_and_pills() {
    let tabbar = fs::read_to_string("ui/shell/tabbar.slint").expect("read tabbar");
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
        tabbar.contains("ThemeTokens.tabbar-surface"),
        "tab strip should use a dedicated tabbar surface instead of reusing the titlebar surface"
    );
    assert!(
        active_tab.contains("ThemeTokens.tab-active-surface")
            && active_tab.contains("ThemeTokens.tab-inactive-surface")
            && active_tab.contains("ThemeTokens.tab-active-indicator")
            && active_tab.contains("ThemeTokens.text-secondary"),
        "active tabs should use semantic tab tokens for active/inactive hierarchy and secondary copy"
    );
    assert!(
        sidebar_button.contains("ThemeTokens.sidebar-hover-surface")
            && sidebar_button.contains("ThemeTokens.sidebar-selected-surface"),
        "activity buttons should use semantic sidebar hover/selected tokens"
    );
    assert!(
        asset_row.contains("ThemeTokens.sidebar-hover-surface")
            && asset_row.contains("ThemeTokens.sidebar-selected-surface"),
        "asset tree rows should share the same sidebar hover/selected token family"
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
        terminal_host.contains("ThemeTokens.panel-surface"),
        "terminal host chrome should use the shared panel surface token rather than a generic control fill"
    );
    assert!(
        theme_spec.contains("TERMINAL_ROW_BANDING_ENABLED")
            && theme_spec.contains("TERMINAL_ROW_BANDING_ALPHA")
            && theme_spec.contains("TERMINAL_BG_GRAIN_ALPHA")
            && theme_spec.contains("TERMINAL_BG_BASE_DARK")
            && theme_spec.contains("TERMINAL_BG_GRADIENT_TOP_DARK")
            && theme_spec.contains("TERMINAL_BG_GRADIENT_BOTTOM_DARK"),
        "terminal palette spec should expose explicit viewport background tuning constants now that renderer-side chrome no longer consumes alternating row stripe colors"
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

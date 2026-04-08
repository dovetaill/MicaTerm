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
        "out property <brush> terminal-row-stripe-surface:",
    ] {
        assert!(
            tokens.contains(token),
            "theme tokens should define `{token}` so dark/light mode share one semantic visual system"
        );
    }
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
        theme_spec.contains("row_bg_even") && theme_spec.contains("row_bg_odd"),
        "terminal palette spec should continue to carry the subtle terminal row stripe contract for renderer-side background banding"
    );
}

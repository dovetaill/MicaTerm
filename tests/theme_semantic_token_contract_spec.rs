use std::fs;

#[test]
fn theme_semantic_token_contract_spec_semantic_theme_tokens_cover_shell_hierarchy_and_states() {
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
        "out property <brush> sidebar-item-focus-border:",
        "out property <brush> panel-scrollbar-track:",
        "out property <brush> panel-scrollbar-thumb:",
        "out property <brush> panel-scrollbar-thumb-active:",
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
fn theme_semantic_token_contract_spec_premium_default_tokens_encode_the_new_calm_surface_ladder() {
    let tokens = fs::read_to_string("ui/theme/tokens.slint").expect("read theme tokens");
    let theme_spec = fs::read_to_string("src/theme/spec.rs").expect("read theme spec");

    assert!(
        tokens.contains("out property <brush> titlebar-background: dark-mode ? #10151d : #f8f9fa;"),
        "titlebar should stay aligned with the refined unified Ayu sheet instead of the older cooler light slab"
    );
    assert!(
        tokens.contains(
            "out property <brush> terminal-frame-background: dark-mode ? #141b24 : #f4f6f8;"
        ),
        "terminal frame should use the refined raised Ayu chrome instead of the older brighter fallback slab"
    );
    assert!(
        tokens.contains("out property <brush> terminal-default-fg: dark-mode ? #c5c1b8 : #5c6166;"),
        "terminal defaults should read as Ayu off-white and cool gray instead of the older premium ladder"
    );
    assert!(
        tokens.contains(
            "out property <brush> sidebar-item-selected-background: dark-mode ? #141b24 : #fff7ea;"
        ),
        "selected sidebar items should use the refined subtle Ayu fill instead of the older boxed control treatment"
    );
    assert!(
        !theme_spec.contains("Catppuccin")
            && !theme_spec.contains("Graphite")
            && !theme_spec.contains("Canvas"),
        "default-theme wording should stop using retired palette names"
    );
}

#[test]
fn theme_semantic_token_contract_spec_boot_time_shell_tokens_match_approved_ayu_defaults() {
    let tokens = fs::read_to_string("ui/theme/tokens.slint").expect("read theme tokens");

    for expected in [
        "out property <brush> titlebar-background: dark-mode ? #10151d : #f8f9fa;",
        "out property <brush> tabbar-background: dark-mode ? #10151d : #f8f9fa;",
        "out property <brush> sidebar-background: dark-mode ? #10151d : #f8f9fa;",
        "out property <brush> sidebar-panel-background: dark-mode ? #111821 : #f4f6f8;",
        "out property <brush> right-panel-background: dark-mode ? #111821 : #f4f6f8;",
        "out property <brush> terminal-frame-background: dark-mode ? #141b24 : #f4f6f8;",
        "out property <brush> separator: dark-mode ? #18212b : #e5e9ef;",
        "out property <brush> hairline: dark-mode ? #1b2530 : #e1e6ec;",
        "out property <brush> text-primary: dark-mode ? #c5c1b8 : #5c6166;",
        "out property <brush> text-secondary: dark-mode ? #9aa4ae : #7a838c;",
        "out property <brush> text-muted: dark-mode ? #7d8790 : #8a939c;",
        "out property <brush> accent: dark-mode ? #e6b450 : #ffaa33;",
        "out property <brush> link-accent: dark-mode ? #e6b450 : #ffaa33;",
        "out property <brush> focus-ring: dark-mode ? #e6b450 : #ffaa33;",
        "out property <brush> tab-active-surface: dark-mode ? #141b24 : #fcfcfc;",
        "out property <brush> tab-hover-surface: dark-mode ? #111821 : #f6f8fa;",
        "out property <brush> tab-active-line: dark-mode ? #e6b450 : #ffaa33;",
        "out property <brush> sidebar-item-hover-background: dark-mode ? #111821 : #eef2f5;",
        "out property <brush> sidebar-item-selected-background: dark-mode ? #141b24 : #fff7ea;",
        "out property <brush> sidebar-item-selected-border: dark-mode ? #e6b450 : #ffaa33;",
        "out property <brush> sidebar-item-focus-border: dark-mode ? #1b2530 : #e5e9ef;",
        "out property <brush> panel-scrollbar-track: dark-mode ? #111821 : #f4f6f8;",
        "out property <brush> panel-scrollbar-thumb: dark-mode ? #2f3944 : #d6dce3;",
        "out property <brush> panel-scrollbar-thumb-active: dark-mode ? #3c4856 : #c6cdd6;",
    ] {
        assert!(
            tokens.contains(expected),
            "boot-time shell token `{expected}` should match the approved Ayu shell neighborhood defaults before runtime projection takes over"
        );
    }
}

#[test]
fn theme_semantic_token_contract_spec_theme_tokens_remain_a_boot_time_parity_snapshot_only() {
    let tokens = fs::read_to_string("ui/theme/tokens.slint").expect("read theme tokens");

    assert!(
        tokens.contains(
            "// Boot-time parity snapshot only: Rust publishes the live runtime shell and terminal palette."
        ),
        "theme token file should explicitly document that it is only a boot-time parity snapshot, not a second live Ayu runtime system"
    );
    assert!(
        !tokens.contains("shell-app-background")
            && !tokens.contains("shell-titlebar-background")
            && !tokens.contains("workspace-session-frame-surface"),
        "theme token file should stay a static boot snapshot instead of growing a parallel runtime property system"
    );
}

#[test]
fn theme_semantic_token_contract_spec_shell_chrome_consumes_semantic_tokens_for_tabs_sidebar_inputs_and_pills(
) {
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
            && active_tab.contains("ThemeTokens.tab-active-indicator")
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
            && theme_spec.contains("pub const TERMINAL_BG_BASE_LIGHT: u32 = 0xf7_f8fa;")
            && theme_spec.contains("pub const TERMINAL_BG_GRADIENT_TOP_LIGHT: u32 = 0xf7_f8fa;")
            && theme_spec.contains("pub const TERMINAL_BG_GRADIENT_BOTTOM_LIGHT: u32 = 0xf7_f8fa;"),
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
fn theme_semantic_token_contract_spec_runtime_shell_palette_properties_are_threaded_through_the_window_tree(
) {
    let app_window = fs::read_to_string("ui/app-window.slint").expect("read app window");
    let titlebar = fs::read_to_string("ui/shell/titlebar.slint").expect("read titlebar");
    let tabbar = fs::read_to_string("ui/shell/tabbar.slint").expect("read tabbar");
    let sidebar = fs::read_to_string("ui/shell/sidebar.slint").expect("read sidebar");
    let assets_sidebar =
        fs::read_to_string("ui/shell/assets-sidebar.slint").expect("read assets sidebar");
    let right_panel = fs::read_to_string("ui/shell/right-panel.slint").expect("read right panel");
    let workspace = fs::read_to_string("ui/shell/workspace-pane.slint").expect("read workspace");

    assert!(
        titlebar.contains(
            "in property <color> shell-titlebar-background: ThemeTokens.titlebar-background;"
        ) && titlebar.contains("in property <color> shell-text-primary: ThemeTokens.text-primary;")
            && titlebar.contains("in property <color> shell-accent: ThemeTokens.accent;"),
        "titlebar should declare runtime shell palette inputs before its active surfaces switch away from ThemeTokens"
    );
    assert!(
        app_window.contains("shell-titlebar-background: root.shell-titlebar-background;")
            && app_window.contains("shell-text-primary: root.shell-text-primary;")
            && app_window.contains("shell-accent: root.shell-accent;"),
        "app window should pass the projected titlebar shell palette into Titlebar"
    );
    assert!(
        tabbar.contains(
            "in property <color> shell-tabbar-background: ThemeTokens.tabbar-background;"
        ) && tabbar
            .contains("in property <color> shell-tab-active: ThemeTokens.tab-active-surface;")
            && tabbar.contains(
                "in property <color> shell-tab-active-indicator: ThemeTokens.tab-active-indicator;"
            ),
        "tabbar should expose runtime shell palette inputs for the projected tab ladder"
    );
    assert!(
        workspace.contains(
            "in property <color> shell-tabbar-background: ThemeTokens.tabbar-background;"
        ) && workspace
            .contains("in property <color> shell-tab-active: ThemeTokens.tab-active-surface;")
            && workspace.contains("shell-tabbar-background: root.shell-tabbar-background;")
            && workspace.contains("shell-tab-active: root.shell-tab-active;")
            && workspace.contains("shell-tab-active-indicator: root.shell-tab-active-indicator;"),
        "workspace pane should accept the runtime tab palette and forward it into TabBar without rewriting the names"
    );
    assert!(
        app_window.contains("shell-tabbar-background: root.shell-tabbar-background;")
            && app_window.contains("shell-tab-active: root.shell-tab-active;")
            && app_window.contains("shell-tab-active-indicator: root.shell-tab-active-indicator;"),
        "app window should pass the projected tab palette into WorkspacePane"
    );
    assert!(
        sidebar.contains("in property <color> shell-sidebar-background: ThemeTokens.sidebar-background;")
            && sidebar.contains("in property <color> shell-sidebar-panel-background: ThemeTokens.sidebar-panel-background;")
            && sidebar.contains("in property <color> shell-sidebar-item-selected-border: ThemeTokens.sidebar-item-selected-border;")
            && sidebar.contains("in property <color> shell-sidebar-item-focus-border: ThemeTokens.sidebar-item-focus-border;")
            && sidebar.contains("shell-sidebar-panel-background: root.shell-sidebar-panel-background;")
            && sidebar.contains("shell-sidebar-item-selected-border: root.shell-sidebar-item-selected-border;"),
        "sidebar should accept the runtime sidebar palette and forward the panel and selected-state colors into AssetsSidebar"
    );
    assert!(
        assets_sidebar.contains("in property <color> shell-sidebar-panel-background: ThemeTokens.sidebar-panel-background;")
            && assets_sidebar.contains("in property <color> shell-text-primary: ThemeTokens.text-primary;")
            && assets_sidebar.contains("in property <color> shell-sidebar-item-selected-border: ThemeTokens.sidebar-item-selected-border;")
            && assets_sidebar.contains("in property <color> shell-sidebar-item-focus-border: ThemeTokens.sidebar-item-focus-border;"),
        "assets sidebar should declare runtime shell palette inputs for its raised panel and item states"
    );
    assert!(
        app_window.contains("shell-sidebar-background: root.shell-sidebar-background;")
            && app_window
                .contains("shell-sidebar-panel-background: root.shell-sidebar-panel-background;")
            && app_window.contains(
                "shell-sidebar-item-selected-border: root.shell-sidebar-item-selected-border;"
            )
            && app_window
                .contains("shell-sidebar-item-focus-border: root.shell-sidebar-item-focus-border;"),
        "app window should pass the projected sidebar palette into Sidebar"
    );
    assert!(
        right_panel.contains(
            "in property <color> shell-right-panel-background: ThemeTokens.right-panel-background;"
        ) && right_panel
            .contains("in property <color> shell-text-primary: ThemeTokens.text-primary;")
            && right_panel.contains("in property <color> shell-accent: ThemeTokens.accent;")
            && right_panel.contains(
                "in property <color> shell-panel-scrollbar-track: ThemeTokens.panel-scrollbar-track;"
            )
            && right_panel.contains(
                "in property <color> shell-panel-scrollbar-thumb: ThemeTokens.panel-scrollbar-thumb;"
            )
            && right_panel.contains(
                "in property <color> shell-panel-scrollbar-thumb-active: ThemeTokens.panel-scrollbar-thumb-active;"
            ),
        "right panel should expose runtime shell palette inputs before switching its live surfaces"
    );
    assert!(
        app_window.contains("shell-right-panel-background: root.shell-right-panel-background;")
            && app_window.contains("shell-text-primary: root.shell-text-primary;")
            && app_window.contains("shell-accent: root.shell-accent;"),
        "app window should pass the projected shell palette into RightPanel"
    );
}

#[test]
fn theme_semantic_token_contract_spec_runtime_shell_palette_consumers_switch_from_tokens_to_live_props(
) {
    let titlebar = fs::read_to_string("ui/shell/titlebar.slint").expect("read titlebar");
    let tabbar = fs::read_to_string("ui/shell/tabbar.slint").expect("read tabbar");
    let active_tab = fs::read_to_string("ui/components/active-tab.slint").expect("read active tab");
    let sidebar = fs::read_to_string("ui/shell/sidebar.slint").expect("read sidebar");
    let assets_sidebar =
        fs::read_to_string("ui/shell/assets-sidebar.slint").expect("read assets sidebar");
    let sidebar_button =
        fs::read_to_string("ui/components/sidebar-nav-button.slint").expect("read sidebar button");
    let asset_row =
        fs::read_to_string("ui/components/asset-node-row.slint").expect("read asset row");
    let right_panel = fs::read_to_string("ui/shell/right-panel.slint").expect("read right panel");
    let workspace = fs::read_to_string("ui/shell/workspace-pane.slint").expect("read workspace");

    assert!(
        titlebar.contains("background: root.shell-titlebar-background;")
            && titlebar.contains("border-color: root.shell-border;")
            && titlebar.contains("colorize: root.shell-text-primary;")
            && titlebar.contains("color: root.shell-text-primary;")
            && titlebar.contains("color: root.shell-text-secondary;")
            && !titlebar.contains("\n    background: ThemeTokens.titlebar-background;\n"),
        "titlebar should consume the live runtime shell palette for its active background and text hierarchy instead of detached titlebar tokens"
    );
    assert!(
        tabbar.contains("background: root.shell-tabbar-background;")
            && tabbar.contains("background: root.shell-titlebar-background;")
            && tabbar.contains("border-color: root.shell-border;")
            && tabbar.contains("? root.shell-tab-inactive")
            && tabbar.contains("colorize: new-tab-touch.has-hover ? root.shell-text-primary : root.shell-text-secondary;")
            && tabbar.contains("shell-tab-active: root.shell-tab-active;")
            && !tabbar.contains("\n    background: ThemeTokens.tabbar-background;\n"),
        "tabbar should switch its active shell surfaces to runtime shell props and forward the tab ladder into ActiveTab"
    );
    assert!(
        active_tab.contains("in property <color> shell-tab-active: ThemeTokens.tab-active-surface;")
            && active_tab.contains("in property <color> shell-tab-hover: ThemeTokens.tab-hover-surface;")
            && active_tab.contains("in property <color> shell-text-primary: ThemeTokens.text-primary;")
            && active_tab.contains("background: root.drag-active")
            && active_tab.contains("? root.shell-tab-active")
            && active_tab.contains("? root.shell-tab-hover")
            && active_tab.contains("background: root.shell-tab-active-indicator;")
            && active_tab.contains("color: root.active || root.drag-active ? root.shell-text-primary : root.shell-text-secondary;"),
        "active tabs should render tab surfaces, active indicator, and text hierarchy from runtime shell props"
    );
    assert!(
        sidebar.contains("background: root.shell-sidebar-background;")
            && sidebar.contains("background: root.shell-separator;")
            && sidebar.contains("shell-separator: root.shell-separator;")
            && sidebar.contains("shell-sidebar-item-selected: root.shell-sidebar-item-selected;")
            && sidebar.contains(
                "shell-sidebar-item-selected-border: root.shell-sidebar-item-selected-border;"
            ),
        "sidebar should switch its shell surfaces and forward item-state runtime props into activity buttons and the assets panel"
    );
    assert!(
        sidebar_button.contains("in property <color> shell-sidebar-item-hover: ThemeTokens.sidebar-item-hover-background;")
            && sidebar_button.contains("in property <color> shell-sidebar-item-selected: ThemeTokens.sidebar-item-selected-background;")
            && sidebar_button.contains("in property <color> shell-sidebar-item-selected-border: ThemeTokens.sidebar-item-selected-border;")
            && sidebar_button.contains("? root.shell-sidebar-item-selected")
            && sidebar_button.contains("? root.shell-sidebar-item-hover")
            && sidebar_button.contains("active-rail := Rectangle")
            && sidebar_button.contains("background: root.shell-sidebar-item-selected-border;")
            && !sidebar_button.contains("border-width: root.active || touch.has-hover ? 1px : 0px;")
            && sidebar_button.contains("colorize: root.active || touch.has-hover ? root.shell-text-primary : root.shell-text-secondary;"),
        "sidebar activity buttons should use runtime sidebar selected and hover state colors, with a subtle fill plus leading accent rail instead of a full active outline box"
    );
    assert!(
        assets_sidebar.contains("background: root.shell-sidebar-panel-background;")
            && assets_sidebar.contains("background: root.shell-separator;")
            && assets_sidebar.contains("color: root.shell-text-primary;")
            && assets_sidebar.contains("color: root.shell-text-secondary;")
            && assets_sidebar.contains(
                "shell-sidebar-item-selected-border: root.shell-sidebar-item-selected-border;"
            )
            && assets_sidebar
                .contains("shell-sidebar-item-focus-border: root.shell-sidebar-item-focus-border;")
            && !assets_sidebar
                .contains("\n    background: ThemeTokens.sidebar-panel-background;\n"),
        "assets sidebar should consume the live runtime shell palette for its raised panel and text hierarchy"
    );
    assert!(
        asset_row.contains("in property <color> shell-focus-ring: ThemeTokens.focus-ring;")
            && asset_row.contains("in property <color> shell-sidebar-item-selected: ThemeTokens.sidebar-item-selected-background;")
            && asset_row.contains("in property <color> shell-sidebar-item-selected-border: ThemeTokens.sidebar-item-selected-border;")
            && asset_row.contains(
                "in property <color> shell-sidebar-item-focus-border: ThemeTokens.sidebar-item-focus-border;"
            )
            && asset_row.contains("border-width: root.focused && !root.selected ? 1px : 0px;")
            && asset_row.contains("border-color: root.shell-sidebar-item-focus-border;")
            && asset_row.contains("? root.shell-sidebar-item-selected")
            && asset_row.contains("? root.shell-sidebar-item-hover")
            && asset_row.contains("active-rail := Rectangle")
            && asset_row.contains("background: root.shell-sidebar-item-selected-border;")
            && !asset_row.contains("border-color: root.shell-focus-ring;")
            && asset_row.contains("color: root.shell-text-primary;")
            && asset_row.contains("color: root.shell-text-secondary;"),
        "asset rows should render selected, hover, focus, and text colors from runtime sidebar props while keeping keyboard focus lower-contrast than the selected accent rail"
    );
    assert!(
        right_panel.contains("background: root.shell-right-panel-background;")
            && right_panel.contains("background: root.shell-separator;")
            && right_panel.contains("border-color: root.shell-border;")
            && right_panel.contains(
                "in property <color> shell-sidebar-item-hover: ThemeTokens.sidebar-item-hover-background;"
            )
            && right_panel.contains(
                "in property <color> shell-sidebar-item-selected: ThemeTokens.sidebar-item-selected-background;"
            )
            && right_panel.contains(
                "in property <color> shell-sidebar-item-selected-border: ThemeTokens.sidebar-item-selected-border;"
            )
            && right_panel.contains("active-rail := Rectangle")
            && right_panel.contains("background: item.selected ? root.shell-sidebar-item-selected")
            && right_panel.contains("background: root.shell-sidebar-item-selected-border;")
            && right_panel.contains("background: root.shell-separator;")
            && right_panel.contains("vertical-scrollbar-policy: always-off;")
            && right_panel.contains("background: root.shell-panel-scrollbar-track;")
            && right_panel.contains("? root.shell-panel-scrollbar-thumb-active")
            && right_panel.contains(": root.shell-panel-scrollbar-thumb;")
            && right_panel.contains("color: root.shell-text-primary;")
            && right_panel.contains("color: root.shell-text-secondary;")
            && !right_panel.contains("ThemeTokens.explorer-row-selected-surface")
            && !right_panel.contains("ThemeTokens.explorer-row-hover-surface")
            && !right_panel.contains("background: ThemeTokens.divider-subtle;")
            && !right_panel.contains("border-color: ThemeTokens.divider-subtle;")
            && !right_panel.contains("\n    background: ThemeTokens.right-panel-background;\n"),
        "right panel should switch its active shell surfaces and row selection states to the runtime shell palette, using the same subtle fill plus accent rail direction as the sidebar tree"
    );
    assert!(
        workspace.contains("background: root.workspace-session-frame-surface;"),
        "workspace shell frame should continue to use the projected session frame surface around the terminal host"
    );
}

#[test]
fn theme_semantic_token_contract_spec_welcome_shell_copy_uses_token_colors_instead_of_opacity_fades(
) {
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

#[test]
fn theme_semantic_token_contract_spec_workspace_sftp_host_stays_on_runtime_shell_and_session_properties(
) {
    let app_window = fs::read_to_string("ui/app-window.slint").expect("read app window");
    let workspace =
        fs::read_to_string("ui/shell/workspace-pane.slint").expect("read workspace pane");
    let host =
        fs::read_to_string("ui/shell/sftp-workspace-host.slint").expect("read sftp workspace host");

    assert!(
        workspace.contains(
            "in property <color> shell-sidebar-item-selected: ThemeTokens.sidebar-item-selected-background;"
        ) && workspace.contains(
            "in property <color> shell-sidebar-item-selected-border: ThemeTokens.sidebar-item-selected-border;"
        ) && workspace.contains("shell-sidebar-item-selected: root.shell-sidebar-item-selected;")
            && workspace.contains(
                "shell-sidebar-item-selected-border: root.shell-sidebar-item-selected-border;"
            ),
        "workspace pane should thread runtime-projected selected-fill and accent-rail colors into the SFTP workspace host instead of leaving it on detached token reads"
    );
    assert!(
        host.contains(
            "in property <color> workspace-session-frame-surface: ThemeTokens.terminal-frame-background;"
        ) && host.contains(
            "in property <color> shell-sidebar-item-selected: ThemeTokens.sidebar-item-selected-background;"
        ) && host.contains(
            "in property <color> shell-sidebar-item-selected-border: ThemeTokens.sidebar-item-selected-border;"
        ) && host.contains("background: root.workspace-session-frame-surface;")
            && host.contains("background: item.selected ? root.shell-sidebar-item-selected")
            && host.contains("background: root.shell-sidebar-item-selected-border;"),
        "SFTP workspace host should consume runtime shell/session properties for its surface and selected-row treatment instead of inventing a detached palette ladder"
    );
    assert!(
        app_window.contains("workspace-sftp-tooltip-overlay := TitlebarTooltip {")
            && !host.contains("import { TitlebarTooltip }"),
        "workspace SFTP tooltips should reuse the shared AppWindow overlay so hardening does not introduce a second local tooltip surface with its own detached palette contract"
    );
    for banned in [
        "#0a0e14", "#10151d", "#111821", "#141b24", "#e6b450", "#ffaa33",
    ] {
        assert!(
            !host.contains(banned),
            "SFTP workspace host should not hardcode Ayu palette literal `{banned}` because runtime shell/session projection remains the single active source of truth"
        );
    }
}

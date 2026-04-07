use mica_term::app::terminal_theme::{preset_for_theme_mode, selection_overlay_rgba};
use mica_term::theme::ThemeMode;
use std::fs;

#[test]
fn dark_theme_maps_terminal_palette_to_catppuccin_mocha() {
    let preset = preset_for_theme_mode(ThemeMode::Dark);

    assert_eq!(preset.name, "Catppuccin Mocha");
    assert_eq!(preset.background, 0x1e_1e2e);
    assert_eq!(preset.foreground, 0xcd_d6f4);
    assert_eq!(preset.cursor_bg, 0xcd_d6f4);
    assert_eq!(preset.cursor_fg, 0x1e_1e2e);
    assert_eq!(preset.scrollbar_thumb, (0x58, 0x5b, 0x70));
    assert_eq!(preset.scrollbar_thumb_active, (0x6c, 0x70, 0x86));
    assert_eq!(preset.jump_to_latest_bg, 0x31_3244);
    assert_eq!(preset.jump_to_latest_hover_bg, 0x45_475a);
    assert_eq!(preset.jump_to_latest_pressed_bg, 0x58_5b70);
    assert_eq!(preset.jump_to_latest_border, 0x6c_7086);
    assert_eq!(preset.jump_to_latest_fg, 0xcd_d6f4);
    assert_eq!(preset.split, (0x31, 0x32, 0x44));
    assert_eq!(preset.ansi[4], (0x89, 0xb4, 0xfa));
}

#[test]
fn light_theme_maps_terminal_palette_to_catppuccin_latte() {
    let preset = preset_for_theme_mode(ThemeMode::Light);

    assert_eq!(preset.name, "Catppuccin Latte");
    assert_eq!(preset.background, 0xef_f1f5);
    assert_eq!(preset.foreground, 0x4c_4f69);
    assert_eq!(preset.cursor_bg, 0x4c_4f69);
    assert_eq!(preset.cursor_fg, 0xef_f1f5);
    assert_eq!(preset.scrollbar_thumb, (0xac, 0xb0, 0xbe));
    assert_eq!(preset.scrollbar_thumb_active, (0x9c, 0xa0, 0xb0));
    assert_eq!(preset.jump_to_latest_bg, 0xcc_d0da);
    assert_eq!(preset.jump_to_latest_hover_bg, 0xbc_c0cc);
    assert_eq!(preset.jump_to_latest_pressed_bg, 0xac_b0be);
    assert_eq!(preset.jump_to_latest_border, 0x9c_a0b0);
    assert_eq!(preset.jump_to_latest_fg, 0x4c_4f69);
    assert_eq!(preset.split, (0xcc, 0xd0, 0xda));
    assert_eq!(preset.ansi[4], (0x1e, 0x66, 0xf5));
}

#[test]
fn selection_overlay_colors_stay_translucent_and_theme_specific() {
    let dark_preset = preset_for_theme_mode(ThemeMode::Dark);
    let light_preset = preset_for_theme_mode(ThemeMode::Light);
    let dark_overlay = selection_overlay_rgba(ThemeMode::Dark);
    let light_overlay = selection_overlay_rgba(ThemeMode::Light);

    assert!(
        dark_preset.selection_bg.3 > 0.0 && dark_preset.selection_bg.3 < 1.0,
        "dark terminal selection should stay translucent so selected glyphs remain readable"
    );
    assert!(
        light_preset.selection_bg.3 > 0.0 && light_preset.selection_bg.3 < 1.0,
        "light terminal selection should stay translucent so selected glyphs remain readable"
    );
    assert_ne!(
        dark_overlay & 0x00ff_ffff,
        light_overlay & 0x00ff_ffff,
        "dark and light selection overlays should keep distinct RGB colors"
    );
    assert!(
        ((dark_overlay >> 24) & 0xff) < 0xff && ((light_overlay >> 24) & 0xff) < 0xff,
        "selection overlays should avoid a fully opaque alpha channel"
    );
}

#[test]
fn slint_terminal_tokens_match_catppuccin_no_frame_defaults() {
    let tokens = fs::read_to_string("ui/theme/tokens.slint").expect("read theme tokens");

    assert!(
        tokens.contains("terminal-default-fg: dark-mode ? #cdd6f4 : #4c4f69;"),
        "Slint no-frame terminal foreground tokens should match the Catppuccin Mocha/Latte defaults used by the Rust fallback preset projection"
    );
    assert!(
        tokens.contains("terminal-default-bg: dark-mode ? #1e1e2e : #eff1f5;"),
        "Slint no-frame terminal background tokens should match the Catppuccin Mocha/Latte defaults used by the Rust fallback preset projection"
    );
    assert!(
        tokens.contains("terminal-cursor-fg: dark-mode ? #1e1e2e : #eff1f5;")
            && tokens.contains("terminal-cursor-bg: dark-mode ? #cdd6f4 : #4c4f69;"),
        "Slint cursor tokens should stay aligned with the Catppuccin fallback preset so no-frame terminal states do not drift from the live terminal palette"
    );
    assert!(
        tokens.contains("terminal-jump-to-latest-surface: dark-mode ? #313244 : #ccd0da;")
            && tokens
                .contains("terminal-jump-to-latest-hover-surface: dark-mode ? #45475a : #bcc0cc;")
            && tokens.contains(
                "terminal-jump-to-latest-pressed-surface: dark-mode ? #585b70 : #acb0be;"
            )
            && tokens.contains("terminal-jump-to-latest-border: dark-mode ? #6c7086 : #9ca0b0;")
            && tokens.contains("terminal-jump-to-latest-fg: dark-mode ? #cdd6f4 : #4c4f69;"),
        "Slint terminal shell chrome fallback tokens should keep the jump-to-latest affordance aligned with the Catppuccin preset when no active terminal surface is driving the workspace projection"
    );
}

#[test]
fn terminal_session_host_reads_terminal_shell_chrome_from_session_properties() {
    let host_source =
        fs::read_to_string("ui/shell/terminal-session-host.slint").expect("read terminal host");

    assert!(
        host_source.contains("in property <color> session-scrollbar-thumb")
            && host_source.contains("in property <color> session-scrollbar-thumb-active"),
        "terminal session host should accept scrollbar thumb colors from the projected terminal session contract instead of hard-coding ThemeTokens values"
    );
    assert!(
        host_source.contains("in property <color> session-jump-to-latest-bg")
            && host_source.contains("in property <color> session-jump-to-latest-hover-bg")
            && host_source.contains("in property <color> session-jump-to-latest-pressed-bg")
            && host_source.contains("in property <color> session-jump-to-latest-border")
            && host_source.contains("in property <color> session-jump-to-latest-fg"),
        "terminal session host should accept paused-follow pill colors from the projected terminal session contract so the shell chrome stays on the same Catppuccin palette source as the terminal surface"
    );
    assert!(
        host_source.contains("? root.session-scrollbar-thumb-active")
            && host_source.contains(": root.session-scrollbar-thumb;")
            && host_source.contains("border-color: root.session-jump-to-latest-border;")
            && host_source.contains("? root.session-jump-to-latest-pressed-bg")
            && host_source.contains("? root.session-jump-to-latest-hover-bg")
            && host_source.contains(": root.session-jump-to-latest-bg;")
            && host_source.contains("color: root.session-jump-to-latest-fg;"),
        "terminal session host should render the scrollbar thumb states and jump-to-latest pill directly from session-scoped shell chrome properties"
    );
}

#[test]
fn terminal_adjacent_shell_chrome_contracts_match_catppuccin_preset_values() {
    let theme_spec = fs::read_to_string("src/theme/spec.rs").expect("read terminal theme spec");
    let terminal_theme =
        fs::read_to_string("src/app/terminal_theme.rs").expect("read terminal theme preset code");
    let tokens = fs::read_to_string("ui/theme/tokens.slint").expect("read theme tokens");
    let terminal_host =
        fs::read_to_string("ui/shell/terminal-session-host.slint").expect("read terminal host");

    assert!(
        theme_spec.contains("jump_to_latest_bg")
            && theme_spec.contains("jump_to_latest_hover_bg")
            && theme_spec.contains("jump_to_latest_pressed_bg")
            && theme_spec.contains("jump_to_latest_border")
            && theme_spec.contains("jump_to_latest_fg"),
        "the shared Catppuccin terminal theme spec should define jump-to-latest shell chrome colors so terminal-adjacent affordances stop drifting from the terminal preset"
    );
    assert!(
        terminal_theme.contains("scrollbar_thumb_active")
            && terminal_theme.contains("jump_to_latest_bg")
            && terminal_theme.contains("jump_to_latest_hover_bg")
            && terminal_theme.contains("jump_to_latest_pressed_bg")
            && terminal_theme.contains("jump_to_latest_border")
            && terminal_theme.contains("jump_to_latest_fg"),
        "the terminal preset projection should expose scrollbar active and jump-to-latest colors so shell chrome can reuse the same Catppuccin source as the terminal surface"
    );
    assert!(
        tokens.contains("terminal-jump-to-latest-surface: dark-mode ? #313244 : #ccd0da;")
            && tokens
                .contains("terminal-jump-to-latest-hover-surface: dark-mode ? #45475a : #bcc0cc;")
            && tokens.contains(
                "terminal-jump-to-latest-pressed-surface: dark-mode ? #585b70 : #acb0be;"
            )
            && tokens.contains("terminal-jump-to-latest-border: dark-mode ? #6c7086 : #9ca0b0;")
            && tokens.contains("terminal-jump-to-latest-fg: dark-mode ? #cdd6f4 : #4c4f69;"),
        "Slint fallback tokens should carry Catppuccin-specific jump-to-latest colors so no-frame terminal states keep the same shell chrome palette"
    );
    assert!(
        terminal_host.contains(
            "in property <color> session-scrollbar-thumb: ThemeTokens.terminal-scrollbar-thumb-surface;"
        ) && terminal_host.contains(
            "in property <color> session-scrollbar-thumb-active: ThemeTokens.terminal-scrollbar-thumb-active-surface;"
        ) && terminal_host.contains(
            "in property <color> session-jump-to-latest-bg: ThemeTokens.terminal-jump-to-latest-surface;"
        ) && terminal_host.contains(
            "in property <color> session-jump-to-latest-hover-bg: ThemeTokens.terminal-jump-to-latest-hover-surface;"
        ) && terminal_host.contains(
            "in property <color> session-jump-to-latest-pressed-bg: ThemeTokens.terminal-jump-to-latest-pressed-surface;"
        ) && terminal_host.contains(
            "in property <color> session-jump-to-latest-border: ThemeTokens.terminal-jump-to-latest-border;"
        ) && terminal_host.contains(
            "in property <color> session-jump-to-latest-fg: ThemeTokens.terminal-jump-to-latest-fg;"
        ) && terminal_host.contains("? root.session-scrollbar-thumb-active")
            && terminal_host.contains(": root.session-scrollbar-thumb;")
            && terminal_host.contains("border-color: root.session-jump-to-latest-border;")
            && terminal_host.contains("? root.session-jump-to-latest-pressed-bg")
            && terminal_host.contains("? root.session-jump-to-latest-hover-bg")
            && terminal_host.contains(": root.session-jump-to-latest-bg;")
            && terminal_host.contains("color: root.session-jump-to-latest-fg;"),
        "TerminalSessionHost should consume terminal-specific shell chrome colors instead of the generic inspector/control tokens for scrollbars and the jump-to-latest pill"
    );
}

use mica_term::app::ssh::runtime::TerminalSurfaceState;
use mica_term::app::terminal_model::{TerminalModelCell, TerminalModelFrame};
use mica_term::app::terminal_semantic::{
    CommandBlockStatus, OutputRuleProfile, OverviewMarkerKind, SemanticAnnotationSet,
    TerminalSemanticSettings, analyze_semantic_annotations,
    analyze_semantic_annotations_with_settings,
};
use mica_term::theme::{
    SearchMatchHighlightStrength, SemanticStyleRole, ThemeMode, ThemeVariant, app_theme_spec,
};
use std::fs;
use uuid::Uuid;

fn semantic_surface(lines: &[&str]) -> TerminalSurfaceState {
    TerminalSurfaceState::from_visible_lines(
        Uuid::new_v4(),
        1,
        lines.len() as u32,
        160,
        lines.iter().map(|line| (*line).to_string()).collect(),
    )
}

fn semantic_model_frame(lines: &[&str]) -> TerminalModelFrame {
    let surface = semantic_surface(lines);
    TerminalModelFrame::from_surface(&surface, None)
}

fn has_role(
    annotations: &SemanticAnnotationSet,
    row: u32,
    needle: &str,
    role: SemanticStyleRole,
) -> bool {
    annotations
        .spans
        .iter()
        .any(|span| span.row == row && span.role == role && span.text == needle)
}

#[test]
fn semantic_pipeline_classifies_input_roles_and_command_status_markers() {
    let frame = semantic_model_frame(&[
        "[dev@mica ~]$ cargo test",
        "src/app/bootstrap.rs:2476:9: ERROR Permission denied while reaching 10.0.0.8:22",
        "https://example.com/docs ./relative/path C:\\Temp\\mica-term\\log.txt 2026-04-22T18:32:10Z",
        "[dev@mica ~]$ git commit -m \"ship it\" ./src $HOME && cargo test --workspace",
    ]);

    let annotations = analyze_semantic_annotations(&frame);

    assert!(has_role(
        &annotations,
        3,
        "[dev@mica ~]$ ",
        SemanticStyleRole::InputPrompt
    ));
    assert!(has_role(
        &annotations,
        3,
        "git",
        SemanticStyleRole::InputCommand
    ));
    assert!(has_role(
        &annotations,
        3,
        "commit",
        SemanticStyleRole::InputSubcommand
    ));
    assert!(has_role(
        &annotations,
        3,
        "-m",
        SemanticStyleRole::InputOption
    ));
    assert!(has_role(
        &annotations,
        3,
        "\"ship it\"",
        SemanticStyleRole::InputString
    ));
    assert!(has_role(
        &annotations,
        3,
        "./src",
        SemanticStyleRole::InputPath
    ));
    assert!(has_role(
        &annotations,
        3,
        "$HOME",
        SemanticStyleRole::InputVariable
    ));
    assert!(has_role(
        &annotations,
        3,
        "&&",
        SemanticStyleRole::InputOperator
    ));

    assert!(has_role(
        &annotations,
        1,
        "src/app/bootstrap.rs:2476:9",
        SemanticStyleRole::OutputLineReference,
    ));
    assert!(has_role(
        &annotations,
        1,
        "ERROR",
        SemanticStyleRole::OutputSeverityError,
    ));
    assert!(has_role(
        &annotations,
        1,
        "Permission denied",
        SemanticStyleRole::OutputFailureKeyword,
    ));
    assert!(has_role(
        &annotations,
        1,
        "10.0.0.8:22",
        SemanticStyleRole::OutputNetworkEndpoint,
    ));
    assert!(has_role(
        &annotations,
        2,
        "https://example.com/docs",
        SemanticStyleRole::OutputUrl,
    ));
    assert!(has_role(
        &annotations,
        2,
        "./relative/path",
        SemanticStyleRole::OutputUnixPath,
    ));
    assert!(has_role(
        &annotations,
        2,
        "C:\\Temp\\mica-term\\log.txt",
        SemanticStyleRole::OutputWindowsPath,
    ));
    assert!(has_role(
        &annotations,
        2,
        "2026-04-22T18:32:10Z",
        SemanticStyleRole::OutputTimestamp,
    ));

    assert_eq!(annotations.command_blocks.len(), 2);
    assert_eq!(
        annotations.command_blocks[0].status,
        CommandBlockStatus::Failure
    );
    assert_eq!(
        annotations.command_blocks[1].status,
        CommandBlockStatus::Running
    );
    assert!(
        annotations
            .overview_markers
            .iter()
            .any(|marker| { marker.row == 0 && marker.kind == OverviewMarkerKind::CommandFailure })
    );
    assert!(
        annotations
            .overview_markers
            .iter()
            .any(|marker| { marker.row == 3 && marker.kind == OverviewMarkerKind::CommandRunning })
    );
}

#[test]
fn semantic_pipeline_detects_diff_and_obvious_json_roles() {
    let frame = semantic_model_frame(&[
        "@@ -1,3 +1,3 @@",
        "- old_value",
        "+ new_value",
        "{",
        "  \"name\": \"mica-term\",",
        "  \"count\": 2,",
        "  \"ok\": true",
        "}",
    ]);

    let annotations = analyze_semantic_annotations(&frame);

    assert!(has_role(
        &annotations,
        0,
        "@@ -1,3 +1,3 @@",
        SemanticStyleRole::OutputDiffHunk
    ));
    assert!(has_role(
        &annotations,
        1,
        "- old_value",
        SemanticStyleRole::OutputDiffRemoved
    ));
    assert!(has_role(
        &annotations,
        2,
        "+ new_value",
        SemanticStyleRole::OutputDiffAdded
    ));
    assert!(has_role(
        &annotations,
        4,
        "\"name\"",
        SemanticStyleRole::OutputJsonKey
    ));
    assert!(has_role(
        &annotations,
        4,
        "\"mica-term\"",
        SemanticStyleRole::OutputJsonString,
    ));
    assert!(has_role(
        &annotations,
        5,
        "2",
        SemanticStyleRole::OutputJsonNumber
    ));
    assert!(has_role(
        &annotations,
        6,
        "true",
        SemanticStyleRole::OutputJsonBoolean
    ));
}

#[test]
fn semantic_pipeline_projects_terminal_search_matches_independently_of_output_rules() {
    let frame = semantic_model_frame(&["https://example.com/docs", "example host example"]);

    let annotations = analyze_semantic_annotations_with_settings(
        &frame,
        TerminalSemanticSettings {
            input_highlighting_enabled: true,
            output_rule_highlighting_enabled: false,
            output_rule_profile: OutputRuleProfile::Default,
            command_decorations_enabled: false,
            overview_markers_enabled: false,
            search_query: Some("example".into()),
        },
    );

    let search_match_count = annotations
        .spans
        .iter()
        .filter(|span| span.role == SemanticStyleRole::OutputGrepMatch)
        .count();

    assert_eq!(search_match_count, 3);
    assert!(has_role(
        &annotations,
        0,
        "example",
        SemanticStyleRole::OutputGrepMatch
    ));
}

#[test]
fn premium_default_theme_maps_roles_to_product_grade_semantic_styles() {
    let theme = app_theme_spec(ThemeMode::Dark, ThemeVariant::PremiumDefault);

    let command = theme.semantic_style(SemanticStyleRole::InputCommand);
    let url = theme.semantic_style(SemanticStyleRole::OutputUrl);
    let error = theme.semantic_style(SemanticStyleRole::OutputSeverityError);
    let running = theme.semantic_style(SemanticStyleRole::CommandStatusRunning);

    assert_eq!(command.foreground, theme.semantic.input_command);
    assert!(
        !url.underline,
        "browser-safe terminal URLs should stay un-underlined by default so shell output keeps a calm terminal-first reading rhythm"
    );
    assert!(
        error.bold,
        "error roles should get stronger text emphasis for fast scanning"
    );
    assert_eq!(running.foreground, theme.decoration.running);
}

#[test]
fn semantic_style_projection_recolors_default_cells_but_preserves_explicit_ansi_foreground() {
    let theme = app_theme_spec(ThemeMode::Dark, ThemeVariant::PremiumDefault);
    let mut frame = semantic_model_frame(&["https://example.com ERROR"]);
    let default_fg = frame.palette.default_fg_rgba;
    let default_bg = frame.palette.default_bg_rgba;
    frame.rows[0].cells = frame.rows[0]
        .text
        .chars()
        .enumerate()
        .map(|(index, ch)| TerminalModelCell {
            row: 0,
            col: index as u32,
            width: 1,
            text: ch.to_string(),
            bold: false,
            underline: false,
            fg_rgba: default_fg,
            bg_rgba: default_bg,
        })
        .collect();
    let annotations = analyze_semantic_annotations(&frame);

    let url_span = annotations
        .spans
        .iter()
        .find(|span| span.role == SemanticStyleRole::OutputUrl)
        .expect("url span")
        .clone();
    let error_span = annotations
        .spans
        .iter()
        .find(|span| span.role == SemanticStyleRole::OutputSeverityError)
        .expect("error span")
        .clone();

    let ansi_fg = 0xff44_5566;
    frame.rows[0]
        .cells
        .iter_mut()
        .find(|cell| cell.col == url_span.start_col)
        .expect("url cell")
        .fg_rgba = ansi_fg;

    let previous = frame.clone();
    frame.apply_semantic_style_overlays(
        Some(&previous),
        theme,
        &annotations.spans,
        SearchMatchHighlightStrength::Balanced,
    );

    let url_cell = frame.rows[0]
        .cells
        .iter()
        .find(|cell| cell.col == url_span.start_col)
        .expect("styled url cell");
    assert_eq!(
        url_cell.fg_rgba, ansi_fg,
        "semantic styling should keep explicit ANSI foregrounds as the truth source"
    );
    assert!(
        !url_cell.underline,
        "semantic styling should keep browser-safe URLs free of permanent underlines and rely on Ctrl+click activation instead"
    );

    let error_cell = frame.rows[0]
        .cells
        .iter()
        .find(|cell| cell.col == error_span.start_col)
        .expect("styled error cell");
    assert_eq!(
        error_cell.fg_rgba,
        0xff00_0000
            | theme
                .semantic_style(SemanticStyleRole::OutputSeverityError)
                .foreground
    );
    assert!(
        error_cell.bold,
        "default-colored cells should adopt semantic emphasis for fast scanning"
    );
    assert!(
        frame.dirty_rows.contains(&0),
        "semantic recoloring should mark the row dirty so bitmap/native presenters repaint it"
    );
}

#[test]
fn semantic_pipeline_respects_feature_toggles_for_spans_and_command_decorations() {
    let frame = semantic_model_frame(&[
        "[dev@mica ~]$ cargo test",
        "ERROR Permission denied https://example.com/docs",
    ]);

    let annotations = analyze_semantic_annotations_with_settings(
        &frame,
        TerminalSemanticSettings {
            input_highlighting_enabled: false,
            output_rule_highlighting_enabled: false,
            output_rule_profile: OutputRuleProfile::Default,
            command_decorations_enabled: false,
            overview_markers_enabled: false,
            search_query: None,
        },
    );

    assert!(
        annotations.spans.is_empty(),
        "when both input and output rule highlighting are disabled, the semantic span layer should stay empty"
    );
    assert!(
        annotations.command_blocks.is_empty(),
        "command decoration opt-out should suppress gutter block projection"
    );
    assert!(
        annotations.overview_markers.is_empty(),
        "command decoration opt-out should suppress overview ruler markers too"
    );
}

#[test]
fn semantic_pipeline_supports_focused_output_rules_and_separate_overview_marker_toggle() {
    let frame = semantic_model_frame(&[
        "[dev@mica ~]$ cargo test",
        "INFO 2026-04-22T18:32:10Z ERROR Permission denied https://example.com/docs",
    ]);

    let annotations = analyze_semantic_annotations_with_settings(
        &frame,
        TerminalSemanticSettings {
            input_highlighting_enabled: true,
            output_rule_highlighting_enabled: true,
            output_rule_profile: OutputRuleProfile::Focused,
            command_decorations_enabled: true,
            overview_markers_enabled: false,
            search_query: None,
        },
    );

    assert!(
        has_role(
            &annotations,
            1,
            "ERROR",
            SemanticStyleRole::OutputSeverityError
        ),
        "focused output rules should still keep high-signal failure cues"
    );
    assert!(
        has_role(
            &annotations,
            1,
            "https://example.com/docs",
            SemanticStyleRole::OutputUrl
        ),
        "focused output rules should keep navigable links"
    );
    assert!(
        !annotations
            .spans
            .iter()
            .any(|span| span.role == SemanticStyleRole::OutputSeverityInfo),
        "focused output rules should drop lower-signal INFO emphasis to keep the terminal quieter"
    );
    assert!(
        !annotations
            .spans
            .iter()
            .any(|span| span.role == SemanticStyleRole::OutputTimestamp),
        "focused output rules should drop timestamps that add scan noise without actionable value"
    );
    assert_eq!(
        annotations.command_blocks.len(),
        1,
        "turning overview markers off should not suppress command block decorations"
    );
    assert!(
        annotations.overview_markers.is_empty(),
        "overview markers should be independently suppressible while gutter decorations stay active"
    );
}

#[test]
fn semantic_style_projection_stays_clean_on_same_theme_but_marks_rows_dirty_on_theme_switch() {
    let dark_theme = app_theme_spec(ThemeMode::Dark, ThemeVariant::PremiumDefault);
    let light_theme = app_theme_spec(ThemeMode::Light, ThemeVariant::PremiumDefault);
    let base_text = "https://example.com ERROR";

    let build_cells = |frame: &mut TerminalModelFrame| {
        let default_fg = frame.palette.default_fg_rgba;
        let default_bg = frame.palette.default_bg_rgba;
        frame.rows[0].cells = frame.rows[0]
            .text
            .chars()
            .enumerate()
            .map(|(index, ch)| TerminalModelCell {
                row: 0,
                col: index as u32,
                width: 1,
                text: ch.to_string(),
                bold: false,
                underline: false,
                fg_rgba: default_fg,
                bg_rgba: default_bg,
            })
            .collect();
    };

    let mut previous = semantic_model_frame(&[base_text]);
    build_cells(&mut previous);
    let annotations = analyze_semantic_annotations(&previous);
    previous.apply_semantic_style_overlays(
        None,
        dark_theme,
        &annotations.spans,
        SearchMatchHighlightStrength::Balanced,
    );

    let mut same_theme = semantic_model_frame(&[base_text]);
    build_cells(&mut same_theme);
    same_theme.apply_semantic_style_overlays(
        Some(&previous),
        dark_theme,
        &annotations.spans,
        SearchMatchHighlightStrength::Balanced,
    );
    assert!(
        same_theme.dirty_rows.is_empty(),
        "reprojecting the same themed semantic styling should not keep rows artificially dirty"
    );

    let mut switched_theme = semantic_model_frame(&[base_text]);
    build_cells(&mut switched_theme);
    switched_theme.apply_semantic_style_overlays(
        Some(&previous),
        light_theme,
        &annotations.spans,
        SearchMatchHighlightStrength::Balanced,
    );
    assert_eq!(
        switched_theme.dirty_rows,
        vec![0],
        "switching theme should invalidate the styled row hash so terminal presenters repaint semantic colors"
    );
}

#[test]
fn search_match_highlight_strength_scales_background_emphasis() {
    let theme = app_theme_spec(ThemeMode::Dark, ThemeVariant::PremiumDefault);
    let build_frame = || {
        let mut frame = semantic_model_frame(&["example"]);
        let default_fg = frame.palette.default_fg_rgba;
        let default_bg = frame.palette.default_bg_rgba;
        frame.rows[0].cells = frame.rows[0]
            .text
            .chars()
            .enumerate()
            .map(|(index, ch)| TerminalModelCell {
                row: 0,
                col: index as u32,
                width: 1,
                text: ch.to_string(),
                bold: false,
                underline: false,
                fg_rgba: default_fg,
                bg_rgba: default_bg,
            })
            .collect();
        frame
    };
    let spans = vec![mica_term::app::terminal_semantic::SemanticSpan {
        row: 0,
        start_col: 0,
        end_col: 6,
        role: SemanticStyleRole::OutputGrepMatch,
        text: "example".into(),
    }];

    let mut subtle = build_frame();
    subtle.apply_semantic_style_overlays(None, theme, &spans, SearchMatchHighlightStrength::Subtle);

    let mut strong = build_frame();
    strong.apply_semantic_style_overlays(None, theme, &spans, SearchMatchHighlightStrength::Strong);

    assert_ne!(
        subtle.rows[0].cells[0].bg_rgba, strong.rows[0].cells[0].bg_rgba,
        "search match emphasis should respond to the user-selected highlight strength instead of rendering every match with the same fill"
    );
}

#[test]
fn native_presenter_payload_threads_semantic_spans_blocks_and_overview_markers() {
    let presenter = fs::read_to_string("src/app/terminal_presenter.rs").expect("read presenter");
    let bootstrap = fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap");
    let renderer_host =
        fs::read_to_string("src/app/terminal_renderer/host.rs").expect("read renderer host");

    assert!(
        presenter.contains("pub semantic_spans: Vec<SemanticSpan>")
            && presenter.contains("pub command_blocks: Vec<CommandBlock>")
            && presenter.contains("pub overview_markers: Vec<OverviewMarker>")
            && presenter.contains("analyze_semantic_annotations_with_settings("),
        "native presenter should project semantic spans, command blocks, and overview markers through one semantic annotation payload"
    );
    assert!(
        presenter.contains("frame_model.apply_semantic_style_overlays(")
            && presenter.contains("app_theme_spec(options.theme_mode, options.theme_variant)")
            && presenter.contains("search_query: options.search_query.clone()")
            && presenter.contains("options.search_match_highlight")
            && renderer_host.contains("pub theme_mode: ThemeMode")
            && renderer_host.contains("pub theme_variant: ThemeVariant")
            && renderer_host.contains("pub search_query: Option<String>")
            && bootstrap.contains("theme_mode: state.theme_mode")
            && bootstrap.contains("theme_variant: state.theme_variant")
            && bootstrap.contains("search_query: search_query.clone()"),
        "semantic spans should flow into the renderer with theme/search context so bitmap/native presenters can restyle default-colored cells without teaching the renderer regex"
    );
    assert!(
        bootstrap.contains("semantic_span_count = presentable_frame.semantic_spans.len()")
            && bootstrap.contains("command_block_count = presentable_frame.command_blocks.len()")
            && bootstrap
                .contains("overview_marker_count = presentable_frame.overview_markers.len()"),
        "bootstrap tracing should keep the richer semantic payload visible while the renderer remains a pure consumer"
    );
}

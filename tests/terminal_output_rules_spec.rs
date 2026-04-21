use mica_term::app::ssh::runtime::TerminalSurfaceState;
use mica_term::app::terminal_model::TerminalModelFrame;
use mica_term::app::terminal_semantic::{
    SemanticStyleRole, analyze_output_rules, analyzed_row_window,
};
use uuid::Uuid;

fn output_rule_surface(session_id: Uuid, seqno: usize, lines: &[&str]) -> TerminalSurfaceState {
    TerminalSurfaceState::from_visible_lines(
        session_id,
        seqno,
        lines.len() as u32,
        160,
        lines.iter().map(|line| (*line).to_string()).collect(),
    )
}

fn output_rule_frame(
    session_id: Uuid,
    seqno: usize,
    lines: &[&str],
    previous: Option<&TerminalModelFrame>,
) -> TerminalModelFrame {
    let surface = output_rule_surface(session_id, seqno, lines);
    TerminalModelFrame::from_surface(&surface, previous)
}

fn has_role(
    spans: &[mica_term::app::terminal_semantic::SemanticSpan],
    role: SemanticStyleRole,
) -> bool {
    spans.iter().any(|span| span.role == role)
}

#[test]
fn output_rules_match_urls_paths_errors_git_diff_and_json_roles() {
    let frame = output_rule_frame(
        Uuid::new_v4(),
        1,
        &[
            "see https://example.com/docs",
            "src/main.rs:42:7: error: boom",
            "+ added line",
            "@@ -1,2 +1,2 @@",
            "{ \"name\": \"mica-term\", \"ok\": true }",
        ],
        None,
    );

    let analysis = analyze_output_rules(&frame, &frame.dirty_rows);

    assert!(has_role(&analysis.spans, SemanticStyleRole::OutputUrl));
    assert!(has_role(&analysis.spans, SemanticStyleRole::OutputFilePath));
    assert!(has_role(
        &analysis.spans,
        SemanticStyleRole::OutputLevelError
    ));
    assert!(has_role(&analysis.spans, SemanticStyleRole::OutputGitAdded));
    assert!(has_role(&analysis.spans, SemanticStyleRole::OutputGitHunk));
    assert!(has_role(&analysis.spans, SemanticStyleRole::OutputJsonKey));
}

#[test]
fn output_rules_only_recompute_dirty_rows_plus_bounded_lookbehind() {
    let session_id = Uuid::new_v4();
    let previous = output_rule_frame(
        session_id,
        1,
        &["row-0", "row-1", "row-2", "row-3", "row-4"],
        None,
    );
    let next = output_rule_frame(
        session_id,
        2,
        &["row-0", "row-1", "row-2", "row-3 changed", "row-4"],
        Some(&previous),
    );

    let analysis = analyze_output_rules(&next, &next.dirty_rows);

    assert_eq!(next.dirty_rows, vec![3]);
    assert_eq!(analysis.analyzed_rows, vec![1, 2, 3]);
    assert_eq!(
        analyzed_row_window(&next, &next.dirty_rows, 2),
        vec![1, 2, 3]
    );
}

use std::fs;
use std::path::Path;

#[test]
fn terminal_tui_smoke_script_exists_and_lists_expected_scenarios() {
    let script_path = Path::new("scripts/dev/terminal-tui-smoke.sh");
    assert!(
        script_path.exists(),
        "terminal TUI smoke script should exist so contributors have a stable entrypoint for manual terminal validation"
    );

    let script =
        fs::read_to_string(script_path).expect("read terminal TUI smoke script source contract");

    for scenario in [
        "all",
        "codex",
        "vim",
        "less",
        "htop",
        "links",
        "glyphs",
        "progress",
    ] {
        assert!(
            script.contains(scenario),
            "terminal TUI smoke script should expose the `{scenario}` scenario"
        );
    }
}

#[test]
fn terminal_tui_smoke_checklist_mentions_core_observation_points() {
    let checklist_path = Path::new("docs/terminal-tui-smoke-checklist.md");
    assert!(
        checklist_path.exists(),
        "terminal TUI smoke checklist should exist so manual verification is repeatable"
    );

    let checklist =
        fs::read_to_string(checklist_path).expect("read terminal TUI smoke checklist contract");

    for observation in ["贴底", "alt-screen", "spinner", "link", "glyph"] {
        assert!(
            checklist.contains(observation),
            "terminal TUI smoke checklist should mention the `{observation}` observation point"
        );
    }
}

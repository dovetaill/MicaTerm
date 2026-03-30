use std::fs;

#[test]
fn follow_up_doc_tracks_linux_macos_backends_and_libghostty_stop_loss() {
    let follow_up = fs::read_to_string("docs/plans/2026-03-30-terminal-rendering-stack-follow-up.md")
        .expect("read follow-up doc");

    assert!(
        follow_up.contains("| Item | Trigger | Owner | Notes |"),
        "follow-up doc should use the concrete backlog table format from the plan"
    );
    assert!(
        follow_up.contains("linux_freetype_fontconfig.rs"),
        "follow-up doc should track the Linux font backend milestone"
    );
    assert!(
        follow_up.contains("macos_coretext.rs"),
        "follow-up doc should track the macOS font backend milestone"
    );
    assert!(
        follow_up.contains("cursor/selection fully into the renderer"),
        "follow-up doc should track moving cursor and selection fully into the renderer"
    );
    assert!(
        follow_up.contains("libghostty"),
        "follow-up doc should record the libghostty stop-loss route"
    );
}

#[test]
fn design_doc_links_to_follow_up_backlog_and_stop_loss_triggers() {
    let design = fs::read_to_string("docs/plans/2026-03-30-terminal-rendering-stack-design.md")
        .expect("read design doc");

    assert!(
        design.contains("terminal-rendering-stack-follow-up.md"),
        "design doc should link to the follow-up backlog document"
    );
    assert!(
        design.contains("libghostty"),
        "design doc should continue to reference the libghostty stop-loss route"
    );
    assert!(
        design.contains("switching to the `libghostty` stop-loss route")
            || design.contains("切换到 `libghostty` 止损路线"),
        "design doc should explicitly call out the trigger conditions for switching to the stop-loss route"
    );
}

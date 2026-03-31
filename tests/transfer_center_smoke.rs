use std::fs;

#[test]
fn transfer_center_renders_running_queued_paused_failed_completed_tabs() {
    let content =
        fs::read_to_string("ui/shell/transfer-center.slint").expect("read transfer center source");

    for label in ["Running", "Queued", "Paused", "Failed", "Completed"] {
        assert!(
            content.contains(&format!("text: \"{label}\"")),
            "transfer center should expose the `{label}` tab"
        );
    }

    assert!(
        content.contains("No transfers yet"),
        "transfer center should expose a lightweight empty state"
    );
}

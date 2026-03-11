use std::fs;

use mica_term::app::tooltip_debug_log::{TooltipDebugEvent, TooltipDebugLog};

#[test]
fn tooltip_debug_log_creates_log_file_and_appends_event_lines() {
    let temp_dir = std::env::temp_dir()
        .join("mica-term")
        .join("tests")
        .join("tooltip-debug-log");
    let _ = fs::remove_dir_all(&temp_dir);

    let logger = TooltipDebugLog::in_directory(temp_dir.join("logs")).unwrap();
    logger
        .append(TooltipDebugEvent {
            phase: "show-tooltip",
            source_id: "nav-button",
            text: "Open menu",
            anchor_x: 24.0,
            anchor_y: 44.0,
        })
        .unwrap();

    let log_path = temp_dir.join("logs").join("titlebar-tooltip.log");
    let content = fs::read_to_string(log_path).unwrap();

    assert!(content.contains("show-tooltip"));
    assert!(content.contains("nav-button"));
    assert!(content.contains("Open menu"));
    assert!(content.contains("anchor_x=24"));

    let _ = fs::remove_dir_all(temp_dir);
}
